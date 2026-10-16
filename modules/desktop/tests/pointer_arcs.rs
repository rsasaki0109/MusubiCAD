//! ADR-104 typed arcs: selected branch, genuine physical output and atomic history.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{GlobalPlane, Workplane};

fn fixture(datum: bool) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:arc_sector").unwrap(),
        "Arc sector",
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
        name: "Arc sector".into(),
        plane: (!datum).then_some(plane),
        datum_plane: datum.then(|| "feature:datum".into()),
    };
    apply_sketch_create_with_kernel(doc, &request, history, kernel).unwrap();
}

#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod mesh_evidence;

fn intent(
    doc: &OcadDocument,
    clockwise: bool,
    close_sector: bool,
    construction: bool,
) -> SketchEdit {
    SketchEdit {
        document_id: Some(doc.metadata.id.as_str().into()),
        revision: feature_tree(doc).unwrap().revision,
        sketch_id: "sketch:new_1".into(),
        operations: vec![SketchForm::Arc {
            id: "ent:arc_1".into(),
            center_x: "2.000 mm".into(),
            center_y: "-3.000 mm".into(),
            start_x: "12.000 mm".into(),
            start_y: "-3.000 mm".into(),
            end_direction_x: "2.000 mm".into(),
            end_direction_y: "17.000 mm".into(),
            clockwise,
            close_sector,
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
            name: "Pointer sector".into(),
            sketch_feature: owner.id.clone(),
            profile_ref: owner.closed_profiles[0].clone(),
            length_expr: "6 mm".into(),
        },
        history,
        kernel,
    )
    .unwrap();
}
fn physical<K: GeometryKernel>(kernel: &K) {
    let d = 0.04 / (3. * std::f64::consts::PI);
    for clockwise in [false, true] {
        let (u, v) = if clockwise {
            (-d / 3., -d / 3.)
        } else {
            (d, d)
        };
        let low = if clockwise {
            [-0.008, -0.013]
        } else {
            [0.002, -0.003]
        };
        for (plane, datum, bounds_min, bounds_max, center) in [
            (
                GlobalPlane::XY,
                false,
                [low[0], low[1], 0.],
                [0.012, 0.007, 0.006],
                [0.002 + u, -0.003 + v, 0.003],
            ),
            (
                GlobalPlane::YZ,
                false,
                [0., low[0], low[1]],
                [0.006, 0.012, 0.007],
                [0.003, 0.002 + u, -0.003 + v],
            ),
            (
                GlobalPlane::XZ,
                false,
                [low[0], -0.006, low[1]],
                [0.012, 0., 0.007],
                [0.002 + u, -0.003, -0.003 + v],
            ),
            (
                GlobalPlane::XY,
                true,
                if clockwise {
                    [0.002, -0.0274, 0.0298]
                } else {
                    [0.012, -0.0194, 0.0298]
                },
                [0.022, -0.0078, if clockwise { 0.0466 } else { 0.0406 }],
                [0.012 + u, -0.0176 + 0.8 * v, 0.0382 - 0.6 * v],
            ),
        ] {
            let mut doc = fixture(datum);
            let mut history = DocumentHistory::default();
            create(&mut doc, plane, datum, &mut history, kernel);
            let before = doc.clone();
            let edit = intent(&doc, clockwise, true, false);
            let plan = plan_sketch_edit_with_kernel(&doc, &edit, kernel).unwrap();
            assert_eq!(doc, before);
            assert_eq!(plan.patch.operations.len(), 6);
            assert!(plan.evidence.volume_m3.is_none());
            apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
            assert_eq!(doc.parameters, before.parameters);
            assert_eq!(doc.feature_nodes, before.feature_nodes);
            assert_eq!(doc.sketches[0].entities.len(), 6);
            assert!(doc.sketches[0].constraints.is_empty());
            assert_eq!(history.undo_len(), 2);
            let source = document_design_state(&doc);
            history.undo(&mut doc).unwrap();
            assert_eq!(document_design_state(&doc), document_design_state(&before));
            history.redo(&mut doc).unwrap();
            assert_eq!(document_design_state(&doc), source);
            let authored = serde_json::to_value(&doc.sketches[0]).unwrap();
            let arc = authored["entities"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["type"] == "arc")
                .unwrap();
            assert_eq!(arc["radius"], "10.000000000000 mm");
            assert_eq!(
                arc["start_point"],
                if clockwise {
                    "ent:arc_1_end"
                } else {
                    "ent:arc_1_start"
                }
            );
            assert_eq!(
                arc["end_point"],
                if clockwise {
                    "ent:arc_1_start"
                } else {
                    "ent:arc_1_end"
                }
            );
            extrude(&mut doc, &mut history, kernel);
            let original = doc.clone();
            let mut cache = opencad_feature::RegenerationCache::new();
            for params in [None, Some(&doc.parameters), None] {
                let mut model = doc.clone().into_part_model();
                model
                    .regenerate_with_cache(
                        kernel,
                        &FeatureRegistry::with_defaults(),
                        params,
                        None,
                        &mut cache,
                    )
                    .unwrap();
                let body = model.active_body().unwrap();
                let mass = kernel.mass_properties(body, 1.).unwrap();
                let volume = if clockwise { 450. } else { 150. } * std::f64::consts::PI * 1e-9;
                assert!((mass.volume_m3 - volume).abs() < 1e-12);
                let area =
                    ((if clockwise { 240. } else { 80. }) * std::f64::consts::PI + 120.) * 1e-6;
                assert!((mass.area_m2 - area).abs() < 1e-10);
                near(mass.center_of_mass, center, 3e-7);
                let bounds = kernel.bounding_box(body).unwrap();
                near(bounds.min, bounds_min, 3e-7);
                near(bounds.max, bounds_max, 3e-7);
                let mesh = kernel
                    .tessellate(
                        body,
                        &opencad_geometry::TessellationSettings {
                            linear_deflection: 1e-5,
                            angular_deflection_deg: 4.,
                        },
                    )
                    .unwrap();
                let (mesh_volume, mesh_center) = mesh_evidence::integrated(&mesh, true);
                assert!((mesh_volume / volume - 1.).abs() < 0.01);
                near(mesh_center, center, 1e-5);
                assert_eq!(doc, original);
            }
        }
    }
}
#[test]
fn pointer_arc_minor_and_clockwise_major_match_independent_placed_physical_oracles() {
    physical(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    physical(&opencad_kernel_occt::OcctGeometryKernel::new());
}

fn drafts<K: GeometryKernel>(kernel: &K) {
    for (closed, construction) in [(false, false), (false, true), (true, true)] {
        let mut doc = fixture(true);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, true, &mut history, kernel);
        let edit = intent(&doc, false, closed, construction);
        let plan = plan_sketch_edit_with_kernel(&doc, &edit, kernel).unwrap();
        assert_eq!(plan.patch.operations.len(), if closed { 6 } else { 4 });
        assert!(plan.evidence.volume_m3.is_none());
        apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
        assert!(verify_editor_document(&doc, kernel)
            .unwrap()
            .volume_m3
            .is_none());
        assert!(feature_tree(&doc)
            .unwrap()
            .rows
            .iter()
            .all(|r| r.closed_profiles.is_empty()));
        if construction {
            assert!(plan.state.sketches[0].solved.profiles.is_empty());
        }
    }
}
#[test]
fn open_and_construction_arcs_remain_editable_without_inventing_a_body() {
    drafts(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    drafts(&opencad_kernel_occt::OcctGeometryKernel::new());
}
fn refusals<K: GeometryKernel>(kernel: &K) {
    let mut doc = fixture(false);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, false, &mut history, kernel);
    let stable = doc.clone();
    let saved = history.clone();
    for case in [
        "zero_radius",
        "tiny_radius",
        "large_radius",
        "end_at_center",
        "same_ray",
        "unitless",
        "angle_unit",
        "nonfinite",
        "parameter",
        "foreign",
        "missing_identity",
        "stale",
    ] {
        let mut edit = intent(&doc, false, true, false);
        match case {
            "foreign" => edit.document_id = Some("doc:foreign".into()),
            "missing_identity" => edit.document_id = None,
            "stale" => edit.revision = "stale".into(),
            _ => {
                if let SketchForm::Arc {
                    start_x,
                    start_y,
                    end_direction_x,
                    end_direction_y,
                    center_x,
                    ..
                } = &mut edit.operations[0]
                {
                    match case {
                        "zero_radius" => *start_x = "2 mm".into(),
                        "tiny_radius" => *start_x = "2.000001 mm".into(),
                        "large_radius" => *start_x = "1000003 mm".into(),
                        "end_at_center" => *end_direction_y = "-3 mm".into(),
                        "same_ray" => {
                            *end_direction_x = "22 mm".into();
                            *end_direction_y = "-3 mm".into();
                        }
                        "unitless" => *center_x = "2".into(),
                        "angle_unit" => *start_y = "-3 deg".into(),
                        "nonfinite" => *start_x = "1e999 mm".into(),
                        _ => *start_x = "radius_parameter".into(),
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
}
#[test]
fn invalid_arc_units_geometry_identity_or_revision_preserve_source_and_history() {
    refusals(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    refusals(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn every_late_derived_id_collision_refuses_the_entire_sector_batch() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    for suffix in ["", "_center", "_start", "_end", "_start_ray", "_end_ray"] {
        let mut doc = fixture(false);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
        let mut occupied = intent(&doc, false, true, false);
        occupied.operations = vec![SketchForm::Point {
            id: format!("ent:arc_1{suffix}"),
            x: "0 mm".into(),
            y: "0 mm".into(),
            create: true,
        }];
        apply_sketch_edit_with_kernel(&mut doc, &occupied, &mut history, &kernel).unwrap();
        let stable = doc.clone();
        let saved = history.clone();
        assert!(apply_sketch_edit_with_kernel(
            &mut doc,
            &intent(&stable, false, true, false),
            &mut history,
            &kernel
        )
        .is_err());
        assert_eq!(doc, stable);
        assert_eq!(history, saved);
    }
}
#[test]
fn arc_expansion_retains_the_existing_64_source_operation_budget() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut doc = fixture(false);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
    for (closed, count, operations) in [(false, 16, 64), (true, 10, 60), (true, 11, 66)] {
        let mut edit = intent(&doc, false, closed, true);
        let one = edit.operations.remove(0);
        edit.operations = (0..count)
            .map(|i| {
                let mut value = one.clone();
                if let SketchForm::Arc { id, .. } = &mut value {
                    *id = format!("ent:batch_{i}");
                }
                value
            })
            .collect();
        let result = plan_sketch_edit_with_kernel(&doc, &edit, &kernel);
        if operations <= 64 {
            assert_eq!(result.unwrap().patch.operations.len(), operations);
        } else {
            assert!(result.is_err());
        }
        assert_eq!(history.undo_len(), 1);
        assert!(doc.sketches[0].entities.is_empty());
    }
}
#[test]
fn arc_creation_cannot_bypass_required_solid_assertions() {
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
    let stable = doc.clone();
    let saved = history.clone();
    assert!(apply_sketch_edit_with_kernel(
        &mut doc,
        &intent(&stable, false, true, false),
        &mut history,
        &kernel
    )
    .is_err());
    assert_eq!(doc, stable);
    assert_eq!(history, saved);
}
#[test]
#[cfg(feature = "occt")]
fn packed_and_expanded_pointer_arc_apply_undo_redo_preserve_source() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    for name in ["arc.ocad", "arc.ocad.d"] {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(name);
        let path = path.to_str().unwrap();
        let mut doc = fixture(true);
        let mut history = DocumentHistory::default();
        create(&mut doc, GlobalPlane::XY, true, &mut history, &kernel);
        opencad_file::write_ocad(path, &doc).unwrap();
        let before = document_design_state(&doc);
        let request = intent(&doc, true, true, false);
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
fn arcs_crossing_zero_keep_the_selected_minor_or_major_branch() {
    fn check<K: GeometryKernel>(kernel: &K) {
        for clockwise in [false, true] {
            let mut doc = fixture(false);
            let mut history = DocumentHistory::default();
            create(&mut doc, GlobalPlane::XY, false, &mut history, kernel);
            let mut edit = intent(&doc, clockwise, true, false);
            if let SketchForm::Arc {
                start_x,
                start_y,
                end_direction_x,
                end_direction_y,
                ..
            } = &mut edit.operations[0]
            {
                *start_x = "11.848077530122 mm".into();
                *start_y = "-4.736481776669 mm".into();
                *end_direction_x = "11.848077530122 mm".into();
                *end_direction_y = "-1.263518223331 mm".into();
            }
            apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
            extrude(&mut doc, &mut history, kernel);
            let mut model = doc.into_part_model();
            model
                .regenerate(kernel, &FeatureRegistry::with_defaults(), None, None)
                .unwrap();
            let body = model.active_body().unwrap();
            let mass = kernel.mass_properties(body, 1.).unwrap();
            let theta = if clockwise { 17. } else { 1. } * std::f64::consts::PI / 9.;
            assert!((mass.volume_m3 - 300. * theta * 1e-9).abs() < 1e-12);
            let distance = 0.04 * (std::f64::consts::PI / 18.).sin() / (3. * theta);
            near(
                mass.center_of_mass,
                [
                    0.002 + if clockwise { -distance } else { distance },
                    -0.003,
                    0.003,
                ],
                3e-7,
            );
            let bounds = kernel.bounding_box(body).unwrap();
            near(
                bounds.min,
                if clockwise {
                    [-0.008, -0.013, 0.]
                } else {
                    [0.002, -0.004736481776669, 0.]
                },
                3e-7,
            );
            near(
                bounds.max,
                if clockwise {
                    [0.011848077530122, 0.007, 0.006]
                } else {
                    [0.012, -0.001263518223331, 0.006]
                },
                3e-7,
            );
        }
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}

#[test]
fn invalid_profile_free_arc_source_cannot_bypass_strict_draft_admission() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut doc = fixture(false);
    let mut history = DocumentHistory::default();
    create(&mut doc, GlobalPlane::XY, false, &mut history, &kernel);
    let edit = intent(&doc, false, false, true);
    apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, &kernel).unwrap();
    for case in ["zero", "full", "missing_endpoint"] {
        let mut bad = doc.clone();
        let arc = bad.sketches[0]
            .entities
            .iter_mut()
            .find_map(|e| {
                if let opencad_sketch::SketchEntity::Arc(a) = e {
                    Some(a)
                } else {
                    None
                }
            })
            .unwrap();
        match case {
            "zero" => arc.end_angle = opencad_sketch::Coord::Literal(0.),
            "full" => arc.end_angle = opencad_sketch::Coord::Literal(std::f64::consts::TAU),
            _ => arc.end_point = None,
        }
        assert!(verify_editor_document(&bad, &kernel).is_err(), "{case}");
    }
}
