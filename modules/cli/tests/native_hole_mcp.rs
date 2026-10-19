//! ADR-111 actual stdio typed creation and independent source/contact/solid grading.
#[test]
fn native_hole_controls_share_verified_mcp_agent_creation_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_native_hole_mcp.py"))
        .arg("--musubicad")
        .arg(env!("CARGO_BIN_EXE_musubicad"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cases"].as_array().unwrap().len(), 8);
    assert_eq!(report["model_turns"], 0);
    for key in [
        "typed_mcp_agent_parity",
        "readonly_atomic_refusal_history",
        "independent_source_contacts_physics_and_adversaries",
    ] {
        assert_eq!(report[key], true, "{key}");
    }
}
