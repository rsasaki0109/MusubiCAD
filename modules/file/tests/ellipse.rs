//! ADR-053: exact source identity and analytical evidence in both kernels.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::FeatureRegistry;
use opencad_file::{
    apply_patch_with_history, document_design_state, read_ocad, read_ocad_zip, write_expanded_dir,
    write_ocad_zip, DocumentHistory, OcadDocument,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::{json, Value};
use std::path::PathBuf;
fn patch(ops: Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":ops})).expect("patch")
}
fn ellipse(major: f64, minor: f64, angle: f64, plane: &str) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:ellipse_plate").expect("id"),
        "Parametric ellipse plate",
    ));
    let p = patch(json!([
        {"type":"add_parameter","id":"param:major_radius","name":"major_radius","expr":format!("{major} m")},
        {"type":"add_parameter","id":"param:minor_radius","name":"minor_radius","expr":format!("{minor} m")},
        {"type":"add_parameter","id":"param:axis_angle_rad","name":"axis_angle_rad","expr":format!("{angle} rad")},
        {"type":"add_parameter","id":"param:thickness","name":"thickness","expr":"6 mm"},
        {"type":"add_sketch","id":"sketch:ellipse","name":"Ellipse","workplane":{"type":"global","plane":plane}},
        {"type":"add_sketch_entity","sketch_id":"sketch:ellipse","entity":{"type":"point","id":"ent:anchor","x":0.,"y":0.}},
        {"type":"add_sketch_entity","sketch_id":"sketch:ellipse","entity":{"type":"point","id":"ent:center","x":0.,"y":0.}},
        {"type":"add_sketch_entity","sketch_id":"sketch:ellipse","entity":{"type":"ellipse","id":"ent:ellipse","center":"ent:center","major_radius":"major_radius","minor_radius":"minor_radius","axis_angle":"axis_angle_rad"}},
        {"type":"add_feature","node":{"id":"feature:ellipse_sketch","name":"Ellipse sketch","definition":{"type":"sketch","sketch_id":"sketch:ellipse"}},"position":opencad_ai::FeaturePosition::end()},
        {"type":"add_feature","node":{"id":"feature:ellipse_body","name":"Ellipse extrusion","definition":{"type":"extrude","sketch_feature":"feature:ellipse_sketch","profile_ref":"sketch:ellipse/profile:outer","extent":{"type":"distance","length":{"value_si":0.006}},"length_expr":"thickness","operation":"new_body"}},"position":opencad_ai::FeaturePosition::end()}
    ]));
    apply_patch_with_history(
        &mut doc,
        &p,
        &mut DocumentHistory::default(),
        "Build ellipse",
    )
    .expect("build");
    doc
}
fn measures<K: GeometryKernel>(
    doc: &OcadDocument,
    kernel: &K,
) -> (
    opencad_geometry::MassProperties,
    opencad_geometry::BoundingBox,
) {
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            None,
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    (
        kernel.mass_properties(body, 1.).expect("mass"),
        kernel.bounding_box(body).expect("bounds"),
    )
}
#[test]
fn both_kernels_match_ellipse_volume_area_rotated_bounds_and_planes() {
    for (a, b, angle, plane) in [
        (0.03, 0.02, 0., "XY"),
        (0.03, 0.02, std::f64::consts::FRAC_PI_6, "XY"),
        (0.03, 0.02, std::f64::consts::FRAC_PI_6, "XZ"),
        (0.025, 0.025, 0., "XY"),
    ] {
        let doc = ellipse(a, b, angle, plane);
        let expected = std::f64::consts::PI * a * b * 0.006;
        let (mock, mock_box) = measures(&doc, &MockGeometryKernel::new());
        let (native, native_box) = measures(&doc, &OcctGeometryKernel::new());
        assert!((mock.volume_m3 - expected).abs() < 1e-12);
        assert!((native.volume_m3 - expected).abs() < 1e-12);
        assert!(
            (mock.area_m2 - native.area_m2).abs() < 5e-9,
            "a={a} b={b} angle={angle} mock={} native={} diff={}",
            mock.area_m2,
            native.area_m2,
            mock.area_m2 - native.area_m2
        );
        let spans = if plane == "XY" {
            [
                2. * (a * angle.cos()).hypot(b * angle.sin()),
                2. * (a * angle.sin()).hypot(b * angle.cos()),
                0.006,
            ]
        } else {
            [
                2. * (a * angle.cos()).hypot(b * angle.sin()),
                0.006,
                2. * (a * angle.sin()).hypot(b * angle.cos()),
            ]
        };
        for (i, size) in spans.into_iter().enumerate() {
            assert!((mock_box.max[i] - mock_box.min[i] - size).abs() < 1e-12);
            assert!((native_box.max[i] - native_box.min[i] - size).abs() < 2.1e-7);
        }
    }
}
#[test]
fn axes_parameter_edits_regenerate_without_replacing_source_expressions() {
    let mut doc = ellipse(0.03, 0.02, 0., "XY");
    let original = doc.sketches[0].entities.clone();
    let before = measures(&doc, &MockGeometryKernel::new()).0.volume_m3;
    let p = patch(
        json!([{"type":"set_parameter","id":"param:major_radius","expr":"40 mm"},{"type":"set_parameter","id":"param:axis_angle_rad","expr":"30 deg"}]),
    );
    apply_patch_with_history(&mut doc, &p, &mut DocumentHistory::default(), "Edit axes")
        .expect("edit");
    assert_eq!(doc.sketches[0].entities, original);
    for volume in [
        measures(&doc, &MockGeometryKernel::new()).0.volume_m3,
        measures(&doc, &OcctGeometryKernel::new()).0.volume_m3,
    ] {
        assert!((volume - std::f64::consts::PI * 0.04 * 0.02 * 0.006).abs() < 1e-12);
        assert!(volume > before);
    }
    let angle = std::f64::consts::FRAC_PI_6;
    for bbox in [
        measures(&doc, &MockGeometryKernel::new()).1,
        measures(&doc, &OcctGeometryKernel::new()).1,
    ] {
        assert!(
            (bbox.max[0] - bbox.min[0] - 2. * (0.04 * angle.cos()).hypot(0.02 * angle.sin())).abs()
                < 2.1e-7
        );
    }
    assert!(doc.sketches[0].entities.iter().all(|e| match e {
        opencad_sketch::SketchEntity::Ellipse(e) => e.resolved_parameters.is_none(),
        _ => true,
    }));
}
#[test]
fn ellipse_wire_roundtrips_and_history_preserve_sources_exactly() {
    let mut doc = ellipse(0.03, 0.02, std::f64::consts::FRAC_PI_6, "XY");
    let original = doc.clone();
    let mut history = DocumentHistory::default();
    let p = patch(json!([{"type":"set_parameter","id":"param:major_radius","expr":"40 mm"}]));
    apply_patch_with_history(&mut doc, &p, &mut history, "Edit ellipse").expect("edit");
    let after = doc.clone();
    let wire = serde_json::to_vec(&history).expect("wire");
    let mut history: DocumentHistory = serde_json::from_slice(&wire).expect("history roundtrip");
    history.undo(&mut doc).expect("undo");
    assert_eq!(doc, original);
    history.redo(&mut doc).expect("redo");
    assert_eq!(doc, after);
    let temp = tempfile::tempdir().expect("temp");
    let expanded = temp.path().join("part.ocad.d");
    write_expanded_dir(&expanded, &doc).expect("write");
    let mut from_expanded = read_ocad(&expanded).expect("expanded");
    let packed = temp.path().join("part.ocad");
    write_ocad_zip(&packed, &doc).expect("pack");
    let mut from_packed = read_ocad_zip(&packed).expect("packed");
    from_expanded.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    from_packed.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let mut source = doc;
    source.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(from_expanded, source);
    assert_eq!(from_packed, source);
    let p = authoring_patch(&document_design_state(&source)).expect("authoring");
    assert_eq!(
        serde_json::from_slice::<DesignPatch>(&serde_json::to_vec(&p).expect("wire"))
            .expect("roundtrip"),
        p
    );
    let legacy = read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/construction_plate.ocad.d"),
    )
    .expect("legacy");
    assert!(legacy
        .sketches
        .iter()
        .flat_map(|s| &s.entities)
        .all(|e| !matches!(e, opencad_sketch::SketchEntity::Ellipse(_))));
}
#[test]
fn invalid_axes_and_implicit_profile_rebinding_refuse_atomically() {
    let mut doc = ellipse(0.03, 0.02, 0., "XY");
    let original = doc.clone();
    for (major, minor, angle) in [
        (json!(0.01), json!(0.02), json!(0.)),
        (json!(0.03), json!(0.), json!(0.)),
        (json!("missing"), json!(0.02), json!(0.)),
        (json!(0.03), json!(0.02), json!("1 mm")),
        (json!(0.03), json!(0.02), json!("major_radius")),
    ] {
        let p = patch(
            json!([{"type":"replace_sketch_entity","sketch_id":"sketch:ellipse","entity":{"type":"ellipse","id":"ent:ellipse","center":"ent:center","major_radius":major,"minor_radius":minor,"axis_angle":angle}}]),
        );
        assert!(apply_patch_with_history(
            &mut doc,
            &p,
            &mut DocumentHistory::default(),
            "Bad ellipse"
        )
        .is_err());
        assert_eq!(doc, original);
    }
    let p = patch(
        json!([{"type":"add_sketch_entity","sketch_id":"sketch:ellipse","entity":{"type":"circle","id":"ent:new_loop","center":"ent:center","radius":0.01}}]),
    );
    assert!(apply_patch_with_history(
        &mut doc,
        &p,
        &mut DocumentHistory::default(),
        "Implicit rebind"
    )
    .is_err());
    assert_eq!(doc, original);
    let p = patch(
        json!([{"type":"set_sketch_construction","sketch_id":"sketch:ellipse","entity_id":"ent:ellipse","construction":true}]),
    );
    assert!(apply_patch_with_history(
        &mut doc,
        &p,
        &mut DocumentHistory::default(),
        "Hide consumed loop"
    )
    .is_err());
    assert_eq!(doc, original);
}
#[test]
#[ignore = "generate verified parametric ellipse example"]
fn generate_ellipse_example() {
    let doc = ellipse(0.03, 0.02, std::f64::consts::FRAC_PI_6, "XY");
    measures(&doc, &MockGeometryKernel::new());
    measures(&doc, &OcctGeometryKernel::new());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    write_expanded_dir(root.join("examples/ellipse_plate.ocad.d"), &doc).expect("example");
    let p = authoring_patch(&document_design_state(&doc)).expect("authoring");
    std::fs::write(
        root.join("examples/agent/create_ellipse_plate_patch.json"),
        serde_json::to_string_pretty(&p).expect("JSON") + "\n",
    )
    .expect("patch");
}

