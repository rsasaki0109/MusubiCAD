//! ADR-105: three-point ellipse intent, independent physical placement and history.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{GlobalPlane, Workplane};

fn fixture(datum: bool) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:pointer_ellipse").unwrap(),
        "Pointer ellipse",
    ));
    if datum {
        doc.feature_nodes.push(FeatureNode::new(
            "feature:datum",
            "Tilted plane",
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
        name: "Pointer ellipse".into(),
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
        operations: vec![SketchForm::EllipseAxes {
            id: "ent:ellipse_1".into(),
            center_x: "2.000 mm".into(),
            center_y: "-3.000 mm".into(),
            major_x: "26.000 mm".into(),
            major_y: "15.000 mm".into(),
            through_x: "14.000 mm".into(),
            through_y: "21.000 mm".into(),
            construction,
        }],
    }
}
fn near(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
    for i in 0..3 {
        assert!(
            (actual[i] - expected[i]).abs() < tolerance,
            "{actual:?} != {expected:?}"
        );
    }
}
fn extrude<K: GeometryKernel>(doc: &mut OcadDocument, history: &mut DocumentHistory, kernel: &K) {
    let tree = feature_tree(doc).unwrap();
    let owner = tree
        .rows
        .iter()
        .find(|r| r.id == "feature:sketch_new_1")
        .unwrap();
    assert_eq!(owner.closed_profiles.len(), 1);
    apply_feature_edit_with_kernel(
        doc,
        &tree.revision,
        FeatureEdit::CreateExtrude {
            id: "feature:pointer_body".into(),
            name: "Pointer ellipse prism".into(),
            sketch_feature: owner.id.clone(),
            profile_ref: owner.closed_profiles[0].clone(),
            length_expr: "6 mm".into(),
        },
        history,
        kernel,
    )
    .unwrap();
}
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod mesh_evidence;

fn physical<K: GeometryKernel>(kernel: &K, native_mesh: bool) {
    let r5 = 5_f64.sqrt();
    let r145 = 145_f64.sqrt();
    let rx = 0.012 * r5;
    let ry = 0.002 * r145;
    for (plane, datum, low, high, center, major, minor) in [
        (
            GlobalPlane::XY,
            false,
            [0.002 - rx, -0.003 - ry, 0.],
            [0.002 + rx, -0.003 + ry, 0.006],
            [0.002, -0.003, 0.003],
            [0.8, 0.6, 0.],
            [-0.6, 0.8, 0.],
        ),
        (
            GlobalPlane::YZ,
            false,
            [0., 0.002 - rx, -0.003 - ry],
            [0.006, 0.002 + rx, -0.003 + ry],
            [0.003, 0.002, -0.003],
            [0., 0.8, 0.6],
            [0., -0.6, 0.8],
        ),
        (
            GlobalPlane::XZ,
            false,
            [0.002 - rx, -0.006, -0.003 - ry],
            [0.002 + rx, 0., -0.003 + ry],
            [0.002, -0.003, -0.003],
            [0.8, 0., 0.6],
            [-0.6, 0., 0.8],
        ),
        (
            GlobalPlane::XY,
            true,
            [0.012 - rx, -0.0194 - 0.0016 * r145, 0.0358 - 0.0012 * r145],
            [0.012 + rx, -0.0158 + 0.0016 * r145, 0.0406 + 0.0012 * r145],
            [0.012, -0.0176, 0.0382],
            [0.8, 0.48, -0.36],
            [-0.6, 0.64, -0.48],
        ),
    ] {
        let mut doc = fixture(datum);
        let mut history = DocumentHistory::default();
        create(&mut doc, plane, datum, &mut history, kernel);
        let before = doc.clone();
        let request = intent(&doc, false);
        let plan = plan_sketch_edit_with_kernel(&doc, &request, kernel).unwrap();
        assert_eq!(doc, before);
        assert_eq!(plan.patch.operations.len(), 2);
        assert!(plan.evidence.volume_m3.is_none());
        apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).unwrap();
        assert_eq!(doc.parameters, before.parameters);
        assert_eq!(doc.feature_nodes, before.feature_nodes);
        assert_eq!(doc.sketches[0].workplane, before.sketches[0].workplane);
        assert_eq!(doc.sketches[0].entities.len(), 2);
        assert!(doc.sketches[0].constraints.is_empty());
        assert_eq!(history.undo_len(), 2);
        let source = document_design_state(&doc);
        history.undo(&mut doc).unwrap();
        assert_eq!(document_design_state(&doc), document_design_state(&before));
        history.redo(&mut doc).unwrap();
        assert_eq!(document_design_state(&doc), source);
        let authored = serde_json::to_value(&doc.sketches[0]).unwrap();
        let ellipse = authored["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["type"] == "ellipse")
            .unwrap();
        assert_eq!(ellipse["center"], "ent:ellipse_1_center");
        assert_eq!(ellipse["major_radius"], "30.000000000000 mm");
        assert_eq!(ellipse["minor_radius"], "20.000000000000 mm");
        let point = authored["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["type"] == "point")
            .unwrap();
        assert_eq!(point["x"], "2.000 mm");
        assert_eq!(point["y"], "-3.000 mm");
        extrude(&mut doc, &mut history, kernel);
        let original = doc.clone();
        let mut cache = opencad_feature::RegenerationCache::new();
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
            let volume = 3600. * std::f64::consts::PI * 1e-9;
            assert!((mass.volume_m3 - volume).abs() < 1e-12);
            // Independent complete elliptic-integral reference, a=30,b=20 mm.
            let area = 1200. * std::f64::consts::PI * 1e-6 + 0.1586543958929059 * 0.006;
            near(mass.center_of_mass, center, 3e-7);
            let bounds = kernel.bounding_box(body).unwrap();
            near(bounds.min, low, 3e-7);
            near(bounds.max, high, 3e-7);
            let result = kernel.tessellate(
                body,
                &opencad_geometry::TessellationSettings {
                    linear_deflection: 1e-5,
                    angular_deflection_deg: 4.,
                },
            );
            if native_mesh {
                let mesh = result.unwrap();
                let (v, c) = mesh_evidence::integrated(&mesh, true);
                assert!((v / volume - 1.).abs() < 0.01);
                near(c, center, 1e-5);
                // Sign-reflected orientations share mass and axis-aligned bounds.
                for (axis, radius) in [(major, 0.03), (minor, 0.02)] {
                    let values = mesh
                        .positions
                        .iter()
                        .map(|p| {
                            (0..3)
                                .map(|i| (p[i] as f64 - center[i]) * axis[i])
                                .sum::<f64>()
                        })
                        .collect::<Vec<_>>();
                    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    assert!(
                        (lo + radius).abs() < 1e-4 && (hi - radius).abs() < 1e-4,
                        "{lo}..{hi} != +/-{radius}"
                    );
                }
            } else {
                assert!(
                    result.is_err(),
                    "default Mock ellipse mesh must not become synthetic evidence"
                );
            }
            assert!(
                (mass.area_m2 - area).abs() < if native_mesh { 5e-9 } else { 1e-12 },
                "{} != {area}",
                mass.area_m2
            );
            assert_eq!(doc, original);
        }
    }
}
#[test]
fn ellipse_pointer_matches_independent_oriented_mass_area_bounds_and_source() {
    physical(&MockGeometryKernel::new(), false);
    #[cfg(feature = "occt")]
    physical(&opencad_kernel_occt::OcctGeometryKernel::new(), true);
}
#[test]
fn construction_ellipse_is_one_edit_without_an_invented_profile_or_body() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = fixture(true);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, true, &mut history, kernel);
        let before = doc.clone();
        apply_sketch_edit_with_kernel(&mut doc, &intent(&before, true), &mut history, kernel)
            .unwrap();
        let state = sketch_editor_state_with_kernel(&doc, kernel).unwrap();
        assert!(state.sketches[0].solved.profiles.is_empty());
        assert!(verify_editor_document(&doc, kernel)
            .unwrap()
            .volume_m3
            .is_none());
        assert_eq!(doc.sketches[0].entities.len(), 2);
        history.undo(&mut doc).unwrap();
        assert_eq!(doc, before);
    }
    check(&MockGeometryKernel::new());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn invalid_ellipse_units_points_identity_and_revision_preserve_source_history() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = fixture(false);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, false, &mut history, kernel);
        let stable = doc.clone();
        let saved = history.clone();
        for case in [
            "zero_major",
            "tiny_major",
            "large_major",
            "on_axis",
            "endpoint",
            "outer_span",
            "near_endpoint",
            "minor_too_large",
            "unitless",
            "angle_unit",
            "nonfinite",
            "parameter",
            "foreign",
            "missing_identity",
            "stale",
        ] {
            let mut request = intent(&doc, false);
            match case {
                "foreign" => request.document_id = Some("doc:foreign".into()),
                "missing_identity" => request.document_id = None,
                "stale" => request.revision = "stale".into(),
                _ => {
                    if let SketchForm::EllipseAxes {
                        major_x,
                        major_y,
                        through_x,
                        through_y,
                        center_x,
                        ..
                    } = &mut request.operations[0]
                    {
                        match case {
                            "zero_major" => {
                                *major_x = "2 mm".into();
                                *major_y = "-3 mm".into();
                            }
                            "tiny_major" => {
                                *major_x = "2.0000001 mm".into();
                                *major_y = "-3 mm".into();
                            }
                            "large_major" => {
                                *major_x = "1000000000 mm".into();
                                *major_y = "1000000000 mm".into();
                            }
                            "on_axis" => {
                                *through_x = "18 mm".into();
                                *through_y = "9 mm".into();
                            }
                            "endpoint" => {
                                *through_x = major_x.clone();
                                *through_y = major_y.clone();
                            }
                            "outer_span" => {
                                *through_x = "82 mm".into();
                                *through_y = "57 mm".into();
                            }
                            "near_endpoint" => {
                                *through_x = "25.999999999 mm".into();
                                *through_y = "15 mm".into();
                            }
                            "minor_too_large" => {
                                *through_x = "-34 mm".into();
                                *through_y = "45 mm".into();
                            }
                            "unitless" => *center_x = "2".into(),
                            "angle_unit" => *major_x = "42 deg".into(),
                            "nonfinite" => *major_x = "1e999 mm".into(),
                            _ => *major_x = "named_length".into(),
                        }
                    }
                }
            }
            assert!(
                apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).is_err(),
                "{case}"
            );
            assert_eq!(doc, stable);
            assert_eq!(history, saved);
        }
    }
    check(&MockGeometryKernel::new());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn ellipse_late_id_collisions_refuse_the_complete_multi_intent_batch() {
    let kernel = MockGeometryKernel::new();
    for suffix in ["", "_center"] {
        let mut doc = fixture(false);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
        let mut occupied = intent(&doc, true);
        occupied.operations = vec![SketchForm::Point {
            id: format!("ent:ellipse_1{suffix}"),
            x: "0 mm".into(),
            y: "0 mm".into(),
            create: true,
        }];
        apply_sketch_edit_with_kernel(&mut doc, &occupied, &mut history, &kernel).unwrap();
        let stable = doc.clone();
        let saved = history.clone();
        let mut request = intent(&doc, true);
        let mut first = request.operations[0].clone();
        if let SketchForm::EllipseAxes { id, .. } = &mut first {
            *id = "ent:first".into();
        }
        request.operations.insert(0, first);
        assert!(apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, &kernel).is_err());
        assert_eq!(doc, stable);
        assert_eq!(history, saved);
    }
}
#[test]
fn ellipse_expansion_preserves_the_64_source_operation_cap() {
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture(false);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
    for count in [32, 33] {
        let mut request = intent(&doc, true);
        let one = request.operations.remove(0);
        request.operations = (0..count)
            .map(|i| {
                let mut v = one.clone();
                if let SketchForm::EllipseAxes { id, .. } = &mut v {
                    *id = format!("ent:batch_{i}");
                }
                v
            })
            .collect();
        let result = plan_sketch_edit_with_kernel(&doc, &request, &kernel);
        if count == 32 {
            assert_eq!(result.unwrap().patch.operations.len(), 64);
        } else {
            assert!(result.is_err());
        }
        assert_eq!(history.undo_len(), 1);
        assert!(doc.sketches[0].entities.is_empty());
    }
}
#[test]
fn ellipse_pointer_cannot_bypass_required_solid_assertions() {
    use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
    let kernel = MockGeometryKernel::new();
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
    let stable = doc.clone();
    let saved = history.clone();
    assert!(apply_sketch_edit_with_kernel(
        &mut doc,
        &intent(&stable, false),
        &mut history,
        &kernel
    )
    .is_err());
    assert_eq!(doc, stable);
    assert_eq!(history, saved);
}
#[test]
#[cfg(feature = "occt")]
fn packed_and_expanded_pointer_ellipse_apply_history_and_stale_refusal() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    for name in ["ellipse.ocad", "ellipse.ocad.d"] {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(name);
        let path = path.to_str().unwrap();
        let mut doc = fixture(true);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, true, &mut history, &kernel);
        opencad_file::write_ocad(path, &doc).unwrap();
        let before = document_design_state(&doc);
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
fn third_point_sides_and_legacy_numeric_ellipse_editing_remain_explicit() {
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture(false);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
    let mut candidates = Vec::new();
    for (x, y) in [
        ("14.000 mm", "21.000 mm"),
        ("28.400 mm", "1.800 mm"),
        ("-10.000 mm", "13.000 mm"),
    ] {
        let mut request = intent(&doc, false);
        if let SketchForm::EllipseAxes {
            through_x,
            through_y,
            ..
        } = &mut request.operations[0]
        {
            *through_x = x.into();
            *through_y = y.into();
        }
        let plan = plan_sketch_edit_with_kernel(&doc, &request, &kernel).unwrap();
        candidates.push(plan.state.sketches[0].source.clone());
    }
    assert_eq!(candidates[0], candidates[1]);
    assert_eq!(candidates[0], candidates[2]);
    let request = intent(&doc, false);
    apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, &kernel).unwrap();
    let before = doc.clone();
    let mut numeric = intent(&doc, false);
    numeric.operations = vec![SketchForm::Ellipse {
        id: "ent:ellipse_1".into(),
        center: "ent:ellipse_1_center".into(),
        major_radius: "60 mm".into(),
        minor_radius: "20 mm".into(),
        axis_angle: "15 deg".into(),
        construction: false,
        create: false,
    }];
    assert_eq!(
        plan_sketch_edit_with_kernel(&doc, &numeric, &kernel)
            .unwrap()
            .patch
            .operations
            .len(),
        1
    );
    apply_sketch_edit_with_kernel(&mut doc, &numeric, &mut history, &kernel).unwrap();
    history.undo(&mut doc).unwrap();
    assert_eq!(doc, before);
}
