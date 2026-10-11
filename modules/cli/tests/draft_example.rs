//! Construction draft example, structural authoring and file migration evidence.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_file::{
    apply_patch_to_document, document_design_state, read_expanded_dir, write_expanded_dir,
    OcadDocument,
};
use std::path::PathBuf;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn authored() -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:draft_plate").expect("id"),
        "Draft Plate",
    ));
    let patch: DesignPatch = serde_json::from_str(
        &std::fs::read_to_string(root().join("examples/agent/create_draft_plate_patch.json"))
            .expect("patch"),
    )
    .expect("json");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    doc
}
#[test]
#[ignore = "refresh committed draft example via structural DesignPatch"]
fn write_draft_example() {
    write_expanded_dir(root().join("examples/draft_plate.ocad.d"), &authored()).expect("write");
}
#[test]
fn draft_example_round_trips_and_rebuilds_through_patch() {
    let expected = authored();
    let actual = read_expanded_dir(root().join("examples/draft_plate.ocad.d")).expect("read");
    assert_eq!(
        document_design_state(&actual),
        document_design_state(&expected)
    );
    let patch = authoring_patch(&document_design_state(&actual)).expect("authoring patch");
    let mut rebuilt = OcadDocument::new(actual.metadata.clone());
    apply_patch_to_document(&mut rebuilt, &patch).expect("rebuild");
    assert_eq!(
        document_design_state(&rebuilt),
        document_design_state(&actual)
    );
    let dir = tempfile::tempdir().expect("temp");
    write_expanded_dir(dir.path(), &rebuilt).expect("write");
    assert_eq!(read_expanded_dir(dir.path()).expect("round trip"), rebuilt);
}

#[test]
fn removing_draft_neutral_datum_is_atomic() {
    let mut doc = authored();
    let before = doc.clone();
    let patch = DesignPatch::new(vec![opencad_ai::PatchOperation::RemoveFeature {
        id: "feature:datum".into(),
    }]);
    assert!(apply_patch_to_document(&mut doc, &patch).is_err());
    assert_eq!(doc, before);
}

#[test]
fn draft_example_occt_volume_and_invalid_patch_are_atomic() {
    use opencad_feature::FeatureRegistry;
    use opencad_geometry::GeometryKernel;
    let mut doc = authored();
    let kernel = opencad_kernel_occt::OcctGeometryKernel::new();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
        )
        .expect("regen");
    let volume = kernel
        .mass_properties(model.active_body().expect("body"), 1.0)
        .expect("mass")
        .volume_m3;
    let expected = (800.0 - 80.0 * 5_f64.to_radians().tan()) * 1e-9;
    assert!((volume - expected).abs() < 1e-13);
    let bounds = kernel
        .bounding_box(model.active_body().expect("body"))
        .expect("bounds");
    assert!((bounds.min[2] - 0.005).abs() < 1e-6);
    assert!((bounds.max[2] - 0.009).abs() < 1e-6);
    let before = doc.clone();
    let patch: DesignPatch = serde_json::from_value(serde_json::json!({
        "operations": [{"type":"set_parameter", "id":"param:draft_angle", "expr":"90 deg"}]
    }))
    .expect("patch");
    assert!(apply_patch_to_document(&mut doc, &patch).is_err());
    assert_eq!(doc, before);
    let patch: DesignPatch = serde_json::from_value(serde_json::json!({
        "operations": [{"type":"set_parameter", "id":"param:draft_angle", "expr":"10 deg"}]
    }))
    .expect("patch");
    apply_patch_to_document(&mut doc, &patch).expect("edit");
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
        )
        .expect("edit regen");
    let volume = kernel
        .mass_properties(model.active_body().expect("body"), 1.0)
        .expect("mass")
        .volume_m3;
    assert!((volume - (800.0 - 80.0 * 10_f64.to_radians().tan()) * 1e-9).abs() < 1e-13);
}
