//! Neutral-plane draft for convex planar solids (ADR-036).
use opencad_core::{Angle, OpenCadError, Result};
use opencad_geometry::{DraftInput, FacePick};
use opencad_sketch::Workplane;
use serde::{Deserialize, Serialize};

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::sketch_bridge::{placement_from_workplane, resolve_workplane};
use crate::topo_resolve::resolve_face_on_body;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftFeature {
    pub target_feature: String,
    /// SI radians; positive narrows toward the neutral plane normal.
    pub angle: Angle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_expr: Option<String>,
    pub neutral_plane: Workplane,
    pub face_refs: Vec<String>,
}

pub fn validate_angle(angle_rad: f64) -> Result<()> {
    if !angle_rad.is_finite()
        || angle_rad.abs() < 1e-8
        || angle_rad.abs() > std::f64::consts::FRAC_PI_4
    {
        return Err(OpenCadError::validation(
            "draft angle must have magnitude between 1e-8 rad and 45 deg",
        ));
    }
    Ok(())
}
impl DraftFeature {
    pub fn validate(&self) -> Result<()> {
        validate_angle(self.angle.radians())?;
        if self.target_feature.is_empty() || self.face_refs.is_empty() {
            return Err(OpenCadError::validation(
                "draft needs a target and semantic face references",
            ));
        }
        for (i, id) in self.face_refs.iter().enumerate() {
            if !id.starts_with("ref:face:") || self.face_refs[..i].contains(id) {
                return Err(OpenCadError::validation(
                    "draft needs unique ref:face: references",
                ));
            }
        }
        Ok(())
    }
}
pub struct DraftFeatureExecutor;
impl Feature for DraftFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "draft"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Draft(def) = &node.definition else {
            return Err(OpenCadError::validation("expected draft"));
        };
        def.validate()?;
        let body = ctx.body_for_feature(&def.target_feature)?;
        let discoveries = ctx.face_discoveries_on(&body)?;
        let faces = def
            .face_refs
            .iter()
            .map(|id| {
                let index = resolve_face_on_body(ctx, id, &discoveries)?;
                let face = discoveries
                    .iter()
                    .find(|face| face.kernel_face_id == index)
                    .ok_or_else(|| {
                        OpenCadError::validation("draft face has no discovered geometry")
                    })?;
                Ok(FacePick {
                    point_m: face.centroid_m.map(f64::from),
                    normal: face.normal_m.map(f64::from),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let plane = placement_from_workplane(&resolve_workplane(&def.neutral_plane, ctx)?)?;
        let [x, y] = [plane.x_axis_m, plane.y_axis_m];
        let normal = crate::datum::normalized([
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ])?;
        let input = DraftInput {
            angle_rad: def.angle.radians(),
            neutral_origin_m: plane.origin_m,
            neutral_normal: normal,
            faces,
        };
        let result = ctx.kernel().draft_body(body, &input)?;
        Ok(FeatureOutput {
            sketch: None,
            body: Some(result),
            datum: None,
        })
    }
}
