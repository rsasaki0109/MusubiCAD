//! ADR-112 native OCCT integration through actual unchanged stdio transports.
#[test]
fn actual_mcp_revolves_match_agent_and_independent_source_physics() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_physical_revolve_mcp.py"))
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
    assert_eq!(report["cases"].as_array().unwrap().len(), 6);
    assert_eq!(report["model_turns"], 0);
    for key in [
        "mcp_agent_parity",
        "readonly_atomic_refusal_history",
        "independent_source_physics",
        "equal_volume_wrong_placement_refused",
        "axis_association_and_source_decoys_refused",
    ] {
        assert_eq!(report[key], true, "{key}");
    }
}
