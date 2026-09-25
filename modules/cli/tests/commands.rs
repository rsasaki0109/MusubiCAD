//! CLI surface regression tests for help formatting and parameter inspection.

use std::path::PathBuf;
use std::process::Command;

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_opencad"))
        .args(args)
        .output()
        .expect("run opencad")
}

fn bracket_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d")
}

#[test]
fn help_heading_and_commands_have_expected_indentation() {
    let output = run_cli(&["--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("\nCOMMANDS:\n"));
    assert!(stdout.contains("\n    help        Show this help\n"));
}

#[test]
fn params_json_lists_evaluated_width_with_explicit_units() {
    let fixture = bracket_fixture();
    let output = run_cli(&["params", fixture.to_str().expect("fixture path"), "--json"]);
    assert!(
        output.status.success(),
        "params failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("params JSON");
    let width = json
        .as_array()
        .expect("parameter rows")
        .iter()
        .find(|row| row["id"] == "param:width")
        .expect("width row");
    assert_eq!(width["expr"], "80 mm");
    assert_eq!(width["value_mm"], 80.0);
}

/// `opencad merge` writes back every merged collection: structural edits from
/// both sides survive, including theirs' sketch edits, and the Feature Graph
/// is re-derived when features change (ADR-013 slice 5).
#[test]
fn merge_command_keeps_structural_edits_from_both_sides() {
    use opencad_ai::DesignPatch;
    use opencad_file::{apply_patch_to_document, read_ocad, write_expanded_dir};

    let base = read_ocad(bracket_fixture()).expect("read bracket");
    let edit = |operations: serde_json::Value| {
        let patch: DesignPatch =
            serde_json::from_value(serde_json::json!({ "operations": operations }))
                .expect("patch json");
        let mut doc = base.clone();
        apply_patch_to_document(&mut doc, &patch).expect("apply");
        doc
    };
    let ours = edit(serde_json::json!([{
        "type": "add_feature",
        "position": { "after": "feature:hole_mount" },
        "node": { "id": "feature:fillet_a", "name": "Fillet A", "definition": {
            "type": "fillet", "target_feature": "feature:hole_mount",
            "radius": { "value_si": 0.001 }, "radius_expr": "fillet_radius",
            "edge_ref": "ref:edge:bracket_top_front", "edge_selector": "top_perimeter" } }
    }]));
    let theirs = edit(serde_json::json!([{
        "type": "add_sketch_entity", "sketch_id": "sketch:base",
        "entity": { "type": "point", "id": "ent:datum", "construction": true, "x": 0.04, "y": 0.03 }
    }]));

    let dir = tempfile::tempdir().expect("tempdir");
    let path = |name: &str| dir.path().join(name);
    write_expanded_dir(path("base.ocad.d"), &base).expect("write base");
    write_expanded_dir(path("ours.ocad.d"), &ours).expect("write ours");
    write_expanded_dir(path("theirs.ocad.d"), &theirs).expect("write theirs");
    let arg = |name: &str| path(name).to_string_lossy().to_string();
    let output = run_cli(&[
        "merge",
        &arg("base.ocad.d"),
        &arg("ours.ocad.d"),
        &arg("theirs.ocad.d"),
        &arg("merged.ocad.d"),
    ]);
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let merged = read_ocad(path("merged.ocad.d")).expect("read merged");
    assert!(merged.sketches[0].find_entity("ent:datum").is_some());
    assert!(merged
        .feature_nodes
        .iter()
        .any(|node| node.id == "feature:fillet_a"));
    assert!(merged
        .feature_graph
        .dependency_edges()
        .iter()
        .any(|edge| edge.source == "feature:hole_mount" && edge.target == "feature:fillet_a"));
}
