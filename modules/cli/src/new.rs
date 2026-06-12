//! `opencad new` command (Task-121+).

use opencad_core::{DocumentId, DocumentMetadata, Result};
use opencad_feature::{
    bracket_hole_ring, bracket_hole_row, bracket_pin_mirror, bracket_pin_ring, bracket_pin_row,
    bracket_semantic_refs, bracket_with_hole,
};
use opencad_file::{write_ocad, OcadDocument};
use opencad_graph::bracket_parameters;

/// Built-in sample document templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocumentTemplate {
    #[default]
    Bracket,
    HoleRow,
    HoleRing,
    PinRow,
    PinRing,
    PinMirror,
}

impl DocumentTemplate {
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "bracket" => Ok(Self::Bracket),
            "hole-row" => Ok(Self::HoleRow),
            "hole-ring" => Ok(Self::HoleRing),
            "pin-row" => Ok(Self::PinRow),
            "pin-ring" => Ok(Self::PinRing),
            "pin-mirror" => Ok(Self::PinMirror),
            _ => Err(opencad_core::OpenCadError::validation(format!(
                "unknown template '{name}'; expected 'bracket', 'hole-row', 'hole-ring', 'pin-row', 'pin-ring', or 'pin-mirror'"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bracket => "bracket",
            Self::HoleRow => "hole-row",
            Self::HoleRing => "hole-ring",
            Self::PinRow => "pin-row",
            Self::PinRing => "pin-ring",
            Self::PinMirror => "pin-mirror",
        }
    }
}

pub fn create_document(path: &str, template: DocumentTemplate) -> Result<()> {
    match template {
        DocumentTemplate::Bracket => create_bracket_document(path),
        DocumentTemplate::HoleRow => create_bracket_hole_row_document(path),
        DocumentTemplate::HoleRing => create_bracket_hole_ring_document(path),
        DocumentTemplate::PinRow => create_bracket_pin_row_document(path),
        DocumentTemplate::PinRing => create_bracket_pin_ring_document(path),
        DocumentTemplate::PinMirror => create_bracket_pin_mirror_document(path),
    }
}

pub fn create_bracket_document(path: &str) -> Result<()> {
    let part = bracket_with_hole()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_001")?,
        "Bracket with Mounting Hole",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    doc.semantic_refs = bracket_semantic_refs();
    write_ocad(path, &doc)
}

pub fn create_bracket_hole_row_document(path: &str) -> Result<()> {
    let part = bracket_hole_row()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_hole_row_001")?,
        "Bracket with Pin Hole Row",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_hole_ring_document(path: &str) -> Result<()> {
    let part = bracket_hole_ring()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_hole_ring_001")?,
        "Bracket with Pin Hole Ring",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_pin_row_document(path: &str) -> Result<()> {
    let part = bracket_pin_row()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_pin_row_001")?,
        "Bracket with Pin Boss Row",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_pin_ring_document(path: &str) -> Result<()> {
    let part = bracket_pin_ring()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_pin_ring_001")?,
        "Bracket with Pin Boss Ring",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_pin_mirror_document(path: &str) -> Result<()> {
    let part = bracket_pin_mirror()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_pin_mirror_001")?,
        "Bracket with Mirrored Pin",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    doc.semantic_refs = bracket_semantic_refs();
    write_ocad(path, &doc)
}
