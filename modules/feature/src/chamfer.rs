//! Chamfer feature: bevel edges on an existing body (Phase 2).

use serde::{Deserialize, Serialize};

use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::FilletEdgeSelector;

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::topo_resolve::{edge_selector_for_edge_ref, edge_selector_for_face_ref};

/// Chamfer feature parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChamferFeature {
    /// Feature id whose body output is chamfered.
    pub target_feature: String,
    pub distance: Length,
    /// Parametric distance expression resolved before regeneration (e.g. `chamfer_distance`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_expr: Option<String>,
    /// Optional persisted face ref (e.g. `ref:face:bracket_top`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub face_ref: Option<String>,
    /// Optional persisted edge ref (e.g. `ref:edge:bracket_top_front`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_ref: Option<String>,
    #[serde(default)]
    pub edge_selector: FilletEdgeSelector,
}

/// Chamfer feature executor wired to `GeometryKernel::chamfer_edges`.
#[derive(Debug, Default)]
pub struct ChamferFeatureExecutor;

impl Feature for ChamferFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "chamfer"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Chamfer(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected chamfer feature, got {}",
                node.definition.feature_type()
            )));
        };

        let body = ctx.body_for_feature(def.target_feature.as_str())?;
        let distance_m = def.distance.meters();
        let selector = if let Some(ref edge_ref) = def.edge_ref {
            edge_selector_for_edge_ref(
                ctx,
                edge_ref,
                def.target_feature.as_str(),
                def.edge_selector.clone(),
            )?
        } else if let Some(ref face_ref) = def.face_ref {
            edge_selector_for_face_ref(ctx, face_ref, def.edge_selector.clone())?
        } else {
            def.edge_selector.clone()
        };
        let result = ctx.kernel().chamfer_edges(body, distance_m, selector)?;
        Ok(FeatureOutput { body: Some(result) })
    }
}

impl ChamferFeature {
    pub fn top_perimeter(
        target_feature: impl Into<String>,
        distance: Length,
        distance_expr: Option<String>,
    ) -> Self {
        Self {
            target_feature: target_feature.into(),
            distance,
            distance_expr,
            face_ref: None,
            edge_ref: None,
            edge_selector: FilletEdgeSelector::TopPerimeter,
        }
    }

    pub fn on_face_ref(
        target_feature: impl Into<String>,
        face_ref: impl Into<String>,
        distance: Length,
        distance_expr: Option<String>,
    ) -> Self {
        Self {
            target_feature: target_feature.into(),
            distance,
            distance_expr,
            face_ref: Some(face_ref.into()),
            edge_ref: None,
            edge_selector: FilletEdgeSelector::TopPerimeter,
        }
    }

    pub fn on_edge_ref(
        target_feature: impl Into<String>,
        edge_ref: impl Into<String>,
        distance: Length,
        distance_expr: Option<String>,
    ) -> Self {
        Self {
            target_feature: target_feature.into(),
            distance,
            distance_expr,
            face_ref: None,
            edge_ref: Some(edge_ref.into()),
            edge_selector: FilletEdgeSelector::TopPerimeter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::{FeatureDefinition, FeatureNode};
    use crate::regenerate::TestRegenContext;
    use opencad_geometry::KernelBody;

    #[test]
    fn chamfer_executor_returns_body() {
        let ctx = TestRegenContext::with_body("feature:base", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:chamfer",
            "Top Chamfer",
            FeatureDefinition::Chamfer(ChamferFeature::top_perimeter(
                "feature:base",
                Length::from_meters(0.0005),
                None,
            )),
        );
        let executor = ChamferFeatureExecutor;
        let output = executor.execute(&node, &ctx).expect("execute");
        assert!(output.body.is_some());
    }

    #[test]
    fn chamfer_rejects_non_positive_distance() {
        let ctx = TestRegenContext::with_body("feature:base", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:chamfer",
            "Top Chamfer",
            FeatureDefinition::Chamfer(ChamferFeature::top_perimeter(
                "feature:base",
                Length::from_meters(0.0),
                None,
            )),
        );
        let executor = ChamferFeatureExecutor;
        let err = executor.execute(&node, &ctx).expect_err("distance");
        assert!(err.to_string().contains("positive"));
    }
}
