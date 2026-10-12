//! ADR-057 admission: authored control IDs and independent analytic solid evidence.
use opencad_ai::DesignPatch;
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_with_history, DocumentHistory, OcadDocument};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::{json, Value};

fn patch(operations: Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":operations})).unwrap()
}
fn document(plane: &str) -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:cubic_plate").unwrap(),
        "Closed cubic plate",
    ));
    let make = patch(json!([
        {"type":"add_parameter","id":"param:a","name":"a","expr":"40 mm"},
        {"type":"add_parameter","id":"param:b","name":"b","expr":"40 mm"},
        {"type":"add_sketch","id":"sketch:cubic","name":"Closed cubic","workplane":{"type":"global","plane":plane}},
        {"type":"add_sketch_entity","sketch_id":"sketch:cubic","entity":{"type":"point","id":"ent:p0","x":0.,"y":0.}},
        {"type":"add_sketch_entity","sketch_id":"sketch:cubic","entity":{"type":"point","id":"ent:p1","x":"a","y":0.}},
        {"type":"add_sketch_entity","sketch_id":"sketch:cubic","entity":{"type":"point","id":"ent:p2","x":0.,"y":"b"}},
        {"type":"add_sketch_entity","sketch_id":"sketch:cubic","entity":{"type":"cubic_bezier","id":"ent:cubic","control_points":["ent:p0","ent:p1","ent:p2","ent:p0"]}},
        {"type":"add_feature","node":{"id":"feature:cubic_sketch","name":"Cubic sketch","definition":{"type":"sketch","sketch_id":"sketch:cubic"}},"position":opencad_ai::FeaturePosition::end()},
        {"type":"add_feature","node":{"id":"feature:cubic_body","name":"Cubic prism","definition":{"type":"extrude","sketch_feature":"feature:cubic_sketch","profile_ref":"profile:bezier:ent:cubic","extent":{"type":"distance","length":{"value_si":0.006}},"operation":"new_body"}},"position":opencad_ai::FeaturePosition::end()}
    ]));
    apply_patch_with_history(
        &mut doc,
        &make,
        &mut DocumentHistory::default(),
        "Create cubic",
    )
    .unwrap();
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
        .unwrap();
    let body = model.active_body().unwrap();
    (
        kernel.mass_properties(body, 1.).unwrap(),
        kernel.bounding_box(body).unwrap(),
    )
}
fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{actual} != {expected}"
    );
}

#[test]
fn both_kernels_cubic_prism_match_analytic_volume_bounds_and_centre() {
    for plane in ["XY", "XZ"] {
        let doc = document(plane);
        let (mock, mock_box) = measures(&doc, &MockGeometryKernel::new());
        let (native, native_box) = measures(&doc, &OcctGeometryKernel::new());
        close(mock.volume_m3, 3. * 0.04 * 0.04 * 0.006 / 20., 1e-15);
        close(native.volume_m3, mock.volume_m3, 1e-14);
        close(native.area_m2, mock.area_m2, 5e-9);
        for i in 0..3 {
            close(native.center_of_mass[i], mock.center_of_mass[i], 1e-9);
            close(native_box.min[i], mock_box.min[i], 2.1e-7);
            close(native_box.max[i], mock_box.max[i], 2.1e-7);
        }
        let curve_axes = if plane == "XY" { [0, 1] } else { [0, 2] };
        for i in curve_axes {
            close(mock_box.max[i], 4. * 0.04 / 9., 1e-15);
            close(mock.center_of_mass[i], 3. * 0.04 / 14., 1e-15);
        }
    }
}

#[test]
fn cubic_source_refuses_unsupported_weights_knots_and_open_endpoint_ids() {
    let source = json!({"type":"cubic_bezier","id":"ent:cubic","control_points":["ent:p0","ent:p1","ent:p2","ent:p0"]});
    assert!(serde_json::from_value::<opencad_sketch::SketchEntity>(source.clone()).is_ok());
    for (key, value) in [
        ("weights", json!([1, 2, 1, 1])),
        ("knots", json!([0, 0, 1])),
        ("degree", json!(2)),
        ("periodic", json!(true)),
    ] {
        let mut invalid = source.clone();
        invalid[key] = value;
        assert!(serde_json::from_value::<opencad_sketch::SketchEntity>(invalid).is_err());
    }
    let mut invalid = source;
    invalid["control_points"][3] = json!("ent:other");
    assert!(serde_json::from_value::<opencad_sketch::SketchEntity>(invalid).is_err());
}

