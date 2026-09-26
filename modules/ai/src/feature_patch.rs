//! Structural feature and semantic-reference operations for `DesignPatch`
//! (ADR-013 slice 4).
//!
//! Feature operations edit `feature_nodes` and the authored display order in
//! `DesignState::feature_order`.  The Feature Graph itself is never authored:
//! final-state validation derives it with [`derive_feature_graph`], which
//! rejects unknown inputs, cycles, and a display order that places a feature
//! before one of its inputs.

use std::collections::BTreeSet;

use opencad_core::{AssertionKind, OpenCadError, Result};
use opencad_feature::{derive_feature_graph, FeatureDefinition, FeatureNode};
use opencad_graph::{parameter_names_in_expr, FeatureGraph, SemanticChange};
use opencad_sketch::Workplane;
use serde::{Deserialize, Serialize};

use crate::patch::{validate_stable_id, PatchOperation};
use crate::sketch_patch::validate_profile_consumers;
use crate::DesignState;

/// Where a feature is placed in the authored display order.
///
/// Serialized as `{"at": "start"}`, `{"at": "end"}`, `{"after": "<id>"}`, or
/// `{"before": "<id>"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeaturePosition {
    At(FeatureOrderEnd),
    After(String),
    Before(String),
}

/// An end of the feature display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureOrderEnd {
    Start,
    End,
}

impl FeaturePosition {
    /// Append after every existing feature.
    pub fn end() -> Self {
        Self::At(FeatureOrderEnd::End)
    }

    fn index_in(&self, order: &[String]) -> Result<usize> {
        let anchor = |id: &str| {
            order.iter().position(|item| item == id).ok_or_else(|| {
                OpenCadError::validation(format!("position anchor '{id}' does not exist"))
            })
        };
        match self {
            Self::At(FeatureOrderEnd::Start) => Ok(0),
            Self::At(FeatureOrderEnd::End) => Ok(order.len()),
            Self::After(id) => anchor(id).map(|index| index + 1),
            Self::Before(id) => anchor(id),
        }
    }
}

/// Whether an operation changes Feature Graph inputs (ADR-013 slice 4).
pub(crate) fn changes_feature_graph(operation: &PatchOperation) -> bool {
    matches!(
        operation,
        PatchOperation::AddFeature { .. }
            | PatchOperation::RemoveFeature { .. }
            | PatchOperation::MoveFeature { .. }
            | PatchOperation::SetFeatureSuppressed { .. }
            | PatchOperation::ReplaceFeatureDefinition { .. }
            | PatchOperation::AddSemanticRef { .. }
            | PatchOperation::RemoveSemanticRef { .. }
    )
}

