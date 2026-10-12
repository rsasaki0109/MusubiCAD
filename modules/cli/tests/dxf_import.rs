//! MCAD-P12-009: native sketch import, explicit units, history and isolation.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_file::{document_design_state, read_ocad, write_expanded_dir, OcadDocument};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args(args)
        .output()
        .expect("CLI")
}
fn success(o: std::process::Output) -> String {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).expect("utf8")
}
fn empty(path: &Path) -> OcadDocument {
    let doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:dxf_plate").expect("id"),
        "DXF plate",
    ));
    write_expanded_dir(path, &doc).expect("empty part");
    doc
}
fn request(method: &str, params: Value, mcp: bool) -> Value {
    let (mode, message) = if mcp {
        (
            "mcp",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":method,"arguments":params}}),
        )
    } else {
        (
            "agent",
            json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
        )
    };
    let mut p = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("transport");
    writeln!(p.stdin.take().expect("stdin"), "{message}").expect("request");
    let o = p.wait_with_output().expect("wait");
    assert!(o.status.success());
    serde_json::from_slice(&o.stdout).expect("JSON")
}
fn import(path: &Path, source: &Path) -> String {
    success(cli(&[
        "import-dxf",
        path.to_str().expect("path"),
        source.to_str().expect("path"),
        "--sketch-id",
        "sketch:dxf",
        "--id",
        "feature:sketch_dxf",
        "--layers",
        "OUTLINE",
    ]))
}
fn extrude(path: &Path) {
    let patch:DesignPatch=serde_json::from_value(json!({"operations":[{"type":"add_feature","node":{"id":"feature:extrude_dxf","name":"DXF extrusion","definition":{"type":"extrude","sketch_feature":"feature:sketch_dxf","profile_ref":"sketch:dxf/profile:outer","extent":{"type":"distance","length":{"value_si":0.006}},"operation":"new_body"}},"position":opencad_ai::FeaturePosition::end()}]})).expect("patch");
    let result = request(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":patch}),
        false,
    );
    assert!(result["error"].is_null(), "{result}");
}
#[test]
fn inspect_dry_run_import_undo_redo_and_transport_preserve_si_geometry() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("plate.ocad.d");
    let before = empty(&path);
    let source = root().join("examples/import/plate_outline.dxf");
    let original = std::fs::read(&source).expect("DXF");
    let inspection: Value = serde_json::from_str(&success(cli(&[
        "inspect-dxf",
        source.to_str().expect("path"),
    ])))
    .expect("inspect");
    assert_eq!(inspection["declared_unit"], "mm");
    assert_eq!(inspection["entities_by_layer"]["OUTLINE"]["LWPOLYLINE"], 1);
    let inspected = request("inspect_dxf", json!({"dxf_path":source}), true);
    assert_eq!(inspected["result"]["isError"], false, "{inspected}");
    let params = json!({"path":path,"dxf_path":source,"sketch_id":"sketch:dxf","feature_id":"feature:sketch_dxf","layers":["OUTLINE"],"dry_run":true});
    let dry = request("opencad.import_dxf", params.clone(), false);
    assert!(dry["error"].is_null(), "{dry}");
    assert_eq!(dry["result"]["verification"]["passed"], true);
    assert_eq!(before, read_ocad(&path).expect("dry run unchanged"));
    let mut apply = params;
    apply["dry_run"] = json!(false);
    let applied = request("opencad.import_dxf", apply, false);
    assert!(applied["error"].is_null(), "{applied}");
    assert_eq!(applied["result"]["primitive_count"], 4);
    assert_eq!(applied["result"]["entity_count"], 8);
    assert_eq!(applied["result"]["history"]["can_undo"], true);
    let imported = read_ocad(&path).expect("imported");
    assert_eq!(imported.sketches.len(), 1);
    let undone = request(
        "opencad.history_undo_document",
        json!({"path":path,"history":applied["result"]["history"]["history"]}),
        false,
    );
    assert!(undone["error"].is_null(), "{undone}");
    assert_eq!(before, read_ocad(&path).expect("undo"));
    let redone = request(
        "opencad.history_redo_document",
        json!({"path":path,"history":undone["result"]["history"]}),
        false,
    );
    assert!(redone["error"].is_null(), "{redone}");
    assert_eq!(imported, read_ocad(&path).expect("redo"));
    assert_eq!(
        original,
        std::fs::read(&source).expect("DXF source unchanged")
    );
}
#[test]
fn native_rectangle_circle_and_arc_extrusions_match_analytic_volumes() {
    let bodies=[("0\nLWPOLYLINE\n8\nOUTLINE\n90\n4\n70\n1\n10\n0\n20\n0\n10\n80\n20\n0\n10\n80\n20\n60\n10\n0\n20\n60\n",0.08*0.06*0.006),
        ("0\nCIRCLE\n8\nOUTLINE\n10\n20\n20\n20\n40\n10\n",std::f64::consts::PI*0.01*0.01*0.006),
        ("0\nARC\n8\nOUTLINE\n10\n0\n20\n0\n40\n20\n50\n0\n51\n90\n0\nLINE\n8\nOUTLINE\n10\n0\n20\n20\n11\n0\n21\n0\n0\nLINE\n8\nOUTLINE\n10\n0\n20\n0\n11\n20\n21\n0\n",std::f64::consts::PI*0.02*0.02*0.006/4.)];
    for (entities, expected) in bodies {
        let dir = tempfile::tempdir().expect("temp");
        let path = dir.path().join("part.ocad.d");
        empty(&path);
        let source = dir.path().join("source.dxf");
        std::fs::write(&source,format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n")).expect("DXF");
        import(&path, &source);
        extrude(&path);
        let output = success(cli(&["regen", path.to_str().expect("path")]));
        let volume: f64 = output
            .lines()
            .find_map(|line| line.strip_prefix("volume_m3:"))
            .expect("volume")
            .trim()
            .parse()
            .expect("number");
        assert!((volume - expected).abs() < 1e-12, "{volume} vs {expected}");
    }
}
#[test]
fn unknown_selected_geometry_unit_conflicts_and_id_collisions_are_atomic() {
    let dir = tempfile::tempdir().expect("temp");
    let path = dir.path().join("part.ocad.d");
    let before = empty(&path);
    let source = root().join("examples/import/plate_outline.dxf");
    for extra in [
        json!({"layers":["OUTLINE","LABELS"]}),
        json!({"source_unit":"m"}),
        json!({"layers":["missing"]}),
    ] {
        let mut params = json!({"path":path,"dxf_path":source,"sketch_id":"sketch:dxf","feature_id":"feature:sketch_dxf","layers":["OUTLINE"]});
        for (k, v) in extra.as_object().expect("object") {
            params[k] = v.clone();
        }
        let result = request("import_dxf", params, true);
        assert_eq!(result["result"]["isError"], true, "{result}");
        assert_eq!(before, read_ocad(&path).expect("unchanged"));
    }
    import(&path, &source);
    let after = read_ocad(&path).expect("after");
    assert!(!cli(&[
        "import-dxf",
        path.to_str().expect("path"),
        source.to_str().expect("path"),
        "--sketch-id",
        "sketch:dxf",
        "--id",
        "feature:sketch_dxf",
        "--layers",
        "OUTLINE"
    ])
    .status
    .success());
    assert_eq!(after, read_ocad(&path).expect("collision unchanged"));
}
#[test]
#[ignore = "generate verified DXF import example"]
fn write_dxf_import_example() {
    let path = root().join("examples/dxf_import_plate.ocad.d");
    empty(&path);
    import(&path, &root().join("examples/import/plate_outline.dxf"));
    extrude(&path);
    let doc = read_ocad(&path).expect("doc");
    let patch = authoring_patch(&document_design_state(&doc)).expect("patch");
    std::fs::write(
        root().join("examples/agent/create_dxf_import_plate_patch.json"),
        serde_json::to_string_pretty(&patch).expect("json") + "\n",
    )
    .expect("patch");
}
