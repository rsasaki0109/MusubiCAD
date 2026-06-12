//! Linear and circular pattern features (Task-095+).

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::BooleanOp;

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::topo_resolve::plane_for_face_ref;

/// How patterned instances combine with the target body.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternOperation {
    #[default]
    Union,
    Cut,
}

/// Linear pattern parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearPatternFeature {
    /// Feature whose body output is repeated.
    pub source_feature: String,
    /// Pattern direction in meters (normalized during execution).
    pub direction_m: [f64; 3],
    pub spacing: Length,
    /// Parametric spacing expression resolved before regeneration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing_expr: Option<String>,
    /// Total instance count including the source body.
    pub count: u32,
    #[serde(default)]
    pub operation: PatternOperation,
    /// Target body for `cut` patterns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

/// Circular pattern parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircularPatternFeature {
    pub source_feature: String,
    pub axis_origin_m: [f64; 3],
    pub axis_direction_m: [f64; 3],
    pub count: u32,
    #[serde(default)]
    pub operation: PatternOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

/// Mirror pattern parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MirrorPatternFeature {
    pub source_feature: String,
    pub plane_origin_m: [f64; 3],
    pub plane_normal_m: [f64; 3],
    /// Optional persisted face ref that defines the mirror plane at regen time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plane_face_ref: Option<String>,
    #[serde(default)]
    pub operation: PatternOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

/// Linear pattern executor: translate + boolean union/cut.
#[derive(Debug, Default)]
pub struct LinearPatternFeatureExecutor;

/// Circular pattern executor: rotate + boolean union/cut.
#[derive(Debug, Default)]
pub struct CircularPatternFeatureExecutor;

/// Mirror pattern executor: mirror + boolean union/cut.
#[derive(Debug, Default)]
pub struct MirrorPatternFeatureExecutor;

impl Feature for LinearPatternFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "linear_pattern"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::LinearPattern(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected linear pattern feature, got {}",
                node.definition.feature_type()
            )));
        };
        let spacing_m = def.spacing.meters();
        if spacing_m <= 0.0 && def.count > 1 {
            return Err(OpenCadError::validation("pattern spacing must be positive"));
        }
        execute_pattern(
            PatternRun {
                ctx,
                source_feature: def.source_feature.as_str(),
                count: def.count,
                operation: def.operation,
                target_feature: def.target_feature.as_deref(),
                spacing_m,
                direction: normalize_direction(def.direction_m)?,
            },
            |ctx, source, index, spacing_m, direction| {
                let offset = [
                    direction[0] * spacing_m * f64::from(index),
                    direction[1] * spacing_m * f64::from(index),
                    direction[2] * spacing_m * f64::from(index),
                ];
                ctx.kernel().translate_body(source, offset)
            },
        )
    }
}

impl Feature for CircularPatternFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "circular_pattern"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::CircularPattern(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected circular pattern feature, got {}",
                node.definition.feature_type()
            )));
        };
        let axis_direction = normalize_direction(def.axis_direction_m)?;
        execute_pattern(
            PatternRun {
                ctx,
                source_feature: def.source_feature.as_str(),
                count: def.count,
                operation: def.operation,
                target_feature: def.target_feature.as_deref(),
                spacing_m: 0.0,
                direction: axis_direction,
            },
            |ctx, source, index, _spacing_m, _direction| {
                let angle = TAU * f64::from(index) / f64::from(def.count);
                ctx.kernel()
                    .rotate_body(source, def.axis_origin_m, axis_direction, angle)
            },
        )
    }
}