#[test]
fn manufacturing_ellipse_alias_refuses_and_construction_ellipse_stays_excluded() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut doc = read_ocad(root.join("examples/construction_plate.ocad.d")).expect("legacy");
    let before = doc.clone();
    let make = |construction| {
        patch(
            json!([{"type":"add_sketch_entity","sketch_id":"sketch:base","entity":{"type":"ellipse","id":"ent:helper_ellipse","construction":construction,"center":"ent:guide_a","major_radius":0.01,"minor_radius":0.005,"axis_angle":0.}}]),
        )
    };
    assert!(apply_patch_with_history(
        &mut doc,
        &make(false),
        &mut DocumentHistory::default(),
        "Implicit alias"
    )
    .is_err());
    assert_eq!(doc, before);
    apply_patch_with_history(
        &mut doc,
        &make(true),
        &mut DocumentHistory::default(),
        "Construction ellipse",
    )
    .expect("helper");
    let volume = measures(&doc, &OcctGeometryKernel::new()).0.volume_m3;
    assert!((volume - 2.88e-5).abs() < 1e-12);
}
#[test]
fn centre_constraints_solve_without_inventing_axis_dof() {
    let mut doc = ellipse(0.03, 0.02, 0., "XY");
    let p = patch(
        json!([{"type":"replace_sketch_entity","sketch_id":"sketch:ellipse","entity":{"type":"point","id":"ent:anchor","x":0.01,"y":0.02}},{"type":"add_sketch_constraint","sketch_id":"sketch:ellipse","constraint":{"type":"coincident","id":"con:centre","a":"ent:anchor","b":"ent:center"}}]),
    );
    apply_patch_with_history(
        &mut doc,
        &p,
        &mut DocumentHistory::default(),
        "Centre constraint",
    )
    .expect("constraint");
    for centre in [
        measures(&doc, &MockGeometryKernel::new()).0.center_of_mass,
        measures(&doc, &OcctGeometryKernel::new()).0.center_of_mass,
    ] {
        assert!((centre[0] - 0.01).abs() < 1e-7);
        assert!((centre[1] - 0.02).abs() < 1e-7);
        assert!((centre[2] - 0.003).abs() < 1e-7);
    }
    for kind in ["radius", "diameter"] {
        let before = doc.clone();
        let p = patch(
            json!([{"type":"add_sketch_constraint","sketch_id":"sketch:ellipse","constraint":{"type":kind,"id":"con:bad_radius","target":"ent:ellipse","expr":"20 mm"}}]),
        );
        assert!(apply_patch_with_history(
            &mut doc,
            &p,
            &mut DocumentHistory::default(),
            "Bad radius"
        )
        .is_err());
        assert_eq!(doc, before);
    }
}

