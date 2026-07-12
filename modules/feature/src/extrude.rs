//! Extrude boss/cut/join feature (Task-089+).

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::{BooleanOp, ExtrudeExtent, ExtrudeOperation};

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::sketch_bridge::{extrude_direction_for_operation, profile_to_solved_with_context};

/// Extrude feature parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtrudeFeature {
    /// Feature id of the parent sketch feature.
    pub sketch_feature: String,
    /// Closed profile reference (e.g. `sketch:base/profile:outer`).
    pub profile_ref: String,
    pub extent: ExtrudeExtent,
    pub operation: ExtrudeOperation,
    /// Parametric length expression resolved before regeneration (e.g. `thickness`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length_expr: Option<String>,
    /// Target body feature for cut/join operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

/// Extrude feature executor wired to `GeometryKernel`.
#[derive(Debug, Default)]
pub struct ExtrudeFeatureExecutor;

impl Feature for ExtrudeFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "extrude"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Extrude(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected extrude feature, got {}",
                node.definition.feature_type()
            )));
        };

        let sketch = ctx.sketch_for_feature(def.sketch_feature.as_str())?;
        let solved = profile_to_solved_with_context(sketch, &def.profile_ref, ctx)?;
        let direction = extrude_direction_for_operation(sketch, ctx, def.operation)?;
        let kernel = ctx.kernel();
        let wire = kernel.make_wire_from_sketch(&solved)?;

        let target_body = def
            .target_feature
            .as_ref()
            .map(|id| ctx.body_for_feature(id.as_str()))
            .transpose()?;

        let body = match def.operation {
            ExtrudeOperation::NewBody => kernel.extrude(
                wire,
                def.extent.clone(),
                ExtrudeOperation::NewBody,
                None,
                direction,
            )?,
            ExtrudeOperation::Cut => {
                let Some(target) = target_body else {
                    return Err(OpenCadError::validation(
                        "cut extrude requires target_feature",
                    ));
                };
                kernel.extrude(
                    wire,
                    def.extent.clone(),
                    ExtrudeOperation::Cut,
                    Some(target),
                    direction,
                )?
            }
            ExtrudeOperation::Join => {
                let new_body = kernel.extrude(
                    wire,
                    def.extent.clone(),
                    ExtrudeOperation::NewBody,
                    None,
                    direction,
                )?;
                let Some(target) = target_body else {
                    return Err(OpenCadError::validation(
                        "join extrude requires target_feature",
                    ));
                };
                kernel.boolean(target, new_body, BooleanOp::Union)?
            }
        };

        Ok(FeatureOutput { body: Some(body) })
    }
}

impl ExtrudeFeature {
    pub fn boss(
        sketch_feature: impl Into<String>,
        profile_ref: impl Into<String>,
        extent: ExtrudeExtent,
    ) -> Self {
        Self {
            sketch_feature: sketch_feature.into(),
            profile_ref: profile_ref.into(),
            extent,
            operation: ExtrudeOperation::NewBody,
            length_expr: None,
            target_feature: None,
        }
    }

    pub fn join(
        sketch_feature: impl Into<String>,
        profile_ref: impl Into<String>,
        target_feature: impl Into<String>,
        extent: ExtrudeExtent,
    ) -> Self {
        Self {
            sketch_feature: sketch_feature.into(),
            profile_ref: profile_ref.into(),
            extent,
            operation: ExtrudeOperation::Join,
            length_expr: None,
            target_feature: Some(target_feature.into()),
        }
    }
}
