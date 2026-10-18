//! ADR-110 actual stdio MCP/Agent offsets and independent source/physical grading.
#[test]
fn selected_offsets_preserve_independent_source_history_and_placed_physics() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_selected_offsets_mcp.py"))
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
        "ordered_independent_copies_and_generated_owners",
        "typed_mcp_agent_parity",
        "readonly_atomic_refusal_history",
        "independent_source_physics_and_adversaries",
    ] {
        assert_eq!(report[key], true, "{key}");
    }
}
