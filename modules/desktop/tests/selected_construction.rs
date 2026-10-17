//! ADR-107: flag-only source intent, real profiles, physical output and atomic history.
use opencad_core::{DocumentId, DocumentMetadata, Length};
use opencad_desktop::{editor::*, sketch_creation::*, sketch_editor::*};
use opencad_feature::{datum::DatumPlaneFeature, FeatureDefinition, FeatureNode, FeatureRegistry};
use opencad_file::{document_design_state, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_sketch::{GlobalPlane, SketchEntity, Workplane};

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
        DocumentId::new("doc:selected_construction").unwrap(),
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
            name: "Construction circle".into(),
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
                radius: "1 cm".into(),
                construction: true,
                create: true,
            },
        ],
    );
    apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).unwrap();
    doc
}
fn flag(id: &str, construction: bool) -> SketchForm {
    SketchForm::Construction {
        id: id.into(),
        construction,
    }
}
fn change<K: GeometryKernel>(
    doc: &mut OcadDocument,
    history: &mut DocumentHistory,
    forms: Vec<SketchForm>,
    kernel: &K,
) {
    let request = edit(doc, forms);
    apply_sketch_edit_with_kernel(doc, &request, history, kernel).unwrap();
}
fn body<K: GeometryKernel>(doc: &mut OcadDocument, history: &mut DocumentHistory, kernel: &K) {
    let tree = feature_tree(doc).unwrap();
    apply_feature_edit_with_kernel(
        doc,
        &tree.revision,
        FeatureEdit::CreateExtrude {
            id: "feature:disc_body".into(),
            name: "Ordinary disc".into(),
            sketch_feature: "feature:sketch_new_1".into(),
            profile_ref: "profile:circle:ent:disc".into(),
            length_expr: "6 mm".into(),
        },
        history,
        kernel,
    )
    .unwrap();
}
fn near(a: [f64; 3], b: [f64; 3], tolerance: f64) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < tolerance, "{a:?} != {b:?}");
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
        let original = doc.clone();
        let mut history = DocumentHistory::default();
        let request = edit(&doc, vec![flag("ent:disc", false)]);
        let plan = plan_sketch_edit_with_kernel(&doc, &request, kernel).unwrap();
        assert_eq!(doc, original);
        assert!(plan.profile_effect.before.is_empty());
        assert_eq!(plan.profile_effect.after[0].entity_ids, ["ent:disc"]);
        assert!(plan.profile_effect.consumers.is_empty());
        assert_eq!(plan.evidence.volume_m3, None);
        assert!(
            matches!(&plan.patch.operations[..], [opencad_ai::PatchOperation::SetSketchConstruction{entity_id,construction:false,..}] if entity_id=="ent:disc")
        );
        apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).unwrap();
        let ordinary = document_design_state(&doc);
        history.undo(&mut doc).unwrap();
        assert_eq!(
            document_design_state(&doc),
            document_design_state(&original)
        );
        history.redo(&mut doc).unwrap();
        assert_eq!(document_design_state(&doc), ordinary);
        body(&mut doc, &mut history, kernel);
        let before_point = doc.clone();
        let plan =
            plan_sketch_edit_with_kernel(&doc, &edit(&doc, vec![flag("ent:a", true)]), kernel)
                .unwrap();
        assert_eq!(plan.profile_effect.before, plan.profile_effect.after);
        assert_eq!(
            plan.profile_effect.consumers[0].feature_id,
            "feature:disc_body"
        );
        change(&mut doc, &mut history, vec![flag("ent:a", true)], kernel);
        let mut expected = original.sketches[0].entities.clone();
        expected
            .iter_mut()
            .find(|e| e.id().as_str() == "ent:disc")
            .unwrap()
            .set_construction(false);
        expected
            .iter_mut()
            .find(|e| e.id().as_str() == "ent:a")
            .unwrap()
            .set_construction(true);
        assert_eq!(doc.sketches[0].entities, expected);
        assert_eq!(
            doc.sketches[0].constraints,
            original.sketches[0].constraints
        );
        assert_eq!(doc.sketches[0].workplane, original.sketches[0].workplane);
        assert_eq!(doc.feature_nodes, before_point.feature_nodes);
        assert_eq!(doc.parameters, original.parameters);
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
        let volume = 600. * std::f64::consts::PI * 1e-9;
        assert!(
            (mass.volume_m3 - volume).abs() < 1e-12,
            "{plane:?} tilted={tilted}: {mass:?}"
        );
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
        let (v, c) = mesh_evidence::integrated(&mesh, false);
        assert!((v / volume - 1.).abs() < 0.01);
        near(c, center, 1e-4);
        let saved = doc.clone();
        let saved_history = history.clone();
        let request = edit(&doc, vec![flag("ent:guide", false), flag("ent:disc", true)]);
        let error = apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel)
            .unwrap_err()
            .to_string();
        assert!(error.contains("profile"), "{error}");
        assert_eq!(doc, saved);
        assert_eq!(history, saved_history);
        let noop = edit(&doc, vec![flag("ent:a", true)]);
        let result = apply_sketch_edit_with_kernel(&mut doc, &noop, &mut history, kernel).unwrap();
        assert!(result.evidence.no_op);
        assert_eq!(history, saved_history);
        assert_eq!(doc, saved);
    }
}
#[test]
fn flag_only_circle_intent_and_point_semantics_match_independent_placed_physics() {
    physical(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    physical(&opencad_kernel_occt::OcctGeometryKernel::new());
}
fn profile_families<K: GeometryKernel>(kernel: &K) {
    let mut doc = fixture(kernel, GlobalPlane::XY, false);
    let mut history = DocumentHistory::default();
    change(
        &mut doc,
        &mut history,
        vec![
            SketchForm::Ellipse {
                id: "ent:ellipse".into(),
                center: "ent:a".into(),
                major_radius: "15 mm + 15 mm".into(),
                minor_radius: "20 mm".into(),
                axis_angle: "30 deg".into(),
                construction: true,
                create: true,
            },
            SketchForm::Rectangle {
                id: "ent:rect".into(),
                x1: "40 mm".into(),
                y1: "0 mm".into(),
                x2: "60 mm".into(),
                y2: "10 mm".into(),
                construction: true,
            },
            SketchForm::Arc {
                id: "ent:sector".into(),
                center_x: "-30 mm".into(),
                center_y: "0 mm".into(),
                start_x: "-20 mm".into(),
                start_y: "0 mm".into(),
                end_direction_x: "-30 mm".into(),
                end_direction_y: "10 mm".into(),
                clockwise: false,
                close_sector: true,
                construction: true,
            },
        ],
        kernel,
    );
    let original = doc.clone();
    let row = sketch_editor_state_with_kernel(&doc, kernel)
        .unwrap()
        .sketches
        .remove(0);
    for kind in ["point", "line", "circle", "ellipse", "arc"] {
        assert!(row.construction_targets.iter().any(|t| t.kind == kind));
    }
    assert!(row
        .constraint_targets
        .circles
        .iter()
        .all(|id| id != "ent:ellipse"));
    let curve_ids: Vec<_> = row
        .construction_targets
        .iter()
        .filter(|t| t.kind != "point")
        .map(|t| t.id.clone())
        .collect();
    let request = edit(&doc, curve_ids.iter().map(|id| flag(id, false)).collect());
    let plan = plan_sketch_edit_with_kernel(&doc, &request, kernel).unwrap();
    assert!(plan.profile_effect.before.is_empty());
    assert_eq!(plan.profile_effect.after.len(), 4);
    assert!(plan
        .profile_effect
        .after
        .iter()
        .any(|p| p.entity_ids == ["ent:ellipse"]));
    assert!(plan
        .profile_effect
        .after
        .iter()
        .any(|p| p.entity_ids.len() == 4));
    assert!(plan
        .profile_effect
        .after
        .iter()
        .any(|p| p.entity_ids.len() == 3));
    apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).unwrap();
    let before_break = doc.clone();
    let plan =
        plan_sketch_edit_with_kernel(&doc, &edit(&doc, vec![flag("ent:rect_e0", true)]), kernel)
            .unwrap();
    assert_eq!(plan.profile_effect.before.len(), 4);
    assert_eq!(plan.profile_effect.after.len(), 3);
    assert_eq!(doc, before_break);
    change(
        &mut doc,
        &mut history,
        curve_ids.iter().map(|id| flag(id, true)).collect(),
        kernel,
    );
    assert_eq!(doc.sketches[0].entities, original.sketches[0].entities);
}
#[test]
fn backend_catalogue_and_exact_closed_loop_effects_cover_all_admitted_families() {
    profile_families(&MockGeometryKernel::with_analytic_primitives());
    #[cfg(feature = "occt")]
    profile_families(&opencad_kernel_occt::OcctGeometryKernel::new());
}
#[test]
fn cubic_helpers_missing_targets_identity_owner_and_revision_remain_excluded() {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let mut doc = fixture(&kernel, GlobalPlane::XY, false);
    let mut history = DocumentHistory::default();
    let saved = doc.clone();
    let saved_history = history.clone();
    for id in ["ent:missing", "ent:unknown_helper"] {
        let request = edit(&doc, vec![flag("ent:guide", false), flag(id, true)]);
        assert!(apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, &kernel).is_err());
        assert_eq!(doc, saved);
        assert_eq!(history, saved_history);
    }
    for foreign in [true, false] {
        let mut request = edit(&doc, vec![flag("ent:a", true)]);
        if foreign {
            request.document_id = Some("doc:foreign".into());
        } else {
            request.revision = "stale".into();
        }
        assert!(apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, &kernel).is_err());
        assert_eq!(doc, saved);
        assert_eq!(history, saved_history);
    }
    doc.feature_nodes
        .iter_mut()
        .find(|n| n.id == "feature:sketch_new_1")
        .unwrap()
        .suppressed = true;
    assert!(
        plan_sketch_edit_with_kernel(&doc, &edit(&doc, vec![flag("ent:a", true)]), &kernel)
            .is_err()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let cubic = opencad_file::read_ocad(root.join("cubic_plate.ocad.d")).unwrap();
    for row in sketch_editor_state_with_kernel(&cubic, &kernel)
        .unwrap()
        .sketches
    {
        assert!(row.construction_targets.is_empty());
        for entity in &row.source.entities {
            let request = SketchEdit {
                document_id: Some(cubic.metadata.id.as_str().into()),
                revision: feature_tree(&cubic).unwrap().revision,
                sketch_id: row.source.id.as_str().into(),
                operations: vec![flag(entity.id().as_str(), true)],
            };
            assert!(plan_sketch_edit_with_kernel(&cubic, &request, &kernel).is_err());
        }
    }
    let mut helper_doc = saved;
    let helper: SketchEntity = serde_json::from_value(serde_json::json!({
        "type":"rectangle","id":"ent:stored_rectangle","construction":true,
        "origin":[0.04,0.0],"size":[0.02,0.01],
        "corner_ids":["ent:helper_p0","ent:helper_p1","ent:helper_p2","ent:helper_p3"],
        "edge_ids":["ent:helper_e0","ent:helper_e1","ent:helper_e2","ent:helper_e3"]
    }))
    .unwrap();
    helper_doc.sketches[0].entities.push(helper);
    let row = sketch_editor_state_with_kernel(&helper_doc, &kernel)
        .unwrap()
        .sketches
        .remove(0);
    assert!(row
        .construction_targets
        .iter()
        .all(|t| !t.id.contains("helper") && t.id != "ent:stored_rectangle"));
    for id in ["ent:stored_rectangle", "ent:helper_p0", "ent:helper_e0"] {
        assert!(plan_sketch_edit_with_kernel(
            &helper_doc,
            &edit(&helper_doc, vec![flag(id, false)]),
            &kernel
        )
        .is_err());
    }
}
#[test]
fn construction_batch_cannot_publish_its_prefix_when_solid_assertions_fail() {
    fn check<K: GeometryKernel>(kernel: &K) {
        use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        let mut history = DocumentHistory::default();
        change(
            &mut doc,
            &mut history,
            vec![flag("ent:disc", false)],
            kernel,
        );
        body(&mut doc, &mut history, kernel);
        doc.assertions.push(Assertion::new(
            "assertion:envelope",
            "Fits envelope",
            AssertionSeverity::Required,
            AssertionKind::BoundingBoxWithin {
                max_m: [0.03, 0.03, 0.01],
            },
        ));
        let saved = doc.clone();
        let saved_history = history.clone();
        let request = edit(
            &doc,
            vec![
                flag("ent:a", true),
                SketchForm::Circle {
                    id: "ent:disc".into(),
                    center: "ent:a".into(),
                    radius: "20 mm".into(),
                    construction: false,
                    create: false,
                },
            ],
        );
        let error = apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel)
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
#[test]
fn consumed_ellipse_rectangle_and_outer_alias_cannot_disappear_or_rebind() {
    fn check<K: GeometryKernel>(kernel: &K, ellipse_only: Option<bool>) {
        for (forms, profile, target, expected_volume) in [
            (
                vec![SketchForm::Ellipse {
                    id: "ent:ellipse".into(),
                    center: "ent:a".into(),
                    major_radius: "15 mm + 15 mm".into(),
                    minor_radius: "20 mm".into(),
                    axis_angle: "30 deg".into(),
                    construction: false,
                    create: true,
                }],
                "profile:ellipse:ent:ellipse",
                "ent:ellipse",
                3600. * std::f64::consts::PI * 1e-9,
            ),
            (
                vec![SketchForm::Rectangle {
                    id: "ent:rect".into(),
                    x1: "40 mm".into(),
                    y1: "0 mm".into(),
                    x2: "60 mm".into(),
                    y2: "10 mm".into(),
                    construction: false,
                }],
                "profile:loop:0",
                "ent:rect_e0",
                1200e-9,
            ),
        ] {
            if ellipse_only.is_some_and(|ellipse| ellipse != (target == "ent:ellipse")) {
                continue;
            }
            let mut doc = fixture(kernel, GlobalPlane::XY, false);
            let mut history = DocumentHistory::default();
            change(&mut doc, &mut history, forms, kernel);
            let tree = feature_tree(&doc).unwrap();
            let result = apply_feature_edit_with_kernel(
                &mut doc,
                &tree.revision,
                FeatureEdit::CreateExtrude {
                    id: "feature:disc_body".into(),
                    name: "Consumed loop".into(),
                    sketch_feature: "feature:sketch_new_1".into(),
                    profile_ref: profile.into(),
                    length_expr: "6 mm".into(),
                },
                &mut history,
                kernel,
            )
            .unwrap();
            assert!((result.evidence.volume_m3.unwrap() - expected_volume).abs() < 1e-12);
            let saved = doc.clone();
            let saved_history = history.clone();
            let request = edit(&doc, vec![flag("ent:a", true), flag(target, true)]);
            assert!(
                apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel).is_err()
            );
            assert_eq!(doc, saved);
            assert_eq!(history, saved_history);
        }
        if ellipse_only == Some(true) {
            return;
        }
        let mut doc = fixture(kernel, GlobalPlane::XY, false);
        let mut history = DocumentHistory::default();
        change(
            &mut doc,
            &mut history,
            vec![
                flag("ent:disc", false),
                SketchForm::Ellipse {
                    id: "ent:other_loop".into(),
                    center: "ent:a".into(),
                    major_radius: "30 mm".into(),
                    minor_radius: "20 mm".into(),
                    axis_angle: "0 deg".into(),
                    construction: false,
                    create: true,
                },
            ],
            kernel,
        );
        body(&mut doc, &mut history, kernel);
        // Hand-authored valid alias fixture: removing circle still leaves a
        // closed ellipse, but must not silently transfer this consumer to it.
        if let FeatureDefinition::Extrude(extrude) = &mut doc
            .feature_nodes
            .iter_mut()
            .find(|n| n.id == "feature:disc_body")
            .unwrap()
            .definition
        {
            extrude.profile_ref = "sketch:new_1/profile:outer".into();
        } else {
            panic!();
        }
        verify_editor_document(&doc, kernel).unwrap();
        let saved = doc.clone();
        let saved_history = history.clone();
        let request = edit(&doc, vec![flag("ent:disc", true)]);
        let error = apply_sketch_edit_with_kernel(&mut doc, &request, &mut history, kernel)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("construction edit changes feature"),
            "{error}"
        );
        assert_eq!(doc, saved);
        assert_eq!(history, saved_history);
    }
    check(&MockGeometryKernel::with_analytic_primitives(), Some(false));
    // Existing exact ellipse record path (not synthetic ID mass); ADR-062's
    // separately bounded primitive mode intentionally does not admit ellipses.
    check(&MockGeometryKernel::new(), Some(true));
    #[cfg(feature = "occt")]
    check(&opencad_kernel_occt::OcctGeometryKernel::new(), None);
}
#[test]
#[cfg(feature = "occt")]
fn construction_files_preserve_readonly_review_atomic_refusal_and_canonical_history() {
    use opencad_desktop::{redo_document_with_history, undo_document_with_history};
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    for name in ["construction.ocad", "construction.ocad.d"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        let path = path.to_str().unwrap();
        let doc = fixture(&kernel, GlobalPlane::XY, true);
        opencad_file::write_ocad(path, &doc).unwrap();
        let original = document_design_state(&doc);
        let request = edit(&doc, vec![flag("ent:disc", false)]);
        plan_document_sketch_edit(path, &request).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            original
        );
        let result = apply_document_sketch_edit(path, &request, None).unwrap();
        assert_eq!(result.history.history.undo_len(), 1);
        let ordinary = document_design_state(&opencad_file::read_ocad(path).unwrap());
        let state = undo_document_with_history(path, result.history.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            original
        );
        let state = redo_document_with_history(path, state.history).unwrap();
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            ordinary
        );
        let snapshot = std::fs::read(if name.ends_with(".d") {
            format!("{path}/checksums.json")
        } else {
            path.into()
        })
        .unwrap();
        assert!(apply_document_sketch_edit(path, &request, Some(state.history)).is_err());
        assert_eq!(
            std::fs::read(if name.ends_with(".d") {
                format!("{path}/checksums.json")
            } else {
                path.into()
            })
            .unwrap(),
            snapshot
        );
        assert_eq!(
            document_design_state(&opencad_file::read_ocad(path).unwrap()),
            ordinary
        );
    }
}
