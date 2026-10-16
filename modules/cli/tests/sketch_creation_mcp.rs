//! Actual MCP/Agent parity and independent world-placement grading (native OCCT).
#[test]
fn actual_mcp_global_sketch_creation_matches_native_physical_and_history_contract() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_sketch_creation_mcp.py"))
        .arg("--musubicad")
        .arg(env!("CARGO_BIN_EXE_musubicad"))
        .output()
        .expect("actual MCP regression");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cases"].as_array().unwrap().len(), 4);
    for key in [
        "typed_mcp_agent_parity",
        "readonly_and_atomic_refusal",
        "canonical_undo_redo",
        "independent_source_and_physical_eval",
        "equal_volume_wrong_plane_refused",
    ] {
        assert_eq!(report[key], true, "{key}");
    }
}
