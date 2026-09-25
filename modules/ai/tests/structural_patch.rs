//! ADR-013 slice 1: structural parameter and assertion operations,
//! final-state validation, and design-state revision v1/v2 rules.

use opencad_ai::{
    build_patch_candidate, design_state_revision_for_version, dry_run_patch_state, rebase_patch,
    ConflictKind, DesignPatch, DesignState, PatchOperation, PatchPrecondition,
    DESIGN_STATE_REVISION_ALGORITHM, DESIGN_STATE_REVISION_VERSION_V1,
    DESIGN_STATE_REVISION_VERSION_V2,
};
use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
use opencad_feature::bracket_with_hole;
use opencad_graph::{bracket_parameters, evaluate_param_graph, SemanticChange};

fn bracket_state() -> DesignState {
    let part = bracket_with_hole().expect("bracket model");
    DesignState::new(bracket_parameters(), part.nodes.into_values().collect())
        .with_authoring(part.sketches.into_values().collect(), Vec::new())
}

fn add_parameter(id: &str, name: &str, expr: &str) -> PatchOperation {
    PatchOperation::AddParameter {
        id: id.into(),
        name: name.into(),
        expr: expr.into(),
    }
}

fn remove_parameter(id: &str) -> PatchOperation {
    PatchOperation::RemoveParameter { id: id.into() }
}

fn parameter_range(id: &str, parameter_name: &str) -> Assertion {
    Assertion::new(
        id,
        "Parameter range",
        AssertionSeverity::Required,
        AssertionKind::ParameterRange {
            parameter_name: parameter_name.into(),
            min_m: 0.001,
            max_m: 0.1,
        },
    )
}

fn revision_guard(state: &DesignState, version: &str) -> PatchPrecondition {
    PatchPrecondition::RevisionEquals {
        algorithm: DESIGN_STATE_REVISION_ALGORITHM.into(),
        version: version.into(),
        digest: design_state_revision_for_version(state, version).expect("revision"),
    }
}

fn apply(
    state: &DesignState,
    operations: Vec<PatchOperation>,
) -> opencad_core::Result<DesignState> {
    build_patch_candidate(state, &DesignPatch::new(operations))
}

#[test]
fn add_parameter_evaluates_and_derives_dependency_edges() {
    let before = bracket_state();
    let after = apply(
        &before,
        vec![add_parameter(
            "param:rib_depth",
            "rib_depth",
            "thickness * 2",
        )],
    )
    .expect("add parameter");

    let values = evaluate_param_graph(&after.parameters).expect("evaluate");
    assert!((values["rib_depth"] - 0.012).abs() < 1e-12);
    assert!(after
        .parameters
        .dependency_edges()
        .iter()
        .any(|edge| edge.source == "param:thickness" && edge.target == "param:rib_depth"));
}

#[test]
fn added_parameters_may_reference_each_other_in_any_order() {
    let after = apply(
        &bracket_state(),
        vec![
            add_parameter("param:rib_pitch", "rib_pitch", "rib_width * 3"),
            add_parameter("param:rib_width", "rib_width", "4 mm"),
        ],
    )
    .expect("forward reference within one patch");

    let values = evaluate_param_graph(&after.parameters).expect("evaluate");
    assert!((values["rib_pitch"] - 0.012).abs() < 1e-12);
}

#[test]
fn add_parameter_rejects_invalid_ids_names_and_duplicates() {
    let state = bracket_state();
    let cases = [
        (
            add_parameter("rib_depth", "rib_depth", "1 mm"),
            "invalid id",
        ),
        (add_parameter("param:Rib", "rib", "1 mm"), "invalid id"),
        (
            add_parameter("param:rib", "2rib", "1 mm"),
            "invalid parameter name",
        ),
        (
            add_parameter("param:rib", "rib depth", "1 mm"),
            "invalid parameter name",
        ),
        (
            add_parameter("param:other_width", "width", "1 mm"),
            "already used by 'param:width'",
        ),
        (
            add_parameter("param:width", "new_width", "1 mm"),
            "already exists",
        ),
    ];
    for (operation, expected) in cases {
        let error = apply(&state, vec![operation.clone()]).expect_err("must reject");
        assert!(
            error.to_string().contains(expected),
            "{operation:?}: expected '{expected}' in '{error}'"
        );
    }
}

#[test]
fn removing_an_unreferenced_parameter_succeeds_and_is_diffed() {
    let base = apply(
        &bracket_state(),
        vec![add_parameter("param:spare", "spare", "3 mm")],
    )
    .expect("add");
    let patch = DesignPatch::new(vec![remove_parameter("param:spare")]);

    let report = dry_run_patch_state(&base, &patch);
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert!(report
        .diff
        .changes
        .contains(&SemanticChange::ParameterChanged {
            id: "param:spare".into(),
            before: "3 mm".into(),
            after: String::new(),
        }));
    let after = build_patch_candidate(&base, &patch).expect("remove");
    assert!(after.parameters.get("param:spare").is_none());
}

