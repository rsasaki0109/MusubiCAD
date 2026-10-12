//! Verified real Agent/CLI poses, transactions and faithful/refused exports.
use opencad_file::read_ocad;
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
fn request(method: &str, params: Value) -> Value {
    let mut c = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .arg("agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("agent");
    writeln!(
        c.stdin.take().expect("stdin"),
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
    )
    .expect("send");
    let out = c.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("JSON")
}
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for e in std::fs::read_dir(from).expect("dir") {
        let e = e.expect("entry");
        if e.file_type().expect("type").is_dir() {
            copy_tree(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).expect("copy");
        }
    }
}
fn follower(result: &Value) -> f64 {
    let p = result["result"]["positions"]
        .as_array()
        .expect("positions")
        .iter()
        .find(|p| p["joint_id"] == "joint:elbow")
        .expect("elbow");
    assert_eq!(p["derived"], true);
    p["position_rad"].as_f64().expect("angle")
}
#[test]
fn joint_pose_api_patch_history_and_invalid_units_are_atomic() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("arm.ocad.d");
    copy_tree(&root.join("examples/coupled_arm.ocad.d"), &path);
    let original = read_ocad(&path).expect("doc");
    let query = json!({"path":path,"positions":{"joint:shoulder":"90deg"}});
    let first = request("opencad.inspect_joint_pose", query.clone());
    assert!(first["error"].is_null(), "{}", first["error"]);
    assert!((follower(&first) + std::f64::consts::FRAC_PI_4).abs() < 1e-12);
    assert_eq!(read_ocad(&path).expect("readonly"), original);
    for positions in [
        json!({"joint:shoulder":"90"}),
        json!({"joint:shoulder":"1m"}),
        json!({"joint:missing":"1rad"}),
        json!({"joint:shoulder":"90deg","joint:elbow":"0rad"}),
        json!({"joint:shoulder":"180deg"}),
    ] {
        let bad = request(
            "opencad.inspect_joint_pose",
            json!({"path":path,"positions":positions}),
        );
        assert!(!bad["error"].is_null());
        assert_eq!(read_ocad(&path).expect("readonly"), original);
    }
    let coupling = json!({"type":"gear","id":"coupling:gear","driver_joint":"joint:shoulder","follower_joint":"joint:elbow","ratio":-0.25});
    let patch = json!({"operations":[{"type":"set_joint_coupling","coupling":coupling}]});
    let dry = request(
        "opencad.patch_dry_run_document",
        json!({"path":path,"patch":patch}),
    );
    assert!(dry["error"].is_null(), "{}", dry["error"]);
    assert_eq!(read_ocad(&path).expect("dry"), original);
    let applied = request(
        "opencad.patch_apply_document",
        json!({"path":path,"patch":patch}),
    );
    assert!(applied["error"].is_null(), "{}", applied["error"]);
    let after = read_ocad(&path).expect("after");
    assert_eq!(
        after.assembly.as_ref().expect("assembly").instances,
        original.assembly.as_ref().expect("original").instances
    );
    let inspected = request("opencad.inspect_joint_pose", query);
    assert!((follower(&inspected) + std::f64::consts::PI / 8.).abs() < 1e-12);
    let undo = request(
        "opencad.history_undo_document",
        json!({"path":path,"history":applied["result"]["history"]}),
    );
    assert!(undo["error"].is_null());
    assert_eq!(read_ocad(&path).expect("undo"), original);
    let redo = request(
        "opencad.history_redo_document",
        json!({"path":path,"history":undo["result"]["history"]}),
    );
    assert!(redo["error"].is_null());
    assert_eq!(read_ocad(&path).expect("redo"), after);
    let cli = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args([
            "inspect-joint-pose",
            path.to_str().expect("path"),
            "joint:shoulder=90deg",
        ])
        .output()
        .expect("CLI");
    assert!(cli.status.success());
    let cli: Value = serde_json::from_slice(&cli.stdout).expect("JSON");
    assert_eq!(cli, inspected["result"]);
}
#[test]
fn gear_urdf_mimic_animation_and_coupled_html_refusal_are_faithful() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/coupled_arm.ocad.d");
    let temp = tempfile::tempdir().expect("temp");
    let urdf = temp.path().join("arm.urdf");
    let result = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args([
            "export",
            path.to_str().expect("path"),
            urdf.to_str().expect("out"),
        ])
        .output()
        .expect("export");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let xml = std::fs::read_to_string(urdf).expect("URDF");
    assert!(xml.contains("<mimic joint=\"shoulder\" multiplier=\"-0.5\" offset=\"0\"/>"));
    let html = temp.path().join("arm.html");
    let result = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args([
            "export",
            path.to_str().expect("path"),
            html.to_str().expect("out"),
        ])
        .output()
        .expect("HTML");
    assert!(!result.status.success());
    assert!(!html.exists());
    let gif = temp.path().join("coupled.gif");
    let result = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args([
            "animate-joints",
            path.to_str().expect("path"),
            gif.to_str().expect("out"),
            "--pose",
            "joint:shoulder=30deg",
            "--frames-per-move",
            "2",
            "--width",
            "120",
            "--height",
            "90",
        ])
        .output()
        .expect("animation");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(gif.exists());
    let invalid = temp.path().join("invalid.gif");
    let result = Command::new(env!("CARGO_BIN_EXE_musubicad"))
        .args([
            "animate-joints",
            path.to_str().expect("path"),
            invalid.to_str().expect("out"),
            "--pose",
            "joint:elbow=30deg",
        ])
        .output()
        .expect("follower");
    assert!(!result.status.success());
    assert!(!invalid.exists());
}
