//! Actual typed arc intent, native physical output and atomic source history.
#[test]
fn actual_mcp_pointer_arc_intent_matches_agent_and_independent_placed_solids() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_pointer_arc_mcp.py"))
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
    assert_eq!(report["cases"].as_array().unwrap().len(), 10);
    for key in [
        "typed_mcp_agent_parity",
        "readonly_atomic_refusal",
        "single_batch_undo_redo",
        "independent_source_and_physical_eval",
        "equal_volume_wrong_placement_refused",
        "invalid_source_decoys_refused",
        "wrong_branch_physical_output_refused",
    ] {
        assert_eq!(report[key], true, "{key}");
    }
}
