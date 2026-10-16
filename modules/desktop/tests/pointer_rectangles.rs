//! Opposite-corner rectangles expand to ordinary source through one guarded patch.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{GlobalPlane, Workplane};

fn fixture(datum: bool) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:pointer_rectangle").unwrap(),
        "Pointer rectangle",
    ));
    if datum {
        doc.feature_nodes.push(FeatureNode::new(
            "feature:datum",
            "Tilted construction plane",
            FeatureDefinition::DatumPlane(DatumPlaneFeature {
                base: Workplane::Custom {
                    origin: [0.01, -0.02, 0.03],
                    normal: [0., 0.6, 0.8],
                    x_axis: [1., 0., 0.],
                },
                offset: Length::from_meters(0.005),
                offset_expr: None,
            }),
        ));
        doc.feature_graph = opencad_feature::derive_feature_graph(
            &doc.feature_nodes,
            &["feature:datum".into()],
            &[],
            &[],
        )
        .unwrap();
    }
    doc
}
fn create<K: GeometryKernel>(
    doc: &mut OcadDocument,
    plane: GlobalPlane,
    datum: bool,
    history: &mut DocumentHistory,
    kernel: &K,
) {
    let state = sketch_creation_state_with_kernel(doc, kernel).unwrap();
    let request = SketchCreate {
        document_id: state.document_id,
        revision: state.revision,
        sketch_id: state.suggested_sketch_id,
        feature_id: state.suggested_feature_id,
        name: "Pointer rectangle".into(),
        plane: (!datum).then_some(plane),
        datum_plane: datum.then(|| "feature:datum".into()),
    };
    apply_sketch_create_with_kernel(doc, &request, history, kernel).unwrap();
}
fn intent(doc: &OcadDocument, construction: bool) -> SketchEdit {
    SketchEdit {
        document_id: Some(doc.metadata.id.as_str().into()),
        revision: feature_tree(doc).unwrap().revision,
        sketch_id: "sketch:new_1".into(),
        operations: vec![SketchForm::Rectangle {
            id: "ent:rectangle_1".into(),
            x1: "2.000 mm".into(),
            y1: "-3.000 mm".into(),
            x2: "22.000 mm".into(),
            y2: "7.000 mm".into(),
            construction,
        }],
    }
}
fn near(actual: [f64; 3], expected: [f64; 3]) {
    for i in 0..3 {
        assert!(
            (actual[i] - expected[i]).abs() < 3e-7,
            "{actual:?} != {expected:?}"
        );
    }
}
fn physical<K: GeometryKernel>(kernel: &K) {
    // Hand-derived corners=(2,-3)..(22,7) mm, positive depth=6 mm.
    // Tilted root y=(0,.8,-.6), n=(0,.6,.8), offset=+5 mm.
    for (plane, datum, low, high, center) in [
        (
            GlobalPlane::XY,
            false,
            [0.002, -0.003, 0.],
            [0.022, 0.007, 0.006],
            [0.012, 0.002, 0.003],
        ),
        (
            GlobalPlane::YZ,
            false,
            [0., 0.002, -0.003],
            [0.006, 0.022, 0.007],
            [0.003, 0.012, 0.002],
        ),
        (
            GlobalPlane::XZ,
            false,
            [0.002, -0.006, -0.003],
            [0.022, 0., 0.007],
            [0.012, -0.003, 0.002],
        ),
        (
            GlobalPlane::XY,
            true,
            [0.012, -0.0194, 0.0298],
            [0.032, -0.0078, 0.0406],
            [0.022, -0.0136, 0.0352],
        ),
    ] {
        let mut doc = fixture(datum);
        let mut history = DocumentHistory::default();
        create(&mut doc, plane, datum, &mut history, kernel);
        let before = doc.clone();
        let request = intent(&doc, false);
        let plan = plan_sketch_edit_with_kernel(&doc, &request, kernel).unwrap();
        assert_eq!(doc, before);
        assert_eq!(plan.patch.operations.len(), 8);
        assert!(plan.evidence.volume_m3.is_none());
        assert_eq!(history.undo_len(), 1);
        apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).unwrap();
        assert_eq!(history.undo_len(), 2);
        assert_eq!(doc.sketches[0].entities.len(), 8);
        assert_eq!(doc.feature_nodes, before.feature_nodes);
        assert_eq!(doc.sketches[0].workplane, before.sketches[0].workplane);
        let rectangle = document_design_state(&doc);
        history.undo(&mut doc).unwrap();
        assert_eq!(document_design_state(&doc), document_design_state(&before));
        history.redo(&mut doc).unwrap();
        assert_eq!(document_design_state(&doc), rectangle);
        let tree = feature_tree(&doc).unwrap();
        let owner = tree
            .rows
            .iter()
            .find(|r| r.id == "feature:sketch_new_1")
            .unwrap();
        assert_eq!(owner.closed_profiles.len(), 1);
        apply_feature_edit_with_kernel(
            &mut doc,
            &tree.revision,
            FeatureEdit::CreateExtrude {
                id: "feature:rectangle_body".into(),
                name: "Rectangle prism".into(),
                sketch_feature: owner.id.clone(),
                profile_ref: owner.closed_profiles[0].clone(),
                length_expr: "6 mm".into(),
            },
            &mut history,
            kernel,
        )
        .unwrap();
        let authored = doc.clone();
        let mut cache = opencad_feature::RegenerationCache::new();
        // Repeat authored (still unit-bearing) source with the same kernel/cache.
        // Both None and an explicitly empty parameter graph must solve it.
        for parameters in [None, Some(&doc.parameters), None] {
            let mut model = doc.clone().into_part_model();
            model
                .regenerate_with_cache(
                    kernel,
                    &FeatureRegistry::with_defaults(),
                    parameters,
                    None,
                    &mut cache,
                )
                .unwrap();
            let body = model.active_body().unwrap();
            let mass = kernel.mass_properties(body, 1.).unwrap();
            assert!((mass.volume_m3 - 1200e-9).abs() < 1e-12);
            assert!((mass.area_m2 - 760e-6).abs() < 1e-10);
            let bounds = kernel.bounding_box(body).unwrap();
            near(bounds.min, low);
            near(bounds.max, high);
            near(mass.center_of_mass, center);
            assert_eq!(doc, authored);
        }
    }
}
#[test]
fn atomic_pointer_rectangle_intent_matches_independent_placed_prisms_in_both_kernels() {
    physical(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    physical(&opencad_kernel_occt::OcctGeometryKernel::new());
}

fn refusals<K: GeometryKernel>(kernel: &K) {
    let mut doc = fixture(false);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, false, &mut history, kernel);
    let stable = doc.clone();
    let saved = history.clone();
    for case in [
        "zero_width",
        "zero_height",
        "unitless",
        "angle",
        "nonfinite",
        "foreign",
        "stale",
    ] {
        let mut edit = intent(&doc, false);
        match case {
            "foreign" => edit.document_id = Some("doc:foreign".into()),
            "stale" => edit.revision = "stale".into(),
            _ => {
                if let SketchForm::Rectangle { x2, y2, .. } = &mut edit.operations[0] {
                    match case {
                        "zero_width" => *x2 = "2 mm".into(),
                        "zero_height" => *y2 = "-3 mm".into(),
                        "unitless" => *x2 = "22".into(),
                        "angle" => *x2 = "22 deg".into(),
                        _ => *x2 = "1e999 mm".into(),
                    }
                }
            }
        }
        assert!(
            apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).is_err(),
            "{case}"
        );
        assert_eq!(doc, stable);
        assert_eq!(history, saved);
    }
    // Occupy the LAST derived edge ID: none of the preceding seven additions may leak.
    let mut occupied = intent(&doc, true);
    occupied.operations = vec![SketchForm::Point {
        id: "ent:rectangle_1_e3".into(),
        x: "50 mm".into(),
        y: "50 mm".into(),
        create: true,
    }];
    apply_sketch_edit_with_kernel(&mut doc, &occupied, &mut history, kernel).unwrap();
    let stable = doc.clone();
    let saved = history.clone();
    assert!(
        apply_sketch_edit_with_kernel(&mut doc, &intent(&stable, false), &mut history, kernel)
            .is_err()
    );
    assert_eq!(doc, stable);
    assert_eq!(history, saved);
}
#[test]
fn invalid_rectangle_or_late_collision_cannot_publish_partial_corners_or_edges() {
    refusals(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    refusals(&opencad_kernel_occt::OcctGeometryKernel::new());
}

#[test]
fn construction_rectangle_stays_a_solved_sketch_without_invented_mass() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut doc = fixture(true);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, true, &mut history, &kernel);
    let plan = plan_sketch_edit_with_kernel(&doc, &intent(&doc, true), &kernel).unwrap();
    assert!(plan.evidence.volume_m3.is_none());
    assert!(plan.evidence.bounds_size_m.is_none());
    assert!(plan.state.sketches[0].solved.profiles.is_empty());
    apply_sketch_edit_with_kernel(&mut doc, &plan.edit, &mut history, &kernel).unwrap();
    assert!(feature_tree(&doc)
        .unwrap()
        .rows
        .iter()
        .all(|r| r.closed_profiles.is_empty()));
}

