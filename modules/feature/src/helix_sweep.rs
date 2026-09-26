//! Helical sweep: a closed profile swept along a helix, for springs and
//! threads (ADR-025, MCAD-P7-026).

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::{BooleanOp, ExtrudeOperation, HelixSpec};

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::sketch_bridge::profile_to_solved_with_context;

/// Helical sweep parameters.  The helix passes through the profile's centre,
/// so its radius follows the profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelixSweepFeature {
    /// Sketch feature holding the closed cross-section.
    pub sketch_feature: String,
    /// Closed profile swept along the helix.
    pub profile_ref: String,
    pub axis_origin_m: [f64; 3],
    pub axis_direction_m: [f64; 3],
    /// Axial advance per turn, in metres.
    pub pitch_m: f64,
    /// Parametric pitch expression resolved before regeneration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch_expr: Option<String>,
    /// Total axial length, in metres.
    pub height_m: f64,
    /// Parametric height expression resolved before regeneration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_expr: Option<String>,
    pub operation: ExtrudeOperation,
    /// Target body feature for cut/join operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

impl HelixSweepFeature {
    pub fn helix(&self) -> HelixSpec {
        HelixSpec {
            axis_origin_m: self.axis_origin_m,
            axis_direction_m: self.axis_direction_m,
            pitch_m: self.pitch_m,
            height_m: self.height_m,
        }
    }

    /// Check the helix and the operation's target.
    pub fn validate(&self) -> Result<()> {
        self.helix().validate()?;
        match (self.operation, &self.target_feature) {
            (ExtrudeOperation::NewBody, _) | (_, Some(_)) => Ok(()),
            (_, None) => Err(OpenCadError::validation(
                "join and cut helical sweeps need target_feature",
            )),
        }
    }
}

/// Helical sweep executor wired to `GeometryKernel::helix_sweep`.
#[derive(Debug, Default)]
pub struct HelixSweepFeatureExecutor;

impl Feature for HelixSweepFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "helix_sweep"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::HelixSweep(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected helix_sweep feature, got {}",
                node.definition.feature_type()
            )));
        };
        def.validate()?;

        let sketch = ctx.sketch_for_feature(def.sketch_feature.as_str())?;
        let profile = profile_to_solved_with_context(sketch, &def.profile_ref, ctx)?;
        let kernel = ctx.kernel();
        let swept = kernel.helix_sweep(&profile, &def.helix())?;
        let body = match (def.operation, &def.target_feature) {
            (ExtrudeOperation::NewBody, _) => swept,
            (operation, Some(target)) => {
                let target = ctx.body_for_feature(target.as_str())?;
                let op = if operation == ExtrudeOperation::Cut {
                    BooleanOp::Subtract
                } else {
                    BooleanOp::Union
                };
                kernel.boolean(target, swept, op)?
            }
            (_, None) => unreachable!("validated above"),
        };
        Ok(FeatureOutput { body: Some(body) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coil(pitch_m: f64, operation: ExtrudeOperation, target: Option<&str>) -> HelixSweepFeature {
        HelixSweepFeature {
            sketch_feature: "feature:sketch_wire".into(),
            profile_ref: "sketch:wire/profile:outer".into(),
            axis_origin_m: [0.0, 0.0, 0.0],
            axis_direction_m: [0.0, 0.0, 1.0],
            pitch_m,
            pitch_expr: None,
            height_m: 0.02,
            height_expr: None,
            operation,
            target_feature: target.map(Into::into),
        }
    }

    #[test]
    fn validation_checks_the_helix_and_boolean_targets() {
        assert!(coil(0.005, ExtrudeOperation::NewBody, None)
            .validate()
            .is_ok());
        assert!(coil(0.005, ExtrudeOperation::Cut, Some("feature:rod"))
            .validate()
            .is_ok());
        let mut flat_axis = coil(0.005, ExtrudeOperation::NewBody, None);
        flat_axis.axis_direction_m = [0.0; 3];
        for (def, message) in [
            (
                coil(0.0, ExtrudeOperation::NewBody, None),
                "pitch must be positive",
            ),
            (flat_axis, "axis direction"),
            (
                coil(0.005, ExtrudeOperation::Join, None),
                "need target_feature",
            ),
        ] {
            let error = def.validate().expect_err(message);
            assert!(error.to_string().contains(message), "{error}");
        }
    }
}