fn node_index(nodes: &[FeatureNode], id: &str) -> Result<usize> {
    nodes
        .iter()
        .position(|node| node.id == id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown feature '{id}'")))
}

/// Apply feature and semantic-reference operations in order with local
/// checks only.
pub(crate) fn apply_feature_operations(
    operations: &[PatchOperation],
    state: &mut DesignState,
) -> Result<()> {
    if !operations.iter().any(changes_feature_graph) {
        return Ok(());
    }
    // In-memory callers that do not transport the display order cannot
    // place features; refuse rather than invent an order.
    if state.feature_order.len() != state.feature_nodes.len() {
        return Err(OpenCadError::validation(
            "feature operations require the complete design state, including feature order",
        ));
    }
    let keep_sorted = state
        .feature_nodes
        .windows(2)
        .all(|pair| pair[0].id <= pair[1].id);
    let mut removed_features = BTreeSet::new();
    let mut removed_refs = BTreeSet::new();

    for operation in operations {
        match operation {
            PatchOperation::AddFeature { node, position } => {
                validate_stable_id(&node.id, "feature")?;
                if node.name.trim().is_empty() {
                    return Err(OpenCadError::validation(format!(
                        "feature '{}' must have a non-empty name",
                        node.id
                    )));
                }
                if removed_features.contains(node.id.as_str()) {
                    return Err(OpenCadError::validation(format!(
                        "feature id '{}' was removed earlier in this patch and cannot be reused",
                        node.id
                    )));
                }
                if state.feature_nodes.iter().any(|item| item.id == node.id) {
                    return Err(OpenCadError::validation(format!(
                        "feature '{}' already exists",
                        node.id
                    )));
                }
                let index = position.index_in(&state.feature_order)?;
                state.feature_order.insert(index, node.id.clone());
                state.feature_nodes.push(node.clone());
            }
            PatchOperation::RemoveFeature { id } => {
                let index = node_index(&state.feature_nodes, id)?;
                state.feature_nodes.remove(index);
                state.feature_order.retain(|item| item != id);
                removed_features.insert(id.as_str());
            }
            PatchOperation::MoveFeature { id, position } => {
                node_index(&state.feature_nodes, id)?;
                if matches!(position, FeaturePosition::After(anchor) | FeaturePosition::Before(anchor) if anchor == id)
                {
                    return Err(OpenCadError::validation(format!(
                        "feature '{id}' cannot be positioned relative to itself"
                    )));
                }
                state.feature_order.retain(|item| item != id);
                let index = position.index_in(&state.feature_order)?;
                state.feature_order.insert(index, id.clone());
            }
            PatchOperation::SetFeatureSuppressed { id, suppressed } => {
                let index = node_index(&state.feature_nodes, id)?;
                state.feature_nodes[index].suppressed = *suppressed;
            }
            PatchOperation::ReplaceFeatureDefinition { id, definition } => {
                let index = node_index(&state.feature_nodes, id)?;
                let node = &mut state.feature_nodes[index];
                if node.definition.feature_type() != definition.feature_type() {
                    return Err(OpenCadError::validation(format!(
                        "feature '{id}' is a '{}'; a definition of type '{}' requires remove_feature and add_feature",
                        node.definition.feature_type(),
                        definition.feature_type()
                    )));
                }
                node.definition = definition.clone();
            }
            PatchOperation::AddSemanticRef { topo_ref } => {
                let ref_id = topo_ref.ref_id.as_str();
                if removed_refs.contains(ref_id) {
                    return Err(OpenCadError::validation(format!(
                        "semantic reference id '{ref_id}' was removed earlier in this patch and cannot be reused"
                    )));
                }
                if state
                    .semantic_refs
                    .iter()
                    .any(|item| item.ref_id.as_str() == ref_id)
                {
                    return Err(OpenCadError::validation(format!(
                        "semantic reference '{ref_id}' already exists"
                    )));
                }
                state.semantic_refs.push(topo_ref.as_ref().clone());
            }
            PatchOperation::RemoveSemanticRef { ref_id } => {
                let index = state
                    .semantic_refs
                    .iter()
                    .position(|item| item.ref_id.as_str() == ref_id)
                    .ok_or_else(|| {
                        OpenCadError::validation(format!("unknown semantic reference '{ref_id}'"))
                    })?;
                state.semantic_refs.remove(index);
                removed_refs.insert(ref_id.as_str());
            }
            _ => {}
        }
    }

    if keep_sorted {
        state
            .feature_nodes
            .sort_by(|left, right| left.id.cmp(&right.id));
    }
    Ok(())
}

/// Derive the Feature Graph of a complete state, if it carries a display
/// order for every feature.
pub(crate) fn derived_feature_graph(state: &DesignState) -> Option<Result<FeatureGraph>> {
    (state.feature_order.len() == state.feature_nodes.len()).then(|| {
        derive_feature_graph(
            &state.feature_nodes,
            &state.feature_order,
            &state.sketches,
            &state.semantic_refs,
        )
    })
}

/// Final-state checks for feature and semantic-reference operations.
pub(crate) fn validate_feature_candidate(
    operations: &[PatchOperation],
    after: &DesignState,
    failures: &mut BTreeSet<String>,
) {
    if !operations.iter().any(changes_feature_graph) {
        return;
    }
    let feature_ids: BTreeSet<&str> = after
        .feature_nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    let ref_ids: BTreeSet<&str> = after
        .semantic_refs
        .iter()
        .map(|topo_ref| topo_ref.ref_id.as_str())
        .collect();
    let mut dependency_failure = false;

    for operation in operations {
        match operation {
            PatchOperation::RemoveFeature { id } if !feature_ids.contains(id.as_str()) => {
                let mut dependents = BTreeSet::new();
                for node in &after.feature_nodes {
                    for (field, input) in node.definition.feature_inputs() {
                        if input == id {
                            dependents.insert(format!("feature {} ({field})", node.id));
                        }
                    }
                }
                for topo_ref in &after.semantic_refs {
                    if topo_ref.semantic.created_by == *id {
                        dependents.insert(format!("semantic reference {}", topo_ref.ref_id));
                    }
                }
                if !dependents.is_empty() {
                    dependency_failure = true;
                    failures.insert(format!(
                        "cannot remove feature '{id}': still used by {}",
                        dependents.into_iter().collect::<Vec<_>>().join(", ")
                    ));
                }
            }
            PatchOperation::RemoveSemanticRef { ref_id } if !ref_ids.contains(ref_id.as_str()) => {
                let mut dependents = BTreeSet::new();
                for node in &after.feature_nodes {
                    for (field, input) in node.definition.reference_inputs() {
                        if input == ref_id {
                            dependents.insert(format!("feature {} ({field})", node.id));
                        }
                    }
                }
                for sketch in &after.sketches {
                    if matches!(&sketch.workplane, Workplane::FaceRef { face_ref } if face_ref == ref_id)
                    {
                        dependents.insert(format!("sketch {} (workplane)", sketch.id));
                    }
                }
                for assertion in &after.assertions {
                    if matches!(&assertion.kind, AssertionKind::RequiredReference { ref_id: target } if target == ref_id)
                    {
                        dependents.insert(format!("assertion {}", assertion.id));
                    }
                }
                if !dependents.is_empty() {
                    dependency_failure = true;
                    failures.insert(format!(
                        "cannot remove semantic reference '{ref_id}': still used by {}",
                        dependents.into_iter().collect::<Vec<_>>().join(", ")
                    ));
                }
            }
            _ => {}
        }
    }

    // Inputs of added or replaced features must resolve in the final state.
    let checked: BTreeSet<&str> = operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::AddFeature { node, .. } => Some(node.id.as_str()),
            PatchOperation::ReplaceFeatureDefinition { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .filter(|id| feature_ids.contains(id))
        .collect();
    for node in after
        .feature_nodes
        .iter()
        .filter(|node| checked.contains(node.id.as_str()))
    {
        check_feature_inputs(node, after, failures);
    }
    validate_profile_consumers(
        after,
        |node, _| checked.contains(node.id.as_str()),
        failures,
    );

    for operation in operations {
        let PatchOperation::AddSemanticRef { topo_ref } = operation else {
            continue;
        };
        let created_by = topo_ref.semantic.created_by.as_str();
        if ref_ids.contains(topo_ref.ref_id.as_str()) && !feature_ids.contains(created_by) {
            failures.insert(format!(
                "semantic reference '{}' is created by unknown feature '{created_by}'",
                topo_ref.ref_id
            ));
        }
    }

    // Dependency failures above already name every dangling input; the
    // derivation would only repeat the first of them.
    if !dependency_failure {
        if let Some(Err(error)) = derived_feature_graph(after) {
            failures.insert(error.to_string());
        }
    }
}

/// Check that a feature's sketch, semantic references, and expression
/// parameters exist in `state`.
pub(crate) fn check_feature_inputs(
    node: &FeatureNode,
    state: &DesignState,
    failures: &mut BTreeSet<String>,
) {
    if let FeatureDefinition::Sketch(def) = &node.definition {
        if !state
            .sketches
            .iter()
            .any(|sketch| sketch.id.as_str() == def.sketch_id)
        {
            failures.insert(format!(
                "feature '{}' references unknown sketch '{}'",
                node.id, def.sketch_id
            ));
        }
    }
    if let FeatureDefinition::ImportedSolid(def) = &node.definition {
        if let Err(error) = def.validate() {
            failures.insert(format!("feature '{}': {error}", node.id));
        }
        match state.attachments.get(&def.source) {
            None => {
                failures.insert(format!(
                    "feature '{}' references unknown attachment '{}'",
                    node.id, def.source
                ));
            }
            Some(bytes) if opencad_core::sha256_hex(bytes) != def.sha256 => {
                failures.insert(format!(
                    "feature '{}' expects sha256 {} for '{}', but the attachment differs",
                    node.id, def.sha256, def.source
                ));
            }
            Some(_) => {}
        }
    }
    for (field, ref_id) in node.definition.reference_inputs() {
        if !state
            .semantic_refs
            .iter()
            .any(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
        {
            failures.insert(format!(
                "feature '{}' {field} references unknown semantic reference '{ref_id}'",
                node.id
            ));
        }
    }
    for (field, expr) in node.definition.expressions() {
        for name in parameter_names_in_expr(expr) {
            if state.parameters.find_by_name(&name).is_none() {
                failures.insert(format!(
                    "feature '{}' {field} '{expr}' references unknown parameter '{name}'",
                    node.id
                ));
            }
        }
    }
}

/// Report features whose relative display position changed.
///
/// Features kept in a longest common subsequence of the before/after order
/// (ties resolved toward the earliest positions) are unmoved; every other
/// feature present in both orders is reported, in the after order.
pub(crate) fn diff_feature_order(before: &[String], after: &[String]) -> Vec<SemanticChange> {
    let common_before: Vec<&String> = before.iter().filter(|id| after.contains(id)).collect();
    let common_after: Vec<&String> = after.iter().filter(|id| before.contains(id)).collect();
    if common_before == common_after {
        return Vec::new();
    }
    let (n, m) = (common_before.len(), common_after.len());
    let mut table = vec![vec![0_usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if common_before[i] == common_after[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let mut kept = BTreeSet::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if common_before[i] == common_after[j] {
            kept.insert(common_before[i].as_str());
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let index_of = |order: &[&String], id: &str| {
        order
            .iter()
            .position(|item| item.as_str() == id)
            .unwrap_or_default()
    };
    common_after
        .iter()
        .filter(|id| !kept.contains(id.as_str()))
        .map(|id| SemanticChange::FeatureMoved {
            id: (*id).clone(),
            before: index_of(&common_before, id).to_string(),
            after: index_of(&common_after, id).to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn moving_one_feature_reports_only_that_feature() {
        let before = ids(&["a", "b", "c", "d"]);
        let after = ids(&["a", "c", "d", "b"]);
        assert_eq!(
            diff_feature_order(&before, &after),
            vec![SemanticChange::FeatureMoved {
                id: "b".into(),
                before: "1".into(),
                after: "3".into(),
            }]
        );
        assert!(diff_feature_order(&before, &before).is_empty());
        // Additions and removals are not moves.
        assert!(diff_feature_order(&before, &ids(&["a", "x", "b", "d"])).is_empty());
    }

    #[test]
    fn positions_resolve_against_the_current_order() {
        let order = ids(&["a", "b"]);
        assert_eq!(FeaturePosition::end().index_in(&order).unwrap(), 2);
        assert_eq!(
            FeaturePosition::At(FeatureOrderEnd::Start)
                .index_in(&order)
                .unwrap(),
            0
        );
        assert_eq!(
            FeaturePosition::After("a".into()).index_in(&order).unwrap(),
            1
        );
        assert_eq!(
            FeaturePosition::Before("b".into())
                .index_in(&order)
                .unwrap(),
            1
        );
        assert!(FeaturePosition::After("z".into()).index_in(&order).is_err());
        assert_eq!(
            serde_json::to_value(FeaturePosition::After("feature:x".into())).unwrap(),
            serde_json::json!({ "after": "feature:x" })
        );
        assert_eq!(
            serde_json::to_value(FeaturePosition::end()).unwrap(),
            serde_json::json!({ "at": "end" })
        );
    }
}
