//! ADR-109: explicit analytic branches, verified solved endpoints and atomic history.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{curve_edit::*, GlobalPlane, SketchEntity, Workplane};
use serde_json::{json, Value};

fn example() -> Value {
    serde_json::from_str(include_str!(
        "../../../examples/agent/trim_selected_guide.json"
    ))
    .unwrap()
}
fn request(doc: &OcadDocument, form: Value) -> SketchEdit {
    SketchEdit {
        document_id: Some(doc.metadata.id.as_str().into()),
        revision: feature_tree(doc).unwrap().revision,
        sketch_id: "sketch:new_1".into(),
        operations: vec![serde_json::from_value(form).unwrap()],
    }
}
fn query(doc: &OcadDocument) -> SketchCurveEditQuery {
    SketchCurveEditQuery {
        document_id: doc.metadata.id.as_str().into(),
        revision: feature_tree(doc).unwrap().revision,
        sketch_id: "sketch:new_1".into(),
        target_id: "ent:guide".into(),
        cutter_id: "ent:disc".into(),
        mode: CurveEditMode::Trim,
        end: CurveEditEnd::End,
    }
}
fn fixture<K: GeometryKernel>(kernel: &K, plane: GlobalPlane, tilted: bool) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:curve_controls").unwrap(),
        "Selected curve edits",
    ));
    let example = example();
    for p in example["parameters"].as_array().unwrap() {
        doc.parameters
            .add_parameter(serde_json::from_value(p.clone()).unwrap())
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
            name: "Selected guide".into(),
            plane: (!tilted).then_some(plane),
            datum_plane: tilted.then(|| "feature:datum".into()),
        },
        &mut history,
        kernel,
    )
    .unwrap();
    let mut edit = request(&doc, example["primitives"][0].clone());
    edit.operations = serde_json::from_value(example["primitives"].clone()).unwrap();
    apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
    // Test setup only; public file/protocol paths below use DesignPatch for constraints.
    doc.sketches[0].constraints = serde_json::from_value(example["constraints"].clone()).unwrap();
    let revision = feature_tree(&doc).unwrap().revision;
    apply_feature_edit_with_kernel(
        &mut doc,
        &revision,
        FeatureEdit::CreateExtrude {
            id: "feature:disc_body".into(),
            name: "Unchanged disc".into(),
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
fn near(a: [f64; 3], b: [f64; 3], tol: f64) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < tol, "{a:?} != {b:?}");
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
        let source = doc.clone();
        let options = sketch_curve_edit_options_with_kernel(&doc, &query(&doc), kernel).unwrap();
        assert_eq!(doc, source);
        assert_eq!(options.intersections.len(), 2);
        for (i, x) in [(0, -0.006), (1, 0.010)] {
            near(
                [
                    options.intersections[i].position_m[0],
                    options.intersections[i].position_m[1],
                    0.,
                ],
                [x, 0.003, 0.],
                1e-10,
            );
        }
        let mut history = DocumentHistory::default();
        let mut bad = example()["curve_edit"].clone();
        bad["remove_constraints"] = json!([]);
        let edit = request(&doc, bad);
        let error =
            apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap_err();
        assert!(
            error.to_string().contains("retained constraints"),
            "{error}"
        );
        assert_eq!(doc, source);
        assert_eq!(history, DocumentHistory::default());
        let edit = request(&doc, example()["curve_edit"].clone());
        let plan = plan_sketch_edit_with_kernel(&doc, &edit, kernel).unwrap();
        assert_eq!(doc, source);
        assert_eq!(plan.patch.operations.len(), 3);
        let effect = plan.curve_edit_effect.unwrap();
        assert_eq!(effect.selected_index, 1);
        near(
            [effect.solved_position_m[0], effect.solved_position_m[1], 0.],
            [0.010, 0.003, 0.],
            1e-9,
        );
        assert_eq!(plan.profile_effect.before.len(), 1);
        assert_eq!(plan.profile_effect.after.len(), 1);
        apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
        for entity in &source.sketches[0].entities {
            if entity.id().as_str() != "ent:guide" {
                assert_eq!(
                    doc.sketches[0].find_entity(entity.id().as_str()),
                    Some(entity)
                );
            }
        }
        assert_eq!(
            doc.sketches[0].constraints,
            vec![source.sketches[0].constraints[0].clone()]
        );
        let Some(SketchEntity::Line(line)) = doc.sketches[0].find_entity("ent:guide") else {
            panic!("line")
        };
        assert_eq!(line.end.as_str(), "ent:trim_end_1");
        let mut model = doc.clone().into_part_model();
        model
            .regenerate(
                kernel,
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                None,
            )
            .unwrap();
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
}
#[test]
fn selected_branches_preserve_authored_source_and_independent_placed_solids() {
    placed(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    placed(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn source_identity_admission_branch_and_explicit_removal_refusals_are_atomic() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let original = fixture(&kernel, GlobalPlane::XY, false);
    let good = request(&original, example()["curve_edit"].clone());
    for mutation in [
        "foreign",
        "stale",
        "missing_identity",
        "bad_branch",
        "existing_point",
        "bad_target",
        "same_curve",
        "duplicate_removal",
        "missing_removal",
        "mixed_batch",
    ] {
        let mut edit = good.clone();
        match mutation {
            "foreign" => edit.document_id = Some("doc:foreign".into()),
            "stale" => edit.revision = "stale".into(),
            "missing_identity" => edit.document_id = None,
            "mixed_batch" => edit.operations.push(SketchForm::Point {
                id: "ent:batch_prefix".into(),
                x: "1 mm".into(),
                y: "2 mm".into(),
                create: true,
            }),
            _ => {
                if let SketchForm::CurveEdit {
                    target_id,
                    cutter_id,
                    new_point_id,
                    intersection_index,
                    remove_constraints,
                    ..
                } = &mut edit.operations[0]
                {
                    match mutation {
                        "bad_branch" => *intersection_index = 42,
                        "existing_point" => *new_point_id = "ent:right".into(),
                        "bad_target" => *target_id = "ent:center".into(),
                        "same_curve" => *cutter_id = target_id.clone(),
                        "duplicate_removal" => {
                            remove_constraints.push(remove_constraints[0].clone())
                        }
                        "missing_removal" => remove_constraints.push("con:missing".into()),
                        _ => unreachable!(),
                    }
                }
            }
        }
        let mut doc = original.clone();
        let mut history = DocumentHistory::default();
        assert!(
            apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, &kernel).is_err(),
            "{mutation}"
        );
        assert_eq!(doc, original);
        assert_eq!(history, DocumentHistory::default());
    }
    for foreign in [true, false] {
        let mut doc = original.clone();
        if foreign {
            doc.feature_nodes
                .iter_mut()
                .find(|n| n.id == "feature:sketch_new_1")
                .unwrap()
                .suppressed = true;
        } else {
            doc.sketches[0].workplane = Workplane::Custom {
                origin: [0.; 3],
                normal: [0., 0., 1.],
                x_axis: [1., 0., 0.],
            };
        }
        let saved = doc.clone();
        let edit = request(&doc, example()["curve_edit"].clone());
        assert!(apply_sketch_edit_with_kernel(
            &mut doc,
            &edit,
            &mut DocumentHistory::default(),
            &kernel
        )
        .is_err());
        assert_eq!(doc, saved);
    }
    // Changing an end must select the other source point, with neither old point rewritten.
    let mut form = example()["curve_edit"].clone();
    form["end"] = json!("start");
    form["intersection_index"] = json!(0);
    let plan = plan_sketch_edit_with_kernel(&original, &request(&original, form), &kernel).unwrap();
    let effect = plan.curve_edit_effect.unwrap();
    near(
        [effect.solved_position_m[0], effect.solved_position_m[1], 0.],
        [-0.006, 0.003, 0.],
        1e-9,
    );
}

#[test]
fn line_extension_arc_end_edits_and_loop_consumers_use_the_same_verified_gateway() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        let mut history = DocumentHistory::default();
        let edit = request(&doc, example()["curve_edit"].clone());
        apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
        let mut setup = request(
            &doc,
            json!({"operation":"point","id":"ent:cut_low","x":"22 mm","y":"-20 mm","create":true}),
        );
        setup.operations=serde_json::from_value(json!([
            {"operation":"point","id":"ent:cut_low","x":"22 mm","y":"-20 mm","create":true},
            {"operation":"point","id":"ent:cut_high","x":"22 mm","y":"20 mm","create":true},
            {"operation":"line","id":"ent:cut","start":"ent:cut_low","end":"ent:cut_high","construction":true},
            {"operation":"point","id":"ent:axis_low","x":"2 mm","y":"-20 mm","create":true},
            {"operation":"point","id":"ent:axis_high","x":"2 mm","y":"20 mm","create":true},
            {"operation":"line","id":"ent:axis","start":"ent:axis_low","end":"ent:axis_high","construction":true},
            {"operation":"arc","id":"ent:arc","center_x":"2 mm","center_y":"-3 mm","start_x":"12 mm","start_y":"-3 mm","end_direction_x":"-8 mm","end_direction_y":"-3 mm","clockwise":false,"close_sector":false,"construction":true},
            {"operation":"rectangle","id":"ent:loop","x1":"40 mm","y1":"40 mm","x2":"60 mm","y2":"50 mm","construction":false},
            {"operation":"point","id":"ent:loop_low","x":"50 mm","y":"30 mm","create":true},
            {"operation":"point","id":"ent:loop_high","x":"50 mm","y":"60 mm","create":true},
            {"operation":"line","id":"ent:loop_cut","start":"ent:loop_low","end":"ent:loop_high","construction":true}
        ])).unwrap();
        apply_sketch_edit_with_kernel(&mut doc, &setup, &mut history, kernel).unwrap();
        for (target, cutter, mode, expected) in [
            ("ent:guide", "ent:cut", "extend", [0.022, 0.003, 0.]),
            ("ent:arc", "ent:axis", "trim", [0.002, 0.007, 0.]),
            ("ent:arc", "ent:axis", "extend", [0.002, -0.013, 0.]),
        ] {
            let form = json!({"operation":"curve_edit","target_id":target,"cutter_id":cutter,"mode":mode,"end":"end","intersection_index":0,"new_point_id":"ent:fresh"});
            let plan =
                plan_sketch_edit_with_kernel(&doc, &request(&doc, form.clone()), kernel).unwrap();
            let effect = plan.curve_edit_effect.unwrap();
            near(
                [effect.solved_position_m[0], effect.solved_position_m[1], 0.],
                expected,
                1e-9,
            );
            let mut candidate = doc.clone();
            let mut h = DocumentHistory::default();
            let edit = request(&candidate, form);
            apply_sketch_edit_with_kernel(&mut candidate, &edit, &mut h, kernel).unwrap();
            for e in &doc.sketches[0].entities {
                if e.id().as_str() != target {
                    assert_eq!(candidate.sketches[0].find_entity(e.id().as_str()), Some(e));
                }
            }
        }
        let row = sketch_editor_state_with_kernel(&doc, kernel).unwrap();
        let profile = row.sketches[0]
            .solved
            .profiles
            .iter()
            .find(|p| {
                p.entity_ids
                    .iter()
                    .any(|e| e.as_str().starts_with("ent:loop"))
            })
            .unwrap();
        let target = profile.entity_ids[0].as_str().to_string();
        let profile_ref = profile
            .profile_ref
            .clone()
            .unwrap_or_else(|| profile.id.clone());
        let form = json!({"operation":"curve_edit","target_id":target,"cutter_id":"ent:loop_cut","mode":"trim","end":"end","intersection_index":0,"new_point_id":"ent:loop_end"});
        let plan =
            plan_sketch_edit_with_kernel(&doc, &request(&doc, form.clone()), kernel).unwrap();
        assert_eq!(plan.profile_effect.before.len(), 2);
        assert_eq!(plan.profile_effect.after.len(), 1);
        let revision = feature_tree(&doc).unwrap().revision;
        apply_feature_edit_with_kernel(
            &mut doc,
            &revision,
            FeatureEdit::CreateExtrude {
                id: "feature:loop_body".into(),
                name: "Consumed rectangle".into(),
                sketch_feature: "feature:sketch_new_1".into(),
                profile_ref,
                length_expr: "6 mm".into(),
            },
            &mut history,
            kernel,
        )
        .unwrap();
        let original = doc.clone();
        let saved = history.clone();
        let edit = request(&doc, form);
        assert!(apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).is_err());
        assert_eq!(doc, original);
        assert_eq!(history, saved);
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
#[cfg(feature = "occt")]
fn packed_and_expanded_curve_reviews_refusals_and_history_preserve_canonical_source() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    for name in ["curve.ocad", "curve.ocad.d"] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(name);
        let path = path.to_str().unwrap();
        let doc = fixture(&kernel, GlobalPlane::XY, true);
        opencad_file::write_ocad(path, &doc).unwrap();
        let original = document_design_state(&doc);
        let edit = request(&doc, example()["curve_edit"].clone());
        inspect_document_curve_edit(path, &query(&doc)).unwrap();
        plan_document_sketch_edit(path, &edit).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            original
        );
        let result = apply_document_sketch_edit(path, &edit, None).unwrap();
        let after = document_design_state(&opencad_file::read_ocad(path).unwrap());
        let history = undo_document_with_history(path, result.history.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            original
        );
        let history = redo_document_with_history(path, history.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            after
        );
        let file = if name.ends_with(".d") {
            format!("{path}/checksums.json")
        } else {
            path.into()
        };
        let bytes = std::fs::read(&file).unwrap();
        assert!(apply_document_sketch_edit(path, &edit, Some(history.history)).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            after
        );
    }
}

