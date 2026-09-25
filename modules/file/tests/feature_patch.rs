//! ADR-013 slice 4: structural feature and semantic-reference operations.

use std::path::PathBuf;

use opencad_ai::{build_patch_candidate, rebase_patch, ConflictKind, DesignPatch, DesignState};
use opencad_feature::FeatureRegistry;
use opencad_file::{
    apply_patch_to_document, document_design_state, dry_run_patch_document, read_ocad, OcadDocument,
};
use opencad_geometry::MockGeometryKernel;
use opencad_graph::SemanticChange;
use serde_json::json;

fn bracket() -> OcadDocument {
    read_ocad(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket.ocad.d"))
        .expect("read bracket")
}

fn patch(operations: serde_json::Value) -> DesignPatch {
    serde_json::from_value(json!({ "operations": operations })).expect("patch json")
}

fn candidate(
    doc: &OcadDocument,
    operations: serde_json::Value,
) -> opencad_core::Result<DesignState> {
    build_patch_candidate(&document_design_state(doc), &patch(operations))
}

fn rejection(doc: &OcadDocument, operations: serde_json::Value) -> String {
    candidate(doc, operations)
        .expect_err("patch must be rejected")
        .to_string()
}

fn top_fillet() -> serde_json::Value {
    json!({
        "type": "add_feature",
        "position": { "after": "feature:hole_mount" },
        "node": {
            "id": "feature:fillet_top_front",
            "name": "Top Front Fillet",
            "definition": {
                "type": "fillet",
                "target_feature": "feature:hole_mount",
                "radius": { "value_si": 0.001 },
                "radius_expr": "fillet_radius",
                "edge_ref": "ref:edge:bracket_top_front",
                "edge_selector": "top_perimeter"
            }
        }
    })
}

#[test]
fn an_added_feature_is_diffed_predicted_persisted_and_regenerated() {
    let doc = bracket();
    let report = dry_run_patch_document(&doc, &patch(json!([top_fillet()])));
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert_eq!(
        report.diff.changes,
        vec![SemanticChange::FeatureAdded {
            id: "feature:fillet_top_front".into(),
            feature_type: "fillet".into()
        }]
    );
    assert_eq!(
        report.impact.predicted_dirty_nodes,
        vec!["feature:fillet_top_front"]
    );

    let mut edited = doc.clone();
    apply_patch_to_document(&mut edited, &patch(json!([top_fillet()]))).expect("apply");
    assert_eq!(
        edited
            .feature_graph
            .ordered_ids()
            .last()
            .map(String::as_str),
        Some("feature:fillet_top_front")
    );
    assert!(edited
        .feature_graph
        .dependency_edges()
        .iter()
        .any(
            |edge| edge.source == "feature:hole_mount" && edge.target == "feature:fillet_top_front"
        ));
    // Value edits never rewrite the persisted graph; feature edits re-derive it.
    let mut value_edit = doc.clone();
    apply_patch_to_document(
        &mut value_edit,
        &DesignPatch::set_parameter("param:width", "90 mm"),
    )
    .expect("value edit");
    assert_eq!(value_edit.feature_graph, doc.feature_graph);

    let parameters = edited.parameters.clone();
    let semantic_refs = edited.semantic_refs.clone();
    let mut model = edited.into_part_model();
    let report = model
        .regenerate(
            &MockGeometryKernel::new(),
            &FeatureRegistry::with_defaults(),
            Some(&parameters),
            Some(&semantic_refs),
        )
        .expect("regenerate");
    assert_eq!(
        report.regenerated.last().map(String::as_str),
        Some("feature:fillet_top_front")
    );
}

#[test]
fn removing_a_used_feature_lists_every_dependent() {
    let message = rejection(
        &bracket(),
        json!([{ "type": "remove_feature", "id": "feature:extrude_base" }]),
    );
    assert!(
        message.contains("cannot remove feature 'feature:extrude_base'"),
        "{message}"
    );
    assert!(
        message.contains("feature feature:hole_mount (target_feature)"),
        "{message}"
    );
    assert!(
        message.contains("semantic reference ref:face:bracket_top"),
        "{message}"
    );
    assert!(
        message.contains("semantic reference ref:edge:bracket_top_front"),
        "{message}"
    );
}

#[test]
fn a_leaf_feature_its_sketch_feature_and_its_sketch_are_removed_together() {
    let doc = bracket();
    let operations = json!([
        { "type": "remove_sketch", "id": "sketch:hole" },
        { "type": "remove_feature", "id": "feature:sketch_hole" },
        { "type": "remove_feature", "id": "feature:hole_mount" }
    ]);
    let after = candidate(&doc, operations.clone()).expect("remove hole");
    assert_eq!(
        after.feature_order,
        vec!["feature:sketch_base", "feature:extrude_base"]
    );
    let graph = after.derive_feature_graph().expect("graph");
    assert_eq!(graph.dependency_edges().len(), 1);

    let report = dry_run_patch_document(&doc, &patch(operations));
    assert!(report
        .diff
        .changes
        .contains(&SemanticChange::FeatureRemoved {
            id: "feature:hole_mount".into()
        }));
    assert!(report
        .diff
        .changes
        .contains(&SemanticChange::SketchRemoved {
            id: "sketch:hole".into()
        }));
}

#[test]
fn moves_must_keep_inputs_before_consumers() {
    let doc = bracket();
    let after = candidate(
        &doc,
        json!([{ "type": "move_feature", "id": "feature:sketch_hole", "position": { "at": "start" } }]),
    )
    .expect("independent sketch can move");
    assert_eq!(after.feature_order[0], "feature:sketch_hole");
    let report = dry_run_patch_document(
        &doc,
        &patch(
            json!([{ "type": "move_feature", "id": "feature:sketch_hole",
                        "position": { "at": "start" } }]),
        ),
    );
    assert_eq!(
        report.diff.changes,
        vec![SemanticChange::FeatureMoved {
            id: "feature:sketch_hole".into(),
            before: "2".into(),
            after: "0".into()
        }]
    );
    assert!(report.impact.predicted_dirty_nodes.is_empty());

    let message = rejection(
        &doc,
        json!([{ "type": "move_feature", "id": "feature:hole_mount",
                 "position": { "before": "feature:extrude_base" } }]),
    );
    assert!(
        message.contains("places 'feature:hole_mount' before its input 'feature:extrude_base'"),
        "{message}"
    );
}

#[test]
fn replacement_suppression_and_input_validation() {
    let doc = bracket();
    let deeper_hole = json!({
        "type": "hole",
        "sketch_feature": "feature:sketch_hole",
        "profile_ref": "sketch:hole/profile:outer",
        "depth": { "type": "distance", "length": { "value_si": 0.004 } },
        "target_feature": "feature:extrude_base",
        "face_ref": "ref:face:bracket_top",
        "depth_expr": "thickness / 2"
    });
    let after = candidate(
        &doc,
        json!([
            { "type": "replace_feature_definition", "id": "feature:hole_mount", "definition": deeper_hole },
            { "type": "set_feature_suppressed", "id": "feature:hole_mount", "suppressed": true }
        ]),
    )
    .expect("replace and suppress");
    let hole = after
        .feature_nodes
        .iter()
        .find(|node| node.id == "feature:hole_mount")
        .expect("hole");
    assert!(hole.suppressed);
    assert!(
        after
            .derive_feature_graph()
            .expect("graph")
            .get("feature:hole_mount")
            .expect("entry")
            .suppressed
    );

    let type_change = rejection(
        &doc,
        json!([{ "type": "replace_feature_definition", "id": "feature:hole_mount",
                 "definition": { "type": "sketch", "sketch_id": "sketch:hole" } }]),
    );
    assert!(type_change.contains("requires remove_feature and add_feature"));

    let mut bad_fillet = top_fillet();
    bad_fillet["node"]["definition"]["target_feature"] = json!("feature:nowhere");
    bad_fillet["node"]["definition"]["radius_expr"] = json!("edge_radius");
    bad_fillet["node"]["definition"]["edge_ref"] = json!("ref:edge:missing");
    let message = rejection(&doc, json!([bad_fillet]));
    assert!(
        message.contains("references unknown parameter 'edge_radius'"),
        "{message}"
    );
    assert!(
        message.contains("unknown semantic reference 'ref:edge:missing'"),
        "{message}"
    );
    assert!(
        message.contains("references unknown feature 'feature:nowhere'"),
        "{message}"
    );

    let open_profile = rejection(
        &doc,
        json!([{
            "type": "add_feature",
            "position": { "at": "end" },
            "node": {
                "id": "feature:pad",
                "name": "Pad",
                "definition": {
                    "type": "extrude",
                    "sketch_feature": "feature:sketch_base",
                    "profile_ref": "sketch:base/profile:missing",
                    "extent": { "type": "distance", "length": { "value_si": 0.002 } },
                    "operation": "new_body"
                }
            }
        }]),
    );
    assert!(
        open_profile.contains("does not resolve in sketch 'sketch:base'"),
        "{open_profile}"
    );
}

#[test]
fn semantic_references_are_added_and_removed_fail_closed() {
    let doc = bracket();
    let used = rejection(
        &doc,
        json!([{ "type": "remove_semantic_ref", "ref_id": "ref:face:bracket_top" }]),
    );
    assert!(
        used.contains("still used by feature feature:hole_mount (face_ref)"),
        "{used}"
    );

    let orphan = rejection(
        &doc,
        json!([{ "type": "add_semantic_ref", "topo_ref": {
            "ref_id": "ref:face:ghost", "kind": "face",
            "semantic": { "created_by": "feature:ghost", "role": "top" } } }]),
    );
    assert!(orphan.contains("created by unknown feature 'feature:ghost'"));

    let after = candidate(
        &doc,
        json!([{ "type": "add_semantic_ref", "topo_ref": {
            "ref_id": "ref:face:hole_wall", "kind": "face",
            "semantic": { "created_by": "feature:hole_mount", "role": "wall" } } }]),
    )
    .expect("add ref");
    assert_eq!(after.semantic_refs.len(), 3);
}

#[test]
fn feature_ids_follow_the_stable_id_rules() {
    let doc = bracket();
    let mut duplicate = top_fillet();
    duplicate["node"]["id"] = json!("feature:hole_mount");
    assert!(rejection(&doc, json!([duplicate])).contains("already exists"));

    let mut bad_id = top_fillet();
    bad_id["node"]["id"] = json!("feature:Fillet");
    assert!(rejection(&doc, json!([bad_id])).contains("invalid id"));

    let mut bad_anchor = top_fillet();
    bad_anchor["position"] = json!({ "after": "feature:missing" });
    assert!(rejection(&doc, json!([bad_anchor])).contains("position anchor 'feature:missing'"));
}

#[test]
fn feature_operations_need_the_feature_order() {
    let doc = bracket();
    let in_memory = DesignState::with_models(
        doc.parameters.clone(),
        doc.feature_nodes.clone(),
        doc.semantic_refs.clone(),
        None,
        None,
    );
    let error = build_patch_candidate(&in_memory, &patch(json!([top_fillet()])))
        .expect_err("no order")
        .to_string();
    assert!(error.contains("including feature order"));
}

#[test]
fn rebase_detects_concurrent_edits_to_the_same_feature() {
    let doc = bracket();
    let base = document_design_state(&doc);
    let ours = patch(json!([
        { "type": "set_feature_suppressed", "id": "feature:hole_mount", "suppressed": true }
    ]));
    let theirs = candidate(
        &doc,
        json!([{ "type": "replace_feature_definition", "id": "feature:hole_mount", "definition": {
            "type": "hole",
            "sketch_feature": "feature:sketch_hole",
            "profile_ref": "sketch:hole/profile:outer",
            "depth": { "type": "distance", "length": { "value_si": 0.003 } },
            "target_feature": "feature:extrude_base",
            "face_ref": "ref:face:bracket_top",
            "depth_expr": "thickness / 2"
        } }]),
    )
    .expect("theirs");
    let conflicts = rebase_patch(&ours, &base, &theirs).expect_err("conflict");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::Feature);
    assert_eq!(conflicts[0].id, "feature:hole_mount");
}

/// MCAD-P7-006: impossible feature values are rejected at dry-run, including
/// value edits that make an existing feature degenerate.
#[test]
fn degenerate_feature_values_are_rejected_before_regeneration() {
    let doc = bracket();
    let zero = dry_run_patch_document(&doc, &DesignPatch::set_parameter("param:thickness", "0 mm"));
    assert!(!zero.validation.is_ok());
    let text = format!("{:?}", zero.validation);
    assert!(text.contains("extrude length must be at least"), "{text}");
    assert!(text.contains("hole depth must be at least"), "{text}");

    let mut flat_fillet = top_fillet();
    flat_fillet["node"]["definition"]["radius_expr"] = json!("0 mm");
    let message = rejection(&doc, json!([flat_fillet]));
    assert!(
        message.contains("fillet radius must be at least"),
        "{message}"
    );

    // A valid value edit still passes.
    let ok = dry_run_patch_document(&doc, &DesignPatch::set_parameter("param:thickness", "8 mm"));
    assert!(ok.validation.is_ok(), "{:?}", ok.validation);
}