impl Feature for MirrorPatternFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "mirror_pattern"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::MirrorPattern(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected mirror pattern feature, got {}",
                node.definition.feature_type()
            )));
        };
        let (plane_origin, plane_normal) = if let Some(ref face_ref) = def.plane_face_ref {
            plane_for_face_ref(ctx, face_ref)?
        } else {
            (def.plane_origin_m, normalize_direction(def.plane_normal_m)?)
        };
        let source = ctx.body_for_feature(def.source_feature.as_str())?;
        let kernel = ctx.kernel();
        let mirrored = kernel.mirror_body(source.clone(), plane_origin, plane_normal)?;

        match def.operation {
            PatternOperation::Union => {
                let body = kernel.boolean(source, mirrored, BooleanOp::Union)?;
                Ok(FeatureOutput { body: Some(body) })
            }
            PatternOperation::Cut => {
                let target_id = def.target_feature.as_deref().ok_or_else(|| {
                    OpenCadError::validation("cut pattern requires target_feature")
                })?;
                let mut result = ctx.body_for_feature(target_id)?;
                result = kernel.boolean(result, source, BooleanOp::Subtract)?;
                result = kernel.boolean(result, mirrored, BooleanOp::Subtract)?;
                Ok(FeatureOutput { body: Some(result) })
            }
        }
    }
}

struct PatternRun<'a> {
    ctx: &'a dyn RegenContext,
    source_feature: &'a str,
    count: u32,
    operation: PatternOperation,
    target_feature: Option<&'a str>,
    spacing_m: f64,
    direction: [f64; 3],
}

fn execute_pattern(
    run: PatternRun<'_>,
    mut instance_at: impl FnMut(&dyn RegenContext, KernelBody, u32, f64, [f64; 3]) -> Result<KernelBody>,
) -> Result<FeatureOutput> {
    if run.count == 0 {
        return Err(OpenCadError::validation("pattern count must be at least 1"));
    }

    let source = run.ctx.body_for_feature(run.source_feature)?;
    let kernel = run.ctx.kernel();

    match run.operation {
        PatternOperation::Union => {
            if run.count == 1 {
                return Ok(FeatureOutput { body: Some(source) });
            }
            let mut result = source.clone();
            for index in 1..run.count {
                let instance =
                    instance_at(run.ctx, source.clone(), index, run.spacing_m, run.direction)?;
                result = kernel.boolean(result, instance, BooleanOp::Union)?;
            }
            Ok(FeatureOutput { body: Some(result) })
        }
        PatternOperation::Cut => {
            let target_id = run
                .target_feature
                .ok_or_else(|| OpenCadError::validation("cut pattern requires target_feature"))?;
            let mut result = run.ctx.body_for_feature(target_id)?;
            for index in 0..run.count {
                let instance = if index == 0 {
                    source.clone()
                } else {
                    instance_at(run.ctx, source.clone(), index, run.spacing_m, run.direction)?
                };
                result = kernel.boolean(result, instance, BooleanOp::Subtract)?;
            }
            Ok(FeatureOutput { body: Some(result) })
        }
    }
}

use opencad_geometry::KernelBody;

impl LinearPatternFeature {
    pub fn new(
        source_feature: impl Into<String>,
        direction_m: [f64; 3],
        spacing: Length,
        count: u32,
    ) -> Self {
        Self {
            source_feature: source_feature.into(),
            direction_m,
            spacing,
            spacing_expr: None,
            count,
            operation: PatternOperation::Union,
            target_feature: None,
        }
    }

    pub fn cut(
        source_feature: impl Into<String>,
        target_feature: impl Into<String>,
        direction_m: [f64; 3],
        spacing: Length,
        count: u32,
    ) -> Self {
        Self {
            source_feature: source_feature.into(),
            direction_m,
            spacing,
            spacing_expr: None,
            count,
            operation: PatternOperation::Cut,
            target_feature: Some(target_feature.into()),
        }
    }
}

impl CircularPatternFeature {
    pub fn new(
        source_feature: impl Into<String>,
        axis_origin_m: [f64; 3],
        axis_direction_m: [f64; 3],
        count: u32,
    ) -> Self {
        Self {
            source_feature: source_feature.into(),
            axis_origin_m,
            axis_direction_m,
            count,
            operation: PatternOperation::Union,
            target_feature: None,
        }
    }
}

