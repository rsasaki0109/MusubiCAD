//! Offset wire admission in both kernels, exact source/patch round trips.
use opencad_feature::FeatureRegistry;
use opencad_file::{document_design_state, read_ocad};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use std::path::PathBuf;
fn example() -> opencad_file::OcadDocument {
    read_ocad(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/offset_plate.ocad.d"))
        .expect("example")
}
fn check_profile<K: GeometryKernel>(kernel: &K) -> opencad_geometry::KernelBody {
    let doc = example();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            None,
        )
        .expect("regenerate");
    let sketch = model.sketches.get("sketch:offset").expect("offset");
    let profile =
        opencad_feature::profile_to_solved(sketch, "sketch:offset/profile:outer").expect("profile");
    assert_eq!(profile.points.len(), 4);
    for (axis, size) in [(0, 0.086), (1, 0.066)] {
        let min = profile
            .points
            .iter()
            .map(|p| p[axis])
            .fold(f64::INFINITY, f64::min);
        let max = profile
            .points
            .iter()
            .map(|p| p[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max - min - size).abs() < 1e-12);
    }
    model.active_body().expect("body").clone()
}
#[test]
fn offset_profile_dimensions_execute_in_mock_and_occt() {
    let mock = MockGeometryKernel::new();
    let body = check_profile(&mock);
    // Existing Mock extrusion returns a synthetic cube (four points + 6 mm).
    // Physical dimensions above belong to its neutral input; this mass is not
    // represented as the real plate volume.
    assert!(
        (mock
            .mass_properties(&body, 1.)
            .expect("mock mass")
            .volume_m3
            - 1e-6)
            .abs()
            < 1e-12
    );
    let native = OcctGeometryKernel::new();
    let body = check_profile(&native);
    assert!(
        (native
            .mass_properties(&body, 1.)
            .expect("native mass")
            .volume_m3
            - 3.4056e-5)
            .abs()
            < 1e-12
    );
    let bbox = native.bounding_box(&body).expect("bounds");
    for (axis, expected) in [(0, 0.086), (1, 0.066), (2, 0.006)] {
        assert!((bbox.max[axis] - bbox.min[axis] - expected).abs() < 2.1e-7);
    }
}
#[test]
fn offset_authoring_patch_roundtrip_preserves_existing_wire_tags() {
    let doc = example();
    let patch = opencad_ai::authoring_patch(&document_design_state(&doc)).expect("authoring");
    let bytes = serde_json::to_vec(&patch).expect("wire");
    let restored: opencad_ai::DesignPatch = serde_json::from_slice(&bytes).expect("roundtrip");
    assert_eq!(patch, restored);
    for value in [
        1.5308084989341922e-18_f64,
        0.08300000000000002,
        1.2345678901234567,
    ] {
        let wire = serde_json::to_vec(&value).expect("number");
        let restored: f64 = serde_json::from_slice(&wire).expect("parse");
        assert_eq!(value.to_bits(), restored.to_bits());
    }
}

#[test]
fn coincident_source_points_do_not_repair_broken_endpoint_identity() {
    let mut state = document_design_state(&example());
    let source = state
        .sketches
        .iter_mut()
        .find(|s| s.id.as_str() == "sketch:base")
        .expect("source");
    let mut duplicate = source.find_entity("ent:c0").expect("point").clone();
    if let opencad_sketch::SketchEntity::Point(point) = &mut duplicate {
        point.base.id = opencad_core::EntityId::new("ent:duplicate").expect("id");
    }
    source.entities.push(duplicate);
    for entity in &mut source.entities {
        if let opencad_sketch::SketchEntity::Line(line) = entity {
            if line.base.id.as_str() == "ent:e3" {
                line.end = opencad_core::EntityId::new("ent:duplicate").expect("id");
            }
        }
    }
    let request: opencad_ai::OffsetRequest = serde_json::from_value(serde_json::json!({
        "source_sketch_id":"sketch:base","curve_ids":["ent:e0","ent:e1","ent:e2","ent:e3"],
        "distance_m":0.003,"association":"independent_copy","join":"mitre_tangent",
        "destination_sketch_id":"sketch:another_copy","name":"Another copy",
        "destination_curve_ids":["ent:new_e0","ent:new_e1","ent:new_e2","ent:new_e3"],
        "destination_point_ids":["ent:new_p0","ent:new_p1","ent:new_p2","ent:new_p3"]
    }))
    .expect("request");
    let before = state.clone();
    let error = opencad_ai::plan_offset(&state, &request).expect_err("must refuse");
    assert!(
        error.to_string().contains("share ordered endpoint IDs"),
        "{error}"
    );
    assert_eq!(state, before);
}
