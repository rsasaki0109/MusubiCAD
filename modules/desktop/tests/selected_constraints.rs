//! ADR-106 integration: selected source, actual solve state and physical results.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{GlobalPlane, SketchEntity, SolveState, Workplane};

fn edit(doc: &OcadDocument, operations: Vec<SketchForm>) -> SketchEdit {
    SketchEdit {
        document_id: Some(doc.metadata.id.as_str().into()),
        revision: feature_tree(doc).unwrap().revision,
        sketch_id: "sketch:new_1".into(),
        operations,
    }
}
fn fixture<K: GeometryKernel>(kernel: &K, plane: GlobalPlane, tilted: bool) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:selected_constraints").unwrap(),
        "Selected constraints",
    ));
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
            name: "Constraint circle".into(),
            plane: (!tilted).then_some(plane),
            datum_plane: tilted.then(|| "feature:datum".into()),
        },
        &mut history,
        kernel,
    )
    .unwrap();
    let request = edit(
        &doc,
        vec![
            SketchForm::Point {
                id: "ent:a".into(),
                x: "2 mm".into(),
                y: "-3 mm".into(),
                create: true,
            },
            SketchForm::Point {
                id: "ent:b".into(),
                x: "26 mm".into(),
                y: "15 mm".into(),
                create: true,
            },
            SketchForm::Line {
                id: "ent:guide".into(),
                start: "ent:a".into(),
                end: "ent:b".into(),
                construction: true,
            },
            SketchForm::Circle {
                id: "ent:disc".into(),
                center: "ent:a".into(),
                radius: "5 mm".into(),
                construction: false,
                create: true,
            },
        ],
    );
    apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).unwrap();
    let tree = feature_tree(&doc).unwrap();
    let owner = tree
        .rows
        .iter()
        .find(|r| r.id == "feature:sketch_new_1")
        .unwrap();
    apply_feature_edit_with_kernel(
        &mut doc,
        &tree.revision,
        FeatureEdit::CreateExtrude {
            id: "feature:disc_body".into(),
            name: "Constrained disc".into(),
            sketch_feature: owner.id.clone(),
            profile_ref: owner.closed_profiles[0].clone(),
            length_expr: "6 mm".into(),
        },
        &mut history,
        kernel,
    )
    .unwrap();
    doc
}
fn forms() -> Vec<SketchForm> {
    vec![
        SketchForm::Horizontal {
            id: "con:control_1".into(),
            line: "ent:guide".into(),
        },
        SketchForm::Distance {
            id: "con:control_2".into(),
            a: "ent:a".into(),
            b: "ent:b".into(),
            expr: "20 mm".into(),
        },
        SketchForm::Radius {
            id: "con:control_3".into(),
            target: "ent:disc".into(),
            expr: "10 mm".into(),
        },
    ]
}
fn near(a: [f64; 3], b: [f64; 3], tol: f64) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < tol, "{a:?} != {b:?}");
    }
}
fn dof(state: &SolveState) -> i32 {
    match state {
        SolveState::UnderConstrained { dof } => *dof,
        SolveState::FullyConstrained => 0,
        _ => panic!("unexpected state {state:?}"),
    }
}
#[path = "../../geometry/tests/support/mesh_evidence.rs"]
mod mesh_evidence;
fn physical<K: GeometryKernel>(kernel: &K) {
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
        let source = doc.clone();
        let mut history = DocumentHistory::default();
        let row = sketch_editor_state_with_kernel(&doc, kernel)
            .unwrap()
            .sketches
            .remove(0);
        assert_eq!(dof(&row.solved.solve_state), 3);
        assert_eq!(row.constraint_targets.points, ["ent:a", "ent:b"]);
        assert_eq!(row.constraint_targets.lines, ["ent:guide"]);
        assert_eq!(row.constraint_targets.circles, ["ent:disc"]);
        for (i, form) in forms().into_iter().enumerate() {
            let request = edit(&doc, vec![form]);
            let before = doc.clone();
            let plan = plan_sketch_edit_with_kernel(&doc, &request, kernel).unwrap();
            assert_eq!(doc, before);
            assert_eq!(
                dof(&plan.state.sketches[0].solved.solve_state),
                2 - i as i32
            );
            assert_eq!(plan.patch.operations.len(), 1);
            assert_eq!(
                plan.state.sketches[0].suggested_constraint_id,
                format!("con:control_{}", i + 2)
            );
            apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).unwrap();
        }
        assert_eq!(doc.sketches[0].entities, source.sketches[0].entities);
        assert_eq!(doc.feature_nodes, source.feature_nodes);
        assert_eq!(doc.parameters, source.parameters);
        assert_eq!(doc.sketches[0].workplane, source.sketches[0].workplane);
        let row = sketch_editor_state_with_kernel(&doc, kernel)
            .unwrap()
            .sketches
            .remove(0);
        assert_eq!(row.solved.solve_state, SolveState::FullyConstrained);
        let SketchEntity::Point(b) = row.solved.find_entity("ent:b").unwrap() else {
            panic!()
        };
        assert!((opencad_sketch::solve::coord_value(&b.x).unwrap() - 0.022).abs() < 1e-8);
        assert!((opencad_sketch::solve::coord_value(&b.y).unwrap() + 0.003).abs() < 1e-8);
        let authored = doc.clone();
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
            let volume = 600. * std::f64::consts::PI * 1e-9;
            assert!((mass.volume_m3 - volume).abs() < 1e-12);
            assert!((mass.area_m2 - 320. * std::f64::consts::PI * 1e-6).abs() < 1e-10);
            near(mass.center_of_mass, center, 3e-7);
            let bounds = kernel.bounding_box(body).unwrap();
            near(bounds.min, low, 3e-7);
            near(bounds.max, high, 3e-7);
            let mesh = kernel
                .tessellate(
                    body,
                    &opencad_geometry::TessellationSettings {
                        linear_deflection: 1e-5,
                        angular_deflection_deg: 4.,
                    },
                )
                .unwrap();
            let (v, c) = mesh_evidence::integrated(&mesh, false);
            assert!((v / volume - 1.).abs() < 0.01);
            near(c, center, 1e-4);
            assert_eq!(doc, authored);
        }
        let constrained = document_design_state(&doc);
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.sketches[0].constraints.len(), 2);
        history.redo(&mut doc).unwrap();
        assert_eq!(document_design_state(&doc), constrained);
        let remove = edit(
            &doc,
            vec![SketchForm::RemoveConstraint {
                id: "con:control_3".into(),
            }],
        );
        let plan = plan_sketch_edit_with_kernel(&doc, &remove, kernel).unwrap();
        assert_eq!(dof(&plan.state.sketches[0].solved.solve_state), 1);
        assert!(
            (plan.evidence.volume_m3.unwrap() - 150. * std::f64::consts::PI * 1e-9).abs() < 1e-12
        );
        apply_sketch_edit_with_kernel(&mut doc, &remove, &mut history, kernel).unwrap();
        history.undo(&mut doc).unwrap();
        assert_eq!(document_design_state(&doc), constrained);
    }
}
#[test]
fn selected_constraints_match_independent_dof_solved_dimensions_and_placed_physics() {
    physical(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    physical(&opencad_kernel_occt::OcctGeometryKernel::new());
}
fn refusals<K: GeometryKernel>(kernel: &K) {
    for tilted in [false, true] {
        let mut doc = fixture(kernel, GlobalPlane::XY, tilted);
        let mut history = DocumentHistory::default();
        let batch = edit(&doc, forms());
        apply_sketch_edit_with_kernel(&mut doc, &batch, &mut history, kernel).unwrap();
        let source = doc.clone();
        let saved = history.clone();
        for (form, reason) in [
            (
                SketchForm::Radius {
                    id: "con:bad".into(),
                    target: "ent:disc".into(),
                    expr: "11 mm".into(),
                },
                "contradictory",
            ),
            (
                SketchForm::Horizontal {
                    id: "con:duplicate".into(),
                    line: "ent:guide".into(),
                },
                "redundant constraints",
            ),
        ] {
            let request = edit(&doc, vec![form]);
            let error = apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("sketch edit requires a successfully solved sketch")
                    && error.contains(reason),
                "{error}"
            );
            assert_eq!(doc, source);
            assert_eq!(history, saved);
        }
        for expr in [
            "10",
            "10 deg",
            "0 mm",
            "-1 mm",
            "1e999 mm",
            "unknown_parameter",
        ] {
            let request = edit(
                &doc,
                vec![SketchForm::Radius {
                    id: "con:bad".into(),
                    target: "ent:disc".into(),
                    expr: expr.into(),
                }],
            );
            assert!(
                apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).is_err()
            );
            assert_eq!(doc, source);
            assert_eq!(history, saved);
        }
        for form in [
            SketchForm::Radius {
                id: "con:bad".into(),
                target: "ent:guide".into(),
                expr: "10 mm".into(),
            },
            SketchForm::Distance {
                id: "con:bad".into(),
                a: "ent:a".into(),
                b: "ent:disc".into(),
                expr: "10 mm".into(),
            },
            SketchForm::Horizontal {
                id: "con:bad".into(),
                line: "ent:disc".into(),
            },
            SketchForm::RemoveConstraint {
                id: "con:unknown".into(),
            },
            forms()[0].clone(),
        ] {
            let request = edit(&doc, vec![form]);
            assert!(
                apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).is_err()
            );
            assert_eq!(doc, source);
            assert_eq!(history, saved);
        }
        let mut request = edit(
            &doc,
            vec![
                SketchForm::RemoveConstraint {
                    id: "con:control_1".into(),
                },
                forms()[0].clone(),
            ],
        );
        assert!(plan_sketch_edit_with_kernel(&doc, &request, kernel)
            .unwrap_err()
            .to_string()
            .contains("cannot be reused"));
        request.operations = vec![SketchForm::RemoveConstraint {
            id: "con:control_1".into(),
        }];
        request.document_id = Some("doc:foreign".into());
        assert!(apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).is_err());
        request.document_id = Some(doc.metadata.id.as_str().into());
        request.revision = "stale".into();
        assert!(apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).is_err());
        assert_eq!(doc, source);
        assert_eq!(history, saved);
    }
}

