//! `opencad new` command (Task-121+).

use opencad_core::{DocumentId, DocumentMetadata, Result};
use opencad_feature::{
    bracket_boss_join, bracket_face_pin, bracket_edge_fillet, bracket_hole_ring, bracket_hole_row,
    bracket_pin_mirror, bracket_pin_ring, bracket_pin_row, bracket_semantic_refs,
    bracket_with_hole, revolve_bushing, revolve_sector,
};
use opencad_file::{write_ocad, OcadDocument};
use opencad_graph::bracket_parameters;

/// Built-in sample document templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocumentTemplate {
    #[default]
    Bracket,
    BossJoin,
    FacePin,
    EdgeFillet,
    HoleRow,
    HoleRing,
    PinRow,
    PinRing,
    PinMirror,
    RevolveBushing,
    RevolveSector,
}

impl DocumentTemplate {
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "bracket" => Ok(Self::Bracket),
            "boss-join" => Ok(Self::BossJoin),
            "face-pin" => Ok(Self::FacePin),
            "edge-fillet" => Ok(Self::EdgeFillet),
            "hole-row" => Ok(Self::HoleRow),
            "hole-ring" => Ok(Self::HoleRing),
            "pin-row" => Ok(Self::PinRow),
            "pin-ring" => Ok(Self::PinRing),
            "pin-mirror" => Ok(Self::PinMirror),
            "revolve-bushing" => Ok(Self::RevolveBushing),
            "revolve-sector" => Ok(Self::RevolveSector),
            _ => Err(opencad_core::OpenCadError::validation(format!(
                "unknown template '{name}'; expected 'bracket', 'boss-join', 'face-pin', 'edge-fillet', 'hole-row', 'hole-ring', 'pin-row', 'pin-ring', 'pin-mirror', 'revolve-bushing', or 'revolve-sector'"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bracket => "bracket",
            Self::BossJoin => "boss-join",
            Self::FacePin => "face-pin",
            Self::EdgeFillet => "edge-fillet",
            Self::HoleRow => "hole-row",
            Self::HoleRing => "hole-ring",
            Self::PinRow => "pin-row",
            Self::PinRing => "pin-ring",
            Self::PinMirror => "pin-mirror",
            Self::RevolveBushing => "revolve-bushing",
            Self::RevolveSector => "revolve-sector",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Bracket,
            Self::BossJoin,
            Self::FacePin,
            Self::EdgeFillet,
            Self::HoleRow,
            Self::HoleRing,
            Self::PinRow,
            Self::PinRing,
            Self::PinMirror,
            Self::RevolveBushing,
            Self::RevolveSector,
        ]
    }
}

pub fn create_document(path: &str, template: DocumentTemplate) -> Result<()> {
    match template {
        DocumentTemplate::Bracket => create_bracket_document(path),
        DocumentTemplate::BossJoin => create_bracket_boss_join_document(path),
        DocumentTemplate::FacePin => create_bracket_face_pin_document(path),
        DocumentTemplate::EdgeFillet => create_bracket_edge_fillet_document(path),
        DocumentTemplate::HoleRow => create_bracket_hole_row_document(path),
        DocumentTemplate::HoleRing => create_bracket_hole_ring_document(path),
        DocumentTemplate::PinRow => create_bracket_pin_row_document(path),
        DocumentTemplate::PinRing => create_bracket_pin_ring_document(path),
        DocumentTemplate::PinMirror => create_bracket_pin_mirror_document(path),
        DocumentTemplate::RevolveBushing => create_revolve_bushing_document(path),
        DocumentTemplate::RevolveSector => create_revolve_sector_document(path),
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

pub fn create_bracket_boss_join_document(path: &str) -> Result<()> {
    let part = bracket_boss_join()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_boss_join_001")?,
        "Bracket with Joined Boss",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_face_pin_document(path: &str) -> Result<()> {
    let part = bracket_face_pin()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_face_pin_001")?,
        "Bracket with Face Pin",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    doc.semantic_refs = bracket_semantic_refs();
    write_ocad(path, &doc)
}

pub fn create_bracket_edge_fillet_document(path: &str) -> Result<()> {
    let part = bracket_edge_fillet()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_edge_fillet_001")?,
        "Bracket with Edge Fillet",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    doc.semantic_refs = bracket_semantic_refs();
    write_ocad(path, &doc)
}

pub fn create_revolve_bushing_document(path: &str) -> Result<()> {
    let part = revolve_bushing()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:revolve_bushing_001")?,
        "Revolved Bushing",
    );
    let doc = OcadDocument::from_part_model(metadata, &part);
    write_ocad(path, &doc)
}

pub fn create_revolve_sector_document(path: &str) -> Result<()> {
    let part = revolve_sector()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:revolve_sector_001")?,
        "Revolved Sector (180°)",
    );
    let doc = OcadDocument::from_part_model(metadata, &part);
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
