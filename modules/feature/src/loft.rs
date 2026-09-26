//! Loft feature: a solid skinned through two or more closed section
//! profiles (ADR-022, MCAD-P7-018).

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::{BooleanOp, ExtrudeOperation};

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::sketch_bridge::profile_to_solved_with_context;

/// One loft cross-section: a closed profile of a sketch feature, placed by
/// that sketch's workplane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoftSection {
    pub sketch_feature: String,
    pub profile_ref: String,
}

/// Loft feature parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoftFeature {
    /// Sections in skinning order; at least two.
    pub sections: Vec<LoftSection>,
    pub operation: ExtrudeOperation,
    /// Target body feature for cut/join operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

impl LoftFeature {
    /// Check the section count and the operation's target.
    pub fn validate(&self) -> Result<()> {
        if self.sections.len() < 2 {
            return Err(OpenCadError::validation("loft needs at least two sections"));
        }
        for (index, section) in self.sections.iter().enumerate() {
            if self.sections[..index]
                .iter()
                .any(|earlier| earlier == section)
            {
                return Err(OpenCadError::validation(format!(
                    "loft section '{}' ({}) is listed twice",
                    section.sketch_feature, section.profile_ref
                )));
            }
        }
        match (self.operation, &self.target_feature) {
            (ExtrudeOperation::NewBody, _) => Ok(()),
            (_, Some(_)) => Ok(()),
            (_, None) => Err(OpenCadError::validation(
                "join and cut lofts need target_feature",
            )),
        }
    }
}

/// Loft executor wired to `GeometryKernel::loft`.
#[derive(Debug, Default)]
pub struct LoftFeatureExecutor;

impl Feature for LoftFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "loft"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Loft(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected loft feature, got {}",
                node.definition.feature_type()
            )));
        };
        def.validate()?;

        let sections = def
            .sections
            .iter()
            .map(|section| {
                let sketch = ctx.sketch_for_feature(section.sketch_feature.as_str())?;
                profile_to_solved_with_context(sketch, &section.profile_ref, ctx)
            })
            .collect::<Result<Vec<_>>>()?;
        let kernel = ctx.kernel();
        let lofted = kernel.loft(&sections)?;
        let body = match (def.operation, &def.target_feature) {
            (ExtrudeOperation::NewBody, _) => lofted,
            (operation, Some(target)) => {
                let target = ctx.body_for_feature(target.as_str())?;
                let op = if operation == ExtrudeOperation::Cut {
                    BooleanOp::Subtract
                } else {
                    BooleanOp::Union
                };
                kernel.boolean(target, lofted, op)?
            }
            (_, None) => unreachable!("validated above"),
        };
        Ok(FeatureOutput { body: Some(body) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(sketch: &str) -> LoftSection {
        LoftSection {
            sketch_feature: sketch.into(),
            profile_ref: format!("{}/profile:outer", sketch.replace("feature:", "sketch:")),
        }
    }

    #[test]
    fn validation_requires_two_distinct_sections_and_a_target_for_booleans() {
        let loft = |sections: Vec<LoftSection>, operation, target: Option<&str>| LoftFeature {
            sections,
            operation,
            target_feature: target.map(Into::into),
        };
        let two = vec![section("feature:low"), section("feature:high")];
        assert!(loft(two.clone(), ExtrudeOperation::NewBody, None)
            .validate()
            .is_ok());
        assert!(
            loft(two.clone(), ExtrudeOperation::Cut, Some("feature:plate"))
                .validate()
                .is_ok()
        );
        for (def, message) in [
            (
                loft(
                    vec![section("feature:low")],
                    ExtrudeOperation::NewBody,
                    None,
                ),
                "at least two sections",
            ),
            (
                loft(
                    vec![section("feature:low"), section("feature:low")],
                    ExtrudeOperation::NewBody,
                    None,
                ),
                "listed twice",
            ),
            (
                loft(two, ExtrudeOperation::Join, None),
                "need target_feature",
            ),
        ] {
            let error = def.validate().expect_err(message);
            assert!(error.to_string().contains(message), "{error}");
        }
    }
}
