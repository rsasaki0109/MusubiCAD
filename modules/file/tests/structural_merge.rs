//! ADR-013 slice 5: structural semantic merge and patch rebase.

use std::path::PathBuf;

use opencad_ai::{
    build_patch_candidate, canonical_design_state_bytes, rebase_patch, semantic_three_way_merge,
    ConflictKind, ConflictReason, DesignPatch, DesignState,
};
use opencad_file::{document_design_state, read_ocad};
use serde_json::json;

fn bracket() -> DesignState {
    let doc =
        read_ocad(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d"))
            .expect("read bracket");
    document_design_state(&doc)
}

fn patch(operations: serde_json::Value) -> DesignPatch {
    serde_json::from_value(json!({ "operations": operations })).expect("patch json")
}

fn apply(state: &DesignState, operations: serde_json::Value) -> DesignState {
    build_patch_candidate(state, &patch(operations)).expect("apply")
}

fn edge_feature(id: &str, kind: &str) -> serde_json::Value {
    let definition = match kind {
        "fillet" => json!({ "type": "fillet", "target_feature": "feature:hole_mount",
                            "radius": { "value_si": 0.001 }, "radius_expr": "fillet_radius",
                            "edge_ref": "ref:edge:bracket_top_front", "edge_selector": "top_perimeter" }),
        _ => json!({ "type": "chamfer", "target_feature": "feature:hole_mount",
                     "distance": { "value_si": 0.001 }, "distance_expr": "chamfer_distance",
                     "edge_ref": "ref:edge:bracket_top_front", "edge_selector": "top_perimeter" }),
    };
    json!({ "type": "add_feature", "position": { "after": "feature:hole_mount" },
            "node": { "id": id, "name": id, "definition": definition } })
}

#[test]
fn independent_structural_edits_merge_and_keep_both_sides() {
    let base = bracket();
    let ours = apply(&base, json!([edge_feature("feature:fillet_a", "fillet")]));
    let theirs = apply(
        &base,
        json!([
            { "type": "add_parameter", "id": "param:rib", "name": "rib", "expr": "4 mm" },
            { "type": "add_sketch_entity", "sketch_id": "sketch:base",
              "entity": { "type": "point", "id": "ent:datum", "construction": true, "x": 0.04, "y": 0.03 } }
        ]),
    );
    let result = semantic_three_way_merge(&base, &ours, &theirs);
    assert!(result.conflicts.is_empty(), "{:?}", result.conflicts);
    let merged = result.merged.expect("merged");
    assert!(merged.parameters.get("param:rib").is_some());
    assert!(merged
        .feature_nodes
        .iter()
        .any(|node| node.id == "feature:fillet_a"));
    // Theirs' sketch edit survives the merge (it used to be dropped).
    assert!(merged.sketches[0].find_entity("ent:datum").is_some());
    assert_eq!(
        merged.feature_order.last().map(String::as_str),
        Some("feature:fillet_a")
    );
    merged.derive_feature_graph().expect("valid merged graph");
}

#[test]
fn merge_result_does_not_depend_on_which_side_is_ours() {
    let base = bracket();
    let left = apply(&base, json!([edge_feature("feature:fillet_a", "fillet")]));
    let right = apply(&base, json!([edge_feature("feature:chamfer_b", "chamfer")]));
    let one = semantic_three_way_merge(&base, &left, &right)
        .merged
        .expect("merge one way");
    let other = semantic_three_way_merge(&base, &right, &left)
        .merged
        .expect("merge other way");
    assert_eq!(
        canonical_design_state_bytes(&one).expect("bytes"),
        canonical_design_state_bytes(&other).expect("bytes")
    );
    // Same-anchor inserts are ordered by feature ID after the anchor.
    assert_eq!(
        &one.feature_order[3..],
        &[
            "feature:hole_mount",
            "feature:chamfer_b",
            "feature:fillet_a"
        ]
    );
}

#[test]
fn structural_merge_conflicts_are_typed() {
    let base = apply(
        &bracket(),
        json!([
            { "type": "add_parameter", "id": "param:rib", "name": "rib", "expr": "4 mm" }
        ]),
    );

    let add_a = apply(
        &base,
        json!([{ "type": "add_parameter", "id": "param:web", "name": "web", "expr": "1 mm" }]),
    );
    let add_b = apply(
        &base,
        json!([{ "type": "add_parameter", "id": "param:web", "name": "web", "expr": "2 mm" }]),
    );
    let conflicts = semantic_three_way_merge(&base, &add_a, &add_b).conflicts;
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::Parameter);
    assert_eq!(conflicts[0].reason, Some(ConflictReason::AddAdd));

    let removed = apply(
        &base,
        json!([{ "type": "remove_parameter", "id": "param:rib" }]),
    );
    let modified = apply(
        &base,
        json!([{ "type": "set_parameter", "id": "param:rib", "expr": "5 mm" }]),
    );
    let conflicts = semantic_three_way_merge(&base, &removed, &modified).conflicts;
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].reason, Some(ConflictReason::RemoveModify));

    let to_start = apply(
        &base,
        json!([
            { "type": "move_feature", "id": "feature:sketch_hole", "position": { "at": "start" } }
        ]),
    );
    let before_extrude = apply(
        &base,
        json!([
            { "type": "move_feature", "id": "feature:sketch_hole", "position": { "before": "feature:extrude_base" } }
        ]),
    );
    let conflicts = semantic_three_way_merge(&base, &to_start, &before_extrude).conflicts;
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].id, "feature_order");
    assert_eq!(conflicts[0].reason, Some(ConflictReason::Order));
}

