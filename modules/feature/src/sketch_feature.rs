//! Sketch feature: stores and validates 2D profile input (Task-088).

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};
use opencad_sketch::Sketch;

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};

/// Sketch feature definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchFeatureDef {
    pub sketch_id: String,
}

/// Sketch feature executor.
#[derive(Debug, Default)]
pub struct SketchFeature;

impl Feature for SketchFeature {
    fn feature_type(&self) -> &'static str {
        "sketch"
    }

    fn execute(&self, node: &FeatureNode, _ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Sketch(_) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected sketch feature, got {}",
                node.definition.feature_type()
            )));
        };
        Ok(FeatureOutput::default())
    }
}

/// Validate that a sketch has at least one profile.
///
/// A sketch may hold only an open chain, such as a sweep path (ADR-023).
/// Consumers that need a closed profile (extrude, hole, revolve, loft,
/// sweep section) check closure when they resolve their `profile_ref`.
pub fn validate_sketch(sketch: &Sketch) -> Result<()> {
    if sketch.profiles.is_empty() {
        return Err(OpenCadError::validation(format!(
            "sketch '{}' has no profiles; call update_profiles() after solving",
            sketch.id
        )));
    }
    Ok(())
}
