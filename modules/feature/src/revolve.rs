//! Revolve feature: sweep a closed profile around an axis.

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::{BooleanOp, ProfilePlane, RevolveInput, RevolveOperation};
use opencad_sketch::workplane::{GlobalPlane, Workplane};

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::sketch_bridge::profile_to_solved;

/// Revolve feature parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevolveFeature {
    pub sketch_feature: String,
    pub profile_ref: String,
    pub axis_origin_m: [f64; 3],
    pub axis_direction_m: [f64; 3],
    pub angle_rad: f64,
    /// Parametric angle expression resolved before regeneration (radians, e.g. `revolve_angle_rad`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle_expr: Option<String>,
    pub operation: RevolveOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

/// Revolve feature executor wired to `GeometryKernel`.
#[derive(Debug, Default)]
pub struct RevolveFeatureExecutor;

impl Feature for RevolveFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "revolve"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Revolve(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected revolve feature, got {}",
                node.definition.feature_type()
            )));
        };

        let sketch = ctx.sketch_for_feature(def.sketch_feature.as_str())?;
        let solved = profile_to_solved(sketch, &def.profile_ref)?;
        let profile_plane = profile_plane_for_workplane(&sketch.workplane)?;
        let kernel = ctx.kernel();

        let target_body = def
            .target_feature
            .as_ref()
            .map(|id| ctx.body_for_feature(id.as_str()))
            .transpose()?;

        let body = match def.operation {
            RevolveOperation::NewBody => kernel.revolve(&RevolveInput {
                sketch: solved,
                profile_plane,
                axis_origin_m: def.axis_origin_m,
                axis_direction_m: def.axis_direction_m,
                angle_rad: def.angle_rad,
                operation: RevolveOperation::NewBody,
                target: None,
            })?,
            RevolveOperation::Cut => {
                let Some(target) = target_body else {
                    return Err(OpenCadError::validation(
                        "cut revolve requires target_feature",
                    ));
                };
                kernel.revolve(&RevolveInput {
                    sketch: solved,
                    profile_plane,
                    axis_origin_m: def.axis_origin_m,
                    axis_direction_m: def.axis_direction_m,
                    angle_rad: def.angle_rad,
                    operation: RevolveOperation::Cut,
                    target: Some(target),
                })?
            }
            RevolveOperation::Join => {
                let new_body = kernel.revolve(&RevolveInput {
                    sketch: solved.clone(),
                    profile_plane,
                    axis_origin_m: def.axis_origin_m,
                    axis_direction_m: def.axis_direction_m,
                    angle_rad: def.angle_rad,
                    operation: RevolveOperation::NewBody,
                    target: None,
                })?;
                let Some(target) = target_body else {
                    return Err(OpenCadError::validation(
                        "join revolve requires target_feature",
                    ));
                };
                kernel.boolean(target, new_body, BooleanOp::Union)?
            }
        };

        Ok(FeatureOutput { body: Some(body) })
    }
}

fn profile_plane_for_workplane(workplane: &Workplane) -> Result<ProfilePlane> {
    match workplane {
        Workplane::Global { plane } => Ok(match plane {
            GlobalPlane::XY => ProfilePlane::Xy,
            GlobalPlane::YZ => ProfilePlane::Yz,
            GlobalPlane::XZ => ProfilePlane::Xz,
        }),
        Workplane::FaceRef { .. } => Err(OpenCadError::validation(
            "revolve does not support face_ref workplanes yet",
        )),
        Workplane::Custom { .. } => Err(OpenCadError::validation(
            "custom workplanes are not supported for revolve yet",
        )),
    }
}

impl RevolveFeature {
    pub fn boss(
        sketch_feature: impl Into<String>,
        profile_ref: impl Into<String>,
        axis_origin_m: [f64; 3],
        axis_direction_m: [f64; 3],
    ) -> Self {
        Self::with_angle(
            sketch_feature,
            profile_ref,
            axis_origin_m,
            axis_direction_m,
            std::f64::consts::TAU,
            None,
        )
    }

    pub fn with_angle(
        sketch_feature: impl Into<String>,
        profile_ref: impl Into<String>,
        axis_origin_m: [f64; 3],
        axis_direction_m: [f64; 3],
        angle_rad: f64,
        angle_expr: Option<String>,
    ) -> Self {
        Self {
            sketch_feature: sketch_feature.into(),
            profile_ref: profile_ref.into(),
            axis_origin_m,
            axis_direction_m,
            angle_rad,
            angle_expr,
            operation: RevolveOperation::NewBody,
            target_feature: None,
        }
    }
}
