//! Closed wire, migration, DesignPatch source, canonical persistence and history.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::{FeatureDefinition, FeatureRegistry};
use opencad_file::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use serde_json::json;
#[path = "../../feature/tests/support/faceted_fillet_fixture.rs"]
mod fixture;
#[path = "compat/pre_faceted_feature.rs"]
mod old;
fn document() -> OcadDocument {
    let (m, p, r) = fixture::fixture();
    let mut d = OcadDocument::from_part_model(
        DocumentMetadata::new(
            DocumentId::new("doc:faceted_fillet").unwrap(),
            "Numerical faceted circular fillet",
        ),
        &m,
    );
    d.parameters = p;
    d.semantic_refs = r;
    d
}
fn volume(d: &OcadDocument) -> f64 {
    let k = MockGeometryKernel::with_analytic_primitives();
    let mut m = d.clone().into_part_model();
    m.regenerate(
        &k,
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
fn closed_wire_genuine_previous_reader_and_no_inference_roundtrips() {
    let d = document();
    let dir = tempfile::tempdir().unwrap();
    let exp = dir.path().join("f.ocad.d");
    write_expanded_dir(&exp, &d).unwrap();
    assert_eq!(read_expanded_dir(&exp).unwrap(), d);
    let a = dir.path().join("a.ocad");
    let b = dir.path().join("b.ocad");
    write_ocad_zip(&a, &d).unwrap();
    write_ocad_zip(&b, &d).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert_eq!(read_ocad_zip(&a).unwrap(), d);
    assert_eq!(migrate::migrate_to_current(d.clone()).unwrap(), d);
    let legacy = read_ocad(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/variable_fillet_plate.ocad.d"),
    )
    .unwrap();
    assert_eq!(migrate::migrate_to_current(legacy.clone()).unwrap(), legacy);
    let def = serde_json::to_value(
        &d.feature_nodes
            .iter()
            .find(|n| n.id == "feature:fillet")
            .unwrap()
            .definition,
    )
    .unwrap();
    let old_error = serde_json::from_value::<old::PreviousFeatureDefinition>(def.clone())
        .err()
        .unwrap()
        .to_string();
    assert!(old_error.contains("unknown variant") && old_error.contains("faceted_circular_fillet"));
    for n in &legacy.feature_nodes {
        assert!(serde_json::from_value::<old::PreviousFeatureDefinition>(
            serde_json::to_value(&n.definition).unwrap()
        )
        .is_ok());
    }
    for field in [
        "representation",
        "verification",
        "quality",
        "minimum_radius",
        "maximum_radius",
        "edge_ref",
    ] {
        let mut bad = def.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<FeatureDefinition>(bad).is_err());
    }
    for (field, value) in [
        ("representation", json!("smooth")),
        ("verification", json!("none")),
        ("minimum_radius", json!({"value_si":0.1,"unit":"mm"})),
        ("unknown", json!(true)),
    ] {
        let mut bad = def.clone();
        bad[field] = value;
        assert!(
            serde_json::from_value::<FeatureDefinition>(bad).is_err(),
            "{field}"
        );
    }
    let mut empty = OcadDocument::new(d.metadata.clone());
    apply_patch_to_document(
        &mut empty,
        &authoring_patch(&document_design_state(&d)).unwrap(),
    )
    .unwrap();
    let mut fresh = d.clone();
    for s in &mut fresh.sketches {
        s.update_profiles().unwrap();
    }
    assert_eq!(document_design_state(&empty), document_design_state(&fresh));
}
#[test]
fn typed_radii_coordinated_stock_edits_and_failed_batches_preserve_history() {
    let mut d = document();
    let original = d.clone();
    let mut history = DocumentHistory::default();
    let before_volume = volume(&d);
    let mut def = serde_json::to_value(
        &d.feature_nodes
            .iter()
            .find(|n| n.id == "feature:fillet")
            .unwrap()
            .definition,
    )
    .unwrap();
    def["maximum_radius"]["value_si"] = json!(0.001);
    let edit = patch(
        json!([{"type":"replace_feature_definition","id":"feature:fillet","definition":def}]),
    );
    assert!(dry_run_patch_document(&d, &edit).validation.is_ok());
    apply_patch_with_history(&mut d, &edit, &mut history, "Reduce maximum radius").unwrap();
    assert!(volume(&d) > before_volume);
    let changed = d.clone();
    history.undo(&mut d).unwrap();
    assert_eq!(d, original);
    history.redo(&mut d).unwrap();
    assert_eq!(d, changed);
    let before = d.clone();
    let old_history = history.clone();
    let bad = patch(
        json!([{"type":"replace_feature_definition","id":"feature:fillet","definition":def},{"type":"set_feature_ref","feature_id":"feature:fillet","field":"edge_ref","ref_id":"ref:edge:missing"}]),
    );
    assert!(
        apply_patch_with_history(&mut d, &bad, &mut history, "Refuse after earlier edit").is_err()
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
    stock["extent"]["length"]["value_si"] = json!(0.025);
    def["maximum_radius"]["value_si"] = json!(0.0015);
    let coordinated = patch(
        json!([{"type":"replace_feature_definition","id":"feature:stock","definition":stock},{"type":"replace_feature_definition","id":"feature:fillet","definition":def}]),
    );
    apply_patch_with_history(
        &mut d,
        &coordinated,
        &mut history,
        "Explicit coordinated stock and radius edit",
    )
    .unwrap();
    assert!(volume(&d) > before_volume);
    history.undo(&mut d).unwrap();
    assert_eq!(d, before);
}
#[test]
#[ignore = "generate the canonical source example through DesignPatch"]
fn write_faceted_circular_fillet_example() {
    let source = document();
    let create = authoring_patch(&document_design_state(&source)).unwrap();
    let mut d = OcadDocument::new(source.metadata.clone());
    apply_patch_to_document(&mut d, &create).unwrap();
    assert!(volume(&d) > 0.);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    write_expanded_dir(root.join("examples/faceted_circular_fillet.ocad.d"), &d).unwrap();
    std::fs::write(
        root.join("examples/agent/create_faceted_circular_fillet_patch.json"),
        serde_json::to_string_pretty(&create).unwrap() + "\n",
    )
    .unwrap();
}
