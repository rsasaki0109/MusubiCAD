//! Actual native MCP stdio/source-aware evaluation, independent of any model host.
#[test]
fn native_mcp_thread_preview_edit_eval_refusal_and_history() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_nominal_thread_mcp.py"))
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
    let evidence: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(evidence["forced_native_verification"], true);
    assert_eq!(evidence["source_aware_eval_passed"], true);
}
