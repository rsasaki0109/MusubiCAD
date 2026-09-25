//! Deterministic DesignPatch change-impact prediction.

use std::collections::{BTreeSet, VecDeque};

use opencad_feature::{FeatureDefinition, FeatureNode};
use opencad_graph::{parameter_names_in_expr, DesignDiff, FeatureGraph, SemanticChange};
use opencad_sketch::Sketch;
use serde::{Deserialize, Serialize};

use crate::DesignState;

pub const CHANGE_IMPACT_VERSION: &str = "opencad.change-impact.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedInputKind {
    Parameter,
    Feature,
    Constraint,
    SemanticReference,
    Assembly,
    Drawing,
    Assertion,
    Sketch,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChangedInput {
    pub kind: ChangedInputKind,
    pub id: String,
}

/// Predicted affected Feature Graph nodes before geometry execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeImpact {
    pub version: String,
    pub no_op: bool,
    pub changed_inputs: Vec<ChangedInput>,
    pub directly_affected_nodes: Vec<String>,
    pub predicted_dirty_nodes: Vec<String>,
}

impl Default for ChangeImpact {
    fn default() -> Self {
        Self {
            version: CHANGE_IMPACT_VERSION.into(),
            no_op: false,
            changed_inputs: Vec::new(),
            directly_affected_nodes: Vec::new(),
            predicted_dirty_nodes: Vec::new(),
        }
    }
}

/// Optional authoring context that does not belong to patchable DesignState.
#[derive(Debug, Clone, Copy, Default)]
pub struct ImpactContext<'a> {
    pub feature_graph: Option<&'a FeatureGraph>,
    pub sketches: &'a [Sketch],
}

pub fn predict_change_impact(
    before: &DesignState,
    after: &DesignState,
    diff: &DesignDiff,
    context: ImpactContext<'_>,
) -> ChangeImpact {
    let changed_inputs = changed_inputs(diff);
    if changed_inputs.is_empty() {
        return ChangeImpact {
            no_op: true,
            ..ChangeImpact::default()
        };
    }

    let mut direct = BTreeSet::new();
    for change in &diff.changes {
        match change {
            SemanticChange::ParameterChanged { id, .. } => {
                let parameter_name = after
                    .parameters
                    .get(id)
                    .or_else(|| before.parameters.get(id))
                    .map(|entry| entry.name.as_str());
                if let Some(name) = parameter_name {
                    for node in &after.feature_nodes {
                        if node_uses_parameter(node, name, context.sketches) {
                            direct.insert(node.id.clone());
                        }
                    }
                }
            }
            SemanticChange::FeatureAdded { id, .. }
            | SemanticChange::FeatureRemoved { id }
            | SemanticChange::FeatureModified { id, .. } => {
                direct.insert(id.clone());
            }
            SemanticChange::TopoRefAdded { ref_id, .. }
            | SemanticChange::TopoRefRemoved { ref_id }
            | SemanticChange::TopoRefModified { ref_id, .. } => {
                for node in &after.feature_nodes {
                    if serialized_value_contains(&node.definition, ref_id) {
                        direct.insert(node.id.clone());
                    }
                }
            }
            // A sketch change dirties the sketch features that consume it;
            // their downstream suffix follows from the Feature Graph.
            SemanticChange::SketchAdded { id: sketch_id }
            | SemanticChange::SketchRemoved { id: sketch_id }
            | SemanticChange::SketchEntityAdded { sketch_id, .. }
            | SemanticChange::SketchEntityRemoved { sketch_id, .. }
            | SemanticChange::SketchConstraintAdded { sketch_id, .. }
            | SemanticChange::SketchConstraintRemoved { sketch_id, .. } => {
                for node in &after.feature_nodes {
                    if matches!(
                        &node.definition,
                        FeatureDefinition::Sketch(def) if def.sketch_id == *sketch_id
                    ) {
                        direct.insert(node.id.clone());
                    }
                }
            }
            _ => {}
        }
    }

    let (directly_affected_nodes, predicted_dirty_nodes) = match context.feature_graph {
        Some(graph) => ordered_impact(graph, &direct),
        None => {
            let nodes = direct.into_iter().collect::<Vec<_>>();
            (nodes.clone(), nodes)
        }
    };

    ChangeImpact {
        no_op: false,
        changed_inputs,
        directly_affected_nodes,
        predicted_dirty_nodes,
        ..ChangeImpact::default()
    }
}