#[test]
fn ellipse_kernel_limits_and_rigid_matrix_placement_are_honest() {
    fn check<K: GeometryKernel>(kernel: &K, mock: bool) {
        let doc = ellipse(0.03, 0.02, 0., "XY");
        let mut model = doc.clone().into_part_model();
        model
            .regenerate(
                kernel,
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                None,
            )
            .expect("regen");
        let sketch = model.sketches.get("sketch:ellipse").expect("sketch");
        let neutral = opencad_feature::profile_to_solved(sketch, "sketch:ellipse/profile:outer")
            .expect("profile");
        assert!(neutral.ellipse.is_some());
        assert!(neutral.points.is_empty());
        assert!(kernel.loft(&[neutral.clone(), neutral.clone()]).is_err());
        assert!(kernel.sweep(&neutral, &neutral).is_err());
        let input = opencad_geometry::RevolveInput {
            sketch: neutral,
            profile_plane: opencad_geometry::ProfilePlane::Xy,
            axis_origin_m: [0.; 3],
            axis_direction_m: [0., 1., 0.],
            angle_rad: std::f64::consts::PI,
            operation: opencad_geometry::RevolveOperation::NewBody,
            target: None,
        };
        assert!(kernel.revolve(&input).is_err());
        let body = model.active_body().expect("body").clone();
        let mesh = kernel.tessellate(&body, &opencad_geometry::TessellationSettings::default());
        if mock {
            assert!(mesh.is_err());
            assert!(kernel
                .boolean(
                    body.clone(),
                    body.clone(),
                    opencad_geometry::BooleanOp::Union
                )
                .is_err());
        } else {
            assert!(mesh.expect("native mesh").triangle_count() > 0);
        }
        let placed = kernel
            .transform_body(
                body,
                opencad_geometry::RigidTransform {
                    translation_m: [0.1, 0.2, 0.3],
                    rotation: [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
                },
            )
            .expect("rigid transform");
        let mass = kernel.mass_properties(&placed, 1.).expect("mass");
        assert!((mass.volume_m3 - std::f64::consts::PI * 0.03 * 0.02 * 0.006).abs() < 1e-12);
        for (axis, expected) in [(0, 0.1), (1, 0.2), (2, 0.303)] {
            assert!((mass.center_of_mass[axis] - expected).abs() < 1e-9);
        }
        let bbox = kernel.bounding_box(&placed).expect("bounds");
        for (axis, size) in [(0, 0.04), (1, 0.06), (2, 0.006)] {
            assert!((bbox.max[axis] - bbox.min[axis] - size).abs() < 2.1e-7);
        }
    }
    check(&MockGeometryKernel::new(), true);
    check(&OcctGeometryKernel::new(), false);
}

#[test]
fn symmetric_ellipse_extrusion_is_centred_and_through_all_refuses() {
    let doc = ellipse(0.03, 0.02, 0., "XY");
    let mut model = doc.clone().into_part_model();
    let kernel = MockGeometryKernel::new();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            None,
        )
        .expect("regen");
    let source = model.sketches.get("sketch:ellipse").expect("source");
    let neutral = opencad_feature::profile_to_solved(source, "sketch:ellipse/profile:outer")
        .expect("profile");
    fn check<K: GeometryKernel>(kernel: &K, profile: &opencad_geometry::SolvedSketch) {
        let wire = kernel.make_wire_from_sketch(profile).expect("wire");
        let body = kernel
            .extrude(
                wire.clone(),
                opencad_geometry::ExtrudeExtent::Symmetric {
                    length: opencad_core::Length::from_meters(0.006),
                },
                opencad_geometry::ExtrudeOperation::NewBody,
                None,
                [0., 0., 1.],
            )
            .expect("symmetric");
        let mass = kernel.mass_properties(&body, 1.).expect("mass");
        assert!((mass.volume_m3 - std::f64::consts::PI * 0.03 * 0.02 * 0.012).abs() < 1e-12);
        assert!(mass.center_of_mass[2].abs() < 1e-12);
        let bbox = kernel.bounding_box(&body).expect("bounds");
        assert!((bbox.min[2] + 0.006).abs() < 1.1e-7);
        assert!((bbox.max[2] - 0.006).abs() < 1.1e-7);
        assert!(kernel
            .extrude(
                wire,
                opencad_geometry::ExtrudeExtent::ThroughAll,
                opencad_geometry::ExtrudeOperation::NewBody,
                None,
                [0., 0., 1.]
            )
            .is_err());
    }
    check(&MockGeometryKernel::new(), &neutral);
    check(&OcctGeometryKernel::new(), &neutral);
}

