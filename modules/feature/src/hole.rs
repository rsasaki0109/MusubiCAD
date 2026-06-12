//! Hole feature: extruded profile cut (Task-093+).

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::ExtrudeExtent;

use crate::extrude::{ExtrudeFeature, ExtrudeFeatureExecutor};
use crate::feature::{
    Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext,
};
use crate::topo_resolve::target_feature_for_face_ref;

/// Hole feature parameters (sketch circle/profile cut into a body).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoleFeature {
    pub sketch_feature: String,
    pub profile_ref: String,
    pub depth: ExtrudeExtent,
    pub target_feature: String,
    /// Optional persisted face ref (e.g. `ref:face:bracket_top`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub face_ref: Option<String>,
    /// Parametric depth expression resolved before regeneration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_expr: Option<String>,
}

/// Hole executor implemented as an extrude cut.
#[derive(Debug, Default)]
pub struct HoleFeatureExecutor;

impl Feature for HoleFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "hole"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Hole(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected hole feature, got {}",
                node.definition.feature_type()
            )));
        };

        let target_feature = if let Some(ref face_ref) = def.face_ref {
            target_feature_for_face_ref(ctx, face_ref, def.target_feature.as_str())?
        } else {
            def.target_feature.clone()
        };

        let extrude_node = FeatureNode::new(
            node.id.clone(),
            node.name.clone(),
            FeatureDefinition::Extrude(ExtrudeFeature {
                sketch_feature: def.sketch_feature.clone(),
                profile_ref: def.profile_ref.clone(),
                extent: def.depth.clone(),
                operation: opencad_geometry::ExtrudeOperation::Cut,
                length_expr: def.depth_expr.clone(),
                target_feature: Some(target_feature),
            }),
        );
        ExtrudeFeatureExecutor.execute(&extrude_node, ctx)
    }
}

impl HoleFeature {
    pub fn through(
        sketch_feature: impl Into<String>,
        profile_ref: impl Into<String>,
        depth: ExtrudeExtent,
        target_feature: impl Into<String>,
    ) -> Self {
        Self {
            sketch_feature: sketch_feature.into(),
            profile_ref: profile_ref.into(),
            depth,
            target_feature: target_feature.into(),
            face_ref: None,
            depth_expr: None,
        }
    }

    pub fn on_face_ref(
        sketch_feature: impl Into<String>,
        profile_ref: impl Into<String>,
        depth: ExtrudeExtent,
        target_feature: impl Into<String>,
        face_ref: impl Into<String>,
    ) -> Self {
        Self {
            sketch_feature: sketch_feature.into(),
            profile_ref: profile_ref.into(),
            depth,
            target_feature: target_feature.into(),
            face_ref: Some(face_ref.into()),
            depth_expr: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::{FeatureDefinition, FeatureNode, RegenContext};
    use crate::regenerate::TestRegenContext;
    use opencad_core::TopoRefId;
    use opencad_geometry::{ExtrudeExtent, KernelBody, TopoRef};
    use opencad_core::Length;

    struct RefContext {
        inner: TestRegenContext,
        semantic_refs: Vec<TopoRef>,
    }

    impl RegenContext for RefContext {
        fn kernel(&self) -> &dyn opencad_geometry::GeometryKernel {
            self.inner.kernel()
        }

        fn sketch_for_feature(&self, sketch_feature_id: &str) -> Result<&opencad_sketch::Sketch> {
            self.inner.sketch_for_feature(sketch_feature_id)
        }

        fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody> {
            self.inner.body_for_feature(feature_id)
        }

        fn semantic_refs(&self) -> &[TopoRef] {
            &self.semantic_refs
        }
    }

    #[test]
    fn hole_face_ref_target_mismatch_is_rejected() {
        let ctx = RefContext {
            inner: TestRegenContext::with_body("feature:extrude_base", KernelBody::new(42)),
            semantic_refs: vec![TopoRef::face(
                TopoRefId::new("ref:face:bracket_top").expect("id"),
                "feature:extrude_base",
                "top",
            )],
        };
        let node = FeatureNode::new(
            "feature:hole_mount",
            "Mounting Hole",
            FeatureDefinition::Hole(HoleFeature::on_face_ref(
                "feature:sketch_hole",
                "sketch:hole/profile:outer",
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                "feature:hole_mount",
                "ref:face:bracket_top",
            )),
        );
        let executor = HoleFeatureExecutor;
        let err = executor.execute(&node, &ctx).expect_err("mismatch");
        assert!(err.to_string().contains("belongs to"));
    }
}
