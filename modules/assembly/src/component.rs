//! Child part reference in an assembly.

use opencad_core::{ComponentId, DocumentId};
use serde::{Deserialize, Serialize};

/// Whether a component references a part or nested assembly document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ComponentSourceKind {
    #[default]
    Part,
    Assembly,
}

/// Reference to an external part document loaded at regeneration time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub id: ComponentId,
    /// Path to the child `.ocad` / `.ocad.d`, relative to the assembly directory.
    pub source_path: String,
    pub source_doc: DocumentId,
    #[serde(default)]
    pub source_kind: ComponentSourceKind,
}

impl Component {
    pub fn new(id: ComponentId, source_path: impl Into<String>, source_doc: DocumentId) -> Self {
        Self {
            id,
            source_path: source_path.into(),
            source_doc,
            source_kind: ComponentSourceKind::Part,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, Result};

    #[test]
    fn component_round_trip() -> Result<()> {
        let component = Component::new(
            ComponentId::new("component:bracket")?,
            "parts/bracket.ocad.d",
            DocumentId::new("doc:bracket_001")?,
        );
        let json = serde_json::to_string(&component).expect("serialize");
        let restored: Component = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(component, restored);
        Ok(())
    }
}