#[test]
fn removing_a_referenced_parameter_fails_and_lists_every_dependent() {
    let state = bracket_state();
    let with_consumers = apply(
        &state,
        vec![
            add_parameter(
                "param:double_thickness",
                "double_thickness",
                "thickness * 2",
            ),
            PatchOperation::AddAssertion {
                assertion: parameter_range("assertion:thickness", "thickness"),
            },
        ],
    )
    .expect("consumers");

    let error = apply(&with_consumers, vec![remove_parameter("param:thickness")])
        .expect_err("referenced parameter");
    let message = error.to_string();
    assert!(message.contains("cannot remove parameter 'param:thickness'"));
    assert!(message.contains("assertion assertion:thickness"));
    assert!(message.contains("feature feature:hole_mount"));
    assert!(message.contains("parameter param:double_thickness"));
    // Dependents are listed in sorted order for deterministic agent repair.
    let assertion = message.find("assertion assertion:thickness").unwrap();
    let feature = message.find("feature feature:hole_mount").unwrap();
    let parameter = message.find("parameter param:double_thickness").unwrap();
    assert!(assertion < feature && feature < parameter);

    let sketch_error =
        apply(&state, vec![remove_parameter("param:hole_diameter")]).expect_err("sketch reference");
    assert!(sketch_error.to_string().contains("sketch sketch:"));
}

#[test]
fn final_state_validation_allows_removing_consumers_in_the_same_patch() {
    let base = apply(
        &bracket_state(),
        vec![
            add_parameter("param:lug", "lug", "5 mm"),
            add_parameter("param:lug_height", "lug_height", "lug * 2"),
        ],
    )
    .expect("base");

    // The removal is listed before the consumer is rewritten; only the final
    // state has to be valid.
    let after = apply(
        &base,
        vec![
            remove_parameter("param:lug"),
            PatchOperation::SetParameter {
                id: "param:lug_height".into(),
                expr: "10 mm".into(),
            },
        ],
    )
    .expect("final state is valid");
    assert!(after.parameters.get("param:lug").is_none());
    assert!(!after
        .parameters
        .dependency_edges()
        .iter()
        .any(|edge| edge.source == "param:lug" || edge.target == "param:lug"));
}

#[test]
fn ids_removed_in_a_patch_cannot_be_reused_in_the_same_patch() {
    let base = apply(
        &bracket_state(),
        vec![add_parameter("param:spare", "spare", "3 mm")],
    )
    .expect("base");
    let error = apply(
        &base,
        vec![
            remove_parameter("param:spare"),
            add_parameter("param:spare", "spare", "4 mm"),
        ],
    )
    .expect_err("reused id");
    assert!(error.to_string().contains("cannot be reused"));
}

#[test]
fn assertions_are_added_removed_and_diffed() {
    let base = bracket_state();
    let patch = DesignPatch::new(vec![PatchOperation::AddAssertion {
        assertion: parameter_range("assertion:width_range", "width"),
    }]);
    let report = dry_run_patch_state(&base, &patch);
    assert!(report.validation.is_ok(), "{:?}", report.validation);
    assert_eq!(
        report.diff.changes,
        vec![SemanticChange::AssertionAdded {
            id: "assertion:width_range".into()
        }]
    );
    assert!(!report.impact.no_op);
    assert!(report.impact.predicted_dirty_nodes.is_empty());

    let with_assertion = build_patch_candidate(&base, &patch).expect("add");
    assert_eq!(with_assertion.assertions.len(), 1);

    let removed = apply(
        &with_assertion,
        vec![PatchOperation::RemoveAssertion {
            id: "assertion:width_range".into(),
        }],
    )
    .expect("remove");
    assert!(removed.assertions.is_empty());
}

#[test]
fn assertions_must_be_valid_and_reference_existing_targets() {
    let state = bracket_state();
    let unknown_parameter = apply(
        &state,
        vec![PatchOperation::AddAssertion {
            assertion: parameter_range("assertion:missing", "missing_parameter"),
        }],
    )
    .expect_err("unknown parameter");
    assert!(unknown_parameter
        .to_string()
        .contains("references unknown parameter 'missing_parameter'"));

    let unknown_reference = apply(
        &state,
        vec![PatchOperation::AddAssertion {
            assertion: Assertion::new(
                "assertion:bore",
                "Bore",
                AssertionSeverity::Required,
                AssertionKind::RequiredReference {
                    ref_id: "ref:missing".into(),
                },
            ),
        }],
    )
    .expect_err("unknown reference");
    assert!(unknown_reference
        .to_string()
        .contains("unknown semantic reference 'ref:missing'"));

    let inverted = Assertion::new(
        "assertion:inverted",
        "Inverted",
        AssertionSeverity::Advisory,
        AssertionKind::MassRange {
            min_kg: 2.0,
            max_kg: 1.0,
        },
    );
    let invalid = apply(
        &state,
        vec![PatchOperation::AddAssertion {
            assertion: inverted,
        }],
    )
    .expect_err("inverted range");
    assert!(invalid.to_string().contains("min <= max"));

    let bad_id = apply(
        &state,
        vec![PatchOperation::AddAssertion {
            assertion: parameter_range("width_range", "width"),
        }],
    )
    .expect_err("bad id");
    assert!(bad_id.to_string().contains("invalid id"));

    let missing = apply(
        &state,
        vec![PatchOperation::RemoveAssertion {
            id: "assertion:none".into(),
        }],
    )
    .expect_err("unknown assertion");
    assert!(missing.to_string().contains("unknown assertion"));
}