#[test]
fn rectangle_batch_cannot_bypass_required_solid_assertions() {
    use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut doc = fixture(false);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
    doc.assertions.push(Assertion::new(
        "assertion:mass",
        "Solid required",
        AssertionSeverity::Required,
        AssertionKind::MassRange {
            min_kg: 0.,
            max_kg: 1.,
        },
    ));
    let before = doc.clone();
    let saved = history.clone();
    let error =
        apply_sketch_edit_with_kernel(&mut doc, &intent(&before, false), &mut history, &kernel)
            .unwrap_err();
    assert!(error
        .to_string()
        .contains("required editor candidate assertions failed"));
    assert_eq!(doc, before);
    assert_eq!(history, saved);
}

#[test]
#[cfg(feature = "occt")]
fn packed_and_expanded_rectangle_history_is_one_atomic_source_edit() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    for name in ["rectangle.ocad", "rectangle.ocad.d"] {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(name);
        let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
        let mut doc = fixture(true);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, true, &mut history, &kernel);
        opencad_file::write_ocad(&path, &doc).unwrap();
        let path = path.to_str().unwrap();
        let before = document_design_state(&opencad_file::read_ocad(path).unwrap());
        let request = intent(&doc, false);
        plan_document_sketch_edit(path, &request).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            before
        );
        let result = apply_document_sketch_edit(path, &request, None).unwrap();
        let after = document_design_state(&opencad_file::read_ocad(path).unwrap());
        assert_eq!(result.history.history.undo_len(), 1);
        let state = undo_document_with_history(path, result.history.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            before
        );
        let state = redo_document_with_history(path, state.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            after
        );
        assert!(apply_document_sketch_edit(path, &request, Some(state.history)).is_err());
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            after
        );
    }
}

