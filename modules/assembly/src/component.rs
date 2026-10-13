//! Child part reference in an assembly.

use std::path::{Component as PathComponent, Path};

use opencad_core::{ComponentId, DocumentId, OpenCadError, Result};
use serde::{Deserialize, Serialize};

/// Whether a component references a part or nested assembly document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ComponentSourceKind {
    #[default]
    Part,
    Assembly,
    /// Mandatory old-reader refusal for parent-owned child coordinates.
    ArticulatedAssembly,
    /// Mandatory refusal in readers predating parent-context constraints.
    FlexibleAssembly,
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

    /// Validate the lexical form of a child path before it is joined to an
    /// assembly root. Child references are deliberately relative and may not
    /// contain parent/root/prefix components; canonical containment is checked
    /// by the regeneration path once the filesystem entry exists.
    pub fn validate_source_path(source_path: &str) -> Result<()> {
        let normalized = source_path.replace('\\', "/");
        if normalized.trim().is_empty() || normalized.contains('\0') {
            return Err(OpenCadError::validation(
                "component source path must be a non-empty relative path",
            ));
        }
        if normalized.starts_with('/')
            || normalized
                .as_bytes()
                .get(1)
                .is_some_and(|character| *character == b':')
        {
            return Err(OpenCadError::validation(format!(
                "component source path '{source_path}' must be relative"
            )));
        }
        for component in Path::new(&normalized).components() {
            if matches!(
                component,
                PathComponent::ParentDir | PathComponent::RootDir | PathComponent::Prefix(_)
            ) {
                return Err(OpenCadError::validation(format!(
                    "component source path '{source_path}' contains a forbidden path component"
                )));
            }
        }
        Ok(())
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

    #[test]
    fn source_path_rejects_absolute_and_parent_paths() {
        assert!(Component::validate_source_path("../outside.ocad.d").is_err());
        assert!(Component::validate_source_path("/tmp/outside.ocad.d").is_err());
        assert!(Component::validate_source_path("C:/outside.ocad.d").is_err());
        assert!(Component::validate_source_path("parts/bracket.ocad.d").is_ok());
    }
}
