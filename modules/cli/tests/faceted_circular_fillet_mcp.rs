//! Actual native MCP authoring/edit/refusal/history and independent physical grader.
#[test]
fn native_mcp_faceted_fillet_source_and_physical_phase() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_faceted_fillet_mcp.py"))
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
    assert_eq!(report["forced_native_verification"], true);
    assert_eq!(report["source_and_physical_eval_passed"], true);
    assert_eq!(report["wrong_phase_equal_volume_refused"], true);
}