#[test]
fn cubic_failed_edit_preserves_source_history_and_previous_outputs() {
    let mut doc = document("XY");
    let original = doc.clone();
    let mut history = DocumentHistory::default();
    for operations in [
        json!([{"type":"replace_sketch_entity","sketch_id":"sketch:cubic","entity":{"type":"point","id":"ent:p1","x":0.,"y":0.}}]),
        json!([{"type":"replace_sketch_entity","sketch_id":"sketch:cubic","entity":{"type":"point","id":"ent:p1","x":"30 deg","y":0.}}]),
        json!([{"type":"remove_sketch_entity","sketch_id":"sketch:cubic","entity_id":"ent:p2"}]),
        json!([{"type":"add_sketch_constraint","sketch_id":"sketch:cubic","constraint":{"type":"radius","id":"con:bad","target":"ent:cubic","expr":"20 mm"}}]),
        json!([{"type":"set_parameter","id":"param:b","expr":"-40 mm"}]),
    ] {
        let candidate = patch(operations);
        assert!(
            apply_patch_with_history(&mut doc, &candidate, &mut history, "Refuse invalid cubic")
                .is_err(),
            "accepted invalid patch: {candidate:?}"
        );
        assert_eq!(doc, original);
        assert_eq!(
            serde_json::to_value(&history).unwrap(),
            serde_json::to_value(DocumentHistory::default()).unwrap()
        );
    }
    let kernel = MockGeometryKernel::new();
    let mut model = doc.clone().into_part_model();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&doc.parameters), None)
        .unwrap();
    let old = model.active_body().unwrap().clone();
    let before = kernel.mass_properties(&old, 1.).unwrap();
    let bad = model.nodes.get_mut("feature:cubic_body").unwrap();
    if let opencad_feature::FeatureDefinition::Extrude(e) = &mut bad.definition {
        e.extent = opencad_geometry::ExtrudeExtent::ThroughAll;
    }
    assert!(model
        .regenerate(&kernel, &registry, Some(&doc.parameters), None)
        .is_err());
    assert_eq!(model.active_body().unwrap(), &old);
    close(
        kernel
            .mass_properties(model.active_body().unwrap(), 1.)
            .unwrap()
            .volume_m3,
        before.volume_m3,
        1e-18,
    );
}

#[test]
fn cubic_control_constraints_and_construction_preserve_profiles() {
    let mut doc = document("XY");
    let ids = doc.sketches[0]
        .entities
        .iter()
        .map(|e| e.id().clone())
        .collect::<Vec<_>>();
    let changes = patch(json!([
        {"type":"add_sketch_constraint","sketch_id":"sketch:cubic","constraint":{"type":"distance","id":"con:control_length","target":{"a":"ent:p0","b":"ent:p1"},"expr":"50 mm"}},
        {"type":"add_sketch_entity","sketch_id":"sketch:cubic","entity":{"type":"cubic_bezier","id":"ent:helper","construction":true,"control_points":["ent:p0","ent:p1","ent:p2","ent:p0"]}}
    ]));
    apply_patch_with_history(
        &mut doc,
        &changes,
        &mut DocumentHistory::default(),
        "Constrain control point",
    )
    .unwrap();
    assert_eq!(doc.sketches[0].profiles.len(), 1);
    assert_eq!(
        doc.sketches[0].profiles[0].entity_ids[0].as_str(),
        "ent:cubic"
    );
    for id in ids {
        assert!(doc.sketches[0].find_entity(id.as_str()).is_some());
    }
    let expected = 3. * 0.05 * 0.04 * 0.006 / 20.;
    close(
        measures(&doc, &MockGeometryKernel::new()).0.volume_m3,
        expected,
        1e-14,
    );
    close(
        measures(&doc, &OcctGeometryKernel::new()).0.volume_m3,
        expected,
        1e-14,
    );
    let before = doc.clone();
    let hide = patch(
        json!([{"type":"set_sketch_construction","sketch_id":"sketch:cubic","entity_id":"ent:cubic","construction":true}]),
    );
    assert!(apply_patch_with_history(
        &mut doc,
        &hide,
        &mut DocumentHistory::default(),
        "Hide consumed curve"
    )
    .is_err());
    assert_eq!(doc, before);
    // Existing exact curves sharing a cubic control must consume its solved
    // coordinates even though its authored parameter expression is retained.
    let reuse = patch(json!([
        {"type":"add_sketch_entity","sketch_id":"sketch:cubic","entity":{"type":"ellipse","id":"ent:shared_ellipse","center":"ent:p1","major_radius":0.01,"minor_radius":0.005,"axis_angle":0.}},
        {"type":"add_feature","node":{"id":"feature:shared_ellipse","name":"Shared control centre","definition":{"type":"extrude","sketch_feature":"feature:cubic_sketch","profile_ref":"profile:ellipse:ent:shared_ellipse","extent":{"type":"distance","length":{"value_si":0.006}},"operation":"new_body"}},"position":opencad_ai::FeaturePosition::end()}
    ]));
    apply_patch_with_history(
        &mut doc,
        &reuse,
        &mut DocumentHistory::default(),
        "Reuse solved control",
    )
    .unwrap();
    for mass in [
        measures(&doc, &MockGeometryKernel::new()).0,
        measures(&doc, &OcctGeometryKernel::new()).0,
    ] {
        close(mass.center_of_mass[0], 0.05, 1e-9);
        close(
            mass.volume_m3,
            std::f64::consts::PI * 0.01 * 0.005 * 0.006,
            1e-14,
        );
    }
}

