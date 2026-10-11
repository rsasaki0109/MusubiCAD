//! MCAD-P10-003: additive wire migration, canonical persistence and patch parity.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_drawing::LinearMeasurement;
use opencad_file::{
    apply_patch_to_document, document_design_state, read_ocad, write_ocad, OcadDocument,
};
use std::path::PathBuf;

fn legacy() -> OcadDocument {
    read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bracket_front_view.ocad.d"),
    )
    .expect("legacy drawing")
}

#[test]
fn legacy_points_migrate_without_inventing_a_semantic_binding() {
    let migrated = opencad_file::migrate::migrate_to_current(legacy()).expect("migrate");
    let measurement = &migrated.drawing.as_ref().expect("drawing").sheets[0].dimensions[0];
    assert!(matches!(
        measurement.measurement,
        LinearMeasurement::Points { .. }
    ));
    assert!((measurement.measured_length_m().expect("length") - 0.08).abs() < 1e-12);
    let dir = tempfile::tempdir().expect("dir");
    write_ocad(dir.path(), &migrated).expect("write");
    assert_eq!(
        read_ocad(dir.path()).expect("read").drawing,
        migrated.drawing
    );
}

#[test]
fn semantic_dimension_round_trips_and_rebuilds_through_design_patch() {
    let mut doc = legacy();
    let patch: DesignPatch = serde_json::from_str(include_str!(
        "../../../examples/agent/semantic_bracket_dimension_patch.json"
    ))
    .expect("patch");
    apply_patch_to_document(&mut doc, &patch).expect("apply");
    let migrated = opencad_file::migrate::migrate_to_current(doc).expect("migrate");
    let dir = tempfile::tempdir().expect("dir");
    write_ocad(dir.path(), &migrated).expect("write");
    let restored = read_ocad(dir.path()).expect("read");
    assert_eq!(restored.drawing, migrated.drawing);
    let json = std::fs::read_to_string(dir.path().join("graph/drawings.json")).expect("json");
    assert!(json.contains("edge_ref"));
    assert!(!json.contains("start_model_m") && !json.contains("end_model_m"));
    let authored = authoring_patch(&document_design_state(&restored)).expect("author");
    let mut rebuilt = OcadDocument::new(restored.metadata.clone());
    apply_patch_to_document(&mut rebuilt, &authored).expect("rebuild");
    assert_eq!(rebuilt.drawing, restored.drawing);
}