#[test]
fn actual_backend_refusal_reasons_preserve_source_history_and_id_tombstones() {
    refusals(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    refusals(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn constraint_batch_cannot_bypass_required_solid_assertions_or_publish_its_prefix() {
    fn check<K: GeometryKernel>(kernel: &K) {
        use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        doc.assertions.push(Assertion::new(
            "assertion:disc_bounds",
            "Disc must fit its envelope",
            AssertionSeverity::Required,
            AssertionKind::BoundingBoxWithin {
                max_m: [0.015, 0.015, 0.01],
            },
        ));
        let mut history = DocumentHistory::default();
        let stable = doc.clone();
        let saved = history.clone();
        let request = edit(&doc, forms());
        let error = apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel)
            .unwrap_err()
            .to_string();
        assert!(error.contains("assertions failed"), "{error}");
        assert_eq!(doc, stable);
        assert_eq!(history, saved);
        let request = edit(
            &doc,
            vec![
                forms()[0].clone(),
                SketchForm::Radius {
                    id: "con:control_1".into(),
                    target: "ent:disc".into(),
                    expr: "10 mm".into(),
                },
            ],
        );
        assert!(apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).is_err());
        assert_eq!(doc, stable);
        assert_eq!(history, saved);
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn vertical_and_coincident_controls_solve_only_disposable_points() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let doc = fixture(kernel, GlobalPlane::XY, false);
        let plan = plan_sketch_edit_with_kernel(
            &doc,
            &edit(
                &doc,
                vec![SketchForm::Vertical {
                    id: "con:vertical".into(),
                    line: "ent:guide".into(),
                }],
            ),
            kernel,
        )
        .unwrap();
        let points = plan.state.sketches[0]
            .solved
            .entities
            .iter()
            .filter_map(|e| {
                if let SketchEntity::Point(p) = e {
                    Some(p)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!(
            (opencad_sketch::solve::coord_value(&points[0].x).unwrap()
                - opencad_sketch::solve::coord_value(&points[1].x).unwrap())
            .abs()
                < 1e-8
        );
        let mut detached = doc.clone();
        detached.sketches[0]
            .entities
            .retain(|e| e.id().as_str() != "ent:guide");
        let plan = plan_sketch_edit_with_kernel(
            &detached,
            &edit(
                &detached,
                vec![SketchForm::Coincident {
                    id: "con:coincident".into(),
                    a: "ent:a".into(),
                    b: "ent:b".into(),
                }],
            ),
            kernel,
        )
        .unwrap();
        let a = plan.state.sketches[0].solved.find_entity("ent:a").unwrap();
        let b = plan.state.sketches[0].solved.find_entity("ent:b").unwrap();
        let (SketchEntity::Point(a), SketchEntity::Point(b)) = (a, b) else {
            panic!()
        };
        assert!(
            (opencad_sketch::solve::coord_value(&a.x).unwrap()
                - opencad_sketch::solve::coord_value(&b.x).unwrap())
            .abs()
                < 1e-8
        );
        assert!(
            (opencad_sketch::solve::coord_value(&a.y).unwrap()
                - opencad_sketch::solve::coord_value(&b.y).unwrap())
            .abs()
                < 1e-8
        );
        assert!(doc.sketches[0].constraints.is_empty());
        assert!(detached.sketches[0].constraints.is_empty());
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn backend_catalogue_excludes_ellipse_and_cubic_controls() {
    let kernel = MockGeometryKernel::new();
    let doc = opencad_file::read_ocad(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cubic_plate.ocad.d"),
    )
    .unwrap();
    let rows = sketch_editor_state_with_kernel(&doc, &kernel)
        .unwrap()
        .sketches;
    assert!(!rows.is_empty());
    for row in rows {
        assert!(row.constraint_targets.points.is_empty());
        assert!(row.constraint_targets.circles.is_empty());
    }
    let doc = opencad_file::read_ocad(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/ellipse_plate.ocad.d"),
    )
    .unwrap();
    let rows = sketch_editor_state_with_kernel(&doc, &kernel)
        .unwrap()
        .sketches;
    assert!(!rows.is_empty());
    for row in rows {
        assert!(row.constraint_targets.circles.is_empty());
    }
}
#[test]
#[cfg(feature = "occt")]
fn selected_constraint_files_roundtrip_review_apply_and_canonical_history() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    for name in ["constraints.ocad", "constraints.ocad.d"] {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(name);
        let path = path.to_str().unwrap();
        let doc = fixture(&kernel, GlobalPlane::XY, true);
        opencad_file::write_ocad(path, &doc).unwrap();
        let before = document_design_state(&doc);
        let request = edit(&doc, forms());
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
