//! Reference to an external 3D model document.

use opencad_core::{DocumentId, OpenCadError, Result};
use serde::{Deserialize, Serialize};

/// Path to a child part or assembly document loaded at render time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelReference {
    /// Path relative to the drawing directory.
    pub source_path: String,
    pub source_doc: DocumentId,
}

impl ModelReference {
    pub fn new(source_path: impl Into<String>, source_doc: DocumentId) -> Self {
        Self {
            source_path: source_path.into(),
            source_doc,
        }
    }

    pub fn validate(&self, drawing_doc_id: &DocumentId) -> Result<()> {
        if &self.source_doc == drawing_doc_id {
            return Err(OpenCadError::validation(
                "drawing view cannot reference the drawing document itself",
            ));
        }
        if self.source_path.trim().is_empty() {
            return Err(OpenCadError::validation(
                "model source_path must not be empty",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::Result;

    #[test]
    fn model_reference_round_trip() -> Result<()> {
        let reference =
            ModelReference::new("parts/bracket.ocad.d", DocumentId::new("doc:bracket_001")?);
        let json = serde_json::to_string(&reference).expect("serialize");
        let restored: ModelReference = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reference, restored);
        Ok(())
    }
}
