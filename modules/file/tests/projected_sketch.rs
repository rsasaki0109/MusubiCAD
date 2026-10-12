//! ADR-054: associated semantic projection, exact curves and cache/history.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::{FeatureRegistry, RegenerationCache};
use opencad_file::{
    apply_patch_with_history, document_design_state, read_ocad, read_ocad_zip, write_expanded_dir,
    write_ocad_zip, DocumentHistory, OcadDocument,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::{json, Value};
use std::path::PathBuf;
fn patch(ops: Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":ops})).expect("patch")
}
fn definition(angle: f64) -> Value {
    json!({"type":"projected_sketch","sketch_id":"sketch:projection","workplane":{"type":"custom","origin":[0.,0.,0.],"normal":[0.,angle.sin(),angle.cos()],"x_axis":[1.,0.,0.]},"projections":[{"source_feature":"feature:source_sketch","source_entity":"ent:source_circle","curve_id":"ent:projected_curve","point_ids":["ent:projected_center"],"construction":false}]})
}
fn example() -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new(
        DocumentId::new("doc:projected_circle").expect("id"),
        "Associated circle projection",
    ));
    let p = patch(json!([
        {"type":"add_parameter","id":"param:radius","name":"radius","expr":"30 mm"},
        {"type":"add_parameter","id":"param:thickness","name":"thickness","expr":"6 mm"},
        {"type":"add_sketch","id":"sketch:source","name":"Source circle","workplane":{"type":"global","plane":"XY"}},
        {"type":"add_sketch_entity","sketch_id":"sketch:source","entity":{"type":"point","id":"ent:source_center","x":0.,"y":0.}},
        {"type":"add_sketch_entity","sketch_id":"sketch:source","entity":{"type":"circle","id":"ent:source_circle","center":"ent:source_center","radius":0.03}},
        {"type":"add_sketch_constraint","sketch_id":"sketch:source","constraint":{"type":"radius","id":"con:source_radius","target":"ent:source_circle","expr":"radius"}},
        {"type":"add_feature","node":{"id":"feature:source_sketch","name":"Source sketch","definition":{"type":"sketch","sketch_id":"sketch:source"}},"position":opencad_ai::FeaturePosition::end()},
        {"type":"add_feature","node":{"id":"feature:projection","name":"Projected sketch","definition":definition(std::f64::consts::FRAC_PI_6)},"position":opencad_ai::FeaturePosition::end()},
        {"type":"add_feature","node":{"id":"feature:projection_body","name":"Projected extrusion","definition":{"type":"extrude","sketch_feature":"feature:projection","profile_ref":"sketch:projection/profile:outer","extent":{"type":"distance","length":{"value_si":0.006}},"length_expr":"thickness","operation":"new_body"}},"position":opencad_ai::FeaturePosition::end()}
    ]));
    apply_patch_with_history(
        &mut doc,
        &p,
        &mut DocumentHistory::default(),
        "Create projection",
    )
    .expect("build");
    doc
}
#[test]
fn tilted_circle_projection_is_an_exact_ellipse_in_both_kernels() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let doc = example();
        let mut model = doc.clone().into_part_model();
        model
            .regenerate(
                kernel,
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                None,
            )
            .expect("regen");
        let sketch = model.outputs["feature:projection"]
            .sketch
            .as_ref()
            .expect("derived sketch");
        assert_eq!(sketch.id.as_str(), "sketch:projection");
        let curve = sketch.find_entity("ent:projected_curve").expect("curve");
        let opencad_sketch::SketchEntity::Ellipse(e) = curve else {
            panic!("tilted circle must be an ellipse")
        };
        let [a, b, angle] = opencad_sketch::solve::ellipse_parameters(e).expect("parameters");
        assert!((a - 0.03).abs() < 1e-12);
        assert!((b - 0.03 * std::f64::consts::FRAC_PI_6.cos()).abs() < 1e-12);
        assert!(angle.abs() < 1e-12);
        let body = model.active_body().expect("body");
        let mass = kernel.mass_properties(body, 1.).expect("mass");
        assert!(
            (mass.volume_m3
                - std::f64::consts::PI * 0.03 * 0.03 * std::f64::consts::FRAC_PI_6.cos() * 0.006)
                .abs()
                < 1e-12
        );
        assert!(model.sketches.get("sketch:projection").is_none());
        assert_eq!(doc.sketches.len(), 1);
    }
    check(&MockGeometryKernel::new());
    check(&OcctGeometryKernel::new());
}
#[test]
fn projection_association_updates_source_radius_and_plane_with_warm_cache() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let mut doc = example();
        let mut model = doc.clone().into_part_model();
        let registry = FeatureRegistry::with_defaults();
        let mut cache = RegenerationCache::with_backend("projection-test");
        model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("cold");
        let cached = model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("warm");
        assert_eq!(cached.cached_nodes.len(), 3);
        let p = patch(json!([{"type":"set_parameter","id":"param:radius","expr":"40 mm"}]));
        apply_patch_with_history(
            &mut doc,
            &p,
            &mut DocumentHistory::default(),
            "Source radius",
        )
        .expect("edit");
        let changed = model
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("source regen");
        assert!(changed
            .regenerated
            .iter()
            .any(|id| id == "feature:projection"));
        let mass = kernel
            .mass_properties(model.active_body().expect("body"), 1.)
            .expect("mass");
        assert!(
            (mass.volume_m3
                - std::f64::consts::PI * 0.04 * 0.04 * std::f64::consts::FRAC_PI_6.cos() * 0.006)
                .abs()
                < 1e-12
        );
        let curve = model.outputs["feature:projection"]
            .sketch
            .as_ref()
            .expect("output")
            .find_entity("ent:projected_curve")
            .expect("stable ID");
        assert!(matches!(curve, opencad_sketch::SketchEntity::Ellipse(_)));
        let p = patch(
            json!([{"type":"replace_feature_definition","id":"feature:projection","definition":definition(std::f64::consts::FRAC_PI_3)}]),
        );
        apply_patch_with_history(
            &mut doc,
            &p,
            &mut DocumentHistory::default(),
            "Target plane",
        )
        .expect("plane edit");
        let mut fresh = doc.clone().into_part_model();
        fresh
            .regenerate_with_cache(kernel, &registry, Some(&doc.parameters), None, &mut cache)
            .expect("plane regen");
        let mass = kernel
            .mass_properties(fresh.active_body().expect("body"), 1.)
            .expect("mass");
        assert!(
            (mass.volume_m3
                - std::f64::consts::PI * 0.04 * 0.04 * std::f64::consts::FRAC_PI_3.cos() * 0.006)
                .abs()
                < 1e-12
        );
        assert!(matches!(
            fresh.outputs["feature:projection"]
                .sketch
                .as_ref()
                .expect("output")
                .find_entity("ent:projected_curve"),
            Some(opencad_sketch::SketchEntity::Ellipse(_))
        ));
    }
    check(&MockGeometryKernel::new());
    check(&OcctGeometryKernel::new());
}
#[test]
fn missing_sources_cycles_collapsed_planes_and_suppression_refuse_atomically() {
    let mut doc = example();
    let before = doc.clone();
    let mut missing = definition(0.);
    missing["projections"][0]["source_entity"] = json!("ent:missing");
    let mut cycle = definition(0.);
    cycle["projections"][0]["source_feature"] = json!("feature:projection");
    let mut wrong_kind = definition(0.);
    wrong_kind["projections"][0]["source_feature"] = json!("feature:projection_body");
    let mut bad_plane = definition(0.);
    bad_plane["workplane"]["x_axis"] = json!([0., 0., 1.]);
    for definition in [
        missing,
        cycle,
        wrong_kind,
        bad_plane,
        definition(std::f64::consts::FRAC_PI_2),
    ] {
        let p = patch(
            json!([{"type":"replace_feature_definition","id":"feature:projection","definition":definition}]),
        );
        assert!(apply_patch_with_history(
            &mut doc,
            &p,
            &mut DocumentHistory::default(),
            "Invalid projection"
        )
        .is_err());
        assert_eq!(doc, before);
    }
    for operations in [
        json!([{"type":"remove_sketch_entity","sketch_id":"sketch:source","entity_id":"ent:source_circle"}]),
        json!([{"type":"set_feature_suppressed","id":"feature:source_sketch","suppressed":true}]),
    ] {
        let p = patch(operations);
        assert!(apply_patch_with_history(
            &mut doc,
            &p,
            &mut DocumentHistory::default(),
            "Source removal"
        )
        .is_err());
        assert_eq!(doc, before);
    }
}
#[test]
fn projection_source_roundtrips_and_history_never_save_generated_coordinates() {
    let mut doc = example();
    let original = doc.clone();
    let mut history = DocumentHistory::default();
    let p = patch(json!([{"type":"set_parameter","id":"param:radius","expr":"40 mm"}]));
    apply_patch_with_history(&mut doc, &p, &mut history, "Edit source").expect("edit");
    let after = doc.clone();
    history.undo(&mut doc).expect("undo");
    assert_eq!(doc, original);
    history.redo(&mut doc).expect("redo");
    assert_eq!(doc, after);
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("projection.ocad.d");
    write_expanded_dir(&path, &doc).expect("save");
    let mut expanded = read_ocad(&path).expect("read");
    assert_eq!(expanded.sketches.len(), 1);
    assert!(expanded
        .sketches
        .iter()
        .all(|s| s.id.as_str() != "sketch:projection"));
    let zip = temp.path().join("projection.ocad");
    write_ocad_zip(&zip, &doc).expect("pack");
    let mut packed = read_ocad_zip(&zip).expect("packed");
    expanded.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    packed.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    doc.feature_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(expanded, doc);
    assert_eq!(packed, doc);
    let patch = authoring_patch(&document_design_state(&doc)).expect("authoring");
    assert_eq!(
        serde_json::from_slice::<DesignPatch>(&serde_json::to_vec(&patch).expect("wire"))
            .expect("roundtrip"),
        patch
    );
}
#[test]
#[ignore = "generate associated semantic projection example"]
fn generate_projected_sketch_example() {
    let doc = example();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    write_expanded_dir(root.join("examples/projected_circle_plate.ocad.d"), &doc).expect("example");
    let patch = authoring_patch(&document_design_state(&doc)).expect("authoring");
    std::fs::write(
        root.join("examples/agent/create_projected_circle_plate_patch.json"),
        serde_json::to_string_pretty(&patch).expect("JSON") + "\n",
    )
    .expect("patch");
}

