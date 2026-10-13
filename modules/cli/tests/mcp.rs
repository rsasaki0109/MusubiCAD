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
fn actual_mcp_semantic_selection_matches_tauri_backend_source_and_history() {
    use opencad_desktop::selection::{
        apply_document_selection_edit, plan_document_selection_edit, SelectionEdit,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut doc = opencad_file::read_ocad(root.join("examples/ellipse_plate.ocad.d")).unwrap();
    doc.semantic_refs.push(opencad_geometry::TopoRef::face(
        opencad_core::TopoRefId::new("ref:face:ellipse_top").unwrap(),
        "feature:ellipse_body",
        "top",
    ));
    let dir = tempfile::tempdir().unwrap();
    let gui = dir.path().join("gui.ocad.d");
    let agent = dir.path().join("agent.ocad.d");
    opencad_file::write_ocad(&gui, &doc).unwrap();
    opencad_file::write_ocad(&agent, &doc).unwrap();
    let mut session = Session::start();
    session.request("initialize", json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"selection-parity","version":"0"}}));
    session.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let frame = session.tool("inspect_selection_frame", json!({"path":agent}));
    let gui_frame = opencad_desktop::load_view_data(gui.to_str().unwrap())
        .unwrap()
        .selection_frame
        .unwrap();
    assert_eq!(frame, serde_json::to_value(&gui_frame).unwrap());
    let edit = json!({"source_revision":frame["source_revision"],"scene_revision":frame["scene_revision"],"target":{"kind":"face_reference","ref_id":"ref:face:ellipse_top"},"field":"length_expr","expr":"10 mm"});
    let typed: SelectionEdit = serde_json::from_value(edit.clone()).unwrap();
    let preview = session.tool("plan_selection_edit", json!({"path":agent,"edit":edit}));
    assert_eq!(
        preview,
        serde_json::to_value(plan_document_selection_edit(gui.to_str().unwrap(), &typed).unwrap())
            .unwrap()
    );
    assert_eq!(opencad_file::read_ocad(&agent).unwrap(), doc);
    let direct = apply_document_selection_edit(gui.to_str().unwrap(), &typed, None).unwrap();
    let result = session.tool("apply_selection_edit", json!({"path":agent,"edit":edit}));
    assert_eq!(result, serde_json::to_value(&direct).unwrap());
    let expected = std::f64::consts::PI * 0.030 * 0.020 * 0.010;
    assert!((result["evidence"]["volume_m3"].as_f64().unwrap() - expected).abs() < 1e-12);
    let after = opencad_file::read_ocad(&agent).unwrap();
    assert_eq!(after, opencad_file::read_ocad(&gui).unwrap());
    assert_eq!(after.parameters, doc.parameters);
    assert_eq!(after.sketches, doc.sketches);
    assert_eq!(after.semantic_refs, doc.semantic_refs);
    let stale = session.request("tools/call",json!({"name":"apply_selection_edit","arguments":{"path":agent,"edit":edit,"history":result["history"]["history"]}}));
    assert_eq!(stale["result"]["isError"], true);
    assert_eq!(opencad_file::read_ocad(&agent).unwrap(), after);
}

