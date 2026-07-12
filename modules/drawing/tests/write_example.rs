//! Generates `examples/bracket_front_view.ocad.d` (run via `cargo test -p opencad-drawing --test write_example -- --ignored`).

use std::path::PathBuf;

use opencad_core::{DocumentId, DocumentMetadata, SheetId, ViewId};
use opencad_drawing::{
    DrawingModel, DrawingView, ModelReference, ProjectionKind, Sheet, A4_HEIGHT_M, A4_WIDTH_M,
};
use opencad_feature::bracket_with_hole;
use opencad_file::{write_expanded_dir, OcadDocument};
use opencad_graph::bracket_parameters;

#[test]
#[ignore = "run manually to refresh examples/bracket_front_view.ocad.d"]
fn write_bracket_front_view_example() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example_root = manifest_dir.join("../../examples/bracket_front_view.ocad.d");
    let child_relative = "parts/bracket.ocad.d";
    let child_path = example_root.join(child_relative);

    if let Some(parent) = child_path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir child parent");
    }

    let part = bracket_with_hole().expect("bracket model");
    let mut child_doc = OcadDocument::from_part_model(
        DocumentMetadata::new(
            DocumentId::new("doc:bracket_001").expect("id"),
            "Bracket with Mounting Hole",
        ),
        &part,
    );
    child_doc.parameters = bracket_parameters();
    write_expanded_dir(&child_path, &child_doc).expect("write child");

    let drawing = DrawingModel {
        sheets: vec![Sheet {
            id: SheetId::new("sheet:a4").expect("id"),
            name: "Sheet 1".into(),
            width_m: A4_WIDTH_M,
            height_m: A4_HEIGHT_M,
            views: vec![DrawingView::new(
                ViewId::new("view:front").expect("id"),
                "Front",
                ModelReference::new(
                    child_relative.replace('\\', "/"),
                    DocumentId::new("doc:bracket_001").expect("id"),
                ),
                ProjectionKind::Front,
                1.0,
                [0.05, 0.05],
            )],
        }],
    }
    .sorted_deterministic();

    let drawing_doc = OcadDocument::from_drawing_model(
        DocumentMetadata::new_drawing(
            DocumentId::new("doc:bracket_front_view").expect("id"),
            "Bracket Front View",
        ),
        drawing,
    );

    write_expanded_dir(&example_root, &drawing_doc).expect("write drawing");
}
