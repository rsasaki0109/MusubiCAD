//! ADR-102: source-bound datum editing, independent placement and atomic refusal.
use opencad_ai::DesignPatch;
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{
    apply_patch_with_history, document_design_state, DocumentHistory, OcadDocument,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_graph::ParameterEntry;
use opencad_sketch::{GlobalPlane, Workplane};

fn fixture(base: Workplane) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:datum_editor").unwrap(),
        "Datum editor",
    ));
    doc.parameters
        .add_parameter(ParameterEntry::new("param:offset", "offset", "5 mm"))
        .unwrap();
    doc.feature_nodes = vec![
        FeatureNode::new(
            "feature:root",
            "Root plane",
            FeatureDefinition::DatumPlane(DatumPlaneFeature {
                base,
                offset: Length::from_meters(0.005),
                offset_expr: Some("offset".into()),
            }),
        ),
        FeatureNode::new(
            "feature:child",
            "Signed child",
            FeatureDefinition::DatumPlane(DatumPlaneFeature {
                base: Workplane::DatumPlane {
                    feature_id: "feature:root".into(),
                },
                offset: Length::from_meters(-0.002),
                offset_expr: None,
            }),
        ),
    ];
    doc.feature_graph = opencad_feature::derive_feature_graph(
        &doc.feature_nodes,
        &["feature:root".into(), "feature:child".into()],
        &[],
        &[],
    )
    .unwrap();
    doc
}
fn tilted() -> Workplane {
    Workplane::Custom {
        origin: [0.010, -0.020, 0.030],
        normal: [0., 0.6, 0.8],
        x_axis: [1., 0., 0.],
    }
}
fn create<K: GeometryKernel>(doc: &OcadDocument, kernel: &K) -> SketchCreate {
    let s = sketch_creation_state_with_kernel(doc, kernel).unwrap();
    SketchCreate {
        document_id: s.document_id,
        revision: s.revision,
        sketch_id: s.suggested_sketch_id,
        feature_id: s.suggested_feature_id,
        name: "Datum ellipse".into(),
        plane: None,
        datum_plane: Some("feature:child".into()),
    }
}
fn edit(doc: &OcadDocument, operations: Vec<SketchForm>) -> SketchEdit {
    SketchEdit {
        document_id: Some(doc.metadata.id.as_str().into()),
        revision: feature_tree(doc).unwrap().revision,
        sketch_id: "sketch:new_1".into(),
        operations,
    }
}
fn point(x: &str, y: &str, create: bool) -> SketchForm {
    SketchForm::Point {
        id: "ent:center".into(),
        x: x.into(),
        y: y.into(),
        create,
    }
}
fn ellipse() -> SketchForm {
    SketchForm::Ellipse {
        id: "ent:ellipse".into(),
        center: "ent:center".into(),
        major_radius: "20 mm".into(),
        minor_radius: "10 mm".into(),
        axis_angle: "0 deg".into(),
        construction: false,
        create: true,
    }
}
fn near(a: [f64; 3], b: [f64; 3], tolerance: f64) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < tolerance, "{a:?} != {b:?}");
    }
}
fn physical<K: GeometryKernel>(kernel: &K) {
    // Hand-derived world positions, independent of the production frame resolver.
    for (base, origin, normal, x, y, low, high) in [
        (
            Workplane::xy(),
            [0., 0., 0.003],
            [0., 0., 1.],
            [1., 0., 0.],
            [0., 1., 0.],
            [-0.02, -0.01, 0.003],
            [0.02, 0.01, 0.009],
        ),
        (
            Workplane::Global {
                plane: GlobalPlane::YZ,
            },
            [0.003, 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [0.003, -0.02, -0.01],
            [0.009, 0.02, 0.01],
        ),
        (
            Workplane::Global {
                plane: GlobalPlane::XZ,
            },
            [0., -0.003, 0.],
            [0., -1., 0.],
            [1., 0., 0.],
            [0., 0., 1.],
            [-0.02, -0.009, -0.01],
            [0.02, -0.003, 0.01],
        ),
        (
            tilted(),
            [0.010, -0.0182, 0.0324],
            [0., 0.6, 0.8],
            [1., 0., 0.],
            [0., 0.8, -0.6],
            [-0.010, -0.0262, 0.0264],
            [0.030, -0.0066, 0.0432],
        ),
    ] {
        let mut doc = fixture(base);
        let baseline = doc.clone();
        let mut history = DocumentHistory::default();
        let create = create(&doc, kernel);
        let plan = plan_sketch_create_with_kernel(&doc, &create, kernel).unwrap();
        assert_eq!(doc, baseline);
        assert_eq!(plan.patch.operations.len(), 2);
        assert!(plan.evidence.volume_m3.is_none());
        assert!(plan.evidence.bounds_size_m.is_none());
        near(
            plan.state.sketches[0].resolved_frame.origin_m,
            origin,
            1e-12,
        );
        near(plan.state.sketches[0].resolved_frame.normal, normal, 1e-12);
        apply_sketch_create_with_kernel(&mut doc, &create, &mut history, kernel).unwrap();
        assert!(doc
            .feature_graph
            .dependency_edges()
            .iter()
            .any(|e| e.source == "feature:child" && e.target == create.feature_id));
        let p = edit(&doc, vec![point("0 mm", "0 mm", true)]);
        apply_sketch_edit_with_kernel(&mut doc, &p, &mut history, kernel).unwrap();
        let e = edit(&doc, vec![ellipse()]);
        apply_sketch_edit_with_kernel(&mut doc, &e, &mut history, kernel).unwrap();
        let tree = feature_tree(&doc).unwrap();
        let profile = tree
            .rows
            .iter()
            .find(|r| r.id == create.feature_id)
            .unwrap()
            .closed_profiles[0]
            .clone();
        apply_feature_edit_with_kernel(
            &mut doc,
            &tree.revision,
            FeatureEdit::CreateExtrude {
                id: "feature:body".into(),
                name: "Placed ellipse".into(),
                sketch_feature: create.feature_id.clone(),
                profile_ref: profile,
                length_expr: "6 mm".into(),
            },
            &mut history,
            kernel,
        )
        .unwrap();
        let source = doc.sketches[0].clone();
        let stale = edit(&doc, vec![point("2 mm", "-3 mm", false)]);
        let patch = DesignPatch::set_parameter("param:offset", "15 mm");
        let mut candidate = doc.clone();
        let mut next = history.clone();
        apply_patch_with_history(&mut candidate, &patch, &mut next, "Move datum").unwrap();
        verify_editor_document(&candidate, kernel).unwrap();
        doc = candidate;
        history = next;
        let stable = doc.clone();
        let before_history = history.clone();
        assert!(apply_sketch_edit_with_kernel(&mut doc, &stale, &mut history, kernel).is_err());
        assert_eq!(doc, stable);
        assert_eq!(history, before_history);
        let moved = edit(&doc, vec![point("2 mm", "-3 mm", false)]);
        apply_sketch_edit_with_kernel(&mut doc, &moved, &mut history, kernel).unwrap();
        assert_eq!(doc.sketches[0].workplane, source.workplane);
        assert_eq!(
            doc.sketches[0].workplane,
            Workplane::DatumPlane {
                feature_id: "feature:child".into()
            }
        );
        assert_eq!(doc.feature_nodes[..2], baseline.feature_nodes);
        let mut model = doc.clone().into_part_model();
        model
            .regenerate(
                kernel,
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                None,
            )
            .unwrap();
        let body = model.active_body().unwrap();
        let mass = kernel.mass_properties(body, 1.).unwrap();
        assert!((mass.volume_m3 - 1200. * std::f64::consts::PI * 1e-9).abs() < 1e-12);
        let bounds = kernel.bounding_box(body).unwrap();
        let delta: [f64; 3] =
            std::array::from_fn(|i| normal[i] * 0.010 + x[i] * 0.002 - y[i] * 0.003);
        near(bounds.min, std::array::from_fn(|i| low[i] + delta[i]), 3e-7);
        near(
            bounds.max,
            std::array::from_fn(|i| high[i] + delta[i]),
            3e-7,
        );
        near(
            mass.center_of_mass,
            std::array::from_fn(|i| origin[i] + normal[i] * 0.003 + delta[i]),
            3e-7,
        );
        let frame = sketch_editor_state_with_kernel(&doc, kernel)
            .unwrap()
            .sketches[0]
            .resolved_frame
            .clone();
        near(
            frame.origin_m,
            std::array::from_fn(|i| origin[i] + normal[i] * 0.010),
            1e-12,
        );
        near(frame.x_axis, x, 1e-12);
        near(frame.y_axis, y, 1e-12);
        assert_eq!(history.undo_len(), 6);
    }
}

#[test]
fn datum_sketch_edits_preserve_authored_association_and_independent_physical_placement() {
    physical(&MockGeometryKernel::new());
    #[cfg(feature = "occt")]
    physical(&opencad_kernel_occt::OcctGeometryKernel::new());
}

#[test]
fn datum_creation_and_edit_require_source_identity_and_exactly_one_admitted_plane() {
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture(tilted());
    let before = doc.clone();
    let good = create(&doc, &kernel);
    let mut history = DocumentHistory::default();
    for (plane, datum) in [
        (None, None),
        (Some(GlobalPlane::XY), Some("feature:child")),
        (None, Some("feature:missing")),
    ] {
        let mut bad = good.clone();
        bad.plane = plane;
        bad.datum_plane = datum.map(str::to_owned);
        assert!(apply_sketch_create_with_kernel(&mut doc, &bad, &mut history, &kernel).is_err());
        assert_eq!(doc, before);
        assert_eq!(history.undo_len(), 0);
    }
    let mut collision = good.clone();
    collision.feature_id = "feature:root".into();
    assert!(apply_sketch_create_with_kernel(&mut doc, &collision, &mut history, &kernel).is_err());
    assert_eq!(doc, before);
    let mut value = serde_json::to_value(&good).unwrap();
    value["frame"] = serde_json::json!({"origin_m":[0,0,0]});
    assert!(serde_json::from_value::<SketchCreate>(value).is_err());
    for field in ["plane", "datum_plane"] {
        let mut value = serde_json::to_value(&good).unwrap();
        value[field] = serde_json::Value::Null;
        assert!(serde_json::from_value::<SketchCreate>(value).is_err());
    }
    apply_sketch_create_with_kernel(&mut doc, &good, &mut history, &kernel).unwrap();
    let stable = doc.clone();
    let h = history.clone();
    let good_edit = edit(&doc, vec![point("0 mm", "0 mm", true)]);
    for document_id in [None, Some("doc:foreign".into())] {
        let mut bad = good_edit.clone();
        bad.document_id = document_id;
        assert!(apply_sketch_edit_with_kernel(&mut doc, &bad, &mut history, &kernel).is_err());
        assert_eq!(doc, stable);
        assert_eq!(history, h);
    }
    let mut foreign = doc.clone();
    foreign.metadata.id = DocumentId::new("doc:foreign").unwrap();
    assert_eq!(feature_tree(&foreign).unwrap().revision, good_edit.revision);
    assert!(plan_sketch_edit_with_kernel(&foreign, &good_edit, &kernel).is_err());
}

#[test]
fn datum_sketch_refuses_unresolved_or_unsupported_chains_with_cold_and_warm_outputs_intact() {
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture(tilted());
    let mut history = DocumentHistory::default();
    let request = create(&doc, &kernel);
    apply_sketch_create_with_kernel(&mut doc, &request, &mut history, &kernel).unwrap();
    for case in [
        "missing",
        "suppressed",
        "axis",
        "cycle",
        "face",
        "zero",
        "skew",
        "nonfinite",
    ] {
        let mut bad = doc.clone();
        match case {
            "missing" => {
                bad.feature_nodes.remove(0);
            }
            "suppressed" => bad.feature_nodes[0].suppressed = true,
            "axis" => {
                bad.feature_nodes[0].definition =
                    FeatureDefinition::DatumAxis(opencad_feature::datum::DatumAxisFeature {
                        origin_m: [0.; 3],
                        direction: [0., 1., 0.],
                        origin_expr: None,
                    })
            }
            _ => {
                let FeatureDefinition::DatumPlane(def) = &mut bad.feature_nodes[0].definition
                else {
                    unreachable!()
                };
                match case {
                    "cycle" => {
                        def.base = Workplane::DatumPlane {
                            feature_id: "feature:child".into(),
                        }
                    }
                    "face" => {
                        def.base = Workplane::FaceRef {
                            face_ref: "ref:some_actual_face".into(),
                        }
                    }
                    "zero" => {
                        def.base = Workplane::Custom {
                            origin: [0.; 3],
                            normal: [0.; 3],
                            x_axis: [1., 0., 0.],
                        }
                    }
                    "skew" => {
                        def.base = Workplane::Custom {
                            origin: [0.; 3],
                            normal: [0., 0., 1.],
                            x_axis: [1., 0., 1.],
                        }
                    }
                    "nonfinite" => {
                        def.offset = Length::from_meters(f64::NAN);
                        def.offset_expr = None;
                    }
                    _ => unreachable!(),
                }
            }
        }
        // Static admission refuses without trusting a previous valid output.
        assert!(
            opencad_feature::datum::admitted_sketch_datum_chain(
                "feature:child",
                bad.feature_nodes.iter()
            )
            .is_err(),
            "{case}"
        );
        let mut model = doc.clone().into_part_model();
        let mut cache = opencad_feature::RegenerationCache::default();
        model
            .regenerate_with_cache(
                &kernel,
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                None,
                &mut cache,
            )
            .unwrap();
        let outputs = model.outputs.clone();
        model.nodes = bad.clone().into_part_model().nodes;
        assert!(
            model
                .regenerate_with_cache(
                    &kernel,
                    &FeatureRegistry::with_defaults(),
                    Some(&doc.parameters),
                    None,
                    &mut cache
                )
                .is_err(),
            "warm {case}"
        );
        assert_eq!(model.outputs, outputs, "warm {case}");
        assert!(
            model
                .regenerate(
                    &kernel,
                    &FeatureRegistry::with_defaults(),
                    Some(&doc.parameters),
                    None
                )
                .is_err(),
            "cold {case}"
        );
        assert_eq!(model.outputs, outputs, "cold {case}");
        if case != "nonfinite" {
            let stable = document_design_state(&bad);
            let h = history.clone();
            let mut e = edit(&doc, vec![point("0 mm", "0 mm", true)]);
            e.revision = opencad_ai::design_state_revision(&document_design_state(&bad)).unwrap();
            assert!(
                apply_sketch_edit_with_kernel(&mut bad, &e, &mut history, &kernel).is_err(),
                "{case}"
            );
            assert_eq!(document_design_state(&bad), stable);
            assert_eq!(history, h);
        }
    }
    let mut bad = doc.clone();
    let mut graph = serde_json::to_value(&bad.feature_graph).unwrap();
    graph["order"].as_array_mut().unwrap().swap(0, 1);
    bad.feature_graph = serde_json::from_value(graph).unwrap();
    assert!(sketch_creation_state_with_kernel(&bad, &kernel).is_err());
}

#[test]
fn datum_drafts_do_not_disable_assertions_or_profile_consumers() {
    use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture(tilted());
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
    let r = create(&doc, &kernel);
    let mut history = DocumentHistory::default();
    let error = apply_sketch_create_with_kernel(&mut doc, &r, &mut history, &kernel).unwrap_err();
    assert!(error
        .to_string()
        .contains("required editor candidate assertions failed"));
    assert_eq!(doc, stable);
    assert_eq!(history.undo_len(), 0);
    doc.assertions.clear();
    let r = create(&doc, &kernel);
    apply_sketch_create_with_kernel(&mut doc, &r, &mut history, &kernel).unwrap();
    let stable = doc.clone();
    let tree = feature_tree(&doc).unwrap();
    assert!(apply_feature_edit_with_kernel(
        &mut doc,
        &tree.revision,
        FeatureEdit::CreateExtrude {
            id: "feature:bad_body".into(),
            name: "No profile".into(),
            sketch_feature: r.feature_id,
            profile_ref: "missing".into(),
            length_expr: "6 mm".into(),
        },
        &mut history,
        &kernel
    )
    .is_err());
    assert_eq!(doc, stable);
}

#[test]
#[cfg(feature = "occt")]
fn datum_creation_edit_and_parameter_history_roundtrip_packed_and_expanded() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    for filename in ["datum.ocad", "datum.ocad.d"] {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join(filename);
        let doc = fixture(tilted());
        opencad_file::write_ocad(&path, &doc).unwrap();
        let path = path.to_str().unwrap();
        let before = document_design_state(&opencad_file::read_ocad(path).unwrap());
        let kernel = MockGeometryKernel::new();
        let create = create(&doc, &kernel);
        let first = apply_document_sketch_create(path, &create, None).unwrap();
        let doc = opencad_file::read_ocad(path).unwrap();
        let e = edit(&doc, vec![point("0 mm", "0 mm", true), ellipse()]);
        let second = apply_document_sketch_edit(path, &e, Some(first.history.history)).unwrap();
        let state = opencad_desktop::set_document_parameter_with_history(
            path,
            "param:offset",
            "15 mm",
            Some(second.history.history),
        )
        .unwrap();
        let after = document_design_state(&opencad_file::read_ocad(path).unwrap());
        let mut h = state.history;
        for _ in 0..3 {
            h = undo_document_with_history(path, h).unwrap().history;
        }
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            before
        );
        for _ in 0..3 {
            h = redo_document_with_history(path, h).unwrap().history;
        }
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            after
        );
        let doc = opencad_file::read_ocad(path).unwrap();
        let before = document_design_state(&doc);
        let history = h.clone();
        let mut bad = e.clone();
        bad.document_id = Some("doc:other".into());
        bad.revision = feature_tree(&doc).unwrap().revision;
        assert!(apply_document_sketch_edit(path, &bad, Some(h)).is_err());
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            before
        );
        assert!(opencad_file::DocumentHistoryState::new(history).can_undo);
    }
}

