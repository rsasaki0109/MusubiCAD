//! ADR-013 slice 2: structural sketch operations on the bracket fixture.

use std::path::PathBuf;

use opencad_ai::{
    build_patch_candidate, rebase_patch, ConflictKind, DesignPatch, DesignState, PatchPrecondition,
};
use opencad_file::expanded_dir::serialize_document_files;
use opencad_file::{
    apply_patch_with_history, document_design_state, dry_run_patch_document, read_ocad,
    write_expanded_dir, DocumentHistory, OcadDocument,
};
use opencad_graph::SemanticChange;
use opencad_sketch::SolveState;
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

fn boss_sketch_operations() -> serde_json::Value {
    json!([
        { "type": "add_sketch", "id": "sketch:boss", "name": "Boss Sketch",
          "workplane": { "type": "global", "plane": "XY" } },
        { "type": "add_sketch_entity", "sketch_id": "sketch:boss",
          "entity": { "type": "point", "id": "ent:boss_center", "x": 0.04, "y": 0.03 } },
        { "type": "add_sketch_entity", "sketch_id": "sketch:boss",
          "entity": { "type": "circle", "id": "ent:boss_circle",
                      "center": "ent:boss_center", "radius": "hole_diameter" } },
        { "type": "add_sketch_constraint", "sketch_id": "sketch:boss",
          "constraint": { "type": "radius", "id": "con:boss_radius",
                          "target": "ent:boss_circle", "expr": "hole_diameter" } }
    ])
}

#[test]
fn a_new_sketch_is_built_from_primitive_operations() {
    let doc = bracket();
    let after = candidate(&doc, boss_sketch_operations()).expect("sketch");
    let boss = after
        .sketches
        .iter()
        .find(|sketch| sketch.id.as_str() == "sketch:boss")
        .expect("boss sketch");
    assert_eq!(boss.entities.len(), 2);
    assert_eq!(boss.constraints.len(), 1);
    assert_eq!(boss.solve_state, SolveState::Unknown);
    assert!(boss.profiles.iter().any(|profile| profile.is_closed()
        && profile.profile_ref.as_deref() == Some("sketch:boss/profile:outer")));

    let report = dry_run_patch_document(&doc, &patch(boss_sketch_operations()));
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert_eq!(
        report.diff.changes,
        vec![SemanticChange::SketchAdded {
            id: "sketch:boss".into()
        }]
    );
}

#[test]
fn a_sketch_edit_dirties_its_sketch_feature_and_downstream_suffix() {
    let doc = bracket();
    let report = dry_run_patch_document(
        &doc,
        &patch(json!([
            { "type": "add_sketch_entity", "sketch_id": "sketch:base",
              "entity": { "type": "point", "id": "ent:datum", "construction": true,
                          "x": 0.04, "y": 0.03 } }
        ])),
    );
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert_eq!(
        report.diff.changes,
        vec![SemanticChange::SketchEntityAdded {
            sketch_id: "sketch:base".into(),
            id: "ent:datum".into()
        }]
    );
    assert_eq!(
        report.impact.directly_affected_nodes,
        vec!["feature:sketch_base"]
    );
    assert_eq!(
        report.impact.predicted_dirty_nodes,
        vec![
            "feature:sketch_base",
            "feature:extrude_base",
            "feature:hole_mount"
        ]
    );
}

#[test]
fn removing_a_point_used_by_lines_lists_every_dangling_reference() {
    let message = rejection(
        &bracket(),
        json!([{ "type": "remove_sketch_entity", "sketch_id": "sketch:base", "entity_id": "ent:c0" }]),
    );
    assert!(
        message.contains("entity 'ent:e0' references missing point 'ent:c0'"),
        "{message}"
    );
    assert!(
        message.contains("entity 'ent:e3' references missing point 'ent:c0'"),
        "{message}"
    );
}

#[test]
fn opening_a_consumed_profile_is_rejected_before_regeneration() {
    let message = rejection(
        &bracket(),
        json!([{ "type": "remove_sketch_entity", "sketch_id": "sketch:base", "entity_id": "ent:e2" }]),
    );
    assert!(
        message.contains("feature 'feature:extrude_base' profile 'sketch:base/profile:outer'"),
        "{message}"
    );
}

