use serde::{Deserialize, Serialize};

use crate::id::DocumentId;
use crate::units::LengthUnit;

/// Whether a document owns part geometry or an assembly of child parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    #[default]
    Part,
    Assembly,
    Drawing,
}

/// Document-level metadata stored in `.ocad`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub id: DocumentId,
    pub name: String,
    pub units: LengthUnit,
    #[serde(default)]
    pub kind: DocumentKind,
    pub created_with: String,
    pub schema_version: String,
}

impl DocumentMetadata {
    pub fn new(id: DocumentId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            units: LengthUnit::Millimeter,
            kind: DocumentKind::Part,
            created_with: format!("OpenCAD {}", env!("CARGO_PKG_VERSION")),
            schema_version: "opencad.document.v0.1".into(),
        }
    }

    pub fn new_assembly(id: DocumentId, name: impl Into<String>) -> Self {
        let mut meta = Self::new(id, name);
        meta.kind = DocumentKind::Assembly;
        meta
    }

    pub fn new_drawing(id: DocumentId, name: impl Into<String>) -> Self {
        let mut meta = Self::new(id, name);
        meta.kind = DocumentKind::Drawing;
        meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_kind_round_trip() {
        let meta = DocumentMetadata::new_assembly(
            DocumentId::new("doc:assembly_001").expect("id"),
            "Assembly",
        );
        let json = serde_json::to_string(&meta).expect("serialize");
        let restored: DocumentMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(meta.kind, restored.kind);
    }

    #[test]
    fn metadata_round_trip() {
        let meta =
            DocumentMetadata::new(DocumentId::new("doc:test").expect("valid id"), "Test Part");
        let json = serde_json::to_string(&meta).expect("serialize");
        let restored: DocumentMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(meta, restored);
    }
}
