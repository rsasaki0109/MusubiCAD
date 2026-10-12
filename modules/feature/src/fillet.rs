//! Fillet feature: round edges on an existing body (Phase 2).

use serde::{Deserialize, Serialize};

use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::FilletEdgeSelector;

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::topo_resolve::{edge_selector_for_edge_ref, edge_selector_for_face_ref};

/// Linear radius law: `radius` is the start radius, `end_radius` the end.
/// Endpoints follow lexicographically increasing model coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FilletRadiusLaw {
    Linear {
        end_radius: Length,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_radius_expr: Option<String>,
    },
}

/// Fillet feature parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilletFeature {
    /// Feature id whose body output is filleted.
    pub target_feature: String,
    pub radius: Length,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_law: Option<FilletRadiusLaw>,
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

        def.validate_selection()?;
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
            let selector =
                edge_selector_for_face_ref(ctx, face_ref, &body, FilletEdgeSelector::TopPerimeter)?;
            if matches!(selector, FilletEdgeSelector::TopPerimeter)
                && !ctx.semantic_refs().iter().any(|reference| {
                    reference.ref_id.as_str() == face_ref
                        && reference.semantic.role.as_deref() == Some("top")
                })
            {
                return Err(OpenCadError::validation(
                    "fillet face_ref did not resolve on the target body",
                ));
            }
            selector
        } else {
            def.edge_selector.clone()
        };
        let selector = if def.radius_law.is_some() {
            let discoveries = ctx.kernel().discover_body_edges(&body)?;
            if discoveries.is_empty() {
                selector
            } else {
                let edge_ref = def
                    .edge_ref
                    .as_deref()
                    .ok_or_else(|| OpenCadError::validation("variable fillet needs edge_ref"))?;
                if let Some(reference) = ctx
                    .semantic_refs()
                    .iter()
                    .find(|reference| reference.ref_id.as_str() == edge_ref)
                {
                    let has_geometric_hint =
                        reference
                            .geometric_fingerprint
                            .as_ref()
                            .is_some_and(|hint| {
                                hint.bbox_hint.is_some()
                                    || hint.kernel_edge_id.is_some_and(|id| {
                                        discoveries.iter().any(|edge| {
                                            edge.kernel_edge_id == id
                                                && reference
                                                    .semantic
                                                    .role
                                                    .as_deref()
                                                    .map_or(true, |role| role == edge.role)
                                        })
                                    })
                            });
                    if !has_geometric_hint
                        && discoveries
                            .iter()
                            .filter(|edge| {
                                reference.semantic.role.as_deref() == Some(edge.role.as_str())
                            })
                            .count()
                            > 1
                    {
                        return Err(OpenCadError::validation(
                            "Ambiguous variable fillet edge: a role-only reference requires a unique edge",
                        ));
                    }
                }
                let resolved = opencad_geometry::provenance::resolve_edge_ref_with_provenance(
                    ctx.semantic_refs(),
                    edge_ref,
                    Some(&discoveries),
                    Default::default(),
                    true,
                )?;
                let kernel_edge_id = resolved.resolved_kernel_id.ok_or_else(|| {
                    OpenCadError::validation("variable fillet edge did not resolve")
                })?;
                FilletEdgeSelector::KernelEdges {
                    kernel_edge_ids: vec![kernel_edge_id],
                }
            }
        } else {
            selector
        };
        let result = match &def.radius_law {
            Some(FilletRadiusLaw::Linear { end_radius, .. }) => {
                ctx.kernel()
                    .variable_fillet_edge(body, radius_m, end_radius.meters(), selector)?
            }
            None => ctx.kernel().fillet_edges(body, radius_m, selector)?,
        };
        Ok(FeatureOutput {
            sketch: None,
            body: Some(result),
            datum: None,
        })
    }
}

impl FilletFeature {
    pub fn validate_selection(&self) -> Result<()> {
        if self
            .face_ref
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
            || self
                .edge_ref
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
        {
            return Err(OpenCadError::validation(
                "fillet semantic references must not be empty",
            ));
        }
        if self.face_ref.is_some() && self.edge_ref.is_some() {
            return Err(OpenCadError::validation(
                "fillet must select a face or an edge, not both",
            ));
        }
        if self.radius_law.is_some() && self.edge_ref.is_none() {
            return Err(OpenCadError::validation(
                "variable fillet requires an explicit semantic edge_ref",
            ));
        }
        Ok(())
    }

    pub fn top_perimeter(
        target_feature: impl Into<String>,
        radius: Length,
        radius_expr: Option<String>,
    ) -> Self {
        Self {
            target_feature: target_feature.into(),
            radius,
            radius_expr,
            radius_law: None,
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
            radius_law: None,
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
            radius_law: None,
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
