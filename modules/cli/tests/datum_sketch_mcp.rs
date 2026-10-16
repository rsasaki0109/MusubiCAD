//! Actual native MCP/Agent source-bound workplane and independent physical eval.
#[test]
fn actual_mcp_datum_sketches_preserve_association_and_physical_motion() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python")
        .arg(root.join("tools/test_datum_sketch_mcp.py"))
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
    assert_eq!(report["cases"].as_array().unwrap().len(), 5);
    for key in [
        "typed_mcp_agent_parity",
        "source_association_and_parameter_motion",
        "readonly_atomic_identity_stale_refusal",
        "canonical_undo_redo",
        "independent_fresh_source_and_physical_eval",
        "equal_volume_wrong_placement_refused",
    ] {
        assert_eq!(report[key], true, "{key}");
    }
}
