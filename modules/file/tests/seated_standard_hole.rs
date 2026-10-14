//! ADR-063 canonical source, migration, structural patches and history.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::FeatureRegistry;
use opencad_file::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::json;
use std::path::PathBuf;
#[path = "../../feature/tests/support/seated_fixture.rs"]
mod fixture;
fn document() -> OcadDocument {
    let (model, params, refs) = fixture::fixture();
    let mut doc = OcadDocument::from_part_model(
        DocumentMetadata::new(
            DocumentId::new("doc:counterbored_plate").unwrap(),
            "M6 counterbored plate",
        ),
        &model,
    );
    doc.parameters = params;
    doc.semantic_refs = refs;
    doc
}
fn volume<K: GeometryKernel>(doc: &OcadDocument, k: &K) -> f64 {
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            k,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
        )
        .unwrap();
    k.mass_properties(model.active_body().unwrap(), 1.)
        .unwrap()
        .volume_m3
}
fn patch(value: serde_json::Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":value})).unwrap()
}
#[test]
fn expanded_packed_migration_and_patch_authored_sources_round_trip() {
    let doc = document();
    let dir = tempfile::tempdir().unwrap();
    write_expanded_dir(dir.path().join("plate.ocad.d"), &doc).unwrap();
    assert_eq!(
        read_expanded_dir(dir.path().join("plate.ocad.d")).unwrap(),
        doc
    );
    let packed = dir.path().join("plate.ocad");
    write_ocad_zip(&packed, &doc).unwrap();
    assert_eq!(read_ocad_zip(&packed).unwrap(), doc);
    assert_eq!(migrate::migrate_to_current(doc.clone()).unwrap(), doc);
    let mut empty = OcadDocument::new(doc.metadata.clone());
    apply_patch_to_document(
        &mut empty,
        &authoring_patch(&document_design_state(&doc)).unwrap(),
    )
    .unwrap();
    assert_eq!(empty.feature_nodes, doc.feature_nodes);
    assert_eq!(empty.semantic_refs, doc.semantic_refs);
    assert_eq!(empty.feature_graph, doc.feature_graph);
    for (a, b) in empty.sketches.iter().zip(&doc.sketches) {
        assert_eq!(a.entities, b.entities);
        assert_eq!(a.constraints, b.constraints);
        assert_eq!(a.workplane, b.workplane);
    }
    assert_eq!(
        serde_json::to_value(&empty.parameters).unwrap(),
        serde_json::to_value(&doc.parameters).unwrap()
    );
    let old = read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/standard_hole_plate.ocad.d"),
    )
    .unwrap();
    assert_eq!(old, migrate::migrate_to_current(old.clone()).unwrap());
}
#[test]
fn parameter_expressions_refusal_and_canonical_undo_redo_preserve_source() {
    let mut doc = document();
    let original = doc.clone();
    let mut history = DocumentHistory::default();
    let k = MockGeometryKernel::with_analytic_primitives();
    let before = volume(&doc, &k);
    let change = patch(
        json!([{"type":"set_feature_expr","feature_id":"feature:seat","field":"seat_depth","expr":"6 mm"}]),
    );
    assert!(dry_run_patch_document(&doc, &change).validation.is_ok());
    let mut candidate = doc.clone();
    apply_patch_to_document(&mut candidate, &change).unwrap();
    let after = volume(&candidate, &k);
    assert!(after < before);
    apply_patch_with_history(&mut doc, &change, &mut history, "Seat depth").unwrap();
    let edited = doc.clone();
    history.undo(&mut doc).unwrap();
    assert_eq!(doc, original);
    assert!((volume(&doc, &k) - before).abs() < 1e-12);
    history.redo(&mut doc).unwrap();
    assert_eq!(doc, edited);
    assert!((volume(&doc, &k) - after).abs() < 1e-12);
    let source = doc.clone();
    let old_history = history.clone();
    let bad = patch(
        json!([{"type":"set_feature_expr","feature_id":"feature:seat","field":"seat_depth","expr":"12 mm"}]),
    );
    assert!(apply_patch_with_history(&mut doc, &bad, &mut history, "Invalid depth").is_err());
    assert_eq!(doc, source);
    assert_eq!(history, old_history);
    let unit = patch(
        json!([{"type":"set_feature_expr","feature_id":"feature:seat","field":"seat_diameter","expr":"30 deg"}]),
    );
    assert!(dry_run_patch_document(&doc, &unit).validation.has_errors());
}
#[test]
#[ignore = "regenerate the canonical patch-authored counterbore example"]
fn write_counterbored_plate_example() {
    let source = document();
    let create = authoring_patch(&document_design_state(&source)).unwrap();
    let mut doc = OcadDocument::new(source.metadata.clone());
    apply_patch_to_document(&mut doc, &create).unwrap();
    let analytic = volume(&doc, &MockGeometryKernel::with_analytic_primitives());
    let native = volume(&doc, &OcctGeometryKernel::new());
    assert!((analytic - native).abs() < 1e-12);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    write_expanded_dir(root.join("examples/counterbored_plate.ocad.d"), &doc).unwrap();
    std::fs::write(
        root.join("examples/agent/create_counterbored_plate_patch.json"),
        serde_json::to_string_pretty(&create).unwrap() + "\n",
    )
    .unwrap();
}
