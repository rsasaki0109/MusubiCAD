//! Linear assembly patterns that expand into placed instances (M3.3).

use opencad_core::{ComponentId, InstanceId, OpenCadError, PatternId, Result};
use opencad_geometry::RigidTransform;
use serde::{Deserialize, Serialize};

use crate::instance::{Instance, Placement};
use crate::model::AssemblyModel;

/// Repeats a component along a direction with fixed spacing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyPattern {
    pub id: PatternId,
    pub component: ComponentId,
    pub count: usize,
    pub spacing_m: f64,
    #[serde(default = "default_direction")]
    pub direction_m: [f64; 3],
    pub base_placement: Placement,
    pub name_prefix: String,
}

fn default_direction() -> [f64; 3] {
    [1.0, 0.0, 0.0]
}

impl AssemblyPattern {
    pub fn validate(&self, model: &AssemblyModel) -> Result<()> {
        if self.count == 0 {
            return Err(OpenCadError::validation(format!(
                "pattern '{}' count must be > 0",
                self.id
            )));
        }
        if self.spacing_m < 0.0 {
            return Err(OpenCadError::validation(format!(
                "pattern '{}' spacing must be >= 0",
                self.id
            )));
        }
        if model.component(&self.component).is_none() {
            return Err(OpenCadError::validation(format!(
                "pattern '{}' references unknown component '{}'",
                self.id, self.component
            )));
        }
        if self.name_prefix.trim().is_empty() {
            return Err(OpenCadError::validation(format!(
                "pattern '{}' name_prefix must not be empty",
                self.id
            )));
        }
        Ok(())
    }
}

pub fn validate_patterns(model: &AssemblyModel) -> Result<()> {
    for pattern in &model.patterns {
        pattern.validate(model)?;
    }
    Ok(())
}

/// Expand pattern definitions into explicit instances (patterns are not mutated).
pub fn expand_patterns(model: &AssemblyModel) -> Result<AssemblyModel> {
    validate_patterns(model)?;

    let mut expanded = model.clone();
    for pattern in &model.patterns {
        for index in 0..pattern.count {
            let offset = [
                pattern.direction_m[0] * pattern.spacing_m * index as f64,
                pattern.direction_m[1] * pattern.spacing_m * index as f64,
                pattern.direction_m[2] * pattern.spacing_m * index as f64,
            ];
            let step = RigidTransform::from_translation(offset);
            let transform = pattern.base_placement.transform.compose(step);
            let instance_id =
                InstanceId::new(format!("instance:{}{}", pattern.name_prefix, index))?;
            expanded.instances.push(Instance {
                id: instance_id,
                component: pattern.component.clone(),
                placement: Placement::new(transform),
                fixed: false,
                name: format!("{} {}", pattern.name_prefix, index),
            });
        }
    }
    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Component;
    use opencad_core::DocumentId;

    #[test]
    fn expands_linear_pattern() -> Result<()> {
        let model = AssemblyModel {
            components: vec![Component::new(
                ComponentId::new("component:bracket")?,
                "parts/bracket.ocad.d",
                DocumentId::new("doc:bracket_001")?,
            )],
            instances: Vec::new(),
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: vec![AssemblyPattern {
                id: PatternId::new("pattern:row")?,
                component: ComponentId::new("component:bracket")?,
                count: 3,
                spacing_m: 0.1,
                direction_m: [1.0, 0.0, 0.0],
                base_placement: Placement::new(RigidTransform::identity()),
                name_prefix: "pin_".into(),
            }],
        };

        let expanded = expand_patterns(&model)?;
        assert_eq!(expanded.instances.len(), 3);
        assert!((expanded.instances[1].placement.transform.translation_m[0] - 0.1).abs() < 1e-9);
        Ok(())
    }
}