fn changed_inputs(diff: &DesignDiff) -> Vec<ChangedInput> {
    let mut inputs = BTreeSet::new();
    for change in &diff.changes {
        let (kind, id) = match change {
            SemanticChange::ParameterChanged { id, .. } => (ChangedInputKind::Parameter, id),
            SemanticChange::FeatureAdded { id, .. }
            | SemanticChange::FeatureRemoved { id }
            | SemanticChange::FeatureModified { id, .. }
            | SemanticChange::FeatureMoved { id, .. } => (ChangedInputKind::Feature, id),
            SemanticChange::ConstraintModified { id, .. } => (ChangedInputKind::Constraint, id),
            SemanticChange::TopoRefAdded { ref_id, .. }
            | SemanticChange::TopoRefRemoved { ref_id }
            | SemanticChange::TopoRefModified { ref_id, .. } => {
                (ChangedInputKind::SemanticReference, ref_id)
            }
            SemanticChange::AssemblyInstanceAdded { id }
            | SemanticChange::AssemblyInstanceRemoved { id }
            | SemanticChange::AssemblyInstanceChanged { id, .. }
            | SemanticChange::AssemblyMateAdded { id }
            | SemanticChange::AssemblyMateRemoved { id }
            | SemanticChange::AssemblyMateChanged { id, .. }
            | SemanticChange::AssemblyConnectorAdded { id }
            | SemanticChange::AssemblyConnectorRemoved { id }
            | SemanticChange::AssemblyConnectorChanged { id, .. } => {
                (ChangedInputKind::Assembly, id)
            }
            SemanticChange::DrawingSheetAdded { id }
            | SemanticChange::DrawingSheetRemoved { id }
            | SemanticChange::DrawingSheetChanged { id, .. }
            | SemanticChange::DrawingViewAdded { id }
            | SemanticChange::DrawingViewRemoved { id }
            | SemanticChange::DrawingViewChanged { id, .. } => (ChangedInputKind::Drawing, id),
            SemanticChange::AssertionAdded { id }
            | SemanticChange::AssertionRemoved { id }
            | SemanticChange::AssertionChanged { id, .. } => (ChangedInputKind::Assertion, id),
            SemanticChange::SketchAdded { id } | SemanticChange::SketchRemoved { id } => {
                (ChangedInputKind::Sketch, id)
            }
            SemanticChange::SketchEntityAdded { sketch_id, .. }
            | SemanticChange::SketchEntityRemoved { sketch_id, .. }
            | SemanticChange::SketchConstraintAdded { sketch_id, .. }
            | SemanticChange::SketchConstraintRemoved { sketch_id, .. } => {
                (ChangedInputKind::Sketch, sketch_id)
            }
            SemanticChange::MassChanged { .. } | SemanticChange::BboxChanged { .. } => continue,
        };
        inputs.insert(ChangedInput {
            kind,
            id: id.clone(),
        });
    }
    inputs.into_iter().collect()
}

fn node_uses_parameter(node: &FeatureNode, name: &str, sketches: &[Sketch]) -> bool {
    if serialized_value_uses_parameter(&node.definition, name) {
        return true;
    }
    let FeatureDefinition::Sketch(definition) = &node.definition else {
        return false;
    };
    sketches
        .iter()
        .find(|sketch| sketch.id.as_str() == definition.sketch_id)
        .is_some_and(|sketch| serialized_value_uses_parameter(sketch, name))
}

pub(crate) fn serialized_value_uses_parameter(value: &impl Serialize, name: &str) -> bool {
    serde_json::to_value(value).ok().is_some_and(|value| {
        json_strings(&value).any(|text| {
            text == name
                || parameter_names_in_expr(text)
                    .iter()
                    .any(|item| item == name)
        })
    })
}

fn serialized_value_contains(value: &impl Serialize, needle: &str) -> bool {
    serde_json::to_value(value)
        .ok()
        .is_some_and(|value| json_strings(&value).any(|text| text == needle))
}

fn json_strings(value: &serde_json::Value) -> Box<dyn Iterator<Item = &str> + '_> {
    match value {
        serde_json::Value::String(text) => Box::new(std::iter::once(text.as_str())),
        serde_json::Value::Array(values) => {
            Box::new(values.iter().flat_map(|value| json_strings(value)))
        }
        serde_json::Value::Object(values) => {
            Box::new(values.values().flat_map(|value| json_strings(value)))
        }
        _ => Box::new(std::iter::empty()),
    }
}

fn ordered_impact(graph: &FeatureGraph, direct: &BTreeSet<String>) -> (Vec<String>, Vec<String>) {
    let mut dirty = direct.clone();
    let mut queue = direct.iter().cloned().collect::<VecDeque<_>>();
    while let Some(source) = queue.pop_front() {
        for edge in graph
            .dependency_edges()
            .iter()
            .filter(|edge| edge.source == source)
        {
            if dirty.insert(edge.target.clone()) {
                queue.push_back(edge.target.clone());
            }
        }
    }
    let order = graph
        .recompute_order()
        .unwrap_or_else(|_| graph.ordered_ids().to_vec());
    let directly_affected_nodes = order
        .iter()
        .filter(|id| direct.contains(*id))
        .cloned()
        .collect();
    let predicted_dirty_nodes = order.into_iter().filter(|id| dirty.contains(id)).collect();
    (directly_affected_nodes, predicted_dirty_nodes)
}
