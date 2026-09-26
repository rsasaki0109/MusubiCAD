//! Intent inspector (MCAD-P6-006): one query answering "what drives this?"
//! and "what will this change?" for a parameter or a semantic reference.
//!
//! The answers are static: they read the Design Graph only and never run
//! the geometry kernel.  Every list is sorted, or in Feature Graph
//! recompute order, so results are deterministic.

use std::collections::{BTreeSet, VecDeque};

use opencad_core::{AssertionKind, Result};
use opencad_graph::{evaluate_param_graph, DependencyEdge};
use opencad_sketch::Sketch;
use serde::{Deserialize, Serialize};

use crate::impact::{node_uses_parameter, ordered_impact, serialized_value_contains};
use crate::query::{get_semantic_ref, parameter_info, QueryParams, SemanticRefInfo};
use crate::ParameterInfo;

/// Everything a parameter drives and is driven by.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterIntent {
    pub parameter: ParameterInfo,
    /// Parameters this one's expression depends on, transitively.
    pub driven_by: Vec<String>,
    /// Parameters whose expressions depend on this one, transitively.
    pub drives_parameters: Vec<String>,
    /// Sketches whose entities or constraints name this parameter or one it
    /// drives.
    pub sketches: Vec<String>,
    /// Features that read this parameter (or one it drives) directly.
    pub directly_affected_features: Vec<String>,
    /// Features an edit would regenerate, in recompute order.
    pub predicted_dirty_features: Vec<String>,
    /// Assertions an edit would re-evaluate: parameter ranges on the affected
    /// parameters, and geometric assertions when any feature regenerates.
    pub assertions: Vec<String>,
}

/// A semantic reference's origin and everything that consumes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceIntent {
    pub reference: SemanticRefInfo,
    /// Features whose definitions name this reference.
    pub consuming_features: Vec<String>,
    /// Assembly mates whose entities name this reference.
    pub consuming_mates: Vec<String>,
    /// `required_reference` assertions on this reference.
    pub assertions: Vec<String>,
    /// Features a change to the reference would regenerate, in recompute
    /// order.
    pub predicted_dirty_features: Vec<String>,
}

pub fn inspect_parameter(params: &QueryParams, id: &str) -> Result<ParameterIntent> {
    let values = evaluate_param_graph(&params.parameters).ok();
    let parameter = parameter_info(&params.parameters, id, values.as_ref())?;
    let edges = params.parameters.dependency_edges();
    let driven_by = reachable(id, edges, Direction::Upstream);
    let drives_parameters = reachable(id, edges, Direction::Downstream);

    // Names of this parameter and everything it drives: an edit to it
    // changes all of their values.
    let names = std::iter::once(id)
        .chain(drives_parameters.iter().map(String::as_str))
        .filter_map(|id| params.parameters.get(id))
        .map(|entry| entry.name.clone())
        .collect::<BTreeSet<_>>();

    let sketches = params
        .sketches
        .iter()
        .filter(|sketch| {
            names
                .iter()
                .any(|name| crate::impact::serialized_value_uses_parameter(*sketch, name))
        })
        .map(|sketch| sketch.id.as_str().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let direct = features_using(params, &names, &params.sketches);
    let (directly_affected_features, predicted_dirty_features) = order(params, &direct);

    let assertions = params
        .assertions
        .iter()
        .filter(|assertion| match &assertion.kind {
            AssertionKind::ParameterRange { parameter_name, .. } => names.contains(parameter_name),
            AssertionKind::MassRange { .. }
            | AssertionKind::BoundingBoxWithin { .. }
            | AssertionKind::BodyCount { .. } => !predicted_dirty_features.is_empty(),
            AssertionKind::RequiredReference { .. }
            | AssertionKind::AssemblyDofAtMost { .. }
            | AssertionKind::InterferenceAtMost { .. } => false,
        })
        .map(|assertion| assertion.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(ParameterIntent {
        parameter,
        driven_by,
        drives_parameters,
        sketches,
        directly_affected_features,
        predicted_dirty_features,
        assertions,
    })
}

pub fn inspect_reference(params: &QueryParams, ref_id: &str) -> Result<ReferenceIntent> {
    let reference = get_semantic_ref(&params.semantic_refs, ref_id)?;
    let direct = params
        .feature_nodes
        .iter()
        .filter(|node| serialized_value_contains(&node.definition, ref_id))
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    let (consuming_features, predicted_dirty_features) = order(params, &direct);
    let consuming_mates = params
        .assembly
        .iter()
        .flat_map(|assembly| &assembly.mates)
        .filter(|mate| serialized_value_contains(*mate, ref_id))
        .map(|mate| mate.id.as_str().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let assertions = params
        .assertions
        .iter()
        .filter(|assertion| {
            matches!(
                &assertion.kind,
                AssertionKind::RequiredReference { ref_id: target } if target == ref_id
            )
        })
        .map(|assertion| assertion.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(ReferenceIntent {
        reference,
        consuming_features,
        consuming_mates,
        assertions,
        predicted_dirty_features,
    })
}

fn features_using(
    params: &QueryParams,
    names: &BTreeSet<String>,
    sketches: &[Sketch],
) -> BTreeSet<String> {
    params
        .feature_nodes
        .iter()
        .filter(|node| {
            names
                .iter()
                .any(|name| node_uses_parameter(node, name, sketches))
        })
        .map(|node| node.id.clone())
        .collect()
}

/// (direct, dirty) in recompute order when the Feature Graph is known.
fn order(params: &QueryParams, direct: &BTreeSet<String>) -> (Vec<String>, Vec<String>) {
    match &params.feature_graph {
        Some(graph) => ordered_impact(graph, direct),
        None => {
            let nodes = direct.iter().cloned().collect::<Vec<_>>();
            (nodes.clone(), nodes)
        }
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Upstream,
    Downstream,
}

/// Parameter ids reachable from `id` along dependency edges, sorted.
fn reachable(id: &str, edges: &[DependencyEdge], direction: Direction) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([id.to_string()]);
    while let Some(current) = queue.pop_front() {
        for edge in edges {
            let (from, to) = match direction {
                Direction::Upstream => (&edge.target, &edge.source),
                Direction::Downstream => (&edge.source, &edge.target),
            };
            if *from == current && to != id && seen.insert(to.clone()) {
                queue.push_back(to.clone());
            }
        }
    }
    seen.into_iter().collect()
}
