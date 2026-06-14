//! AI-readable design summaries (Task-147+).

use opencad_core::Result;
use opencad_feature::FeatureNode;
use opencad_graph::{evaluate_param_graph, FeatureGraph, ParamGraph};
use serde::{Deserialize, Serialize};

use crate::query::{feature_order, list_features, list_parameters, ParameterInfo};

/// In-memory explain parameters.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExplainParams {
    pub parameters: ParamGraph,
    pub feature_nodes: Vec<FeatureNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_graph: Option<FeatureGraph>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sketch_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureSummary {
    pub id: String,
    pub name: String,
    pub feature_type: String,
    pub suppressed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignExplanation {
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_name: Option<String>,
    pub parameter_count: usize,
    pub feature_count: usize,
    pub sketch_count: usize,
    pub parameters: Vec<ParameterInfo>,
    pub features: Vec<FeatureSummary>,
    pub feature_order: Vec<String>,
}

pub fn explain_design(params: &ExplainParams) -> Result<DesignExplanation> {
    let values = evaluate_param_graph(&params.parameters).ok();
    let parameters = list_parameters(&params.parameters, values.as_ref());
    let features: Vec<FeatureSummary> = list_features(&params.feature_nodes)
        .into_iter()
        .map(|item| FeatureSummary {
            id: item.id,
            name: item.name,
            feature_type: item.feature_type,
            suppressed: item.suppressed,
        })
        .collect();
    let feature_order = feature_order(&params.feature_graph, &params.feature_nodes)?;
    let sketch_count = params.sketch_count.unwrap_or_else(|| {
        params
            .feature_nodes
            .iter()
            .filter(|node| node.definition.feature_type() == "sketch")
            .count()
    });

    let summary = build_summary(
        params.document_name.as_deref(),
        &parameters,
        &features,
        &feature_order,
        sketch_count,
    );

    Ok(DesignExplanation {
        summary,
        document_name: params.document_name.clone(),
        parameter_count: parameters.len(),
        feature_count: features.len(),
        sketch_count,
        parameters,
        features,
        feature_order,
    })
}

fn build_summary(
    document_name: Option<&str>,
    parameters: &[ParameterInfo],
    features: &[FeatureSummary],
    feature_order: &[String],
    sketch_count: usize,
) -> String {
    let title = document_name.unwrap_or("design");
    let param_bits: Vec<String> = parameters
        .iter()
        .map(|param| {
            if let Some(value_m) = param.value_m {
                format!("{}={} m ({})", param.name, value_m, param.expr)
            } else {
                format!("{}={}", param.name, param.expr)
            }
        })
        .collect();
    let feature_bits: Vec<String> = feature_order
        .iter()
        .filter_map(|id| features.iter().find(|feature| &feature.id == id))
        .map(|feature| {
            if feature.suppressed {
                format!("{}:{} (suppressed)", feature.id, feature.feature_type)
            } else {
                format!("{}:{}", feature.id, feature.feature_type)
            }
        })
        .collect();

    format!(
        "{title}: {param_count} parameters, {feature_count} features, {sketch_count} sketches. \
         Parameters: [{params}]. Feature chain: [{features}].",
        param_count = parameters.len(),
        feature_count = features.len(),
        sketch_count = sketch_count,
        params = param_bits.join(", "),
        features = feature_bits.join(" -> "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_feature::bracket_with_hole;
    use opencad_graph::bracket_parameters;

    #[test]
    fn explains_bracket_with_hole() {
        let part = bracket_with_hole().expect("model");
        let explanation = explain_design(&ExplainParams {
            parameters: bracket_parameters(),
            feature_nodes: part.nodes.into_values().collect(),
            feature_graph: Some(part.graph),
            sketch_count: Some(2),
            document_name: Some("Bracket with Hole".into()),
        })
        .expect("explain");

        assert_eq!(explanation.parameter_count, 8);
        assert_eq!(explanation.feature_count, 4);
        assert_eq!(explanation.sketch_count, 2);
        assert!(explanation.summary.contains("Bracket with Hole"));
        assert!(explanation.summary.contains("feature:extrude_base:extrude"));
        assert_eq!(explanation.feature_order.len(), 4);
    }
}
