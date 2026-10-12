//! Semantic selection plans are backend commands, not renderer/name guesses.
use opencad_desktop::selection::{
    plan_selection_with_kernel, resolve_selection_source, SelectionBinding, SelectionEdit,
    SelectionFrame, SelectionHit, SelectionTarget,
};
use opencad_file::{read_ocad, OcadDocument};
use opencad_geometry::{
    GeometryKernel, MockGeometryKernel, ReferenceProvenance, ReferenceStatus, TopoRefKind,
};

fn fixture() -> OcadDocument {
    read_ocad(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/ellipse_plate.ocad.d"),
    )
    .unwrap()
}
fn target() -> SelectionTarget {
    SelectionTarget::SketchEntity {
        sketch_id: "sketch:ellipse".into(),
        entity_id: "ent:ellipse".into(),
    }
}
fn frame(doc: &OcadDocument) -> SelectionFrame {
    // Source-only frame fixture: Mock does not pretend to render an ellipse.
    // Production transports always rebuild the actual native scene themselves.
    let mut binding = resolve_selection_source(doc, &target(), &[]).unwrap();
    binding.hits.push(SelectionHit::SketchLine { index: 0 });
    SelectionFrame {
        source_revision: opencad_ai::design_state_revision(&opencad_file::document_design_state(
            doc,
        ))
        .unwrap(),
        scene_revision: "source-binding-test-frame".into(),
        bindings: vec![binding],
        provenance: Vec::new(),
    }
}
fn edit(frame: &SelectionFrame) -> SelectionEdit {
    SelectionEdit {
        source_revision: frame.source_revision.clone(),
        scene_revision: frame.scene_revision.clone(),
        target: target(),
        field: "length_expr".into(),
        expr: "10 mm".into(),
    }
}
fn analytic<K: GeometryKernel>(kernel: &K) {
    let mut doc = fixture();
    let before = doc.clone();
    let frame = frame(&doc);
    let plan = plan_selection_with_kernel(&doc, &frame, &edit(&frame), kernel).unwrap();
    assert_eq!(doc, before, "preview has no mutation");
    // Ellipse axes are independently read from its source parameter definitions.
    let values = opencad_graph::evaluate_param_graph_strict(&doc.parameters).unwrap();
    let expected = std::f64::consts::PI * values["major_radius"] * values["minor_radius"] * 0.010;
    assert!((plan.evidence.volume_m3.unwrap() - expected).abs() < 1e-12);
    opencad_file::apply_patch_to_document(&mut doc, &plan.patch).unwrap();
    assert_eq!(doc.sketches, before.sketches);
    assert_eq!(doc.parameters, before.parameters);
    let def = &doc
        .feature_nodes
        .iter()
        .find(|n| n.id == "feature:ellipse_body")
        .unwrap()
        .definition;
    let opencad_feature::FeatureDefinition::Extrude(extrude) = def else {
        panic!("extrusion")
    };
    assert_eq!(extrude.length_expr.as_deref(), Some("10 mm"));
    assert_eq!(extrude.sketch_feature, "feature:ellipse_sketch");
    assert_eq!(extrude.profile_ref, "sketch:ellipse/profile:outer");
}

#[test]
fn selection_review_and_apply_preserve_semantic_source_ids_and_both_kernel_volume() {
    analytic(&MockGeometryKernel::new());
    #[cfg(feature = "occt")]
    analytic(&opencad_kernel_occt::OcctGeometryKernel::new());
}

#[test]
fn selection_patch_refuses_missing_ambiguous_and_stale_source_bindings() {
    let doc = fixture();
    let frame = frame(&doc);
    let kernel = MockGeometryKernel::new();
    for bad in [
        SelectionEdit {
            source_revision: "old".into(),
            ..edit(&frame)
        },
        SelectionEdit {
            scene_revision: "old".into(),
            ..edit(&frame)
        },
        SelectionEdit {
            target: SelectionTarget::SketchEntity {
                sketch_id: "sketch:ellipse".into(),
                entity_id: "missing".into(),
            },
            ..edit(&frame)
        },
        SelectionEdit {
            field: "angle_expr".into(),
            ..edit(&frame)
        },
    ] {
        assert!(plan_selection_with_kernel(&doc, &frame, &bad, &kernel).is_err());
    }
    let mut multiple = doc.clone();
    let mut body = multiple
        .feature_nodes
        .iter()
        .find(|n| n.id == "feature:ellipse_body")
        .unwrap()
        .clone();
    body.id = "feature:second_consumer".into();
    opencad_file::apply_patch_to_document(
        &mut multiple,
        &opencad_ai::DesignPatch::new(vec![opencad_ai::PatchOperation::AddFeature {
            node: body,
            position: opencad_ai::FeaturePosition::end(),
        }]),
    )
    .unwrap();
    assert!(resolve_selection_source(&multiple, &target(), &[])
        .unwrap_err()
        .to_string()
        .contains("ambiguous"));
    assert_eq!(doc, fixture());
}