#[test]
fn chained_projection_preserves_association_and_analytic_volume() {
    let mut doc = example();
    let p = patch(json!([
        {"type":"add_feature","node":{"id":"feature:second_projection","name":"Second projection","definition":{"type":"projected_sketch","sketch_id":"sketch:second_projection","workplane":{"type":"global","plane":"XY"},"projections":[{"source_feature":"feature:projection","source_entity":"ent:projected_curve","curve_id":"ent:second_curve","point_ids":["ent:second_center"]}]}},"position":{"before":"feature:projection_body"}},
        {"type":"replace_feature_definition","id":"feature:projection_body","definition":{"type":"extrude","sketch_feature":"feature:second_projection","profile_ref":"sketch:second_projection/profile:outer","extent":{"type":"distance","length":{"value_si":0.006}},"operation":"new_body"}}
    ]));
    apply_patch_with_history(
        &mut doc,
        &p,
        &mut DocumentHistory::default(),
        "Chain projection",
    )
    .expect("chain");
    fn check<K: GeometryKernel>(doc: &OcadDocument, kernel: &K) {
        let mut model = doc.clone().into_part_model();
        model
            .regenerate(
                kernel,
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                None,
            )
            .expect("regen");
        let mass = kernel
            .mass_properties(model.active_body().expect("body"), 1.)
            .expect("mass");
        assert!(
            (mass.volume_m3
                - std::f64::consts::PI
                    * 0.03
                    * 0.03
                    * std::f64::consts::FRAC_PI_6.cos().powi(2)
                    * 0.006)
                .abs()
                < 1e-12
        );
    }
    check(&doc, &MockGeometryKernel::new());
    check(&doc, &OcctGeometryKernel::new());
}
#[test]
fn shared_line_vertices_retain_semantic_identity_and_ambiguity_refuses() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let doc = read_ocad(root.join("examples/construction_plate.ocad.d")).expect("source");
    let source = opencad_feature::projected_sketch::resolve_design_sketch(
        &doc.feature_nodes,
        &doc.sketches,
        &doc.parameters,
        "feature:sketch_base",
    )
    .expect("solved source");
    let mut def:opencad_feature::ProjectedSketchFeature=serde_json::from_value(json!({"sketch_id":"sketch:line_projection","workplane":{"type":"global","plane":"XY"},"projections":(0..4).map(|i|json!({"source_feature":"feature:sketch_base","source_entity":format!("ent:e{i}"),"curve_id":format!("ent:projected_e{i}"),"point_ids":[format!("ent:projected_p{i}"),format!("ent:projected_p{}",(i+1)%4)]})).collect::<Vec<_>>()})).expect("definition");
    let generated =
        opencad_feature::projected_sketch::project_sketch(&def, "Lines", |_| Ok(source.clone()))
            .expect("projection");
    assert_eq!(generated.entities.len(), 8);
    assert!(generated.profiles.iter().any(|p| p.is_closed()));
    let profile =
        opencad_feature::profile_to_solved(&generated, "sketch:line_projection/profile:outer")
            .expect("wire");
    assert_eq!(profile.points.len(), 4);
    assert!(
        (profile
            .points
            .iter()
            .map(|p| p[0])
            .fold(f64::NEG_INFINITY, f64::max)
            - 0.08)
            .abs()
            < 1e-12
    );
    let mut wrong = def.clone();
    wrong.projections[1].point_ids[0] = "ent:projected_p0".into();
    assert!(
        opencad_feature::projected_sketch::project_sketch(&wrong, "Wrong sharing", |_| Ok(
            source.clone()
        ))
        .is_err()
    );
    let mut ambiguous = source.clone();
    ambiguous
        .entities
        .push(source.find_entity("ent:e0").expect("edge").clone());
    assert!(
        opencad_feature::projected_sketch::project_sketch(&def, "Ambiguous", |_| Ok(
            ambiguous.clone()
        ))
        .is_err()
    );
    def.workplane = opencad_sketch::Workplane::Global {
        plane: opencad_sketch::GlobalPlane::YZ,
    };
    assert!(
        opencad_feature::projected_sketch::project_sketch(&def, "Collapse", |_| Ok(source.clone()))
            .is_err()
    );
}
#[test]
fn failed_projection_regeneration_retains_previous_cached_outputs() {
    let doc = example();
    let kernel = OcctGeometryKernel::new();
    let mut model = doc.clone().into_part_model();
    let registry = FeatureRegistry::with_defaults();
    let mut cache = RegenerationCache::with_backend("projection-native");
    model
        .regenerate_with_cache(&kernel, &registry, Some(&doc.parameters), None, &mut cache)
        .expect("cold");
    let previous = model.active_body().expect("body").clone();
    let previous_sketch = model.outputs["feature:projection"].sketch.clone();
    let mut bad = doc.parameters.clone();
    bad.set_expr("param:radius", "0 mm").expect("bad radius");
    assert!(model
        .regenerate_with_cache(&kernel, &registry, Some(&bad), None, &mut cache)
        .is_err());
    assert_eq!(model.active_body().expect("body"), &previous);
    assert_eq!(model.outputs["feature:projection"].sketch, previous_sketch);
}
