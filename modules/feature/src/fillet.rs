//! Fillet feature: round edges on an existing body (Phase 2).

use serde::{Deserialize, Serialize};

use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::FilletEdgeSelector;

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::topo_resolve::{edge_selector_for_edge_ref, edge_selector_for_face_ref};

/// Fillet feature parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilletFeature {
    /// Feature id whose body output is filleted.
    pub target_feature: String,
    pub radius: Length,
    /// Parametric radius expression resolved before regeneration (e.g. `fillet_radius`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_expr: Option<String>,
    /// Optional persisted face ref (e.g. `ref:face:bracket_top`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub face_ref: Option<String>,
    /// Optional persisted edge ref (e.g. `ref:edge:bracket_top_front`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_ref: Option<String>,
    #[serde(default)]
    pub edge_selector: FilletEdgeSelector,
}

/// Fillet feature executor wired to `GeometryKernel::fillet_edges`.
#[derive(Debug, Default)]
pub struct FilletFeatureExecutor;

impl Feature for FilletFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "fillet"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Fillet(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected fillet feature, got {}",
                node.definition.feature_type()
            )));
        };

        let body = ctx.body_for_feature(def.target_feature.as_str())?;
        let radius_m = def.radius.meters();
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
        let result = ctx.kernel().fillet_edges(body, radius_m, selector)?;
        Ok(FeatureOutput { body: Some(result) })
    }
}

impl FilletFeature {
    pub fn top_perimeter(
        target_feature: impl Into<String>,
        radius: Length,
        radius_expr: Option<String>,
    ) -> Self {
        Self {
            target_feature: target_feature.into(),
            radius,
            radius_expr,
            face_ref: None,
            edge_ref: None,
            edge_selector: FilletEdgeSelector::TopPerimeter,
        }
    }

    pub fn on_face_ref(
        target_feature: impl Into<String>,
        face_ref: impl Into<String>,
        radius: Length,
        radius_expr: Option<String>,
    ) -> Self {
        Self {
            target_feature: target_feature.into(),
            radius,
            radius_expr,
            face_ref: Some(face_ref.into()),
            edge_ref: None,
            edge_selector: FilletEdgeSelector::TopPerimeter,
        }
    }

    pub fn on_edge_ref(
        target_feature: impl Into<String>,
        edge_ref: impl Into<String>,
        radius: Length,
        radius_expr: Option<String>,
    ) -> Self {
        Self {
            target_feature: target_feature.into(),
            radius,
            radius_expr,
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
    fn fillet_executor_returns_body() {
        let ctx = TestRegenContext::with_body("feature:base", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:fillet",
            "Top Fillet",
            FeatureDefinition::Fillet(FilletFeature::top_perimeter(
                "feature:base",
                Length::from_meters(0.001),
                None,
            )),
        );
        let executor = FilletFeatureExecutor;
        let output = executor.execute(&node, &ctx).expect("execute");
        assert!(output.body.is_some());
    }

    #[test]
    fn fillet_rejects_non_positive_radius() {
        let ctx = TestRegenContext::with_body("feature:base", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:fillet",
            "Top Fillet",
            FeatureDefinition::Fillet(FilletFeature::top_perimeter(
                "feature:base",
                Length::from_meters(0.0),
                None,
            )),
        );
        let executor = FilletFeatureExecutor;
        let err = executor.execute(&node, &ctx).expect_err("radius");
        assert!(err.to_string().contains("positive"));
    }
}
