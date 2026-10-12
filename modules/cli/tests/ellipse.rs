//! Verified ellipse source edits and atomic refusal through the real Agent API.
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
fn verified_ellipse_edits_history_and_unit_failures_preserve_documents() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let original = read_ocad(root.join("examples/ellipse_plate.ocad.d")).expect("example");
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("part.ocad.d");
    write_expanded_dir(&path, &original).expect("write");
    let patch = json!({"operations":[{"type":"set_parameter","id":"param:major_radius","expr":"40 mm"},{"type":"set_parameter","id":"param:axis_angle_rad","expr":"45 deg"}]});
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
    let regenerated = request("opencad.regen_document", json!({"path":path}));
    assert!(regenerated["error"].is_null(), "{regenerated}");
    let volume = regenerated["result"]["volume_m3"].as_f64().expect("volume");
    assert!((volume - std::f64::consts::PI * 0.04 * 0.02 * 0.006).abs() < 1e-12);
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
    for (id, expr) in [
        ("param:minor_radius", "100 mm"),
        ("param:axis_angle_rad", "1 mm"),
        ("param:major_radius", "20 deg"),
        ("param:axis_angle_rad", "major_radius"),
    ] {
        let bad = json!({"operations":[{"type":"set_parameter","id":id,"expr":expr}]});
        let response = request(
            "opencad.patch_apply_document",
            json!({"path":path,"patch":bad}),
        );
        assert!(!response["error"].is_null(), "{id}: {expr}: {response}");
        assert_eq!(read_ocad(&path).expect("unchanged"), after);
    }
}
