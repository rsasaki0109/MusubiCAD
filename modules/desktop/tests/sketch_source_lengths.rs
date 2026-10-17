//! ADR-108: source expressions must become correct SI seeds, never persisted literals.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_feature::{
    datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry, RegenerationCache,
};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{Coord, GlobalPlane, SketchEntity, Workplane};

fn request(doc: &OcadDocument, operations: Vec<SketchForm>) -> SketchEdit {
    SketchEdit {
        document_id: Some(doc.metadata.id.as_str().into()),
        revision: feature_tree(doc).unwrap().revision,
        sketch_id: "sketch:new_1".into(),
        operations,
    }
}
fn radius(source: &str) -> SketchForm {
    SketchForm::Circle {
        id: "ent:disc".into(),
        center: "ent:center".into(),
        radius: source.into(),
        construction: false,
        create: false,
    }
}
fn fixture<K: GeometryKernel>(kernel: &K, plane: GlobalPlane, tilted: bool) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:source_lengths").unwrap(),
        "Source lengths",
    ));
    for (name, expr) in [
        ("origin_x", "1 mm"),
        ("origin_y", "-2 mm"),
        ("diameter", "20 mm"),
    ] {
        doc.parameters
            .add_parameter(opencad_graph::ParameterEntry::new(
                format!("param:{name}"),
                name,
                expr,
            ))
            .unwrap();
    }
    if tilted {
        doc.feature_nodes.push(FeatureNode::new(
            "feature:datum",
            "Tilted datum",
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
    let state = sketch_creation_state_with_kernel(&doc, kernel).unwrap();
    let mut history = DocumentHistory::default();
    apply_sketch_create_with_kernel(
        &mut doc,
        &SketchCreate {
            document_id: state.document_id,
            revision: state.revision,
            sketch_id: state.suggested_sketch_id,
            feature_id: state.suggested_feature_id,
            name: "Expression circle".into(),
            plane: (!tilted).then_some(plane),
            datum_plane: tilted.then(|| "feature:datum".into()),
        },
        &mut history,
        kernel,
    )
    .unwrap();
    let edit = request(
        &doc,
        vec![
            SketchForm::Point {
                id: "ent:center".into(),
                x: "origin_x + 1 mm".into(),
                y: "origin_y - 1 mm".into(),
                create: true,
            },
            SketchForm::Circle {
                id: "ent:disc".into(),
                center: "ent:center".into(),
                radius: "5 mm + 5 mm".into(),
                construction: false,
                create: true,
            },
        ],
    );
    apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
    let revision = feature_tree(&doc).unwrap().revision;
    apply_feature_edit_with_kernel(
        &mut doc,
        &revision,
        FeatureEdit::CreateExtrude {
            id: "feature:disc_body".into(),
            name: "Expression body".into(),
            sketch_feature: "feature:sketch_new_1".into(),
            profile_ref: "profile:circle:ent:disc".into(),
            length_expr: "6 mm".into(),
        },
        &mut history,
        kernel,
    )
    .unwrap();
    doc
}
fn near(a: [f64; 3], b: [f64; 3], tolerance: f64) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < tolerance, "{a:?} != {b:?}");
    }
}
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod mesh_evidence;
fn placed<K: GeometryKernel>(kernel: &K) {
    for (plane, tilted, low, high, center) in [
        (
            GlobalPlane::XY,
            false,
            [-0.008, -0.013, 0.],
            [0.012, 0.007, 0.006],
            [0.002, -0.003, 0.003],
        ),
        (
            GlobalPlane::YZ,
            false,
            [0., -0.008, -0.013],
            [0.006, 0.012, 0.007],
            [0.003, 0.002, -0.003],
        ),
        (
            GlobalPlane::XZ,
            false,
            [-0.008, -0.006, -0.013],
            [0.012, 0., 0.007],
            [0.002, -0.003, -0.003],
        ),
        (
            GlobalPlane::XY,
            true,
            [0.002, -0.0274, 0.0298],
            [0.022, -0.0078, 0.0466],
            [0.012, -0.0176, 0.0382],
        ),
    ] {
        let mut doc = fixture(kernel, plane, tilted);
        let mut history = DocumentHistory::default();
        for source in ["5 mm + 5 mm", "diameter / 4 + 5 mm"] {
            let edit = request(&doc, vec![radius(source)]);
            let saved = doc.clone();
            let plan = plan_sketch_edit_with_kernel(&doc, &edit, kernel).unwrap();
            assert_eq!(doc, saved);
            if let Some(volume) = plan.evidence.volume_m3 {
                assert!((volume - 600. * std::f64::consts::PI * 1e-9).abs() < 1e-12);
            } else {
                assert!(plan.evidence.no_op);
            }
            apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
            let source_snapshot = document_design_state(&doc);
            let state = sketch_editor_state_with_kernel(&doc, kernel).unwrap();
            let row = &state.sketches[0];
            let Some(SketchEntity::Point(point)) = row.solved.find_entity("ent:center") else {
                panic!("point")
            };
            let (Coord::Literal(x), Coord::Literal(y)) = (&point.x, &point.y) else {
                panic!("resolved point")
            };
            assert!((x - 0.002).abs() < 1e-10 && (y + 0.003).abs() < 1e-10);
            let mut model = doc.clone().into_part_model();
            model
                .regenerate(
                    kernel,
                    &FeatureRegistry::with_defaults(),
                    Some(&doc.parameters),
                    None,
                )
                .unwrap();
            assert_eq!(document_design_state(&doc), source_snapshot);
            let solid = model.active_body().unwrap();
            let mass = kernel.mass_properties(solid, 1.).unwrap();
            assert!((mass.volume_m3 - 600. * std::f64::consts::PI * 1e-9).abs() < 1e-12);
            assert!((mass.area_m2 - 320. * std::f64::consts::PI * 1e-6).abs() < 1e-10);
            near(mass.center_of_mass, center, 3e-7);
            let bounds = kernel.bounding_box(solid).unwrap();
            near(bounds.min, low, 3e-7);
            near(bounds.max, high, 3e-7);
            let mesh = kernel
                .tessellate(
                    solid,
                    &opencad_geometry::TessellationSettings {
                        linear_deflection: 1e-5,
                        angular_deflection_deg: 4.,
                    },
                )
                .unwrap();
            let (volume, centroid) = mesh_evidence::integrated(&mesh, false);
            assert!((volume / mass.volume_m3 - 1.).abs() < 0.01);
            near(centroid, center, 1e-4);
        }
        let source = document_design_state(&doc);
        let registry = FeatureRegistry::with_defaults();
        let mut cache = RegenerationCache::new();
        for (diameter, expected, cached) in [
            ("20 mm", 600., false),
            ("20 mm", 600., true),
            ("40 mm", 1350., false),
        ] {
            let mut current = doc.clone();
            current
                .parameters
                .set_expr("param:diameter", diameter)
                .unwrap();
            let mut model = current.into_part_model();
            let report = model
                .regenerate_with_cache(
                    kernel,
                    &registry,
                    Some(&{
                        let mut parameters = doc.parameters.clone();
                        parameters.set_expr("param:diameter", diameter).unwrap();
                        parameters
                    }),
                    None,
                    &mut cache,
                )
                .unwrap();
            if cached {
                assert!(report
                    .cached_nodes
                    .iter()
                    .any(|id| id == "feature:disc_body"));
            }
            let mass = kernel
                .mass_properties(model.active_body().unwrap(), 1.)
                .unwrap();
            assert!((mass.volume_m3 - expected * std::f64::consts::PI * 1e-9).abs() < 1e-12);
        }
        assert_eq!(document_design_state(&doc), source);
    }
}
#[test]
fn expression_circles_match_independent_placed_physics_and_source_driven_cache() {
    placed(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    placed(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn invalid_source_lengths_refuse_raw_and_typed_patches_without_history_changes() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let doc = fixture(&kernel, GlobalPlane::XY, false);
    for expr in [
        "5 deg",
        "5 furlong",
        "1 mm / 0",
        "missing_radius",
        "tool_angle",
    ] {
        let mut current = doc.clone();
        current
            .parameters
            .add_parameter(opencad_graph::ParameterEntry::new(
                "param:tool_angle",
                "tool_angle",
                "10 deg",
            ))
            .unwrap();
        let original = current.clone();
        let mut history = DocumentHistory::default();
        let edit = request(&current, vec![radius(expr)]);
        assert!(
            apply_sketch_edit_with_kernel(&mut current, &edit, &mut history, &kernel).is_err(),
            "{expr}"
        );
        assert_eq!(current, original);
        assert_eq!(history, DocumentHistory::default());
        let mut entity = doc.sketches[0].find_entity("ent:disc").unwrap().clone();
        let SketchEntity::Circle(circle) = &mut entity else {
            panic!("circle")
        };
        circle.radius = Coord::expr(expr).unwrap();
        let patch =
            opencad_ai::DesignPatch::new(vec![opencad_ai::PatchOperation::ReplaceSketchEntity {
                sketch_id: "sketch:new_1".into(),
                entity,
            }]);
        let mut state = document_design_state(&current);
        let before = state.clone();
        assert!(patch.apply_to_state(&mut state).is_err(), "{expr}");
        assert_eq!(state, before);
    }
    // Existing constraints still drive seeds; an expression is not a new fixed constraint.
    let mut current = doc;
    let mut history = DocumentHistory::default();
    let edit=request(&current,vec![serde_json::from_value(serde_json::json!({"operation":"radius","id":"con:drive","target":"ent:disc","expr":"12 mm"})).unwrap()]);
    apply_sketch_edit_with_kernel(&mut current, &edit, &mut history, &kernel).unwrap();
    let row = sketch_editor_state_with_kernel(&current, &kernel).unwrap();
    let Some(SketchEntity::Circle(circle)) = row.sketches[0].solved.find_entity("ent:disc") else {
        panic!("circle")
    };
    assert!(matches!(circle.radius,Coord::Literal(r) if (r-0.012).abs()<1e-8));
    assert!(
        matches!(&current.sketches[0].find_entity("ent:disc"),Some(SketchEntity::Circle(circle)) if circle.radius==Coord::expr("5 mm + 5 mm").unwrap())
    );
}
#[test]
#[cfg(feature = "occt")]
fn expression_files_preserve_review_refusal_and_canonical_undo_redo() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    for name in ["lengths.ocad", "lengths.ocad.d"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        let path = path.to_str().unwrap();
        let doc = fixture(&kernel, GlobalPlane::XY, true);
        opencad_file::write_ocad(path, &doc).unwrap();
        let source = document_design_state(&doc);
        let edit = request(&doc, vec![radius("diameter / 4 + 5 mm")]);
        plan_document_sketch_edit(path, &edit).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            source
        );
        let result = apply_document_sketch_edit(path, &edit, None).unwrap();
        let after = opencad_file::read_ocad(path).unwrap();
        let desired = document_design_state(&after);
        let state = undo_document_with_history(path, result.history.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            source
        );
        let state = redo_document_with_history(path, state.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            desired
        );
        let file = if name.ends_with(".d") {
            format!("{path}/checksums.json")
        } else {
            path.into()
        };
        let bytes = std::fs::read(&file).unwrap();
        for bad in [request(&after, vec![radius("5 deg")]), edit] {
            assert!(apply_document_sketch_edit(path, &bad, Some(state.history.clone())).is_err());
            assert_eq!(std::fs::read(&file).unwrap(), bytes);
            assert_eq!(
                document_design_state(&opencad_file::read_ocad(path).unwrap()),
                desired
            );
        }
    }
}