#[test]
fn opposite_corner_directions_have_identical_ccw_source_and_no_helper_or_implied_constraints() {
    use opencad_sketch::SketchEntity;
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut canonical = None;
    for (x1, y1, x2, y2) in [
        ("2.000 mm", "-3.000 mm", "22.000 mm", "7.000 mm"),
        ("22.000 mm", "7.000 mm", "2.000 mm", "-3.000 mm"),
        ("2.000 mm", "7.000 mm", "22.000 mm", "-3.000 mm"),
        ("22.000 mm", "-3.000 mm", "2.000 mm", "7.000 mm"),
    ] {
        let mut doc = fixture(false);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
        let mut edit = intent(&doc, false);
        edit.operations = vec![SketchForm::Rectangle {
            id: "ent:rectangle_1".into(),
            x1: x1.into(),
            y1: y1.into(),
            x2: x2.into(),
            y2: y2.into(),
            construction: false,
        }];
        apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, &kernel).unwrap();
        let sketch = &doc.sketches[0];
        assert_eq!(
            sketch
                .entities
                .iter()
                .filter(|e| matches!(e, SketchEntity::Point(_)))
                .count(),
            4
        );
        assert_eq!(
            sketch
                .entities
                .iter()
                .filter(|e| matches!(e, SketchEntity::Line(_)))
                .count(),
            4
        );
        assert!(sketch.constraints.is_empty());
        let value = serde_json::to_value(&sketch.entities).unwrap();
        let expected = [(2., -3.), (22., -3.), (22., 7.), (2., 7.)];
        for (i, (x, y)) in expected.into_iter().enumerate() {
            assert_eq!(value[i]["id"], format!("ent:rectangle_1_p{i}"));
            assert_eq!(value[i]["x"], format!("{x:.3} mm"));
            assert_eq!(value[i]["y"], format!("{y:.3} mm"));
            assert_eq!(value[4 + i]["id"], format!("ent:rectangle_1_e{i}"));
            assert_eq!(value[4 + i]["start"], format!("ent:rectangle_1_p{i}"));
            assert_eq!(
                value[4 + i]["end"],
                format!("ent:rectangle_1_p{}", (i + 1) % 4)
            );
        }
        if let Some(wanted) = &canonical {
            assert_eq!(&value, wanted);
        } else {
            canonical = Some(value);
        }
    }
}

#[test]
fn expanded_source_operations_remain_bounded_and_construction_edges_do_not_form_profiles() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut doc = fixture(false);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
    let mut edit = intent(&doc, true);
    edit.operations = (0..8)
        .map(|i| SketchForm::Rectangle {
            id: format!("ent:rectangle_{i}"),
            x1: format!("{} mm", i * 30),
            y1: "0 mm".into(),
            x2: format!("{} mm", i * 30 + 20),
            y2: "10 mm".into(),
            construction: true,
        })
        .collect();
    let plan = plan_sketch_edit_with_kernel(&doc, &edit, &kernel).unwrap();
    assert_eq!(plan.patch.operations.len(), 64);
    assert!(plan.state.sketches[0].solved.profiles.is_empty());
    edit.operations.push(SketchForm::Rectangle {
        id: "ent:rectangle_9".into(),
        x1: "300 mm".into(),
        y1: "0 mm".into(),
        x2: "320 mm".into(),
        y2: "10 mm".into(),
        construction: true,
    });
    let stable = doc.clone();
    let saved = history.clone();
    let error = apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, &kernel).unwrap_err();
    assert!(error.to_string().contains("64"), "{error}");
    assert_eq!(doc, stable);
    assert_eq!(history, saved);
}