impl MirrorPatternFeature {
    pub fn new(
        source_feature: impl Into<String>,
        plane_origin_m: [f64; 3],
        plane_normal_m: [f64; 3],
    ) -> Self {
        Self {
            source_feature: source_feature.into(),
            plane_origin_m,
            plane_normal_m,
            plane_face_ref: None,
            operation: PatternOperation::Union,
            target_feature: None,
        }
    }

    pub fn across_face_ref(
        source_feature: impl Into<String>,
        plane_face_ref: impl Into<String>,
    ) -> Self {
        Self {
            source_feature: source_feature.into(),
            plane_origin_m: [0.0, 0.0, 0.0],
            plane_normal_m: [0.0, 0.0, 1.0],
            plane_face_ref: Some(plane_face_ref.into()),
            operation: PatternOperation::Union,
            target_feature: None,
        }
    }

    pub fn cut(
        source_feature: impl Into<String>,
        target_feature: impl Into<String>,
        plane_origin_m: [f64; 3],
        plane_normal_m: [f64; 3],
    ) -> Self {
        Self {
            source_feature: source_feature.into(),
            plane_origin_m,
            plane_normal_m,
            plane_face_ref: None,
            operation: PatternOperation::Cut,
            target_feature: Some(target_feature.into()),
        }
    }
}

