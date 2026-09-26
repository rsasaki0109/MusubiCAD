//! Sweep feature: a closed profile swept along a path of lines and arcs
//! (ADR-023, MCAD-P7-022).

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::{BooleanOp, ExtrudeOperation};

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::sketch_bridge::{path_to_solved_with_context, profile_to_solved_with_context};

/// Sweep feature parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepFeature {
    /// Sketch feature holding the closed cross-section.
    pub sketch_feature: String,
    /// Closed profile swept along the path.
    pub profile_ref: String,
    /// Sketch feature whose lines and endpoint arcs form one path chain.
    pub path_sketch_feature: String,
    pub operation: ExtrudeOperation,
    /// Target body feature for cut/join operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

impl SweepFeature {
    /// Check the operation's target and that profile and path differ.
    pub fn validate(&self) -> Result<()> {
        if self.sketch_feature == self.path_sketch_feature {
            return Err(OpenCadError::validation(
                "sweep profile and path must come from different sketches",
            ));
        }
        match (self.operation, &self.target_feature) {
            (ExtrudeOperation::NewBody, _) | (_, Some(_)) => Ok(()),
            (_, None) => Err(OpenCadError::validation(
                "join and cut sweeps need target_feature",
            )),
        }
    }
}

/// Sweep executor wired to `GeometryKernel::sweep`.
#[derive(Debug, Default)]
pub struct SweepFeatureExecutor;

impl Feature for SweepFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "sweep"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Sweep(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected sweep feature, got {}",
                node.definition.feature_type()
            )));
        };
        def.validate()?;

        let profile_sketch = ctx.sketch_for_feature(def.sketch_feature.as_str())?;
        let profile = profile_to_solved_with_context(profile_sketch, &def.profile_ref, ctx)?;
        let path_sketch = ctx.sketch_for_feature(def.path_sketch_feature.as_str())?;
        let path = path_to_solved_with_context(path_sketch, &profile, ctx)?;
        let kernel = ctx.kernel();
        let swept = kernel.sweep(&profile, &path)?;
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

    fn sweep(path: &str, operation: ExtrudeOperation, target: Option<&str>) -> SweepFeature {
        SweepFeature {
            sketch_feature: "feature:sketch_profile".into(),
            profile_ref: "sketch:profile/profile:outer".into(),
            path_sketch_feature: path.into(),
            operation,
            target_feature: target.map(Into::into),
        }
    }

    #[test]
    fn validation_checks_sketches_and_boolean_targets() {
        assert!(
            sweep("feature:sketch_path", ExtrudeOperation::NewBody, None)
                .validate()
                .is_ok()
        );
        assert!(sweep(
            "feature:sketch_path",
            ExtrudeOperation::Cut,
            Some("feature:plate")
        )
        .validate()
        .is_ok());
        for (def, message) in [
            (
                sweep("feature:sketch_profile", ExtrudeOperation::NewBody, None),
                "different sketches",
            ),
            (
                sweep("feature:sketch_path", ExtrudeOperation::Join, None),
                "need target_feature",
            ),
        ] {
            let error = def.validate().expect_err(message);
            assert!(error.to_string().contains(message), "{error}");
        }
    }
}