#[test]
fn cubic_source_control_edit_invalidates_warm_cache_and_keeps_expressions() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = document("XY");
        let source = doc.sketches[0].entities.clone();
        let mut cache = opencad_feature::RegenerationCache::new();
        let registry = FeatureRegistry::with_defaults();
        let mut model = doc.clone().into_part_model();
        model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .unwrap();
        let before = model.active_body().unwrap().clone();
        apply_patch_with_history(
            &mut doc,
            &patch(json!([{"type":"set_parameter","id":"param:a","expr":"60 mm"}])),
            &mut DocumentHistory::default(),
            "Edit control parameter",
        )
        .unwrap();
        assert_eq!(doc.sketches[0].entities, source);
        model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .unwrap();
        let after = model.active_body().unwrap().clone();
        assert_ne!(before, after);
        close(
            kernel.mass_properties(&after, 1.).unwrap().volume_m3,
            3. * 0.06 * 0.04 * 0.006 / 20.,
            1e-14,
        );
        model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .unwrap();
        assert_eq!(model.active_body().unwrap(), &after);
        let source_wire =
            serde_json::to_value(model.sketches.get("sketch:cubic").unwrap()).unwrap();
        let mut bad_parameters = doc.parameters.clone();
        bad_parameters.set_expr("param:b", "-40 mm").unwrap();
        assert!(model
            .regenerate_with_cache(kernel, &registry, Some(&bad_parameters), None, &mut cache)
            .is_err());
        assert_eq!(model.active_body().unwrap(), &after);
        assert_eq!(
            serde_json::to_value(model.sketches.get("sketch:cubic").unwrap()).unwrap(),
            source_wire
        );
        model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .unwrap();
        assert_eq!(model.active_body().unwrap(), &after);
        assert_eq!(doc.sketches[0].entities, source);
    }
    check(&MockGeometryKernel::new());
    check(&OcctGeometryKernel::new());
}

#[test]
fn cubic_old_new_expanded_packed_patch_history_roundtrip() {
    let mut doc = document("XY");
    let original = doc.clone();
    let mut history = DocumentHistory::default();
    let edit = patch(json!([{"type":"set_parameter","id":"param:b","expr":"30 mm"}]));
    apply_patch_with_history(&mut doc, &edit, &mut history, "Edit cubic").unwrap();
    let after = doc.clone();
    let mut history: DocumentHistory =
        serde_json::from_slice(&serde_json::to_vec(&history).unwrap()).unwrap();
    history.undo(&mut doc).unwrap();
    assert_eq!(doc, original);
    history.redo(&mut doc).unwrap();
    assert_eq!(doc, after);
    let temp = tempfile::tempdir().unwrap();
    let expanded = temp.path().join("cubic.ocad.d");
    let packed = temp.path().join("cubic.ocad");
    opencad_file::write_expanded_dir(&expanded, &doc).unwrap();
    opencad_file::write_ocad_zip(&packed, &doc).unwrap();
    doc.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    for mut restored in [
        opencad_file::read_ocad(&expanded).unwrap(),
        opencad_file::read_ocad_zip(&packed).unwrap(),
    ] {
        restored.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(restored, doc);
    }
    let source = opencad_ai::authoring_patch(&opencad_file::document_design_state(&doc)).unwrap();
    assert_eq!(
        source,
        serde_json::from_slice::<DesignPatch>(&serde_json::to_vec(&source).unwrap()).unwrap()
    );
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let legacy = opencad_file::read_ocad(root.join("examples/ellipse_plate.ocad.d")).unwrap();
    assert!(legacy
        .sketches
        .iter()
        .flat_map(|s| &s.entities)
        .all(|e| !matches!(e, opencad_sketch::SketchEntity::CubicBezier(_))));
}