fn normalize_direction(direction_m: [f64; 3]) -> Result<[f64; 3]> {
    let length = (direction_m[0] * direction_m[0]
        + direction_m[1] * direction_m[1]
        + direction_m[2] * direction_m[2])
        .sqrt();
    if length <= 1e-12 {
        return Err(OpenCadError::validation(
            "pattern axis/direction must be a non-zero vector",
        ));
    }
    Ok([
        direction_m[0] / length,
        direction_m[1] / length,
        direction_m[2] / length,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::{FeatureDefinition, FeatureNode, FeatureOutput};
    use crate::regenerate::TestRegenContext;
    use opencad_geometry::KernelBody;

    #[test]
    fn linear_pattern_rejects_zero_direction() {
        let ctx = TestRegenContext::with_body("feature:hole_mount", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:hole_row",
            "Hole Row",
            FeatureDefinition::LinearPattern(LinearPatternFeature::new(
                "feature:hole_mount",
                [0.0, 0.0, 0.0],
                Length::from_meters(0.01),
                2,
            )),
        );
        let err = LinearPatternFeatureExecutor
            .execute(&node, &ctx)
            .expect_err("direction");
        assert!(err.to_string().contains("direction"));
    }

    #[test]
    fn linear_pattern_unions_translated_instances() {
        let ctx = TestRegenContext::with_body("feature:hole_mount", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:hole_row",
            "Hole Row",
            FeatureDefinition::LinearPattern(LinearPatternFeature::new(
                "feature:hole_mount",
                [1.0, 0.0, 0.0],
                Length::from_meters(0.01),
                3,
            )),
        );
        let output = LinearPatternFeatureExecutor
            .execute(&node, &ctx)
            .expect("pattern");
        assert!(output.body.is_some());
    }

    #[test]
    fn linear_cut_pattern_subtracts_from_target() {
        let mut ctx = TestRegenContext::with_body("feature:tool", KernelBody::new(10));
        ctx.outputs.insert(
            "feature:base".into(),
            FeatureOutput {
                body: Some(KernelBody::new(20)),
            },
        );
        let node = FeatureNode::new(
            "feature:hole_row",
            "Hole Row",
            FeatureDefinition::LinearPattern(LinearPatternFeature::cut(
                "feature:tool",
                "feature:base",
                [1.0, 0.0, 0.0],
                Length::from_meters(0.01),
                2,
            )),
        );
        let output = LinearPatternFeatureExecutor
            .execute(&node, &ctx)
            .expect("cut pattern");
        assert!(output.body.is_some());
    }

    #[test]
    fn circular_pattern_rotates_instances() {
        let ctx = TestRegenContext::with_body("feature:boss", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:boss_ring",
            "Boss Ring",
            FeatureDefinition::CircularPattern(CircularPatternFeature::new(
                "feature:boss",
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                4,
            )),
        );
        let output = CircularPatternFeatureExecutor
            .execute(&node, &ctx)
            .expect("circular pattern");
        assert!(output.body.is_some());
    }

    #[test]
    fn mirror_pattern_unions_source_and_reflection() {
        let ctx = TestRegenContext::with_body("feature:boss", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:boss_pair",
            "Boss Pair",
            FeatureDefinition::MirrorPattern(MirrorPatternFeature::new(
                "feature:boss",
                [0.04, 0.0, 0.0],
                [1.0, 0.0, 0.0],
            )),
        );
        let output = MirrorPatternFeatureExecutor
            .execute(&node, &ctx)
            .expect("mirror pattern");
        assert!(output.body.is_some());
    }

    #[test]
    fn mirror_pattern_uses_plane_face_ref() {
        use crate::feature::RegenContext;
        use opencad_core::{Result, TopoRefId};
        use opencad_geometry::{FaceRefDiscovery, TopoRef};

        struct RefContext {
            inner: TestRegenContext,
            semantic_refs: Vec<TopoRef>,
            face_discoveries: Vec<FaceRefDiscovery>,
        }

        impl RegenContext for RefContext {
            fn kernel(&self) -> &dyn opencad_geometry::GeometryKernel {
                self.inner.kernel()
            }

            fn sketch_for_feature(
                &self,
                sketch_feature_id: &str,
            ) -> Result<&opencad_sketch::Sketch> {
                self.inner.sketch_for_feature(sketch_feature_id)
            }

            fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody> {
                self.inner.body_for_feature(feature_id)
            }

            fn semantic_refs(&self) -> &[TopoRef] {
                &self.semantic_refs
            }

            fn face_discoveries(&self) -> &[FaceRefDiscovery] {
                &self.face_discoveries
            }
        }

        let ctx = RefContext {
            inner: TestRegenContext::with_body("feature:boss", KernelBody::new(42)),
            semantic_refs: vec![TopoRef::face(
                TopoRefId::new("ref:face:bracket_top").expect("id"),
                "feature:extrude_base",
                "top",
            )],
            face_discoveries: vec![FaceRefDiscovery {
                kernel_face_id: 1,
                role: "top".into(),
                normal_m: [0.0, 0.0, 1.0],
                centroid_m: [0.04, 0.03, 0.006],
                feature_id: Some("feature:extrude_base".into()),
            }],
        };
        let node = FeatureNode::new(
            "feature:boss_pair",
            "Boss Pair",
            FeatureDefinition::MirrorPattern(MirrorPatternFeature::across_face_ref(
                "feature:boss",
                "ref:face:bracket_top",
            )),
        );
        let output = MirrorPatternFeatureExecutor
            .execute(&node, &ctx)
            .expect("mirror face ref");
        assert!(output.body.is_some());
    }

    #[test]
    fn mirror_cut_pattern_subtracts_from_target() {
        let mut ctx = TestRegenContext::with_body("feature:tool", KernelBody::new(10));
        ctx.outputs.insert(
            "feature:base".into(),
            FeatureOutput {
                body: Some(KernelBody::new(20)),
            },
        );
        let node = FeatureNode::new(
            "feature:hole_pair",
            "Hole Pair",
            FeatureDefinition::MirrorPattern(MirrorPatternFeature::cut(
                "feature:tool",
                "feature:base",
                [0.04, 0.0, 0.0],
                [1.0, 0.0, 0.0],
            )),
        );
        let output = MirrorPatternFeatureExecutor
            .execute(&node, &ctx)
            .expect("mirror cut");
        assert!(output.body.is_some());
    }
}