#[test]
fn constraints_can_be_removed_and_are_diffed() {
    let doc = bracket();
    let operations = json!([{ "type": "remove_sketch_constraint", "sketch_id": "sketch:base", "constraint_id": "con:width" }]);
    let after = candidate(&doc, operations.clone()).expect("remove constraint");
    assert_eq!(after.sketches[0].constraints.len(), 5);
    let report = dry_run_patch_document(&doc, &patch(operations));
    assert_eq!(
        report.diff.changes,
        vec![SemanticChange::SketchConstraintRemoved {
            sketch_id: "sketch:base".into(),
            id: "con:width".into()
        }]
    );
}

#[test]
fn removing_a_sketch_used_by_a_feature_fails() {
    let message = rejection(
        &bracket(),
        json!([{ "type": "remove_sketch", "id": "sketch:hole" }]),
    );
    assert!(
        message.contains(
            "cannot remove sketch 'sketch:hole': still used by feature feature:sketch_hole"
        ),
        "{message}"
    );
}

#[test]
fn sketch_references_and_expressions_must_resolve() {
    let doc = bracket();
    let unknown_parameter = rejection(
        &doc,
        json!([{ "type": "add_sketch_constraint", "sketch_id": "sketch:hole",
                 "constraint": { "type": "diameter", "id": "con:hole_dia",
                                 "target": "ent:hole_circle", "expr": "bore_size" } }]),
    );
    assert!(unknown_parameter.contains("references unknown parameter 'bore_size'"));

    let missing_entity = rejection(
        &doc,
        json!([{ "type": "add_sketch_constraint", "sketch_id": "sketch:hole",
                 "constraint": { "type": "horizontal", "id": "con:flat", "line": "ent:nothing" } }]),
    );
    assert!(
        missing_entity.contains("constraint 'con:flat' references missing entity 'ent:nothing'")
    );

    let unknown_face = rejection(
        &doc,
        json!([{ "type": "add_sketch", "id": "sketch:top", "name": "Top",
                 "workplane": { "type": "face_ref", "face_ref": "ref:face:missing" } }]),
    );
    assert!(unknown_face.contains("unknown semantic reference 'ref:face:missing'"));
}

#[test]
fn sketch_ids_follow_the_stable_id_rules() {
    let doc = bracket();
    let cases = [
        (
            json!([{ "type": "add_sketch", "id": "sketch:Top", "name": "Top",
                     "workplane": { "type": "global", "plane": "XY" } }]),
            "invalid id 'sketch:Top'",
        ),
        (
            json!([{ "type": "add_sketch", "id": "sketch:hole", "name": "Again",
                     "workplane": { "type": "global", "plane": "XY" } }]),
            "sketch 'sketch:hole' already exists",
        ),
        (
            json!([{ "type": "add_sketch_entity", "sketch_id": "sketch:base",
                     "entity": { "type": "point", "id": "ent:c1", "x": 0.0, "y": 0.0 } }]),
            "entity 'ent:c1' already exists in sketch 'sketch:base'",
        ),
        (
            json!([{ "type": "add_sketch_entity", "sketch_id": "sketch:base",
                     "entity": { "type": "rectangle", "id": "ent:pad",
                                 "origin": [0.0, 0.0], "size": [0.01, 0.01],
                                 "corner_ids": ["ent:c0", "ent:pad_c1", "ent:pad_c2", "ent:pad_c3"] } }]),
            "entity 'ent:c0' already exists in sketch 'sketch:base'",
        ),
        (
            json!([{ "type": "add_sketch", "id": "sketch:tilted", "name": "Tilted",
                     "workplane": { "type": "custom", "origin": [0.0, 0.0, 0.0],
                                    "normal": [0.0, 0.0, 1.0], "x_axis": [0.0, 0.5, 1.0] } }]),
            "must be perpendicular",
        ),
        (
            json!([
                { "type": "remove_sketch_constraint", "sketch_id": "sketch:base", "constraint_id": "con:width" },
                { "type": "add_sketch_constraint", "sketch_id": "sketch:base",
                  "constraint": { "type": "horizontal", "id": "con:width", "line": "ent:e0" } }
            ]),
            "cannot be reused",
        ),
    ];
    for (operations, expected) in cases {
        let message = rejection(&doc, operations.clone());
        assert!(
            message.contains(expected),
            "{operations}: expected '{expected}' in '{message}'"
        );
    }
}

