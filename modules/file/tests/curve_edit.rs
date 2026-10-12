//! Neutral profile/Mock evidence and old/new patch wire admission.
use opencad_ai::{plan_curve_edit, CurveEditRequest};
use opencad_feature::FeatureRegistry;
use opencad_file::{document_design_state, read_ocad};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn trimmed_guide_mock_regeneration_preserves_analytic_profile_dimensions() {
    let doc = read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/trim_extend_plate.ocad.d"),
    )
    .expect("example");
    let kernel = MockGeometryKernel::new();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            None,
        )
        .expect("mock");
    let sketch = model.sketches.get("sketch:base").expect("sketch");
    let profile =
        opencad_feature::profile_to_solved(sketch, "sketch:base/profile:outer").expect("profile");
    assert_eq!(profile.points.len(), 4);
    let max_x = profile
        .points
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = profile
        .points
        .iter()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((max_x - 0.08).abs() < 1e-12);
    assert!((max_y - 0.06).abs() < 1e-12);
    assert!(
        (kernel
            .mass_properties(model.active_body().expect("body"), 1.)
            .expect("mock mass")
            .volume_m3
            - 1e-6)
            .abs()
            < 1e-12
    );
}

#[test]
fn curve_edit_requires_explicit_mode_and_preserves_original_source() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let doc = read_ocad(root.join("examples/trim_extend_plate.ocad.d")).expect("example");
    let state = document_design_state(&doc);
    let before = state.clone();
    let request:CurveEditRequest=serde_json::from_value(json!({"sketch_id":"sketch:base","target_id":"ent:guide_line","cutter_id":"ent:e1","mode":"extend","end":"end","new_point_id":"ent:extended_end"})).expect("request");
    let plan = plan_curve_edit(&state, &request).expect("plan");
    let wire = serde_json::to_value(&plan.patch).expect("wire");
    let restored: opencad_ai::DesignPatch = serde_json::from_value(wire).expect("patch roundtrip");
    assert_eq!(restored, plan.patch);
    assert_eq!(state, before);
    assert!(serde_json::from_value::<CurveEditRequest>(json!({"sketch_id":"sketch:base","target_id":"ent:guide_line","cutter_id":"ent:e1","end":"end","new_point_id":"ent:new"})).is_err());
}