#[test]
fn a_merge_that_combines_into_an_invalid_state_is_refused() {
    let base = bracket();
    let removes_hole = apply(
        &base,
        json!([{ "type": "remove_feature", "id": "feature:hole_mount" }]),
    );
    let fillets_hole = apply(&base, json!([edge_feature("feature:fillet_a", "fillet")]));
    let result = semantic_three_way_merge(&base, &removes_hole, &fillets_hole);
    assert!(result.merged.is_none());
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(
        result.conflicts[0].reason,
        Some(ConflictReason::InvalidResult)
    );
    assert!(result.conflicts[0]
        .theirs
        .as_deref()
        .unwrap_or_default()
        .contains("feature:hole_mount"));
}

#[test]
fn rebase_drops_additions_the_new_base_already_contains() {
    let base = bracket();
    let ours = patch(json!([
        { "type": "add_parameter", "id": "param:rib", "name": "rib", "expr": "4 mm" },
        { "type": "set_parameter", "id": "param:width", "expr": "90 mm" }
    ]));
    let new_base = apply(
        &base,
        json!([
            { "type": "add_parameter", "id": "param:rib", "name": "rib", "expr": "4 mm" }
        ]),
    );
    let rebased = rebase_patch(&ours, &base, &new_base).expect("rebase");
    assert_eq!(rebased.operations.len(), 1);
    build_patch_candidate(&new_base, &rebased).expect("rebased patch applies");
}

#[test]
fn rebase_reports_typed_structural_conflicts() {
    let base = apply(
        &bracket(),
        json!([
            { "type": "add_parameter", "id": "param:rib", "name": "rib", "expr": "4 mm" }
        ]),
    );

    let remove_rib = patch(json!([{ "type": "remove_parameter", "id": "param:rib" }]));
    let changed = apply(
        &base,
        json!([{ "type": "set_parameter", "id": "param:rib", "expr": "6 mm" }]),
    );
    let conflicts = rebase_patch(&remove_rib, &base, &changed).expect_err("remove/modify");
    assert_eq!(conflicts[0].reason, Some(ConflictReason::RemoveModify));

    let add_after_hole = patch(json!([edge_feature("feature:fillet_a", "fillet")]));
    let hole_gone = apply(
        &base,
        json!([
            { "type": "remove_feature", "id": "feature:hole_mount" }
        ]),
    );
    let conflicts = rebase_patch(&add_after_hole, &base, &hole_gone).expect_err("anchor");
    assert!(conflicts.iter().any(|conflict| conflict.reason
        == Some(ConflictReason::AnchorMissing)
        && conflict.id == "feature:fillet_a"));

    // Independent targets that combine into an invalid patch on the new base.
    let remove_radius = patch(json!([
        { "type": "remove_parameter", "id": "param:fillet_radius" }
    ]));
    let uses_radius = apply(&base, json!([edge_feature("feature:fillet_a", "fillet")]));
    let conflicts = rebase_patch(&remove_radius, &base, &uses_radius).expect_err("invalid");
    assert_eq!(conflicts[0].reason, Some(ConflictReason::InvalidResult));
    assert!(conflicts[0]
        .theirs
        .as_deref()
        .unwrap_or_default()
        .contains("feature:fillet_a"));
}