#[test]
fn v2_revision_observes_sketches_and_assertions_but_v1_does_not() {
    let state = bracket_state();
    let mut sketch_edited = state.clone();
    sketch_edited.sketches[0].name.push_str(" (edited)");
    let with_assertion = apply(
        &state,
        vec![PatchOperation::AddAssertion {
            assertion: parameter_range("assertion:width_range", "width"),
        }],
    )
    .expect("assertion");

    let v1 =
        |s: &DesignState| design_state_revision_for_version(s, DESIGN_STATE_REVISION_VERSION_V1);
    let v2 =
        |s: &DesignState| design_state_revision_for_version(s, DESIGN_STATE_REVISION_VERSION_V2);
    assert_eq!(v1(&state).unwrap(), v1(&sketch_edited).unwrap());
    assert_eq!(v1(&state).unwrap(), v1(&with_assertion).unwrap());
    assert_ne!(v2(&state).unwrap(), v2(&sketch_edited).unwrap());
    assert_ne!(v2(&state).unwrap(), v2(&with_assertion).unwrap());

    // A v2 guard taken before a concurrent sketch edit rejects the patch.
    let patch = DesignPatch {
        preconditions: vec![revision_guard(&state, DESIGN_STATE_REVISION_VERSION_V2)],
        ..DesignPatch::new(vec![add_parameter("param:spare", "spare", "1 mm")])
    };
    assert!(build_patch_candidate(&state, &patch).is_ok());
    let stale = build_patch_candidate(&sketch_edited, &patch).expect_err("stale");
    assert!(stale.to_string().contains("revision mismatch"));
}

#[test]
fn v1_revision_guards_value_edits_but_not_structural_patches() {
    let state = bracket_state();
    let guard = revision_guard(&state, DESIGN_STATE_REVISION_VERSION_V1);

    let value_edit = DesignPatch {
        preconditions: vec![guard.clone()],
        ..DesignPatch::set_parameter("param:width", "90 mm")
    };
    assert!(build_patch_candidate(&state, &value_edit).is_ok());

    let structural = DesignPatch {
        preconditions: vec![guard],
        ..DesignPatch::new(vec![add_parameter("param:spare", "spare", "1 mm")])
    };
    let error = build_patch_candidate(&state, &structural).expect_err("v1 too weak");
    assert!(error
        .to_string()
        .contains("too weak for structural operations"));
}

#[test]
fn legacy_apply_to_document_refuses_structural_operations() {
    let state = bracket_state();
    let mut parameters = state.parameters.clone();
    let mut nodes = state.feature_nodes.clone();
    let mut refs = state.semantic_refs.clone();
    let error = DesignPatch::new(vec![add_parameter("param:spare", "spare", "1 mm")])
        .apply_to_document(&mut parameters, &mut nodes, &mut refs, None, None)
        .expect_err("structural");
    assert!(error
        .to_string()
        .contains("require the complete design state"));
    assert_eq!(parameters, state.parameters);
}

#[test]
fn failed_structural_patch_leaves_the_state_unchanged() {
    let state = bracket_state();
    let mut target = state.clone();
    let patch = DesignPatch::new(vec![
        add_parameter("param:spare", "spare", "1 mm"),
        remove_parameter("param:thickness"),
    ]);
    // The first operation succeeds locally; the final state is invalid.
    assert!(patch.apply_to_state(&mut target).is_err());
    assert_eq!(target, state);
    let error = build_patch_candidate(&state, &patch).expect_err("referenced");
    assert!(error.to_string().contains("param:thickness"));
    let report = dry_run_patch_state(&state, &patch);
    assert!(!report.validation.is_ok());
    assert_eq!(state, bracket_state());
}

#[test]
fn patches_above_the_operation_limit_are_rejected() {
    let operations = (0..=opencad_ai::MAX_PATCH_OPERATIONS)
        .map(|index| add_parameter(&format!("param:p{index}"), &format!("p{index}"), "1 mm"))
        .collect();
    let error = apply(&bracket_state(), operations).expect_err("limit");
    assert!(error.to_string().contains("at most"));
}

#[test]
fn rebase_detects_concurrent_structural_edits_to_the_same_id() {
    let base = bracket_state();
    let patch = DesignPatch::new(vec![add_parameter("param:rib", "rib", "4 mm")]);

    let identical = apply(&base, vec![add_parameter("param:rib", "rib", "4 mm")]).unwrap();
    assert!(rebase_patch(&patch, &base, &identical).is_ok());

    let divergent = apply(&base, vec![add_parameter("param:rib", "rib", "5 mm")]).unwrap();
    let conflicts = rebase_patch(&patch, &base, &divergent).expect_err("add/add");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, ConflictKind::Parameter);
    assert_eq!(conflicts[0].id, "param:rib");
}
