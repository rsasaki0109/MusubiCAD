//! Closed tapered rib source, canonical history and authoring admission.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::FeatureRegistry;
use opencad_file::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::json;
use std::path::PathBuf;
#[path = "../../feature/tests/support/tapered_rib_fixture.rs"]
mod fixture;
fn document() -> OcadDocument {
    let (model, params, refs) = fixture::fixture();
    let mut doc = OcadDocument::from_part_model(
        DocumentMetadata::new(
            DocumentId::new("doc:tapered_rib_plate").unwrap(),
            "Square-ended tapered rib plate",
        ),
        &model,
    );
    doc.parameters = params;
    doc.semantic_refs = refs;
    doc
}
fn volume<K: GeometryKernel>(doc: &OcadDocument, k: &K) -> f64 {
    let mut m = doc.clone().into_part_model();
    m.regenerate(
        k,
        &FeatureRegistry::with_defaults(),
        Some(&doc.parameters),
        Some(&doc.semantic_refs),
    )
    .unwrap();
    k.mass_properties(m.active_body().unwrap(), 1.)
        .unwrap()
        .volume_m3
}
fn patch(ops: serde_json::Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":ops})).unwrap()
}
#[test]
fn expanded_packed_migration_and_authored_patches_preserve_tapered_source() {
    let doc = document();
    let dir = tempfile::tempdir().unwrap();
    let expanded = dir.path().join("part.ocad.d");
    write_expanded_dir(&expanded, &doc).unwrap();
    assert_eq!(read_expanded_dir(&expanded).unwrap(), doc);
    let packed = dir.path().join("part.ocad");
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
    assert_eq!(empty.feature_graph, doc.feature_graph);
    assert_eq!(empty.semantic_refs, doc.semantic_refs);
    assert_eq!(empty.parameters, doc.parameters);
    for (a, b) in empty.sketches.iter().zip(&doc.sketches) {
        assert_eq!(a.entities, b.entities);
        assert_eq!(a.constraints, b.constraints);
        assert_eq!(a.workplane, b.workplane);
    }
    let legacy = read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/rib_plate.ocad.d"),
    )
    .unwrap();
    assert_eq!(migrate::migrate_to_current(legacy.clone()).unwrap(), legacy);
}
#[test]
fn typed_height_edits_units_refs_and_history_are_protected() {
    let k = MockGeometryKernel::with_analytic_primitives();
    let mut doc = document();
    let baseline = doc.clone();
    let v0 = volume(&doc, &k);
    let mut history = DocumentHistory::default();
    let edit = patch(
        json!([{"type":"set_feature_expr","feature_id":"feature:rib","field":"rib_height","expr":"12 mm"}]),
    );
    assert!(dry_run_patch_document(&doc, &edit).validation.is_ok());
    apply_patch_with_history(&mut doc, &edit, &mut history, "Rib height").unwrap();
    let edited = doc.clone();
    let v1 = volume(&doc, &k);
    assert!((v1 - 13224e-9).abs() < 1e-12);
    assert!((v1 - v0).abs() > 1e-8);
    history.undo(&mut doc).unwrap();
    assert_eq!(doc, baseline);
    history.redo(&mut doc).unwrap();
    assert_eq!(doc, edited);
    for expr in ["0 mm", "100 mm", "1001 m", "5 deg", "missing_height"] {
        let bad = patch(
            json!([{"type":"set_feature_expr","feature_id":"feature:rib","field":"rib_height","expr":expr}]),
        );
        let before = doc.clone();
        let old = history.clone();
        assert!(
            apply_patch_with_history(&mut doc, &bad, &mut history, "Invalid rib height").is_err(),
            "{expr}"
        );
        assert_eq!(doc, before);
        assert_eq!(history, old);
    }
    let mut zero = serde_json::to_value(
        &doc.feature_nodes
            .iter()
            .find(|n| n.id == "feature:rib")
            .unwrap()
            .definition,
    )
    .unwrap();
    zero["height"]["value_si"] = 0_f64.into();
    let bad_fallback =
        patch(json!([{"type":"replace_feature_definition","id":"feature:rib","definition":zero}]));
    assert!(dry_run_patch_document(&doc, &bad_fallback)
        .validation
        .has_errors());
    let mut missing_line = serde_json::to_value(
        &doc.feature_nodes
            .iter()
            .find(|n| n.id == "feature:rib")
            .unwrap()
            .definition,
    )
    .unwrap();
    missing_line["centerline_ref"] = "ent:missing".into();
    let bad_line = patch(
        json!([{"type":"replace_feature_definition","id":"feature:rib","definition":missing_line}]),
    );
    assert!(dry_run_patch_document(&doc, &bad_line)
        .validation
        .has_errors());
    let wrong = patch(
        json!([{"type":"set_feature_expr","feature_id":"feature:extrude_base","field":"rib_height","expr":"5 deg"}]),
    );
    assert!(dry_run_patch_document(&doc, &wrong).validation.has_errors());
    let missing = patch(
        json!([{"type":"set_feature_ref","feature_id":"feature:rib","field":"face_ref","ref_id":"ref:face:missing"}]),
    );
    assert!(dry_run_patch_document(&doc, &missing)
        .validation
        .has_errors());
}
#[test]
#[ignore = "regenerate the canonical patch-authored tapered rib example"]
fn write_tapered_rib_example() {
    let source = document();
    let create = authoring_patch(&document_design_state(&source)).unwrap();
    let mut doc = OcadDocument::new(source.metadata.clone());
    apply_patch_to_document(&mut doc, &create).unwrap();
    let a = volume(&doc, &MockGeometryKernel::with_analytic_primitives());
    let b = volume(&doc, &OcctGeometryKernel::new());
    assert!((a - b).abs() < 1e-12);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    write_expanded_dir(root.join("examples/tapered_rib_plate.ocad.d"), &doc).unwrap();
    std::fs::write(
        root.join("examples/agent/create_tapered_rib_plate_patch.json"),
        serde_json::to_string_pretty(&create).unwrap() + "\n",
    )
    .unwrap();
}
