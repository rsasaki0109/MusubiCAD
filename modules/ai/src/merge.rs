//! Semantic three-way merge and patch rebase for Design Graph state.

use std::collections::{BTreeMap, BTreeSet};

use opencad_feature::FeatureNode;
use serde::{Deserialize, Serialize};

use crate::{DesignPatch, DesignState, PatchOperation, PatchPrecondition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    Parameter,
    Feature,
    Assembly,
    Drawing,
    UnsupportedStructure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticConflict {
    pub kind: ConflictKind,
    pub id: String,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticMergeResult {
    pub merged: Option<DesignState>,
    pub conflicts: Vec<SemanticConflict>,
}

/// Merge independent parameter and feature edits by stable semantic ID.
/// Structural additions/removals are reported explicitly until graph edge merging is available.
pub fn semantic_three_way_merge(
    base: &DesignState,
    ours: &DesignState,
    theirs: &DesignState,
) -> SemanticMergeResult {
    let mut merged = ours.clone();
    let mut conflicts = Vec::new();
    let ids: BTreeSet<_> = base
        .parameters
        .parameter_ids()
        .into_iter()
        .chain(ours.parameters.parameter_ids())
        .chain(theirs.parameters.parameter_ids())
        .collect();
    for id in ids {
        let values = (
            base.parameters.get(&id).map(|p| p.expr.as_str()),
            ours.parameters.get(&id).map(|p| p.expr.as_str()),
            theirs.parameters.get(&id).map(|p| p.expr.as_str()),
        );
        match values {
            (Some(b), Some(o), Some(t)) if o == b && t != b => {
                if merged.parameters.set_expr(&id, t).is_err() {
                    conflicts.push(conflict(ConflictKind::Parameter, &id, values));
                }
            }
            (Some(b), Some(o), Some(t)) if o != b && t != b && o != t => {
                conflicts.push(conflict(ConflictKind::Parameter, &id, values));
            }
            (Some(_), Some(_), Some(_)) => {}
            _ => conflicts.push(conflict(ConflictKind::UnsupportedStructure, &id, values)),
        }
    }

    merge_features(base, ours, theirs, &mut merged, &mut conflicts);
    merge_optional_model(
        "assembly",
        ConflictKind::Assembly,
        &base.assembly,
        &ours.assembly,
        &theirs.assembly,
        &mut merged.assembly,
        &mut conflicts,
    );
    merge_optional_model(
        "drawing",
        ConflictKind::Drawing,
        &base.drawing,
        &ours.drawing,
        &theirs.drawing,
        &mut merged.drawing,
        &mut conflicts,
    );
    SemanticMergeResult {
        merged: conflicts.is_empty().then_some(merged),
        conflicts,
    }
}

fn merge_features(
    base: &DesignState,
    ours: &DesignState,
    theirs: &DesignState,
    merged: &mut DesignState,
    conflicts: &mut Vec<SemanticConflict>,
) {
    let maps = [base, ours, theirs].map(|state| {
        state
            .feature_nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>()
    });
    let ids: BTreeSet<_> = maps.iter().flat_map(|map| map.keys().copied()).collect();
    for id in ids {
        match (maps[0].get(id), maps[1].get(id), maps[2].get(id)) {
            (Some(b), Some(o), Some(t)) if *o == *b && *t != *b => {
                if let Some(node) = merged.feature_nodes.iter_mut().find(|node| node.id == id) {
                    *node = (*t).clone();
                }
            }
            (Some(b), Some(o), Some(t)) if *o != *b && *t != *b && *o != *t => {
                conflicts.push(feature_conflict(ConflictKind::Feature, id, b, o, t));
            }
            (Some(_), Some(_), Some(_)) => {}
            (b, o, t) => conflicts.push(SemanticConflict {
                kind: ConflictKind::UnsupportedStructure,
                id: id.to_string(),
                base: b.and_then(|v| serde_json::to_string(*v).ok()),
                ours: o.and_then(|v| serde_json::to_string(*v).ok()),
                theirs: t.and_then(|v| serde_json::to_string(*v).ok()),
            }),
        }
    }
}

/// Rebase a patch onto a newer state, rejecting IDs changed since its base.
pub fn rebase_patch(
    patch: &DesignPatch,
    old_base: &DesignState,
    new_base: &DesignState,
) -> Result<DesignPatch, Vec<SemanticConflict>> {
    let mut conflicts = Vec::new();
    let touched: BTreeSet<&str> = patch
        .operations
        .iter()
        .filter_map(|op| match op {
            PatchOperation::SetParameter { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    for id in touched {
        let values = (
            old_base.parameters.get(id).map(|p| p.expr.as_str()),
            old_base.parameters.get(id).map(|p| p.expr.as_str()),
            new_base.parameters.get(id).map(|p| p.expr.as_str()),
        );
        if values.0 != values.2 {
            conflicts.push(conflict(ConflictKind::Parameter, id, values));
        }
    }
    let old_features: BTreeMap<_, _> = old_base
        .feature_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let new_features: BTreeMap<_, _> = new_base
        .feature_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    for feature_id in patch.operations.iter().filter_map(|op| match op {
        PatchOperation::SetFeatureExpr { feature_id, .. }
        | PatchOperation::SetFeatureRef { feature_id, .. } => Some(feature_id.as_str()),
        _ => None,
    }) {
        if old_features.get(feature_id) != new_features.get(feature_id) {
            conflicts.push(SemanticConflict {
                kind: ConflictKind::Feature,
                id: feature_id.into(),
                base: old_features
                    .get(feature_id)
                    .and_then(|value| serde_json::to_string(*value).ok()),
                ours: old_features
                    .get(feature_id)
                    .and_then(|value| serde_json::to_string(*value).ok()),
                theirs: new_features
                    .get(feature_id)
                    .and_then(|value| serde_json::to_string(*value).ok()),
            });
        }
    }
    let touches_assembly = patch.operations.iter().any(|op| {
        matches!(
            op,
            PatchOperation::SetInstancePlacement { .. }
                | PatchOperation::SetMateDistance { .. }
                | PatchOperation::AddConnector { .. }
        )
    });
    if touches_assembly && old_base.assembly != new_base.assembly {
        conflicts.push(model_conflict(
            ConflictKind::Assembly,
            "assembly",
            &old_base.assembly,
            &new_base.assembly,
        ));
    }
    let touches_drawing = patch.operations.iter().any(|op| {
        matches!(
            op,
            PatchOperation::SetDrawingViewScale { .. }
                | PatchOperation::SetDrawingViewOrigin { .. }
        )
    });
    if touches_drawing && old_base.drawing != new_base.drawing {
        conflicts.push(model_conflict(
            ConflictKind::Drawing,
            "drawing",
            &old_base.drawing,
            &new_base.drawing,
        ));
    }
    if !conflicts.is_empty() {
        return Err(conflicts);
    }
    let mut rebased = patch.clone();
    for precondition in &mut rebased.preconditions {
        if let PatchPrecondition::ParameterExprEquals { id, expr } = precondition {
            if let Some(current) = new_base.parameters.get(id) {
                *expr = current.expr.clone();
            }
        }
    }
    Ok(rebased)
}

fn merge_optional_model<T>(
    id: &str,
    kind: ConflictKind,
    base: &Option<T>,
    ours: &Option<T>,
    theirs: &Option<T>,
    merged: &mut Option<T>,
    conflicts: &mut Vec<SemanticConflict>,
) where
    T: Clone + PartialEq + Serialize,
{
    if ours == base && theirs != base {
        *merged = theirs.clone();
    } else if ours != base && theirs != base && ours != theirs {
        conflicts.push(SemanticConflict {
            kind,
            id: id.into(),
            base: serde_json::to_string(base).ok(),
            ours: serde_json::to_string(ours).ok(),
            theirs: serde_json::to_string(theirs).ok(),
        });
    }
}

fn model_conflict<T: Serialize>(
    kind: ConflictKind,
    id: &str,
    old: &Option<T>,
    new: &Option<T>,
) -> SemanticConflict {
    SemanticConflict {
        kind,
        id: id.into(),
        base: serde_json::to_string(old).ok(),
        ours: serde_json::to_string(old).ok(),
        theirs: serde_json::to_string(new).ok(),
    }
}

fn conflict(
    kind: ConflictKind,
    id: &str,
    values: (Option<&str>, Option<&str>, Option<&str>),
) -> SemanticConflict {
    SemanticConflict {
        kind,
        id: id.to_string(),
        base: values.0.map(str::to_string),
        ours: values.1.map(str::to_string),
        theirs: values.2.map(str::to_string),
    }
}

fn feature_conflict(
    kind: ConflictKind,
    id: &str,
    base: &FeatureNode,
    ours: &FeatureNode,
    theirs: &FeatureNode,
) -> SemanticConflict {
    SemanticConflict {
        kind,
        id: id.to_string(),
        base: serde_json::to_string(base).ok(),
        ours: serde_json::to_string(ours).ok(),
        theirs: serde_json::to_string(theirs).ok(),
    }
}

#[cfg(test)]
mod tests {
    use opencad_graph::{ParamGraph, ParameterEntry};

    use super::*;

    fn state(width: &str, height: &str) -> DesignState {
        let mut graph = ParamGraph::new();
        graph
            .add_parameter(ParameterEntry::new("param:width", "width", width))
            .unwrap();
        graph
            .add_parameter(ParameterEntry::new("param:height", "height", height))
            .unwrap();
        DesignState::new(graph, vec![])
    }

    #[test]
    fn merges_independent_parameter_changes() {
        let result = semantic_three_way_merge(
            &state("80 mm", "60 mm"),
            &state("100 mm", "60 mm"),
            &state("80 mm", "70 mm"),
        );
        let merged = result.merged.unwrap();
        assert_eq!(merged.parameters.get("param:width").unwrap().expr, "100 mm");
        assert_eq!(merged.parameters.get("param:height").unwrap().expr, "70 mm");
    }

    #[test]
    fn reports_same_parameter_conflict() {
        let result = semantic_three_way_merge(
            &state("80 mm", "60 mm"),
            &state("100 mm", "60 mm"),
            &state("120 mm", "60 mm"),
        );
        assert!(result.merged.is_none());
        assert_eq!(result.conflicts[0].id, "param:width");
    }

    #[test]
    fn rebases_patch_when_touched_parameter_is_unchanged() {
        let patch = DesignPatch::set_parameter("param:width", "100 mm");
        let rebased =
            rebase_patch(&patch, &state("80 mm", "60 mm"), &state("80 mm", "70 mm")).unwrap();
        assert_eq!(rebased.operations, patch.operations);
    }
}
