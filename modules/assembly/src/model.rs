//! Top-level assembly document model.

use opencad_core::{DocumentId, OpenCadError, Result};
use serde::{Deserialize, Serialize};

use crate::component::Component;
use crate::connector::{validate_connectors, Connector};
use crate::instance::Instance;
use crate::mate::Mate;
use crate::pattern::{validate_patterns, AssemblyPattern};

/// Assembly design graph: child part references and placed instances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AssemblyModel {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<Instance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mates: Vec<Mate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connectors: Vec<Connector>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<AssemblyPattern>,
}

impl AssemblyModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn component(&self, id: &opencad_core::ComponentId) -> Option<&Component> {
        self.components.iter().find(|component| &component.id == id)
    }

    pub fn sorted_deterministic(mut self) -> Self {
        self.components
            .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        self.instances
            .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        self.mates.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        self.connectors
            .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        self.patterns
            .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        self
    }

    pub fn validate(&self, assembly_doc_id: &DocumentId) -> Result<()> {
        self.validate_no_self_reference(assembly_doc_id)?;
        let instance_ids: Vec<_> = self.instances.iter().map(|i| i.id.clone()).collect();
        crate::mate::validate_mates(&self.mates, &instance_ids, &self.connectors)?;
        validate_connectors(&self.connectors, &instance_ids)?;
        validate_patterns(self)?;
        Ok(())
    }

    /// Reject direct self-reference through `Component.source_doc`.
    pub fn validate_no_self_reference(&self, assembly_doc_id: &DocumentId) -> Result<()> {
        for component in &self.components {
            if &component.source_doc == assembly_doc_id {
                return Err(OpenCadError::validation(format!(
                    "component '{}' references the assembly document itself (cycle)",
                    component.id
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::{Instance, Placement};
    use opencad_core::{ComponentId, DocumentId, InstanceId};
    use opencad_geometry::RigidTransform;

    #[test]
    fn assembly_model_round_trip() -> Result<()> {
        let model = AssemblyModel {
            components: vec![Component::new(
                ComponentId::new("component:bracket")?,
                "parts/bracket.ocad.d",
                DocumentId::new("doc:bracket_001")?,
            )],
            instances: vec![Instance::new(
                InstanceId::new("instance:left")?,
                ComponentId::new("component:bracket")?,
                Placement::new(RigidTransform::identity()),
                "Left",
            )],
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        };
        let json = serde_json::to_string(&model).expect("serialize");
        let restored: AssemblyModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(model, restored);
        Ok(())
    }

    #[test]
    fn self_reference_rejected() -> Result<()> {
        let assembly_id = DocumentId::new("doc:assembly_001")?;
        let model = AssemblyModel {
            components: vec![Component::new(
                ComponentId::new("component:self")?,
                "self.ocad.d",
                assembly_id.clone(),
            )],
            instances: Vec::new(),
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        };
        assert!(model.validate_no_self_reference(&assembly_id).is_err());
        Ok(())
    }
}
