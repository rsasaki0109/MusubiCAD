//! End-to-end `musubicad mcp` session over real stdio (ADR-014): an agent
//! creates an empty part, authors a plate with a through hole, and verifies
//! the regenerated OCCT geometry (integration test: requires OCCT).

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

/// Volume agreement with the analytic plate-minus-hole volume, in
/// cubic meters (1e-3 mm^3).
const VOLUME_TOLERANCE_M3: f64 = 1e-12;

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Session {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_musubicad"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn musubicad mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("write");
        self.stdin.flush().expect("flush");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read response");
        let response: Value = serde_json::from_str(&line).expect("JSON response");
        assert_eq!(response["id"], id, "{response}");
        response
    }

    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let result = response["result"].clone();
        assert_eq!(result["isError"], false, "{name}: {result}");
        result["structuredContent"].clone()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn actual_mcp_cubic_control_edit_matches_independent_analytic_grader() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let original =
        opencad_file::read_ocad(root.join("examples/cubic_plate.ocad.d")).expect("cubic example");
    let dir = tempfile::tempdir().expect("temp");
    let part = dir.path().join("cubic.ocad.d");
    opencad_file::write_expanded_dir(&part, &original).expect("write");
    let mut session = Session::start();
    session.request("initialize",json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"cubic-e2e","version":"0"}}));
    session.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let patch = json!({"operations":[{"type":"set_parameter","id":"param:a","expr":"60 mm"}]});
    session.tool("patch_dry_run", json!({"path":part,"patch":patch}));
    assert_eq!(opencad_file::read_ocad(&part).expect("dry"), original);
    let applied = session.tool("patch_apply", json!({"path":part,"patch":patch}));
    let after = opencad_file::read_ocad(&part).expect("after");
    assert_eq!(after.sketches, original.sketches);
    let regen = session.tool("regen_document", json!({"path":part}));
    let volume = regen["volume_m3"].as_f64().expect("volume");
    assert!((volume - 3. * 0.06 * 0.04 * 0.006 / 20.).abs() < 1e-14);
    let mut history: opencad_file::DocumentHistory =
        serde_json::from_value(applied["history"].clone()).expect("history source wire");
    let mut snapshot = after.clone();
    history.undo(&mut snapshot).expect("undo");
    assert_eq!(snapshot, original);
    history.redo(&mut snapshot).expect("redo");
    assert_eq!(snapshot, after);
    let bad = session.request("tools/call",json!({"name":"patch_apply","arguments":{"path":part,"patch":{"operations":[{"type":"set_parameter","id":"param:b","expr":"-40 mm"}]}}}));
    assert_eq!(bad["result"]["isError"], true);
    assert_eq!(opencad_file::read_ocad(&part).expect("refusal"), after);
}

#[test]
fn an_agent_authors_reviews_and_applies_a_part_over_mcp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let part = dir.path().join("plate.ocad.d");
    let part_path = part.to_string_lossy().to_string();
    let patch: Value = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/agent/author_plate_from_empty_patch.json"),
        )
        .expect("read example patch"),
    )
    .expect("patch JSON");

    let mut session = Session::start();
    let init = session.request(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "e2e", "version": "0" }
        }),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    session.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));

    session.tool(
        "new_document",
        json!({
            "path": part_path, "kind": "part", "document_id": "doc:plate", "name": "Plate"
        }),
    );

    let dry_run = session.tool(
        "patch_dry_run",
        json!({ "path": part_path, "patch": patch }),
    );
    assert_eq!(dry_run["validation"]["messages"], json!([]), "{dry_run}");
    // Authoring from an empty part: no body before, a plate after.
    assert_eq!(dry_run["verification"]["passed"], json!(true), "{dry_run}");
    assert!(
        dry_run["verification"]["geometry"]["after_triangles"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "{dry_run}"
    );
    assert_eq!(
        dry_run["impact"]["predicted_dirty_nodes"],
        json!([
            "feature:sketch_base",
            "feature:plate",
            "feature:sketch_hole",
            "feature:through_hole"
        ])
    );

    let review_dir = dir.path().join("review").to_string_lossy().to_string();
    let review = session.tool(
        "review_patch",
        json!({
            "path": part_path, "patch": patch, "output_dir": review_dir
        }),
    );
    assert!(PathBuf::from(review["review_html"].as_str().expect("path")).exists());

    session.tool("patch_apply", json!({ "path": part_path, "patch": patch }));
    let regen = session.tool("regen_document", json!({ "path": part_path }));
    // Ready features regenerate in ID order, so the plate precedes the
    // hole sketch.
    assert_eq!(
        regen["regenerated"],
        json!([
            "feature:sketch_base",
            "feature:plate",
            "feature:sketch_hole",
            "feature:through_hole"
        ])
    );

    let (width, depth, thickness, radius) = (0.06_f64, 0.04_f64, 0.005_f64, 0.005_f64);
    // Circle profiles are exact (ADR-020).
    let hole_area = std::f64::consts::PI * radius * radius;
    let expected = (width * depth - hole_area) * thickness;
    let volume = regen["volume_m3"].as_f64().expect("volume");
    assert!(
        (volume - expected).abs() <= VOLUME_TOLERANCE_M3,
        "volume {volume} m^3, expected {expected} m^3"
    );

    // The applied document is a normal, valid MusubiCAD document.
    let validated = session.tool("validate_document", json!({ "path": part_path }));
    assert_eq!(validated["valid"], true);
}
