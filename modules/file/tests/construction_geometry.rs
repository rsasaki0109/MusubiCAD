//! MCAD-P13-001: existing source flags edited through transactional patches.
use opencad_ai::{authoring_patch, rebase_patch, ConflictKind, DesignPatch};
use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::{bracket_base_plate, FeatureRegistry};
use opencad_file::{
    apply_patch_with_history, document_design_state, dry_run_patch_document, read_ocad_zip,
    write_expanded_dir, write_ocad_zip, DocumentHistory, OcadDocument,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_graph::{bracket_parameters, SemanticChange};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::json;
use std::path::PathBuf;

fn patch(value: serde_json::Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":value})).expect("patch")
}
fn set(id: &str, construction: bool) -> DesignPatch {
    patch(
        json!([{"type":"set_sketch_construction","sketch_id":"sketch:base","entity_id":id,"construction":construction}]),
    )
}
fn plate() -> OcadDocument {
    let mut doc = OcadDocument::from_part_model(
        DocumentMetadata::new(
            DocumentId::new("doc:construction_plate").expect("id"),
            "Construction guide plate",
        ),
        &bracket_base_plate().expect("plate"),
    );
    doc.parameters = bracket_parameters();
    let guide = patch(json!([
        {"type":"add_parameter","id":"param:guide_length","name":"guide_length","expr":"40 mm"},
        {"type":"add_sketch_entity","sketch_id":"sketch:base","entity":{"type":"point","id":"ent:guide_a","construction":true,"x":0.02,"y":0.03}},
        {"type":"add_sketch_entity","sketch_id":"sketch:base","entity":{"type":"point","id":"ent:guide_b","construction":true,"x":0.06,"y":0.03}},
        {"type":"add_sketch_entity","sketch_id":"sketch:base","entity":{"type":"line","id":"ent:guide_line","start":"ent:guide_a","end":"ent:guide_b"}},
        {"type":"add_sketch_entity","sketch_id":"sketch:base","entity":{"type":"circle","id":"ent:guide_circle","construction":true,"center":"ent:guide_a","radius":0.01}},
        {"type":"add_sketch_constraint","sketch_id":"sketch:base","constraint":{"type":"horizontal","id":"con:guide_horizontal","line":"ent:guide_line"}},
        {"type":"add_sketch_constraint","sketch_id":"sketch:base","constraint":{"type":"distance","id":"con:guide_length","target":{"line":"ent:guide_line"},"expr":"guide_length"}}
    ]));
    apply_patch_with_history(
        &mut doc,
        &guide,
        &mut DocumentHistory::default(),
        "Add guide",
    )
    .expect("guide");
    doc
}

#[test]
fn construction_edit_preserves_ids_and_constraint_geometry() {
    let mut doc = plate();
    let before = doc.clone();
    let edit = set("ent:guide_line", true);
    let dry = dry_run_patch_document(&doc, &edit);
    assert!(dry.validation.is_ok(), "{:?}", dry.validation);
    assert!(dry
        .diff
        .changes
        .contains(&SemanticChange::SketchEntityModified {
            sketch_id: "sketch:base".into(),
            id: "ent:guide_line".into()
        }));
    apply_patch_with_history(
        &mut doc,
        &edit,
        &mut DocumentHistory::default(),
        "Construction",
    )
    .expect("edit");
    let old = &before.sketches[0];
    let new = &doc.sketches[0];
    assert_eq!(old.constraints, new.constraints);
    assert_eq!(
        old.entities.iter().map(|e| e.id()).collect::<Vec<_>>(),
        new.entities.iter().map(|e| e.id()).collect::<Vec<_>>()
    );
    let mut restored = new.find_entity("ent:guide_line").expect("line").clone();
    restored.set_construction(false);
    assert_eq!(&restored, old.find_entity("ent:guide_line").expect("old"));
    let no_op = dry_run_patch_document(&doc, &edit);
    assert!(no_op.validation.is_ok());
    assert!(no_op.diff.changes.is_empty());
}

#[test]
fn construction_profile_alias_rebinding_refuses_atomically() {
    let mut doc = plate();
    let before = doc.clone();
    let mut history = DocumentHistory::default();
    for edit in [
        set("ent:guide_circle", false),
        set("ent:e0", true),
        set("ent:missing", true),
    ] {
        assert!(apply_patch_with_history(&mut doc, &edit, &mut history, "Invalid edit").is_err());
        assert_eq!(doc, before);
        assert_eq!(history, DocumentHistory::default());
    }
}

#[test]
fn stale_construction_edits_report_entity_conflicts() {
    let doc = plate();
    let base = document_design_state(&doc);
    let mut theirs = base.clone();
    let entity = theirs.sketches[0]
        .entities
        .iter_mut()
        .find(|e| e.id().as_str() == "ent:guide_line")
        .expect("line");
    let opencad_sketch::SketchEntity::Line(line) = entity else {
        panic!("line");
    };
    std::mem::swap(&mut line.start, &mut line.end);
    let conflicts =
        rebase_patch(&set("ent:guide_line", true), &base, &theirs).expect_err("conflict");
    assert!(conflicts
        .iter()
        .any(|c| c.kind == ConflictKind::Sketch && c.id.contains("ent:guide_line")));
}

#[test]
fn construction_constraints_still_solve_after_parameter_edits() {
    let mut doc = plate();
    let mut history = DocumentHistory::default();
    apply_patch_with_history(
        &mut doc,
        &set("ent:guide_line", true),
        &mut history,
        "Construction",
    )
    .expect("flag");
    let edit = patch(json!([{"type":"set_parameter","id":"param:guide_length","expr":"50 mm"}]));
    apply_patch_with_history(&mut doc, &edit, &mut history, "Length").expect("parameter");
    let kernel = OcctGeometryKernel::new();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            None,
        )
        .expect("solve");
    let sketch = model.sketches.get("sketch:base").expect("sketch");
    let point = |id: &str| {
        let Some(opencad_sketch::SketchEntity::Point(p)) = sketch.find_entity(id) else {
            panic!("point");
        };
        let (opencad_sketch::Coord::Literal(x), opencad_sketch::Coord::Literal(y)) = (&p.x, &p.y)
        else {
            panic!("solved");
        };
        [*x, *y]
    };
    let [a, b] = [point("ent:guide_a"), point("ent:guide_b")];
    assert!(((b[0] - a[0]).hypot(b[1] - a[1]) - 0.05).abs() < 1e-7);
    assert!((b[1] - a[1]).abs() < 1e-7);
    assert!(
        (kernel
            .mass_properties(model.active_body().expect("body"), 1.)
            .expect("mass")
            .volume_m3
            - 2.88e-5)
            .abs()
            < 1e-12
    );
}