#[test]
fn datum_profile_free_drafts_preserve_ownership_and_no_parameter_solve_guards() {
    use opencad_core::{ConstraintId, EntityId, Expression};
    use opencad_sketch::{CircleEntity, Constraint, Coord, EntityBase, PointEntity, SketchEntity};
    let kernel = MockGeometryKernel::new();
    let mut doc = fixture(tilted());
    doc.parameters = opencad_graph::ParamGraph::new();
    let FeatureDefinition::DatumPlane(root) = &mut doc.feature_nodes[0].definition else {
        unreachable!()
    };
    root.offset_expr = None;
    let request = create(&doc, &kernel);
    let mut history = DocumentHistory::default();
    apply_sketch_create_with_kernel(&mut doc, &request, &mut history, &kernel).unwrap();
    let mut model = doc.into_part_model();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = opencad_feature::RegenerationCache::default();
    model
        .regenerate_with_cache(&kernel, &registry, None, None, &mut cache)
        .unwrap();
    let outputs = model.outputs.clone();
    for case in [
        "owner_suppressed",
        "ambiguous_owner",
        "consumer",
        "contradiction",
    ] {
        let mut bad = model.clone();
        let mut warm = cache.clone();
        match case {
            "owner_suppressed" => bad.nodes.get_mut(&request.feature_id).unwrap().suppressed = true,
            "ambiguous_owner" => {
                bad.add_node(FeatureNode::new(
                    "feature:other_owner",
                    "Ambiguous",
                    FeatureDefinition::Sketch(opencad_feature::SketchFeatureDef {
                        sketch_id: request.sketch_id.clone(),
                    }),
                ))
                .unwrap();
            }
            "consumer" => {
                bad.add_node(FeatureNode::new(
                    "feature:consumer",
                    "Missing profile",
                    FeatureDefinition::Extrude(opencad_feature::ExtrudeFeature {
                        sketch_feature: request.feature_id.clone(),
                        profile_ref: "missing".into(),
                        extent: opencad_geometry::ExtrudeExtent::Distance {
                            length: Length::from_meters(0.006),
                        },
                        operation: opencad_geometry::ExtrudeOperation::NewBody,
                        length_expr: None,
                        target_feature: None,
                    }),
                ))
                .unwrap();
                bad.add_dependency(&request.feature_id, "feature:consumer")
                    .unwrap();
            }
            "contradiction" => {
                let s = bad.sketches.get_mut(&request.sketch_id).unwrap();
                s.entities.push(SketchEntity::Point(PointEntity {
                    base: EntityBase {
                        id: EntityId::new("ent:center").unwrap(),
                        construction: false,
                    },
                    x: Coord::Literal(0.),
                    y: Coord::Literal(0.),
                }));
                s.entities.push(SketchEntity::Circle(CircleEntity {
                    base: EntityBase {
                        id: EntityId::new("ent:guide").unwrap(),
                        construction: true,
                    },
                    center: EntityId::new("ent:center").unwrap(),
                    radius: Coord::Literal(0.005),
                }));
                for (id, expr) in [("con:one", "5 mm"), ("con:two", "6 mm")] {
                    s.constraints.push(Constraint::Radius {
                        id: ConstraintId::new(id).unwrap(),
                        target: EntityId::new("ent:guide").unwrap(),
                        expr: Expression::new(expr).unwrap(),
                    });
                }
            }
            _ => unreachable!(),
        }
        assert!(bad.prepare_sketches().is_err(), "{case}");
        assert!(
            bad.regenerate_with_cache(&kernel, &registry, None, None, &mut warm)
                .is_err(),
            "{case}"
        );
        assert_eq!(bad.outputs, outputs, "{case}");
    }
}