#[test]
fn open_no_solid_drafts_assertion_failures_and_foreign_endpoint_collisions_are_bounded() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = OcadDocument::new(DocumentMetadata::new(
            DocumentId::new("doc:open_trim").unwrap(),
            "Open trim",
        ));
        let mut history = DocumentHistory::default();
        let state = sketch_creation_state_with_kernel(&doc, kernel).unwrap();
        apply_sketch_create_with_kernel(
            &mut doc,
            &SketchCreate {
                document_id: state.document_id,
                revision: state.revision,
                sketch_id: state.suggested_sketch_id,
                feature_id: state.suggested_feature_id,
                name: "Openable rectangle".into(),
                plane: Some(GlobalPlane::XY),
                datum_plane: None,
            },
            &mut history,
            kernel,
        )
        .unwrap();
        let mut setup = request(
            &doc,
            json!({"operation":"point","id":"ent:low","x":"10 mm","y":"-10 mm","create":true}),
        );
        setup.operations=serde_json::from_value(json!([
            {"operation":"rectangle","id":"ent:rectangle","x1":"0 mm","y1":"0 mm","x2":"20 mm","y2":"10 mm","construction":false},
            {"operation":"point","id":"ent:low","x":"10 mm","y":"-10 mm","create":true},
            {"operation":"point","id":"ent:high","x":"10 mm","y":"20 mm","create":true},
            {"operation":"line","id":"ent:cutter","start":"ent:low","end":"ent:high","construction":true},
            {"operation":"ellipse","id":"ent:ellipse","center":"ent:low","major_radius":"4 mm","minor_radius":"2 mm","axis_angle":"0 deg","construction":true,"create":true}
        ])).unwrap();
        apply_sketch_edit_with_kernel(&mut doc, &setup, &mut history, kernel).unwrap();
        let state = sketch_editor_state_with_kernel(&doc, kernel).unwrap();
        assert!(!state.sketches[0]
            .curve_edit_targets
            .cutters
            .iter()
            .any(|id| id == "ent:ellipse"));
        let target = state.sketches[0].solved.profiles[0].entity_ids[0].as_str();
        let form = json!({"operation":"curve_edit","target_id":target,"cutter_id":"ent:cutter","mode":"trim","end":"end","intersection_index":0,"new_point_id":"ent:fresh"});
        let plan =
            plan_sketch_edit_with_kernel(&doc, &request(&doc, form.clone()), kernel).unwrap();
        assert_eq!(plan.profile_effect.before.len(), 1);
        assert!(plan.profile_effect.after.is_empty());
        assert_eq!(plan.evidence.volume_m3, None);
        let edit = request(&doc, form);
        apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();

        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        let state = sketch_creation_state_with_kernel(&doc, kernel).unwrap();
        let other = state.suggested_sketch_id.clone();
        apply_sketch_create_with_kernel(
            &mut doc,
            &SketchCreate {
                document_id: state.document_id,
                revision: state.revision,
                sketch_id: other.clone(),
                feature_id: state.suggested_feature_id,
                name: "Foreign endpoint".into(),
                plane: Some(GlobalPlane::YZ),
                datum_plane: None,
            },
            &mut history,
            kernel,
        )
        .unwrap();
        let mut edit = request(
            &doc,
            json!({"operation":"point","id":"ent:remote","x":"0 mm","y":"0 mm","create":true}),
        );
        edit.sketch_id = other;
        apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).unwrap();
        let saved = doc.clone();
        let saved_history = history.clone();
        let mut form = example()["curve_edit"].clone();
        form["new_point_id"] = json!("ent:remote");
        let edit = request(&doc, form);
        assert!(apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel).is_err());
        assert_eq!(doc, saved);
        assert_eq!(history, saved_history);
        // Required assertions cannot be bypassed by a curve-only source edit.
        doc.assertions.push(opencad_core::Assertion::new(
            "assertion:envelope",
            "Too small envelope",
            opencad_core::AssertionSeverity::Required,
            opencad_core::AssertionKind::BoundingBoxWithin {
                max_m: [0.019, 0.03, 0.01],
            },
        ));
        let saved = doc.clone();
        let edit = request(&doc, example()["curve_edit"].clone());
        let error = apply_sketch_edit_with_kernel(&mut doc, &edit, &mut history, kernel)
            .unwrap_err()
            .to_string();
        assert!(error.contains("assertions failed"), "{error}");
        assert_eq!(doc, saved);
        assert_eq!(history, saved_history);
    }
    check(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new());
}
