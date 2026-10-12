//! Verified associated-source edits and read-only derived inspection.
use opencad_file::{read_ocad, write_expanded_dir};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};
fn request(method: &str, params: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg("agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    writeln!(
        child.stdin.take().expect("stdin"),
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
    )
    .expect("send");
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("JSON")
}
#[test]
fn projected_source_edits_inspection_history_and_refusal_preserve_graph_truth() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let original = read_ocad(root.join("examples/projected_circle_plate.ocad.d")).expect("example");
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("part.ocad.d");
    write_expanded_dir(&path, &original).expect("write");
    let inspected = request(
        "opencad.inspect_projected_sketch",
        json!({"path":path,"feature_id":"feature:projection"}),
    );
    assert!(inspected["error"].is_null(), "{inspected}");
    assert_eq!(inspected["result"]["derived"], true);
    assert_eq!(read_ocad(&path).expect("readonly"), original);
    let patch = json!({"operations":[{"type":"set_parameter","id":"param:radius","expr":"40 mm"}]});
    let dry = request(
        "opencad.patch_dry_run_document",
        json!({"path":path,"patch":patch}),
    );
    assert!(dry["error"].is_null(), "{dry}");
    assert_eq!(read_ocad(&path).expect("dry"), original);
    let applied = request(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":patch}),
    );
    assert!(applied["error"].is_null(), "{applied}");
    let after = read_ocad(&path).expect("after");
    assert_eq!(after.sketches, original.sketches);
    assert_eq!(after.sketches.len(), 1);
    let inspected = request(
        "opencad.inspect_projected_sketch",
        json!({"path":path,"feature_id":"feature:projection"}),
    );
    assert!(inspected["error"].is_null(), "{inspected}");
    let curve = inspected["result"]["sketch"]["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .find(|e| e["id"] == "ent:projected_curve")
        .expect("curve");
    assert!((curve["major_radius"].as_f64().expect("major") - 0.04).abs() < 1e-9);
    assert!(
        (curve["minor_radius"].as_f64().expect("minor") - 0.04 * std::f64::consts::FRAC_PI_6.cos())
            .abs()
            < 1e-9
    );
    let undo = request(
        "opencad.history_undo_document",
        json!({"path":path,"history":applied["result"]["history"]}),
    );
    assert!(undo["error"].is_null(), "{undo}");
    assert_eq!(read_ocad(&path).expect("undo"), original);
    let redo = request(
        "opencad.history_redo_document",
        json!({"path":path,"history":undo["result"]["history"]}),
    );
    assert!(redo["error"].is_null(), "{redo}");
    assert_eq!(read_ocad(&path).expect("redo"), after);
    let bad = request(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":{"operations":[{"type":"remove_sketch_entity","sketch_id":"sketch:source","entity_id":"ent:source_circle"}]},"verify":false}),
    );
    assert!(!bad["error"].is_null(), "{bad}");
    assert_eq!(read_ocad(&path).expect("unchanged"), after);
}
