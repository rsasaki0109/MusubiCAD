//! End-to-end `opencad mcp` session over real stdio (ADR-014): an agent
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
        let mut child = Command::new(env!("CARGO_BIN_EXE_opencad"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn opencad mcp");
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
