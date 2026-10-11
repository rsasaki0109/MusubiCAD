//! Construction datum example, structural authoring and file migration evidence.
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
        DocumentId::new("doc:datum_plate").expect("id"),
        "Datum Plate",
    ));
    let patch: DesignPatch = serde_json::from_str(
        &std::fs::read_to_string(root().join("examples/agent/create_datum_plate_patch.json"))
            .expect("patch"),
    )
    .expect("json");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    doc
}
#[test]
#[ignore = "refresh committed datum example via structural DesignPatch"]
fn write_datum_example() {
    write_expanded_dir(root().join("examples/datum_plate.ocad.d"), &authored()).expect("write");
}
#[test]
fn datum_example_round_trips_and_rebuilds_through_patch() {
    let expected = authored();
    let actual = read_expanded_dir(root().join("examples/datum_plate.ocad.d")).expect("read");
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
fn removing_a_consumed_datum_is_atomic() {
    let mut doc = authored();
    let before = doc.clone();
    let patch = DesignPatch::new(vec![opencad_ai::PatchOperation::RemoveFeature {
        id: "feature:datum".into(),
    }]);
    assert!(apply_patch_to_document(&mut doc, &patch).is_err());
    assert_eq!(doc, before);
}