#[test]
fn native_ellipse_cut_and_join_use_existing_feature_boolean_paths() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (operation, a, b, expected) in [
        (
            "cut",
            0.02,
            0.01,
            2.88e-5 - std::f64::consts::PI * 0.02 * 0.01 * 0.006,
        ),
        ("join", 0.1, 0.08, std::f64::consts::PI * 0.1 * 0.08 * 0.006),
    ] {
        let mut doc = read_ocad(root.join("examples/construction_plate.ocad.d")).expect("plate");
        let p = patch(json!([
            {"type":"add_sketch","id":"sketch:operation_ellipse","name":"Ellipse operation","workplane":{"type":"global","plane":"XY"}},
            {"type":"add_sketch_entity","sketch_id":"sketch:operation_ellipse","entity":{"type":"point","id":"ent:operation_center","x":0.04,"y":0.03}},
            {"type":"add_sketch_entity","sketch_id":"sketch:operation_ellipse","entity":{"type":"ellipse","id":"ent:operation_ellipse","center":"ent:operation_center","major_radius":a,"minor_radius":b,"axis_angle":0.}},
            {"type":"add_feature","node":{"id":"feature:operation_sketch","name":"Ellipse operation sketch","definition":{"type":"sketch","sketch_id":"sketch:operation_ellipse"}},"position":opencad_ai::FeaturePosition::end()},
            {"type":"add_feature","node":{"id":"feature:operation_body","name":"Ellipse operation","definition":{"type":"extrude","sketch_feature":"feature:operation_sketch","profile_ref":"sketch:operation_ellipse/profile:outer","extent":{"type":"distance","length":{"value_si":0.006}},"operation":operation,"target_feature":"feature:extrude_base"}},"position":opencad_ai::FeaturePosition::end()}
        ]));
        apply_patch_with_history(
            &mut doc,
            &p,
            &mut DocumentHistory::default(),
            "Ellipse operation",
        )
        .expect("patch");
        let volume = measures(&doc, &OcctGeometryKernel::new()).0.volume_m3;
        assert!(
            (volume - expected).abs() < 1e-12,
            "{operation}: {volume} expected {expected}"
        );
        let mut model = doc.clone().into_part_model();
        assert!(model
            .regenerate(
                &MockGeometryKernel::new(),
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                None
            )
            .is_err());
    }
}

