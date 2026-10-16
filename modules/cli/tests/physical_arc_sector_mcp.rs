//! Actual verified sector patching, native output and atomic source history.
#[test]
fn actual_mcp_arc_sector_matches_agent_and_independent_physical_evidence() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_physical_arc_sector_mcp.py"))
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
    assert_eq!(report["cases"].as_array().unwrap().len(), 5);
    assert_eq!(report["model_turns"], 0);
    for key in [
        "mcp_agent_parity",
        "verified_readonly_atomic_refusal",
        "single_batch_undo_redo",
        "independent_source_and_physical_eval",
        "equal_volume_wrong_placement_refused",
        "equal_analytic_volume_wrong_circle_refused",
        "invalid_source_decoys_refused",
    ] {
        assert_eq!(report[key], true, "{key}");
    }
}
