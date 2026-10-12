//! Integration evidence for the shared GUI/Agent editor gateway.
use opencad_ai::FeaturePosition;
use opencad_desktop::editor::{
    apply_feature_edit_with_kernel, feature_tree, plan_feature_edit_with_kernel,
    verify_editor_document, FeatureEdit,
};
use opencad_file::{read_ocad, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};

fn fixture() -> OcadDocument {
    read_ocad(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cubic_plate.ocad.d"),
    )
    .unwrap()
}

fn apply<K: GeometryKernel>(
    doc: &mut OcadDocument,
    history: &mut DocumentHistory,
    kernel: &K,
    edit: FeatureEdit,
) -> opencad_desktop::editor::EditorEvidence {
    let revision = feature_tree(doc).unwrap().revision;
    apply_feature_edit_with_kernel(doc, &revision, edit, history, kernel)
        .unwrap()
        .evidence
}

fn analytic_edits<K: GeometryKernel>(kernel: &K) {
    let mut doc = fixture();
    let mut history = DocumentHistory::default();
    let before = doc.clone();
    let evidence = apply(
        &mut doc,
        &mut history,
        kernel,
        FeatureEdit::CreateExtrude {
            id: "feature:second".into(),
            name: "Second cubic prism".into(),
            sketch_feature: "feature:cubic_sketch".into(),
            profile_ref: "profile:bezier:ent:cubic".into(),
            length_expr: "8 mm".into(),
        },
    );
    assert!((evidence.volume_m3.unwrap() - 0.15 * 0.04 * 0.04 * 0.008).abs() < 1e-12);
    let evidence = apply(
        &mut doc,
        &mut history,
        kernel,
        FeatureEdit::SetExpression {
            id: "feature:second".into(),
            field: "length_expr".into(),
            expr: "10 mm".into(),
        },
    );
    assert!((evidence.volume_m3.unwrap() - 2.4e-6).abs() < 1e-12);
    // OCCT bounding boxes include shape tolerance (0.1 µm at each end).
    for (actual, expected) in
        evidence
            .bounds_size_m
            .unwrap()
            .into_iter()
            .zip([4.0 * 0.04 / 9.0, 4.0 * 0.04 / 9.0, 0.010])
    {
        assert!((actual - expected).abs() < 3e-7, "{actual} vs {expected}");
    }
    apply(
        &mut doc,
        &mut history,
        kernel,
        FeatureEdit::Move {
            id: "feature:second".into(),
            position: FeaturePosition::Before("feature:cubic_body".into()),
        },
    );
    assert_eq!(doc.feature_graph.ordered_ids()[1], "feature:second");
    let evidence = apply(
        &mut doc,
        &mut history,
        kernel,
        FeatureEdit::Suppress {
            id: "feature:cubic_body".into(),
            suppressed: true,
        },
    );
    assert!((evidence.volume_m3.unwrap() - 2.4e-6).abs() < 1e-12);
    let after = doc.clone();
    for _ in 0..4 {
        history.undo(&mut doc).unwrap();
        verify_editor_document(&doc, kernel).unwrap();
    }
    assert_eq!(doc, before);
    for _ in 0..4 {
        history.redo(&mut doc).unwrap();
        verify_editor_document(&doc, kernel).unwrap();
    }
    assert_eq!(doc, after);
}

#[test]
fn feature_create_edit_reorder_suppress_match_both_kernel_analytic_fixture() {
    analytic_edits(&MockGeometryKernel::new());
    #[cfg(feature = "occt")]
    analytic_edits(&opencad_kernel_occt::OcctGeometryKernel::new());
}

#[test]
fn dependency_invalid_reorder_or_suppression_preserves_previous_document() {
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture();
    let before = doc.clone();
    let mut history = DocumentHistory::default();
    for edit in [
        FeatureEdit::Suppress {
            id: "feature:cubic_sketch".into(),
            suppressed: true,
        },
        FeatureEdit::Move {
            id: "feature:cubic_body".into(),
            position: FeaturePosition::Before("feature:cubic_sketch".into()),
        },
        FeatureEdit::SetExpression {
            id: "feature:cubic_body".into(),
            field: "length_expr".into(),
            expr: "90 deg".into(),
        },
        FeatureEdit::SetExpression {
            id: "feature:cubic_body".into(),
            field: "length_expr".into(),
            expr: "8".into(),
        },
    ] {
        let revision = feature_tree(&doc).unwrap().revision;
        assert!(
            apply_feature_edit_with_kernel(&mut doc, &revision, edit, &mut history, &kernel)
                .is_err()
        );
        assert_eq!(doc, before);
        assert!(!history.can_undo());
    }
}

