//! Rib example, structural authoring and file migration evidence.

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
        DocumentId::new("doc:rib_plate").expect("id"),
        "Rib Plate",
    ));
    let patch: DesignPatch = serde_json::from_str(
        &std::fs::read_to_string(root().join("examples/agent/create_rib_plate_patch.json"))
            .expect("patch"),
    )
    .expect("json");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    doc
}

#[test]
#[ignore = "refresh committed rib example via structural DesignPatch"]
fn write_rib_example() {
    write_expanded_dir(root().join("examples/rib_plate.ocad.d"), &authored()).expect("write");
}

#[test]
fn rib_example_round_trips_and_rebuilds_through_patch() {
    let expected = authored();
    let actual = read_expanded_dir(root().join("examples/rib_plate.ocad.d")).expect("read");
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
fn rib_example_occt_volume_and_invalid_patch_are_atomic() {
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
    let expected = (80.0 * 60.0 * 4.0 + 40.0 * 4.0 * 12.0) * 1e-9;
    assert!((volume - expected).abs() < 1e-13);

    let before = doc.clone();
    let patch: DesignPatch = serde_json::from_value(serde_json::json!({
        "operations": [{"type":"set_parameter", "id":"param:rib_thickness", "expr":"0 mm"}]
    }))
    .expect("patch");
    assert!(apply_patch_to_document(&mut doc, &patch).is_err());
    assert_eq!(doc, before);

    let patch: DesignPatch = serde_json::from_value(serde_json::json!({
        "operations": [{"type":"set_parameter", "id":"param:rib_height", "expr":"20 mm"}]
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
    let expected = (80.0 * 60.0 * 4.0 + 40.0 * 4.0 * 20.0) * 1e-9;
    assert!((volume - expected).abs() < 1e-13);
}
