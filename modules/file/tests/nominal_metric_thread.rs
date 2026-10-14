//! ADR-069 persisted source, migration, structural patches and canonical history.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::FeatureRegistry;
use opencad_file::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use serde_json::json;
use std::path::PathBuf;
#[path = "../../feature/tests/support/nominal_thread_fixture.rs"]
mod fixture;
fn document() -> OcadDocument {
    let (m, p, r) = fixture::fixture();
    let mut d = OcadDocument::from_part_model(
        DocumentMetadata::new(
            DocumentId::new("doc:nominal_thread").unwrap(),
            "Nominal M6x1 basic flat-root thread",
        ),
        &m,
    );
    d.parameters = p;
    d.semantic_refs = r;
    d
}
fn volume<K: GeometryKernel>(d: &OcadDocument, k: &K) -> f64 {
    let mut m = d.clone().into_part_model();
    m.regenerate(
        k,
        &FeatureRegistry::with_defaults(),
        Some(&d.parameters),
        Some(&d.semantic_refs),
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
fn packed_expanded_authoring_and_no_inference_migration() {
    let d = document();
    let dir = tempfile::tempdir().unwrap();
    let expanded = dir.path().join("thread.ocad.d");
    write_expanded_dir(&expanded, &d).unwrap();
    assert_eq!(read_expanded_dir(&expanded).unwrap(), d);
    let a = dir.path().join("a.ocad");
    let b = dir.path().join("b.ocad");
    write_ocad_zip(&a, &d).unwrap();
    write_ocad_zip(&b, &d).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert_eq!(read_ocad_zip(&a).unwrap(), d);
    assert_eq!(migrate::migrate_to_current(d.clone()).unwrap(), d);
    let mut empty = OcadDocument::new(d.metadata.clone());
    apply_patch_to_document(
        &mut empty,
        &authoring_patch(&document_design_state(&d)).unwrap(),
    )
    .unwrap();
    // Profiles are disposable derived data: compare against fresh source rediscovery.
    let mut rediscovered = d.clone();
    for sketch in &mut rediscovered.sketches {
        sketch.update_profiles().unwrap();
    }
    assert_eq!(
        document_design_state(&empty),
        document_design_state(&rediscovered)
    );
    let legacy = read_ocad(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/cylindrical_draft.ocad.d"),
    )
    .unwrap();
    assert_eq!(migrate::migrate_to_current(legacy.clone()).unwrap(), legacy);
}
#[test]
fn typed_hand_turn_ref_edits_and_failed_batch_preserve_history() {
    let mut d = document();
    let original = d.clone();
    let mut history = DocumentHistory::default();
    let k = MockGeometryKernel::with_analytic_primitives();
    let v = volume(&d, &k);
    let mut def = serde_json::to_value(
        &d.feature_nodes
            .iter()
            .find(|n| n.id == "feature:thread")
            .unwrap()
            .definition,
    )
    .unwrap();
    def["handedness"] = "left".into();
    let edit = patch(
        json!([{"type":"replace_feature_definition","id":"feature:thread","definition":def}]),
    );
    assert!(dry_run_patch_document(&d, &edit).validation.is_ok());
    apply_patch_with_history(&mut d, &edit, &mut history, "Left nominal thread").unwrap();
    let left = d.clone();
    assert!((volume(&d, &k) - v).abs() < 1e-15);
    assert!((volume(&d, &opencad_kernel_occt::OcctGeometryKernel::new()) - v).abs() < v * 2e-5);
    history.undo(&mut d).unwrap();
    assert_eq!(d, original);
    history.redo(&mut d).unwrap();
    assert_eq!(d, left);
    let before = d.clone();
    let old_history = history.clone();
    let mut bad_def = def.clone();
    bad_def["handedness"] = "right".into();
    let bad = patch(
        json!([{"type":"replace_feature_definition","id":"feature:thread","definition":bad_def},{"type":"set_feature_ref","feature_id":"feature:thread","field":"face_ref","ref_id":"ref:face:missing"}]),
    );
    assert!(
        apply_patch_with_history(&mut d, &bad, &mut history, "Fail after successful edit").is_err()
    );
    assert_eq!(d, before);
    assert_eq!(history, old_history);
    let mut stock = serde_json::to_value(
        &d.feature_nodes
            .iter()
            .find(|n| n.id == "feature:stock")
            .unwrap()
            .definition,
    )
    .unwrap();
    stock["extent"]["length"]["value_si"] = 0.004.into();
    def["turns"] = 4.into();
    let coordinate = patch(
        json!([{"type":"replace_feature_definition","id":"feature:stock","definition":stock},{"type":"replace_feature_definition","id":"feature:thread","definition":def}]),
    );
    apply_patch_with_history(&mut d, &coordinate, &mut history, "Four whole pitches").unwrap();
    assert!((volume(&d, &k) - v / 2.).abs() < 1e-15);
    history.undo(&mut d).unwrap();
    assert_eq!(d, before);
    let mut circle = serde_json::to_value(&d.sketches[0].entities[1]).unwrap();
    circle["radius"] = 0.004.into();
    stock["extent"]["length"]["value_si"] = 0.005.into();
    def["size"] = "m8".into();
    let size_edit = patch(
        json!([{"type":"replace_sketch_entity","sketch_id":"sketch:stock","entity":circle},{"type":"replace_feature_definition","id":"feature:stock","definition":stock},{"type":"replace_feature_definition","id":"feature:thread","definition":def}]),
    );
    apply_patch_with_history(&mut d, &size_edit, &mut history, "M8 four whole pitches").unwrap();
    let native = volume(&d, &opencad_kernel_occt::OcctGeometryKernel::new());
    let analytic = volume(&d, &k);
    assert!((native - analytic).abs() < analytic * 2e-5);
    history.undo(&mut d).unwrap();
    assert_eq!(d, before);
    let unsupported = patch(
        json!([{"type":"set_feature_expr","feature_id":"feature:thread","field":"length_expr","expr":"4 mm"}]),
    );
    assert!(dry_run_patch_document(&d, &unsupported)
        .validation
        .has_errors());
}
#[test]
#[ignore = "generate the canonical source example through structural DesignPatch"]
fn write_nominal_thread_example() {
    let source = document();
    let create = authoring_patch(&document_design_state(&source)).unwrap();
    let mut d = OcadDocument::new(source.metadata.clone());
    apply_patch_to_document(&mut d, &create).unwrap();
    let a = volume(&d, &MockGeometryKernel::with_analytic_primitives());
    let b = volume(&d, &opencad_kernel_occt::OcctGeometryKernel::new());
    assert!((a - b).abs() < a * 2e-5);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    write_expanded_dir(root.join("examples/nominal_metric_thread.ocad.d"), &d).unwrap();
    std::fs::write(
        root.join("examples/agent/create_nominal_metric_thread_patch.json"),
        serde_json::to_string_pretty(&create).unwrap() + "\n",
    )
    .unwrap();
}