fn dimensions<K: GeometryKernel>(doc: &OcadDocument, kernel: &K, volume: f64, bounds: [f64; 3]) {
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            kernel,
            &FeatureRegistry::with_defaults(),
            Some(&doc.parameters),
            None,
        )
        .expect("regenerate");
    let sketch = model.sketches.get("sketch:base").expect("sketch");
    let solved =
        opencad_feature::profile_to_solved(sketch, "sketch:base/profile:outer").expect("profile");
    assert_eq!(solved.points.len(), 4);
    assert!(solved
        .points
        .iter()
        .all(|p| p[0] >= -1e-9 && p[0] <= 0.08 + 1e-9 && p[1] >= -1e-9 && p[1] <= 0.06 + 1e-9));
    let body = model.active_body().expect("body");
    assert!((kernel.mass_properties(body, 1.).expect("mass").volume_m3 - volume).abs() < 1e-12);
    for (actual, expected) in kernel
        .bounding_box(body)
        .expect("bbox")
        .size()
        .into_iter()
        .zip(bounds)
    {
        // OCCT expands each bound by 1e-7 m; size includes both gaps plus roundoff.
        assert!(
            (actual - expected).abs() < 2.1e-7,
            "bbox size {actual} vs {expected}"
        );
    }
}

#[test]
fn construction_edit_round_trips_with_history_and_native_volume() {
    let mut doc = plate();
    let before = doc.clone();
    let mut history = DocumentHistory::default();
    apply_patch_with_history(
        &mut doc,
        &set("ent:guide_line", true),
        &mut history,
        "Construction",
    )
    .expect("edit");
    let dir = tempfile::tempdir().expect("temp");
    let packed = dir.path().join("plate.ocad");
    write_ocad_zip(&packed, &doc).expect("write");
    assert_eq!(read_ocad_zip(&packed).expect("read"), doc);
    let expected = doc.clone();
    history.undo(&mut doc).expect("undo");
    assert_eq!(doc, before);
    history.redo(&mut doc).expect("redo");
    assert_eq!(doc, expected);
    dimensions(
        &doc,
        &OcctGeometryKernel::new(),
        2.88e-5,
        [0.08, 0.06, 0.006],
    );
    // Legacy Mock emits a 10 mm cube (four profile points + six mm extent).
    dimensions(&doc, &MockGeometryKernel::new(), 1e-6, [0.01; 3]);
}

#[test]
#[ignore = "generate verified construction geometry example"]
fn generate_construction_example() {
    let mut doc = plate();
    apply_patch_with_history(
        &mut doc,
        &set("ent:guide_line", true),
        &mut DocumentHistory::default(),
        "Construction",
    )
    .expect("edit");
    dimensions(
        &doc,
        &OcctGeometryKernel::new(),
        2.88e-5,
        [0.08, 0.06, 0.006],
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    write_expanded_dir(root.join("examples/construction_plate.ocad.d"), &doc).expect("example");
    let patch = authoring_patch(&document_design_state(&doc)).expect("patch");
    std::fs::write(
        root.join("examples/agent/create_construction_plate_patch.json"),
        serde_json::to_string_pretty(&patch).expect("json") + "\n",
    )
    .expect("authoring");
}
