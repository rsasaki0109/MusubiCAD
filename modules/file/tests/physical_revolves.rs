//! ADR-112 feature/patch integration; OCCT integration tests require native OCCT.
use opencad_ai::{DesignPatch, PatchOperation};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::{FeatureDefinition, FeatureRegistry};
use opencad_file::{
    apply_patch_to_document, apply_patch_with_history, document_design_state, read_ocad,
    write_ocad, DocumentHistory, OcadDocument,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use std::f64::consts::PI;
fn patch() -> DesignPatch {
    serde_json::from_str(include_str!(
        "../../../examples/agent/physical_rectangular_revolve_patch.json"
    ))
    .unwrap()
}
fn empty() -> OcadDocument {
    OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:physical_rectangular_revolve").unwrap(),
        "Physical rectangular revolve",
    ))
}
fn measure<K: GeometryKernel>(
    doc: &OcadDocument,
    k: &K,
    angle: f64,
    inner: f64,
    outer: f64,
    axis: f64,
) {
    let mut model = doc.clone().into_part_model();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = opencad_feature::RegenerationCache::with_backend("ADR-112-session");
    let cold = model
        .regenerate_with_cache(
            k,
            &registry,
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
            &mut cache,
        )
        .unwrap();
    assert_eq!(cold.regenerated.len(), 3);
    let saved = model.outputs.clone();
    let warm = model
        .regenerate_with_cache(
            k,
            &registry,
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
            &mut cache,
        )
        .unwrap();
    assert_eq!(warm.cached_nodes.len(), 3);
    assert_eq!(warm.trace.geometry_kernel_call_count, 0);
    assert_eq!(model.outputs, saved);
    let b = model.active_body().unwrap().clone();
    let m = k.mass_properties(&b, 1.).unwrap();
    let v = (outer * outer - inner * inner) * angle * 0.02 / 2.;
    assert!((m.volume_m3 - v).abs() < 1e-12);
    let d = 2. * (outer.powi(3) - inner.powi(3)) / (3. * (outer * outer - inner * inner) * angle);
    for (a, b) in
        m.center_of_mass
            .into_iter()
            .zip([axis + d * angle.sin(), 0.003, -d * (1. - angle.cos())])
    {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }
    let bounds = k.bounding_box(&b).unwrap();
    assert!((bounds.min[1] + 0.007).abs() < 3e-7 && (bounds.max[1] - 0.013).abs() < 3e-7);
    // A failed downstream evaluation must retain the previous output handles.
    let saved = model.outputs.clone();
    let mut bad = doc.parameters.clone();
    bad.set_expr("param:turn_angle", "0 deg").unwrap();
    assert!(model
        .regenerate_with_cache(
            k,
            &registry,
            Some(&bad),
            Some(&doc.semantic_refs),
            &mut cache
        )
        .is_err());
    assert_eq!(model.outputs, saved);
    assert!((k.mass_properties(&b, 1.).unwrap().volume_m3 - v).abs() < 1e-12);
}
fn physical<K: GeometryKernel>(k: &K) {
    let mut doc = empty();
    let initial = document_design_state(&doc);
    let mut history = DocumentHistory::default();
    apply_patch_with_history(&mut doc, &patch(), &mut history, "Create physical revolve").unwrap();
    let authored = doc.clone();
    measure(&doc, k, PI / 2., 0.01, 0.02, 0.005);
    assert_eq!(doc, authored);
    assert!(doc
        .feature_graph
        .dependency_edges()
        .iter()
        .any(|e| e.source == "feature:revolve_axis" && e.target == "feature:physical_revolve"));
    let created = document_design_state(&doc);
    history.undo(&mut doc).unwrap();
    assert_eq!(document_design_state(&doc), initial);
    history.redo(&mut doc).unwrap();
    assert_eq!(document_design_state(&doc), created);
    let edit = DesignPatch::new(vec![
        PatchOperation::SetParameter {
            id: "param:axis_u".into(),
            expr: "10 mm".into(),
        },
        PatchOperation::SetParameter {
            id: "param:turn_angle".into(),
            expr: "180 deg".into(),
        },
    ]);
    apply_patch_with_history(&mut doc, &edit, &mut history, "Move associated axis").unwrap();
    measure(&doc, k, PI, 0.005, 0.015, 0.01);
    history.undo(&mut doc).unwrap();
    assert_eq!(document_design_state(&doc), created);
    let mut wrong = doc.parameters.clone();
    wrong.set_expr("param:turn_angle", "90 mm").unwrap();
    assert!(doc
        .clone()
        .into_part_model()
        .regenerate(k, &FeatureRegistry::with_defaults(), Some(&wrong), None)
        .is_err());
    let FeatureDefinition::Revolve(def) = &doc
        .feature_nodes
        .iter()
        .find(|n| n.id == "feature:physical_revolve")
        .unwrap()
        .definition
    else {
        panic!("revolve")
    };
    assert_eq!(def.angle_expr.as_deref(), Some("turn_angle"));
    assert_eq!(def.axis_feature.as_deref(), Some("feature:revolve_axis"));
    assert_eq!(doc, authored);
}
#[test]
fn physical_mock_feature_patch_axis_expressions_and_failed_regeneration_preserve_source() {
    physical(&MockGeometryKernel::with_analytic_primitives());
}
#[test]
fn native_feature_patch_axis_expressions_and_failed_regeneration_preserve_source() {
    physical(&OcctGeometryKernel::new());
}
#[test]
fn portable_revolve_expanded_packed_and_canonical_history_round_trip() {
    let mut doc = empty();
    apply_patch_to_document(&mut doc, &patch()).unwrap();
    let expected = document_design_state(&doc);
    let dir = tempfile::tempdir().unwrap();
    for name in ["part.ocad.d", "part.ocad"] {
        let path = dir.path().join(name);
        write_ocad(&path, &doc).unwrap();
        let loaded = read_ocad(&path).unwrap();
        assert_eq!(document_design_state(&loaded), expected);
        measure(
            &loaded,
            &MockGeometryKernel::with_analytic_primitives(),
            PI / 2.,
            0.01,
            0.02,
            0.005,
        );
        measure(
            &loaded,
            &OcctGeometryKernel::new(),
            PI / 2.,
            0.01,
            0.02,
            0.005,
        );
    }
}