#[test]
fn preview_does_not_write_and_stale_revision_refuses() {
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture();
    let mut history = DocumentHistory::default();
    let revision = feature_tree(&doc).unwrap().revision;
    let edit = FeatureEdit::SetExpression {
        id: "feature:cubic_body".into(),
        field: "length_expr".into(),
        expr: "10 mm".into(),
    };
    let before = doc.clone();
    let plan = plan_feature_edit_with_kernel(&doc, &revision, edit.clone(), &kernel).unwrap();
    assert!(!plan.patch.preconditions.is_empty());
    assert_eq!(doc, before);
    apply_feature_edit_with_kernel(&mut doc, &revision, edit.clone(), &mut history, &kernel)
        .unwrap();
    let after = doc.clone();
    assert!(
        apply_feature_edit_with_kernel(&mut doc, &revision, edit, &mut history, &kernel).is_err()
    );
    assert_eq!(doc, after);
    assert_eq!(history.undo_len(), 1);
}

#[test]
fn required_mass_assertion_refuses_candidate_and_preserves_history() {
    use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture();
    doc.assertions.push(Assertion::new(
        "assertion:mass",
        "Mass limit",
        AssertionSeverity::Required,
        AssertionKind::MassRange {
            min_kg: 0.0,
            max_kg: 0.004,
        },
    ));
    verify_editor_document(&doc, &kernel).unwrap();
    let before = doc.clone();
    let revision = feature_tree(&doc).unwrap().revision;
    let mut history = DocumentHistory::default();
    let error = apply_feature_edit_with_kernel(
        &mut doc,
        &revision,
        FeatureEdit::SetExpression {
            id: "feature:cubic_body".into(),
            field: "length_expr".into(),
            expr: "10 mm".into(),
        },
        &mut history,
        &kernel,
    )
    .unwrap_err();
    assert!(error.to_string().contains("assertions failed"));
    assert_eq!(doc, before);
    assert!(!history.can_undo());
}

#[test]
fn no_op_edit_does_not_regenerate_or_add_history_to_a_broken_part() {
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture();
    let mut history = DocumentHistory::default();
    let edit = FeatureEdit::SetExpression {
        id: "feature:cubic_body".into(),
        field: "length_expr".into(),
        expr: "6 mm".into(),
    };
    let revision = feature_tree(&doc).unwrap().revision;
    apply_feature_edit_with_kernel(&mut doc, &revision, edit.clone(), &mut history, &kernel)
        .unwrap();
    // An existing broken source is not repaired or falsely called regenerated by
    // a no-op. This also proves no kernel execution can happen on this path.
    doc.parameters.set_expr("param:b", "0 mm").unwrap();
    assert!(verify_editor_document(&doc, &kernel).is_err());
    let before = doc.clone();
    let previous_history = history.clone();
    let revision = feature_tree(&doc).unwrap().revision;
    let result =
        apply_feature_edit_with_kernel(&mut doc, &revision, edit, &mut history, &kernel).unwrap();
    assert!(result.evidence.no_op);
    assert!(result.evidence.regenerated.is_empty());
    assert_eq!(doc, before);
    assert_eq!(history, previous_history);
}

#[cfg(feature = "occt")]
#[test]
fn failed_geometry_after_staged_edit_preserves_source_history_and_output() {
    use opencad_desktop::{preview_document, set_document_parameter_with_history};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cubic.ocad.d");
    opencad_file::write_ocad(&path, &fixture()).unwrap();
    let path = path.to_str().unwrap();
    let state = set_document_parameter_with_history(path, "param:a", "50 mm", None).unwrap();
    let before = read_ocad(path).unwrap();
    let source = opencad_file::expanded_dir::serialize_document_files(&before).unwrap();
    let output = preview_document(path).unwrap();
    let history = serde_json::to_value(&state.history).unwrap();
    // The source parameter edit is valid; regenerating the now collinear cubic
    // is refused after the candidate has been staged.
    assert!(set_document_parameter_with_history(
        path,
        "param:b",
        "0 mm",
        Some(state.history.clone())
    )
    .is_err());
    assert_eq!(
        opencad_file::expanded_dir::serialize_document_files(&read_ocad(path).unwrap()).unwrap(),
        source
    );
    assert_eq!(serde_json::to_value(state.history).unwrap(), history);
    assert_eq!(preview_document(path).unwrap(), output);
}