#[test]
fn face_binding_refuses_consumed_missing_ambiguous_and_alias_records() {
    use opencad_core::TopoRefId;
    use opencad_geometry::TopoRef;
    let mut doc = fixture();
    doc.semantic_refs.push(TopoRef::face(
        TopoRefId::new("ref:face:selection").unwrap(),
        "feature:ellipse_body",
        "top",
    ));
    let target = SelectionTarget::FaceReference {
        ref_id: "ref:face:selection".into(),
    };
    let good = ReferenceProvenance {
        ref_id: "ref:face:selection".into(),
        kind: TopoRefKind::Face,
        status: ReferenceStatus::Fingerprint,
        created_by: "feature:ellipse_body".into(),
        role: Some("top".into()),
        resolved_kernel_id: Some(17),
        candidate_count: 1,
        reason: "metadata fixture".into(),
        candidates: Vec::new(),
        required: false,
    };
    let bound: SelectionBinding =
        resolve_selection_source(&doc, &target, std::slice::from_ref(&good)).unwrap();
    assert_eq!(bound.feature_id, "feature:ellipse_body");
    for status in [
        ReferenceStatus::Consumed,
        ReferenceStatus::Missing,
        ReferenceStatus::Ambiguous,
    ] {
        let bad = ReferenceProvenance {
            status,
            ..good.clone()
        };
        assert!(resolve_selection_source(&doc, &target, &[bad]).is_err());
    }
    let alias = ReferenceProvenance {
        ref_id: "ref:face:alias".into(),
        ..good.clone()
    };
    assert!(resolve_selection_source(&doc, &target, &[good, alias]).is_err());
}

#[test]
fn failed_selection_candidate_assertion_preserves_source() {
    let mut doc = fixture();
    doc.assertions.push(opencad_core::Assertion::new(
        "assertion:selection_mass",
        "Selection mass limit",
        opencad_core::AssertionSeverity::Required,
        opencad_core::AssertionKind::MassRange {
            min_kg: 0.0,
            max_kg: 0.04,
        },
    ));
    let kernel = MockGeometryKernel::new();
    opencad_desktop::editor::verify_editor_document(&doc, &kernel).unwrap();
    let frame = frame(&doc);
    let before = doc.clone();
    let error = plan_selection_with_kernel(&doc, &frame, &edit(&frame), &kernel).unwrap_err();
    assert!(error.to_string().contains("assertions failed"));
    assert_eq!(doc, before);
}

#[cfg(feature = "occt")]
#[test]
fn actual_native_scene_is_revision_bound_and_stale_edit_preserves_file_history() {
    use opencad_desktop::{
        load_view_data, selection::apply_document_selection_edit,
        selection::plan_document_selection_edit,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("selection.ocad.d");
    let mut native_doc = fixture();
    native_doc
        .semantic_refs
        .push(opencad_geometry::TopoRef::face(
            opencad_core::TopoRefId::new("ref:face:ellipse_top").unwrap(),
            "feature:ellipse_body",
            "top",
        ));
    opencad_file::write_ocad(&path, &native_doc).unwrap();
    let path = path.to_str().unwrap();
    let frame = load_view_data(path).unwrap().selection_frame.unwrap();
    let native_target = SelectionTarget::FaceReference {
        ref_id: "ref:face:ellipse_top".into(),
    };
    assert!(
        frame.bindings.iter().any(|b| b.target == native_target),
        "{:?}",
        frame.bindings
    );
    assert_eq!(
        frame.scene_revision,
        load_view_data(path)
            .unwrap()
            .selection_frame
            .unwrap()
            .scene_revision
    );
    let request = SelectionEdit {
        target: native_target,
        ..edit(&frame)
    };
    let before = read_ocad(path).unwrap();
    let mut other_document = before.clone();
    other_document.metadata.id = opencad_core::DocumentId::new("doc:other_selection").unwrap();
    let other_path = dir.path().join("other.ocad.d");
    opencad_file::write_ocad(&other_path, &other_document).unwrap();
    let other_frame = load_view_data(other_path.to_str().unwrap())
        .unwrap()
        .selection_frame
        .unwrap();
    assert_eq!(frame.source_revision, other_frame.source_revision);
    assert_ne!(
        frame.scene_revision, other_frame.scene_revision,
        "document identity binds an otherwise identical scene"
    );
    assert!(plan_document_selection_edit(other_path.to_str().unwrap(), &request).is_err());
    assert_eq!(read_ocad(&other_path).unwrap(), other_document);
    plan_document_selection_edit(path, &request).unwrap();
    assert_eq!(read_ocad(path).unwrap(), before);
    let result = apply_document_selection_edit(path, &request, None).unwrap();
    let source =
        opencad_file::expanded_dir::serialize_document_files(&read_ocad(path).unwrap()).unwrap();
    let history = result.history.clone();
    assert!(apply_document_selection_edit(path, &request, Some(history.history.clone())).is_err());
    assert_eq!(
        source,
        opencad_file::expanded_dir::serialize_document_files(&read_ocad(path).unwrap()).unwrap()
    );
    assert_eq!(result.history, history);
}