#[test]
fn sketch_and_assertion_preconditions_are_checked() {
    let doc = bracket();
    let state = document_design_state(&doc);
    let guarded = |preconditions: Vec<PatchPrecondition>| DesignPatch {
        preconditions,
        ..DesignPatch::set_parameter("param:width", "90 mm")
    };
    assert!(build_patch_candidate(
        &state,
        &guarded(vec![
            PatchPrecondition::SketchExists {
                id: "sketch:base".into()
            },
            PatchPrecondition::SketchEntityExists {
                sketch_id: "sketch:hole".into(),
                entity_id: "ent:hole_circle".into()
            },
        ])
    )
    .is_ok());

    let error = build_patch_candidate(
        &state,
        &guarded(vec![
            PatchPrecondition::SketchEntityExists {
                sketch_id: "sketch:hole".into(),
                entity_id: "ent:missing".into(),
            },
            PatchPrecondition::AssertionExists {
                id: "assertion:none".into(),
            },
        ]),
    )
    .expect_err("preconditions")
    .to_string();
    assert!(error.contains("assertion 'assertion:none' does not exist"));
    assert!(error.contains("entity 'ent:missing' does not exist in sketch 'sketch:hole'"));
}

#[test]
fn sketch_edits_persist_and_failures_change_nothing() {
    let mut doc = bracket();
    let mut history = DocumentHistory::default();
    apply_patch_with_history(
        &mut doc,
        &patch(boss_sketch_operations()),
        &mut history,
        "boss",
    )
    .expect("apply");

    let dir = tempfile::tempdir().expect("tempdir");
    write_expanded_dir(dir.path(), &doc).expect("write");
    let restored = read_ocad(dir.path()).expect("read back");
    assert!(restored
        .sketches
        .iter()
        .any(|sketch| sketch.id.as_str() == "sketch:boss"));
    assert_eq!(
        serialize_document_files(&restored).expect("serialize"),
        serialize_document_files(&doc).expect("serialize")
    );

    let snapshot = serialize_document_files(&doc).expect("serialize");
    let history_before = history.clone();
    let failing = patch(json!([
        { "type": "add_sketch_entity", "sketch_id": "sketch:boss",
          "entity": { "type": "point", "id": "ent:extra", "x": 0.0, "y": 0.0 } },
        { "type": "remove_sketch", "id": "sketch:hole" }
    ]));
    assert!(apply_patch_with_history(&mut doc, &failing, &mut history, "bad").is_err());
    assert_eq!(serialize_document_files(&doc).expect("serialize"), snapshot);
    assert_eq!(history, history_before);
}

#[test]
fn rebase_detects_concurrent_edits_to_the_same_sketch_member() {
    let doc = bracket();
    let base = document_design_state(&doc);
    let ours = patch(json!([
        { "type": "add_sketch_entity", "sketch_id": "sketch:base",
          "entity": { "type": "point", "id": "ent:datum", "x": 0.01, "y": 0.01 } }
    ]));
    let theirs = candidate(
        &doc,
        json!([
            { "type": "add_sketch_entity", "sketch_id": "sketch:base",
              "entity": { "type": "point", "id": "ent:datum", "x": 0.02, "y": 0.02 } }
        ]),
    )
    .expect("theirs");
    let conflicts = rebase_patch(&ours, &base, &theirs).expect_err("conflict");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::Sketch);
    assert_eq!(conflicts[0].id, "sketch:base/ent:datum");

    let independent = candidate(
        &doc,
        json!([{ "type": "add_sketch_entity", "sketch_id": "sketch:hole",
                 "entity": { "type": "point", "id": "ent:mark", "x": 0.0, "y": 0.0 } }]),
    )
    .expect("independent");
    assert!(rebase_patch(&ours, &base, &independent).is_ok());
}

#[test]
fn an_edited_sketch_still_regenerates() {
    use opencad_feature::FeatureRegistry;
    use opencad_geometry::MockGeometryKernel;

    let mut doc = bracket();
    let mut history = DocumentHistory::default();
    let edit = patch(json!([
        { "type": "add_sketch_entity", "sketch_id": "sketch:base",
          "entity": { "type": "point", "id": "ent:datum", "construction": true,
                      "x": 0.04, "y": 0.03 } },
        { "type": "remove_sketch_constraint", "sketch_id": "sketch:base",
          "constraint_id": "con:height" }
    ]));
    apply_patch_with_history(&mut doc, &edit, &mut history, "edit sketch").expect("apply");

    let parameters = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let report = model
        .regenerate(
            &MockGeometryKernel::new(),
            &FeatureRegistry::with_defaults(),
            Some(&parameters),
            None,
        )
        .expect("regenerate");
    assert!(report
        .regenerated
        .contains(&"feature:extrude_base".to_string()));
    assert!(model.active_body().is_some());
}
