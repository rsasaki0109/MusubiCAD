//! Test fixtures for desktop preview tests.

use std::path::Path;

use opencad_core::{DocumentId, DocumentMetadata};
use opencad_feature::bracket_base_plate;
use opencad_file::{write_expanded_dir, OcadDocument};
use opencad_graph::bracket_parameters;

pub fn write_bracket_fixture_at(path: &Path) {
    let part = bracket_base_plate().expect("model");
    let metadata =
        DocumentMetadata::new(DocumentId::new("doc:bracket_001").expect("id"), "Bracket");
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_expanded_dir(path, &doc).expect("write");
}