#[test]
fn cubic_native_and_reference_metrics_survive_rigid_placement_and_refuse_other_contexts() {
    fn check<K: GeometryKernel>(kernel: &K, mock: bool) {
        let doc = document("XY");
        let mut model = doc.clone().into_part_model();
        model
            .regenerate(
                kernel,
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                None,
            )
            .unwrap();
        let sketch = model.sketches.get("sketch:cubic").unwrap();
        let profile =
            opencad_feature::profile_to_solved(sketch, "profile:bezier:ent:cubic").unwrap();
        assert!(profile.cubic_bezier.is_some());
        assert!(profile.points.is_empty());
        assert!(kernel.loft(&[profile.clone(), profile.clone()]).is_err());
        assert!(kernel.sweep(&profile, &profile).is_err());
        let wire = kernel.make_wire_from_sketch(&profile).unwrap();
        assert!(kernel
            .extrude(
                wire.clone(),
                opencad_geometry::ExtrudeExtent::ThroughAll,
                opencad_geometry::ExtrudeOperation::NewBody,
                None,
                [0., 0., 1.]
            )
            .is_err());
        assert!(kernel
            .extrude(
                wire.clone(),
                opencad_geometry::ExtrudeExtent::Distance {
                    length: opencad_core::Length::from_meters(0.006)
                },
                opencad_geometry::ExtrudeOperation::Cut,
                Some(model.active_body().unwrap().clone()),
                [0., 0., 1.]
            )
            .is_err());
        let centred = kernel
            .extrude(
                wire,
                opencad_geometry::ExtrudeExtent::Symmetric {
                    length: opencad_core::Length::from_meters(0.003),
                },
                opencad_geometry::ExtrudeOperation::NewBody,
                None,
                [0., 0., 1.],
            )
            .unwrap();
        close(
            kernel.mass_properties(&centred, 1.).unwrap().center_of_mass[2],
            0.,
            1e-10,
        );
        let body = model.active_body().unwrap().clone();
        let mesh = kernel.tessellate(&body, &opencad_geometry::TessellationSettings::default());
        for transform in [
            opencad_geometry::RigidTransform {
                rotation: [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                translation_m: [0.; 3],
            },
            opencad_geometry::RigidTransform {
                rotation: [[1., 1e-7, 0.], [0., 1., 0.], [0., 0., 1.]],
                translation_m: [0.; 3],
            },
            opencad_geometry::RigidTransform::from_translation([f64::NAN, 0., 0.]),
        ] {
            assert!(kernel.transform_body(body.clone(), transform).is_err());
        }
        assert!(kernel
            .translate_body(body.clone(), [f64::NAN, 0., 0.])
            .is_err());
        if mock {
            assert!(mesh.is_err());
            assert!(kernel.inertia_about_com(&body, 1.).is_err());
        } else {
            assert!(mesh.unwrap().triangle_count() > 0);
            assert!(kernel.discover_body_edges(&body).unwrap().is_empty());
        }
        let moved = kernel
            .transform_body(
                body,
                opencad_geometry::RigidTransform {
                    rotation: [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
                    translation_m: [0.1, 0.2, 0.3],
                },
            )
            .unwrap();
        let mass = kernel.mass_properties(&moved, 1.).unwrap();
        close(mass.volume_m3, 1.44e-6, 1e-14);
        close(mass.center_of_mass[0], 0.1 - 3. * 0.04 / 14., 1e-9);
        let bounds = kernel.bounding_box(&moved).unwrap();
        close(bounds.min[0], 0.1 - 4. * 0.04 / 9., 2.1e-7);
        close(bounds.max[1], 0.2 + 4. * 0.04 / 9., 2.1e-7);
    }
    check(&MockGeometryKernel::new(), true);
    check(&OcctGeometryKernel::new(), false);
}

#[test]
#[ignore = "generate verified closed cubic example"]
fn generate_cubic_example() {
    let doc = document("XY");
    measures(&doc, &MockGeometryKernel::new());
    measures(&doc, &OcctGeometryKernel::new());
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    opencad_file::write_expanded_dir(root.join("examples/cubic_plate.ocad.d"), &doc).unwrap();
    let source = opencad_ai::authoring_patch(&opencad_file::document_design_state(&doc)).unwrap();
    std::fs::write(
        root.join("examples/agent/create_cubic_plate_patch.json"),
        serde_json::to_string_pretty(&source).unwrap() + "\n",
    )
    .unwrap();
}