#[test]
fn warm_cache_tracks_resolved_ellipse_axes_and_angle_without_persisting_them() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = ellipse(0.03, 0.02, 0., "XY");
        let mut model = doc.clone().into_part_model();
        let registry = FeatureRegistry::with_defaults();
        let mut cache = opencad_feature::RegenerationCache::with_backend("ellipse-regression");
        model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("cold");
        let warm = model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("warm");
        assert_eq!(warm.cached_nodes.len(), 2);
        let edit = patch(
            json!([{"type":"set_parameter","id":"param:major_radius","expr":"40 mm"},{"type":"set_parameter","id":"param:minor_radius","expr":"10 mm"},{"type":"set_parameter","id":"param:axis_angle_rad","expr":"45 deg"}]),
        );
        apply_patch_with_history(
            &mut doc,
            &edit,
            &mut DocumentHistory::default(),
            "Cached axes edit",
        )
        .expect("edit");
        let changed = model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("changed");
        assert!(
            changed
                .regenerated
                .iter()
                .any(|id| id == "feature:ellipse_body"),
            "ellipse body was incorrectly reused: {changed:?}"
        );
        let body = model.active_body().expect("body");
        let mass = kernel.mass_properties(body, 1.).expect("mass");
        assert!((mass.volume_m3 - std::f64::consts::PI * 0.04 * 0.01 * 0.006).abs() < 1e-12);
        let bbox = kernel.bounding_box(body).expect("bounds");
        let span = 2.
            * (0.04_f64 * std::f64::consts::FRAC_1_SQRT_2)
                .hypot(0.01 * std::f64::consts::FRAC_1_SQRT_2);
        assert!((bbox.max[0] - bbox.min[0] - span).abs() < 2.1e-7);
        let unchanged = model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("unchanged");
        assert_eq!(unchanged.cached_nodes.len(), 2);
        let edit =
            patch(json!([{"type":"set_parameter","id":"param:axis_angle_rad","expr":"0 deg"}]));
        apply_patch_with_history(
            &mut doc,
            &edit,
            &mut DocumentHistory::default(),
            "Angle-only cached edit",
        )
        .expect("angle edit");
        let angle_only = model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("angle-only regen");
        assert!(angle_only
            .regenerated
            .iter()
            .any(|id| id == "feature:ellipse_body"));
        let body = model.active_body().expect("angle body");
        let bbox = kernel.bounding_box(body).expect("angle bounds");
        assert!((bbox.max[0] - bbox.min[0] - 0.08).abs() < 2.1e-7);
        let previous = body.clone();
        let mut bad_params = doc.parameters.clone();
        bad_params
            .set_expr("param:minor_radius", "100 mm")
            .expect("invalid dimensions");
        assert!(model
            .regenerate_with_cache(kernel, &registry, Some(&bad_params), None, &mut cache)
            .is_err());
        assert_eq!(model.active_body().expect("previous output"), &previous);
        let wire = serde_json::to_string(&doc).expect("source");
        assert!(!wire.contains("resolved_parameters"));
        assert!(wire.contains("major_radius"));
    }
    check(&MockGeometryKernel::new());
    check(&OcctGeometryKernel::new());
}