#[test]
fn feature_tree_ui_and_agent_commands_produce_same_source_and_history() {
    use opencad_desktop::{
        apply_document_feature_edit, list_document_features, plan_document_feature_edit,
        FeatureEdit,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let doc = opencad_file::read_ocad(root.join("examples/cubic_plate.ocad.d")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let gui = dir.path().join("gui.ocad.d");
    let agent = dir.path().join("agent.ocad.d");
    opencad_file::write_ocad(&gui, &doc).unwrap();
    opencad_file::write_ocad(&agent, &doc).unwrap();
    let scenario: Value = serde_json::from_slice(
        &std::fs::read(root.join("examples/agent/feature_tree_edit.json")).unwrap(),
    )
    .unwrap();
    let mut session = Session::start();
    session.request("initialize", json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"editor-parity","version":"0"}}));
    session.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let mut history = None;
    for edit in scenario["edits"].as_array().unwrap() {
        let tree = session.tool("list_features", json!({"path":agent}));
        assert_eq!(
            tree,
            serde_json::to_value(list_document_features(gui.to_str().unwrap()).unwrap()).unwrap()
        );
        let before = opencad_file::read_ocad(&agent).unwrap();
        let plan = session.tool(
            "plan_feature_edit",
            json!({"path":agent,"revision":tree["revision"],"edit":edit}),
        );
        let typed: FeatureEdit = serde_json::from_value(edit.clone()).unwrap();
        let gui_plan = plan_document_feature_edit(
            gui.to_str().unwrap(),
            tree["revision"].as_str().unwrap(),
            typed.clone(),
        )
        .unwrap();
        assert_eq!(plan, serde_json::to_value(gui_plan).unwrap());
        assert_eq!(opencad_file::read_ocad(&agent).unwrap(), before);
        let gui_result = apply_document_feature_edit(
            gui.to_str().unwrap(),
            tree["revision"].as_str().unwrap(),
            typed,
            history.clone(),
        )
        .unwrap();
        let mut args = json!({"path":agent,"revision":tree["revision"],"edit":edit});
        if let Some(h) = &history {
            args["history"] = serde_json::to_value(h).unwrap();
        }
        let agent_result = session.tool("apply_feature_edit", args);
        assert_eq!(agent_result, serde_json::to_value(&gui_result).unwrap());
        assert_eq!(
            opencad_file::read_ocad(&agent).unwrap(),
            opencad_file::read_ocad(&gui).unwrap()
        );
        history = Some(gui_result.history.history);
    }
    let result = opencad_desktop::editor::verify_editor_document_native(
        &opencad_file::read_ocad(&agent).unwrap(),
    )
    .unwrap();
    assert!((result.volume_m3.unwrap() - 2.4e-6).abs() < 1e-12);
    let source = opencad_file::read_ocad(&agent).unwrap();
    let stale = session.request("tools/call",json!({"name":"apply_feature_edit","arguments":{"path":agent,"revision":"stale","edit":scenario["edits"][0]}}));
    assert_eq!(stale["result"]["isError"], true);
    assert_eq!(opencad_file::read_ocad(&agent).unwrap(), source);
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

#[test]
fn actual_mcp_sketch_editor_matches_tauri_backend_source_and_history() {
    use opencad_desktop::sketch_editor::{
        apply_document_sketch_edit, inspect_document_sketches, plan_document_sketch_edit,
        SketchEdit, SketchForm,
    };
    let dir = tempfile::tempdir().unwrap();
    let desktop = dir.path().join("desktop.ocad.d");
    let agent = dir.path().join("agent.ocad.d");
    let doc = opencad_file::read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/ellipse_plate.ocad.d"),
    )
    .unwrap();
    opencad_file::write_ocad(&desktop, &doc).unwrap();
    opencad_file::write_ocad(&agent, &doc).unwrap();
    let desktop = desktop.to_str().unwrap();
    let agent = agent.to_str().unwrap();
    let mut session = Session::start();
    session.request("initialize",json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"sketch-parity","version":"1"}}));
    let state = session.tool("inspect_sketches", json!({"path":agent}));
    assert_eq!(
        state,
        serde_json::to_value(inspect_document_sketches(desktop).unwrap()).unwrap()
    );
    let edit = SketchEdit {
        revision: state["revision"].as_str().unwrap().into(),
        sketch_id: "sketch:ellipse".into(),
        operations: vec![SketchForm::Ellipse {
            id: "ent:ellipse".into(),
            center: "ent:center".into(),
            major_radius: "40 mm".into(),
            minor_radius: "15 mm".into(),
            axis_angle: "0 deg".into(),
            construction: false,
            create: false,
        }],
    };
    let before = opencad_file::read_ocad(agent).unwrap();
    let plan = session.tool("plan_sketch_edit", json!({"path":agent,"edit":edit}));
    assert_eq!(
        plan,
        serde_json::to_value(plan_document_sketch_edit(desktop, &edit).unwrap()).unwrap()
    );
    assert_eq!(opencad_file::read_ocad(agent).unwrap(), before);
    let result = session.tool("apply_sketch_edit", json!({"path":agent,"edit":edit}));
    let expected = apply_document_sketch_edit(desktop, &edit, None).unwrap();
    assert_eq!(result, serde_json::to_value(expected).unwrap());
    assert_eq!(
        opencad_file::read_ocad(agent).unwrap(),
        opencad_file::read_ocad(desktop).unwrap()
    );
    assert!(
        (result["evidence"]["volume_m3"].as_f64().unwrap()
            - std::f64::consts::PI * 0.04 * 0.015 * 0.006)
            .abs()
            < VOLUME_TOLERANCE_M3
    );
    let stale = session.request(
        "tools/call",
        json!({"name":"apply_sketch_edit","arguments":{"path":agent,"edit":edit}}),
    );
    assert_eq!(stale["result"]["isError"], true);
}