#[test]
fn analytic_plans_use_resolved_source_points_circles_and_arcs() {
    use opencad_ai::{plan_curve_edit, CurveEditRequest};
    use opencad_sketch::curve_edit::{CurveEditEnd, CurveEditMode};
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut doc = fixture(&kernel, GlobalPlane::XY, false);
    let mut history = DocumentHistory::default();
    let edit = request(
        &doc,
        vec![
            SketchForm::Point {
                id: "ent:left".into(),
                x: "origin_x - 19 mm".into(),
                y: "origin_y + 5 mm".into(),
                create: true,
            },
            SketchForm::Point {
                id: "ent:right".into(),
                x: "origin_x + 21 mm".into(),
                y: "origin_y + 5 mm".into(),
                create: true,
            },
            SketchForm::Line {
                id: "ent:guide".into(),
                start: "ent:left".into(),
                end: "ent:right".into(),
                construction: true,
            },
        ],
    );
    apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, &kernel).unwrap();
    doc.sketches[0].entities.push(serde_json::from_value(serde_json::json!({"type":"arc","id":"ent:arc","construction":true,"center":"ent:center","radius":"diameter / 2","start_angle":"0 deg","end_angle":"180 deg"})).unwrap());
    let state = document_design_state(&doc);
    let saved = state.clone();
    for cutter in ["ent:disc", "ent:arc"] {
        for (index, x) in [(0, -0.006), (1, 0.010)] {
            let edit = CurveEditRequest {
                sketch_id: "sketch:new_1".into(),
                target_id: "ent:guide".into(),
                cutter_id: cutter.into(),
                mode: CurveEditMode::Trim,
                end: CurveEditEnd::End,
                new_point_id: "ent:cut_end".into(),
                intersection_index: Some(index),
                remove_constraints: vec![],
            };
            let plan = plan_curve_edit(&state, &edit).unwrap();
            let position = plan.intersections[index].position_m;
            assert!((position[0] - x).abs() < 1e-10 && (position[1] - 0.003).abs() < 1e-10);
            assert_eq!(state, saved);
            let mut ambiguous = edit;
            ambiguous.intersection_index = None;
            assert!(plan_curve_edit(&state, &ambiguous).is_err());
        }
    }
    for value in ["20 deg", "20 furlong", "1 mm / 0"] {
        let patch = opencad_ai::DesignPatch::new(vec![opencad_ai::PatchOperation::SetParameter {
            id: "param:diameter".into(),
            expr: value.into(),
        }]);
        let mut candidate = state.clone();
        assert!(patch.apply_to_state(&mut candidate).is_err(), "{value}");
        assert_eq!(candidate, saved);
    }
    let mut invalid = doc.clone();
    let Some(SketchEntity::Point(point)) = invalid.sketches[0]
        .entities
        .iter_mut()
        .find(|e| e.id().as_str() == "ent:left")
    else {
        panic!("point")
    };
    point.x = Coord::expr("5 deg").unwrap();
    assert!(sketch_editor_state_with_kernel(&invalid, &kernel).is_err());
}

#[test]
fn expression_edits_keep_assertion_failure_atomic_in_both_kernels() {
    fn check<K: GeometryKernel>(kernel: &K) {
        use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        let mut history = DocumentHistory::default();
        doc.assertions.push(Assertion::new(
            "assertion:envelope",
            "Envelope",
            AssertionSeverity::Required,
            AssertionKind::BoundingBoxWithin {
                max_m: [0.03, 0.03, 0.01],
            },
        ));
        let saved = doc.clone();
        let original_history = history.clone();
        let edit = request(&doc, vec![radius("diameter + 5 mm")]);
        let error = apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel)
            .unwrap_err()
            .to_string();
        assert!(error.contains("assertions failed"), "{error}");
        assert_eq!(doc, saved);
        assert_eq!(history, original_history);
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
