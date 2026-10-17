//! Actual selected constraint gateways, source intent and independently placed physics.
#[test]
fn actual_mcp_constraints_match_agent_diagnostics_history_and_physical_oracles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_selected_constraint_mcp.py"))
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
        "typed_mcp_agent_parity",
        "readonly_atomic_refusal",
        "actual_backend_dof_and_dimensions",
        "constraint_remove_and_undo_redo",
        "independent_source_and_physical_eval",
        "equal_volume_wrong_placement_refused",
        "direct_radius_source_decoy_refused",
    ] {
        assert_eq!(report[key], true, "{key}");
    }
}
