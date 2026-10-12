//! Read-only native exploded previews and unchanged mechanical evidence.
use opencad_file::read_ocad;
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
        .expect("agent");
    writeln!(
        child.stdin.take().expect("stdin"),
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
    )
    .expect("write");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).expect("JSON")
}
#[test]
fn preview_is_deterministic_readonly_and_cli_matches_agent() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/robot_arm_assembly.ocad.d");
    let before = read_ocad(&path).expect("doc");
    let params = json!({"path":path,"width":120,"height":90,"exploded_view":"view:exploded"});
    let first = request("opencad.preview_document", params.clone());
    let second = request("opencad.preview_document", params);
    assert!(first["error"].is_null(), "{}", first["error"]);
    assert_eq!(first, second);
    assert_eq!(first["result"]["exploded_view"], "view:exploded");
    let plain = request(
        "opencad.preview_document",
        json!({"path":path,"width":120,"height":90}),
    );
    assert!(plain["error"].is_null());
    assert_ne!(first["result"]["bounds_mm"], plain["result"]["bounds_mm"]);
    assert_eq!(first["result"]["triangles"], plain["result"]["triangles"]);
    let temp = tempfile::tempdir().expect("temp");
    let output = temp.path().join("exploded.png");
    let out = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args([
            "preview",
            path.to_str().expect("path"),
            output.to_str().expect("out"),
            "--width",
            "120",
            "--height",
            "90",
            "--exploded-view",
            "view:exploded",
        ])
        .output()
        .expect("preview");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(first["result"]["png_base64"].as_str().expect("png"))
        .expect("decode");
    assert_eq!(std::fs::read(output).expect("PNG"), bytes);
    let bad = request(
        "opencad.preview_document",
        json!({"path":path,"exploded_view":"view:missing"}),
    );
    assert!(!bad["error"].is_null());
    assert_eq!(read_ocad(path).expect("readonly"), before);
}
#[test]
fn rigid_child_assembly_presentation_keeps_physical_reports() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/bom_nested_assembly.ocad.d");
    let mut doc = read_ocad(&path).expect("nested");
    doc.assembly.as_mut().expect("assembly").exploded_views.push(serde_json::from_value(json!({"id":"view:nested","name":"Rigid child","offsets":[{"instance_id":"instance:left","translation_m":[0.2,0.,0.]}]})).expect("view"));
    let before = doc.clone();
    let normal = opencad_desktop::preview::load_assembly_presentation(
        path.to_str().expect("path"),
        &doc,
        &[],
        None,
    )
    .expect("mechanical");
    let exploded = opencad_desktop::preview::load_assembly_presentation(
        path.to_str().expect("path"),
        &doc,
        &[],
        Some("view:nested"),
    )
    .expect("presentation");
    assert_eq!(normal.interference_count, exploded.interference_count);
    assert_eq!(
        normal.report.scene.bounding_box,
        exploded.report.scene.bounding_box
    );
    assert_eq!(normal.report.scene.mass, exploded.report.scene.mass);
    assert_eq!(normal.report.instance_count, exploded.report.instance_count);
    assert_eq!(normal.report.dof, exploded.report.dof);
    assert_eq!(doc, before);
    assert_ne!(normal.scene, exploded.scene);
}
