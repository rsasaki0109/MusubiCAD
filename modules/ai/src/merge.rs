//! Semantic three-way merge and patch rebase for Design Graph state.

use std::collections::{BTreeMap, BTreeSet};

use opencad_feature::FeatureNode;
use opencad_graph::{ParamGraph, ParameterEntry};
use serde::{Deserialize, Serialize};

use crate::feature_patch::FeaturePosition;
use crate::state::canonical_json_bytes;
use crate::validation::{build_patch_candidate, validate_design_state};
use crate::{design_state_revision, DesignPatch, DesignState, PatchOperation, PatchPrecondition};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    Parameter,
    Feature,
    Assembly,
    Drawing,
    Assertion,
    Sketch,
    SemanticReference,
    UnsupportedStructure,
}

/// Why a structural conflict occurred (ADR-013 §7).  Value conflicts on
/// an existing object carry no reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictReason {
    /// Both sides created the same stable ID with different content.
    AddAdd,
    /// One side removed an object the other side changed or kept editing.
    RemoveModify,
    /// A feature position anchor no longer exists on the other side.
    AnchorMissing,
    /// Feature display orders diverge, or the merged order breaks a dependency.
    Order,
    /// The combined result fails structural validation.
    InvalidResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticConflict {
    pub kind: ConflictKind,
    pub id: String,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ConflictReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticMergeResult {
    pub merged: Option<DesignState>,
    pub conflicts: Vec<SemanticConflict>,
}

/// Three-way merge of complete design states by stable semantic ID.
///
/// Every collection is merged per ID with the same rule: equal sides agree;
/// a side equal to base yields to the other side's change, addition, or
/// removal; any other combination is a conflict (`add_add` when both sides
/// created the ID, `remove_modify` when one side removed it).  Retained IDs
/// keep base order and additions follow in ID order, so the result does not
/// depend on which side is "ours".  The feature display order is merged
/// separately (see [`merge_feature_order`]), and the combined state must pass
/// [`validate_design_state`] or the merge reports an `invalid_result`
/// conflict instead of returning a state.
pub fn semantic_three_way_merge(
    base: &DesignState,
    ours: &DesignState,
    theirs: &DesignState,
) -> SemanticMergeResult {
    let mut conflicts = Vec::new();
    let mut merged = ours.clone();

    merged.parameters = merge_parameters(base, ours, theirs, &mut conflicts);
    merged.feature_nodes = merge_by_id(
        ConflictKind::Feature,
        [
            &base.feature_nodes,
            &ours.feature_nodes,
            &theirs.feature_nodes,
        ],
        |node| node.id.clone(),
        &mut conflicts,
    );
    // Documents store feature nodes sorted by ID; keep that convention.
    merged
        .feature_nodes
        .sort_by(|left, right| left.id.cmp(&right.id));
    merged.sketches = merge_by_id(
        ConflictKind::Sketch,
        [&base.sketches, &ours.sketches, &theirs.sketches],
        |sketch| sketch.id.as_str().to_string(),
        &mut conflicts,
    );
    merged.semantic_refs = merge_by_id(
        ConflictKind::SemanticReference,
        [
            &base.semantic_refs,
            &ours.semantic_refs,
            &theirs.semantic_refs,
        ],
        |topo_ref| topo_ref.ref_id.as_str().to_string(),
        &mut conflicts,
    );
    merged.assertions = merge_by_id(
        ConflictKind::Assertion,
        [&base.assertions, &ours.assertions, &theirs.assertions],
        |assertion| assertion.id.clone(),
        &mut conflicts,
    );
    let merged_features: BTreeSet<String> = merged
        .feature_nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();
    merged.feature_order = merge_feature_order(
        [
            &base.feature_order,
            &ours.feature_order,
            &theirs.feature_order,
        ],
        &merged_features,
        &mut conflicts,
    );

    match (&base.assembly, &ours.assembly, &theirs.assembly) {
        (Some(b), Some(o), Some(t)) => {
            let mut assembly = o.clone();
            assembly.components = merge_by_id(
                ConflictKind::Assembly,
                [&b.components, &o.components, &t.components],
                |item| item.id.as_str().to_string(),
                &mut conflicts,
            );
            assembly.instances = merge_by_id(
                ConflictKind::Assembly,
                [&b.instances, &o.instances, &t.instances],
                |item| item.id.as_str().to_string(),
                &mut conflicts,
            );
            assembly.mates = merge_by_id(
                ConflictKind::Assembly,
                [&b.mates, &o.mates, &t.mates],
                |item| item.id.as_str().to_string(),
                &mut conflicts,
            );
            assembly.connectors = merge_by_id(
                ConflictKind::Assembly,
                [&b.connectors, &o.connectors, &t.connectors],
                |item| item.id.as_str().to_string(),
                &mut conflicts,
            );
            assembly.patterns = merge_by_id(
                ConflictKind::Assembly,
                [&b.patterns, &o.patterns, &t.patterns],
                |item| item.id.as_str().to_string(),
                &mut conflicts,
            );
            merged.assembly = Some(assembly);
        }
        _ => merge_optional_model(
            "assembly",
            ConflictKind::Assembly,
            &base.assembly,
            &ours.assembly,
            &theirs.assembly,
            &mut merged.assembly,
            &mut conflicts,
        ),
    }
    match (&base.drawing, &ours.drawing, &theirs.drawing) {
        (Some(b), Some(o), Some(t)) => {
            let mut drawing = o.clone();
            drawing.sheets = merge_by_id(
                ConflictKind::Drawing,
                [&b.sheets, &o.sheets, &t.sheets],
                |sheet| sheet.id.as_str().to_string(),
                &mut conflicts,
            );
            merged.drawing = Some(drawing);
        }
        _ => merge_optional_model(
            "drawing",
            ConflictKind::Drawing,
            &base.drawing,
            &ours.drawing,
            &theirs.drawing,
            &mut merged.drawing,
            &mut conflicts,
        ),
    }

    if conflicts.is_empty() {
        if let Err(error) = validate_design_state(&merged) {
            conflicts.push(SemanticConflict {
                kind: ConflictKind::UnsupportedStructure,
                id: "merged".into(),
                base: None,
                ours: None,
                theirs: Some(error.to_string()),
                reason: Some(ConflictReason::InvalidResult),
            });
        }
    }
    sort_conflicts(&mut conflicts);
    SemanticMergeResult {
        merged: conflicts.is_empty().then_some(merged),
        conflicts,
    }
}

/// Canonical bytes used to compare one object across the three sides.
fn identity<T: Serialize>(value: &T) -> Vec<u8> {
    canonical_json_bytes(value).unwrap_or_default()
}

fn display<T: Serialize>(value: Option<&T>) -> Option<String> {
    value.and_then(|value| serde_json::to_string(value).ok())
}

/// Reason for a three-way conflict given which sides hold the object.
fn conflict_reason(base: bool, ours: bool, theirs: bool) -> Option<ConflictReason> {
    match (base, ours, theirs) {
        (false, true, true) => Some(ConflictReason::AddAdd),
        (true, false, _) | (true, _, false) => Some(ConflictReason::RemoveModify),
        _ => None,
    }
}

/// Merge one collection by stable ID (see [`semantic_three_way_merge`]).
fn merge_by_id<T: Clone + Serialize>(
    kind: ConflictKind,
    [base, ours, theirs]: [&Vec<T>; 3],
    id_of: impl Fn(&T) -> String,
    conflicts: &mut Vec<SemanticConflict>,
) -> Vec<T> {
    let index = |items: &Vec<T>| -> BTreeMap<String, T> {
        items
            .iter()
            .map(|item| (id_of(item), item.clone()))
            .collect()
    };
    let (b, o, t) = (index(base), index(ours), index(theirs));
    let mut chosen: BTreeMap<String, T> = BTreeMap::new();
    let ids: BTreeSet<&String> = b.keys().chain(o.keys()).chain(t.keys()).collect();
    for id in ids {
        let (bv, ov, tv) = (b.get(id), o.get(id), t.get(id));
        let (bi, oi, ti) = (bv.map(identity), ov.map(identity), tv.map(identity));
        let pick = if oi == ti {
            ov
        } else if oi == bi {
            tv
        } else if ti == bi {
            ov
        } else {
            conflicts.push(SemanticConflict {
                kind: kind.clone(),
                id: id.clone(),
                base: display(bv),
                ours: display(ov),
                theirs: display(tv),
                reason: conflict_reason(bv.is_some(), ov.is_some(), tv.is_some()),
            });
            ov
        };
        if let Some(value) = pick {
            chosen.insert(id.clone(), value.clone());
        }
    }
    // Retained IDs keep base order; additions follow in ID order.
    let mut result = Vec::with_capacity(chosen.len());
    for item in base {
        if let Some(value) = chosen.remove(&id_of(item)) {
            result.push(value);
        }
    }
    result.extend(chosen.into_values());
    result
}

/// Merge parameters by ID, comparing authored fields only (the runtime
/// `dirty` flag is ignored), then keep dependency edges both sides still
/// agree on plus edges either side added.
fn merge_parameters(
    base: &DesignState,
    ours: &DesignState,
    theirs: &DesignState,
    conflicts: &mut Vec<SemanticConflict>,
) -> ParamGraph {
    let authored = |state: &DesignState| -> Vec<ParameterEntry> {
        state
            .parameters
            .entries()
            .cloned()
            .map(|mut entry| {
                entry.dirty = false;
                entry
            })
            .collect()
    };
    let entries = merge_by_id(
        ConflictKind::Parameter,
        [&authored(base), &authored(ours), &authored(theirs)],
        |entry| entry.id.clone(),
        conflicts,
    );
    let edges = |state: &DesignState| -> Vec<(String, String)> {
        state
            .parameters
            .dependency_edges()
            .iter()
            .map(|edge| (edge.source.clone(), edge.target.clone()))
            .collect()
    };
    let (base_edges, ours_edges, theirs_edges) = (edges(base), edges(ours), edges(theirs));
    let ids: BTreeSet<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    let keep = |edge: &(String, String)| {
        let (in_base, in_ours, in_theirs) = (
            base_edges.contains(edge),
            ours_edges.contains(edge),
            theirs_edges.contains(edge),
        );
        ids.contains(edge.0.as_str())
            && ((in_ours && in_theirs) || (!in_base && (in_ours || in_theirs)))
    };
    let mut merged_edges: Vec<(String, String)> = base_edges
        .iter()
        .filter(|edge| keep(edge))
        .cloned()
        .collect();
    let added: BTreeSet<(String, String)> = ours_edges
        .iter()
        .chain(&theirs_edges)
        .filter(|edge| !base_edges.contains(edge) && keep(edge))
        .cloned()
        .collect();
    merged_edges.extend(added);

    let mut graph = ParamGraph::new();
    for entry in entries {
        // IDs are unique after the per-ID merge.
        let _ = graph.add_parameter(entry);
    }
    for (source, target) in merged_edges {
        let _ = graph.add_dependency(source, target);
    }
    graph
}

/// Merge the authored feature display order.
///
/// Features present on every side keep one side's relative order: the side
/// that changed it, or either when both agree.  Diverging reorders conflict
/// (`order`).  Features added by one side are inserted after the nearest
/// preceding retained feature in that side's order; runs from both sides at
/// the same anchor are ordered by their first feature ID, so swapping the
/// sides yields the same order (ADR-013 §7).
fn merge_feature_order(
    [base, ours, theirs]: [&Vec<String>; 3],
    merged: &BTreeSet<String>,
    conflicts: &mut Vec<SemanticConflict>,
) -> Vec<String> {
    let base_set: BTreeSet<&String> = base.iter().collect();
    let everywhere = |id: &&String| {
        merged.contains(*id) && base_set.contains(*id) && ours.contains(id) && theirs.contains(id)
    };
    let common =
        |order: &Vec<String>| -> Vec<String> { order.iter().filter(everywhere).cloned().collect() };
    let (common_base, common_ours, common_theirs) = (common(base), common(ours), common(theirs));
    let sequence = if common_ours == common_base || common_ours == common_theirs {
        common_theirs
    } else if common_theirs == common_base {
        common_ours
    } else {
        conflicts.push(SemanticConflict {
            kind: ConflictKind::Feature,
            id: "feature_order".into(),
            base: serde_json::to_string(base).ok(),
            ours: serde_json::to_string(ours).ok(),
            theirs: serde_json::to_string(theirs).ok(),
            reason: Some(ConflictReason::Order),
        });
        common_ours
    };

    let retained: BTreeSet<&String> = sequence.iter().collect();
    // anchor (None = start) -> runs of added features, one run per side.
    let mut runs: BTreeMap<Option<String>, Vec<Vec<String>>> = BTreeMap::new();
    for side in [ours, theirs] {
        let mut anchor: Option<String> = None;
        let mut run: Vec<String> = Vec::new();
        for id in side {
            if retained.contains(id) {
                if !run.is_empty() {
                    runs.entry(anchor.clone())
                        .or_default()
                        .push(std::mem::take(&mut run));
                }
                anchor = Some(id.clone());
            } else if merged.contains(id) && !base_set.contains(id) {
                run.push(id.clone());
            }
        }
        if !run.is_empty() {
            runs.entry(anchor).or_default().push(run);
        }
    }
    let mut placed: BTreeSet<String> = BTreeSet::new();
    let mut result = Vec::new();
    let mut place = |id: String, placed: &mut BTreeSet<String>| {
        if placed.insert(id.clone()) {
            result.push(id);
        }
    };
    let mut anchors: Vec<Option<String>> = vec![None];
    anchors.extend(sequence.iter().cloned().map(Some));
    for anchor in anchors {
        if let Some(id) = &anchor {
            place(id.clone(), &mut placed);
        }
        if let Some(mut side_runs) = runs.remove(&anchor) {
            side_runs.sort();
            for id in side_runs.into_iter().flatten() {
                place(id, &mut placed);
            }
        }
    }
    // Retained features missing from `sequence` (possible only after an
    // order conflict) and any stragglers are appended in ID order.
    for id in merged {
        if placed.insert(id.clone()) {
            result.push(id.clone());
        }
    }
    result
}

fn sort_conflicts(conflicts: &mut Vec<SemanticConflict>) {
    conflicts.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.base.cmp(&right.base))
            .then_with(|| left.ours.cmp(&right.ours))
            .then_with(|| left.theirs.cmp(&right.theirs))
    });
    conflicts.dedup();
}

/// Rebase a patch onto a newer state, rejecting IDs changed since its base.
///
/// Structural conflicts carry a [`ConflictReason`].  An add whose object the
/// new base already contains with identical content is dropped from the
/// rebased patch.  A feature position anchor missing from the new base is an
/// `anchor_missing` conflict, and the rebased patch must still apply to the
/// new base or the rebase reports an `invalid_result` conflict.
pub fn rebase_patch(
    patch: &DesignPatch,
    old_base: &DesignState,
    new_base: &DesignState,
) -> Result<DesignPatch, Vec<SemanticConflict>> {
    // Compute the patch's desired state once.  Conflict values therefore show
    // the actual requested result (ours), rather than incorrectly repeating
    // the old base value.  The candidate builder also validates all operation
    // groups before a rebase can be returned.
    let desired = match build_patch_candidate(old_base, patch) {
        Ok(desired) => desired,
        Err(error) => {
            return Err(vec![SemanticConflict {
                kind: ConflictKind::UnsupportedStructure,
                id: "patch".into(),
                base: None,
                ours: None,
                theirs: Some(error.to_string()),
                reason: None,
            }]);
        }
    };

    // BTreeMap deduplicates repeated operations targeting the same semantic ID
    // and makes both conflict discovery and output independent of operation
    // order.  Values are compared as canonical serialized source/state bytes,
    // never by approximate or exact geometry floating-point equality.
    let mut targets = BTreeMap::new();
    for operation in &patch.operations {
        if let Some(target) = patch_target(operation) {
            targets.entry(target).or_insert_with(|| operation.clone());
        }
    }
    let mut conflicts = Vec::new();
    let mut already_applied = BTreeSet::new();
    for (target, operation) in targets {
        let base = target_snapshot(old_base, &operation);
        let ours = target_snapshot(&desired, &operation);
        let theirs = target_snapshot(new_base, &operation);
        if base.identity != theirs.identity && theirs.identity != ours.identity {
            let reason = conflict_reason(
                base.display.is_some(),
                ours.display.is_some(),
                theirs.display.is_some(),
            );
            conflicts.push(SemanticConflict {
                kind: target.kind,
                id: target.id,
                base: base.display,
                ours: ours.display,
                theirs: theirs.display,
                reason,
            });
        } else if base.display.is_none()
            && theirs.display.is_some()
            && theirs.identity == ours.identity
        {
            // The new base already contains this exact addition.
            already_applied.insert(target);
        }
    }
    let created_features: BTreeSet<&str> = patch
        .operations
        .iter()
        .filter_map(|operation| match operation {
            PatchOperation::AddFeature { node, .. } => Some(node.id.as_str()),
            _ => None,
        })
        .collect();
    for operation in &patch.operations {
        let (id, position) = match operation {
            PatchOperation::AddFeature { node, position } => (node.id.as_str(), position),
            PatchOperation::MoveFeature { id, position } => (id.as_str(), position),
            _ => continue,
        };
        let (FeaturePosition::After(anchor) | FeaturePosition::Before(anchor)) = position else {
            continue;
        };
        if !created_features.contains(anchor.as_str())
            && !new_base.feature_order.iter().any(|item| item == anchor)
        {
            conflicts.push(SemanticConflict {
                kind: ConflictKind::Feature,
                id: id.to_string(),
                base: Some(anchor.clone()),
                ours: None,
                theirs: None,
                reason: Some(ConflictReason::AnchorMissing),
            });
        }
    }
    if !conflicts.is_empty() {
        // `targets` is ordered, but keep this invariant explicit if the target
        // collection changes in the future.
        conflicts.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.id.cmp(&right.id))
                .then_with(|| left.base.cmp(&right.base))
                .then_with(|| left.ours.cmp(&right.ours))
                .then_with(|| left.theirs.cmp(&right.theirs))
        });
        conflicts.dedup_by(|left, right| {
            left.kind == right.kind
                && left.id == right.id
                && left.base == right.base
                && left.ours == right.ours
                && left.theirs == right.theirs
        });
        return Err(conflicts);
    }

    let mut rebased = patch.clone();
    rebased.operations.retain(|operation| {
        patch_target(operation).map_or(true, |target| !already_applied.contains(&target))
    });
    for precondition in &mut rebased.preconditions {
        match precondition {
            PatchPrecondition::RevisionEquals {
                algorithm,
                version,
                digest,
            } => {
                *algorithm = crate::DESIGN_STATE_REVISION_ALGORITHM.to_string();
                *version = crate::DESIGN_STATE_REVISION_VERSION.to_string();
                *digest = design_state_revision(new_base).map_err(|error| {
                    vec![SemanticConflict {
                        kind: ConflictKind::UnsupportedStructure,
                        id: "revision".into(),
                        base: None,
                        ours: None,
                        theirs: Some(error.to_string()),
                        reason: None,
                    }]
                })?;
            }
            PatchPrecondition::ParameterExprEquals { id, expr } => {
                if let Some(current) = new_base.parameters.get(id) {
                    *expr = current.expr.clone();
                }
            }
            PatchPrecondition::FeatureExists { .. }
            | PatchPrecondition::TopoRefExists { .. }
            | PatchPrecondition::SketchExists { .. }
            | PatchPrecondition::SketchEntityExists { .. }
            | PatchPrecondition::AssertionExists { .. } => {}
        }
    }
    // Independent targets can still combine into an invalid state, e.g. the
    // new base added a consumer of an object this patch removes.
    if let Err(error) = build_patch_candidate(new_base, &rebased) {
        return Err(vec![SemanticConflict {
            kind: ConflictKind::UnsupportedStructure,
            id: "patch".into(),
            base: None,
            ours: None,
            theirs: Some(error.to_string()),
            reason: Some(ConflictReason::InvalidResult),
        }]);
    }
    Ok(rebased)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PatchTarget {
    kind: ConflictKind,
    id: String,
}

fn patch_target(operation: &PatchOperation) -> Option<PatchTarget> {
    if let Some((kind, id)) = model_structure_target(operation) {
        return Some(PatchTarget {
            kind,
            id: id.to_string(),
        });
    }
    let (kind, id) = match operation {
        PatchOperation::SetParameter { id, .. }
        | PatchOperation::AddParameter { id, .. }
        | PatchOperation::RemoveParameter { id } => (ConflictKind::Parameter, id.clone()),
        PatchOperation::AddAssertion { assertion } => {
            (ConflictKind::Assertion, assertion.id.clone())
        }
        PatchOperation::RemoveAssertion { id } => (ConflictKind::Assertion, id.clone()),
        PatchOperation::AddSketch { id, .. } | PatchOperation::RemoveSketch { id } => {
            (ConflictKind::Sketch, id.clone())
        }
        PatchOperation::AddFeature { node, .. } => (ConflictKind::Feature, node.id.clone()),
        PatchOperation::RemoveFeature { id }
        | PatchOperation::MoveFeature { id, .. }
        | PatchOperation::SetFeatureSuppressed { id, .. }
        | PatchOperation::ReplaceFeatureDefinition { id, .. } => {
            (ConflictKind::Feature, id.clone())
        }
        PatchOperation::AddSemanticRef { topo_ref } => (
            ConflictKind::SemanticReference,
            topo_ref.ref_id.as_str().to_string(),
        ),
        PatchOperation::RemoveSemanticRef { ref_id } => {
            (ConflictKind::SemanticReference, ref_id.clone())
        }
        PatchOperation::AddSketchEntity { sketch_id, entity } => {
            (ConflictKind::Sketch, format!("{sketch_id}/{}", entity.id()))
        }
        PatchOperation::RemoveSketchEntity {
            sketch_id,
            entity_id,
        } => (ConflictKind::Sketch, format!("{sketch_id}/{entity_id}")),
        PatchOperation::AddSketchConstraint {
            sketch_id,
            constraint,
        } => (
            ConflictKind::Sketch,
            format!("{sketch_id}/{}", constraint.id()),
        ),
        PatchOperation::RemoveSketchConstraint {
            sketch_id,
            constraint_id,
        } => (ConflictKind::Sketch, format!("{sketch_id}/{constraint_id}")),
        PatchOperation::SetFeatureExpr { feature_id, .. }
        | PatchOperation::SetFeatureRef { feature_id, .. } => {
            (ConflictKind::Feature, feature_id.clone())
        }
        PatchOperation::AssignFaceRef { ref_id, .. } => {
            (ConflictKind::UnsupportedStructure, ref_id.clone())
        }
        PatchOperation::SetInstancePlacement { instance_id, .. } => {
            (ConflictKind::Assembly, instance_id.clone())
        }
        PatchOperation::SetMateDistance { mate_id, .. } => {
            (ConflictKind::Assembly, mate_id.clone())
        }
        PatchOperation::AddConnector { id, .. } => (ConflictKind::Assembly, id.clone()),
        PatchOperation::SetDrawingViewScale { view_id, .. }
        | PatchOperation::SetDrawingViewOrigin { view_id, .. } => {
            (ConflictKind::Drawing, view_id.clone())
        }
        // Handled by `model_structure_target` above.
        _ => return None,
    };
    Some(PatchTarget { kind, id })
}

/// A feature node with its display position, so a concurrent move of the
/// same feature is observed as a change.
fn feature_with_position<'a>(
    state: &'a DesignState,
    id: &str,
) -> Option<(&'a FeatureNode, Option<usize>)> {
    state
        .feature_nodes
        .iter()
        .find(|node| node.id == id)
        .map(|node| (node, state.feature_order.iter().position(|item| item == id)))
}

fn find_sketch<'a>(state: &'a DesignState, id: &str) -> Option<&'a opencad_sketch::Sketch> {
    state
        .sketches
        .iter()
        .find(|sketch| sketch.id.as_str() == id)
}

#[derive(Debug, Clone)]
struct TargetSnapshot {
    identity: Vec<u8>,
    display: Option<String>,
}

/// Stable target of an assembly or drawing structural operation.
fn model_structure_target(operation: &PatchOperation) -> Option<(ConflictKind, &str)> {
    let target = match operation {
        PatchOperation::AddComponent { component } => {
            (ConflictKind::Assembly, component.id.as_str())
        }
        PatchOperation::AddInstance { instance } => (ConflictKind::Assembly, instance.id.as_str()),
        PatchOperation::AddMate { mate } => (ConflictKind::Assembly, mate.id.as_str()),
        PatchOperation::AddAssemblyPattern { pattern } => {
            (ConflictKind::Assembly, pattern.id.as_str())
        }
        PatchOperation::RemoveComponent { id }
        | PatchOperation::RemoveInstance { id }
        | PatchOperation::RemoveMate { id }
        | PatchOperation::RemoveConnector { id }
        | PatchOperation::RemoveAssemblyPattern { id } => (ConflictKind::Assembly, id.as_str()),
        PatchOperation::AddSheet { sheet } => (ConflictKind::Drawing, sheet.id.as_str()),
        PatchOperation::AddDrawingView { view, .. } => (ConflictKind::Drawing, view.id.as_str()),
        PatchOperation::AddDrawingDimension { dimension, .. } => {
            (ConflictKind::Drawing, dimension.id.as_str())
        }
        PatchOperation::RemoveSheet { id } | PatchOperation::RemoveDrawingDimension { id } => {
            (ConflictKind::Drawing, id.as_str())
        }
        PatchOperation::RemoveDrawingView { view_id } => (ConflictKind::Drawing, view_id.as_str()),
        _ => return None,
    };
    Some(target)
}

/// Canonical snapshot of the assembly or drawing object with `id`, looked
/// up across every collection that can hold that ID prefix.
fn model_object_snapshot(state: &DesignState, kind: &ConflictKind, id: &str) -> TargetSnapshot {
    let value = match kind {
        ConflictKind::Assembly => state.assembly.as_ref().and_then(|assembly| {
            let find = |value: Option<serde_json::Value>| value;
            find(
                assembly
                    .components
                    .iter()
                    .find(|item| item.id.as_str() == id)
                    .and_then(|item| serde_json::to_value(item).ok()),
            )
            .or_else(|| {
                assembly
                    .instances
                    .iter()
                    .find(|item| item.id.as_str() == id)
                    .and_then(|item| serde_json::to_value(item).ok())
            })
            .or_else(|| {
                assembly
                    .mates
                    .iter()
                    .find(|item| item.id.as_str() == id)
                    .and_then(|item| serde_json::to_value(item).ok())
            })
            .or_else(|| {
                assembly
                    .connectors
                    .iter()
                    .find(|item| item.id.as_str() == id)
                    .and_then(|item| serde_json::to_value(item).ok())
            })
            .or_else(|| {
                assembly
                    .patterns
                    .iter()
                    .find(|item| item.id.as_str() == id)
                    .and_then(|item| serde_json::to_value(item).ok())
            })
        }),
        ConflictKind::Drawing => state.drawing.as_ref().and_then(|drawing| {
            drawing
                .sheets
                .iter()
                .find(|sheet| sheet.id.as_str() == id)
                .and_then(|sheet| serde_json::to_value(sheet).ok())
                .or_else(|| {
                    drawing
                        .sheets
                        .iter()
                        .flat_map(|sheet| sheet.views.iter())
                        .find(|view| view.id.as_str() == id)
                        .and_then(|view| serde_json::to_value(view).ok())
                })
                .or_else(|| {
                    drawing
                        .sheets
                        .iter()
                        .flat_map(|sheet| sheet.dimensions.iter())
                        .find(|dimension| dimension.id.as_str() == id)
                        .and_then(|dimension| serde_json::to_value(dimension).ok())
                })
        }),
        _ => None,
    };
    snapshot(value.as_ref())
}

fn target_snapshot(state: &DesignState, operation: &PatchOperation) -> TargetSnapshot {
    if let Some((kind, id)) = model_structure_target(operation) {
        return model_object_snapshot(state, &kind, id);
    }
    match operation {
        PatchOperation::SetParameter { id, .. } => {
            snapshot_parameter(state.parameters.get(id).map(|entry| &entry.expr))
        }
        // Structural targets compare the whole entry: an add or remove must
        // conflict with any concurrent change to the same stable ID.
        PatchOperation::AddParameter { id, .. } | PatchOperation::RemoveParameter { id } => {
            snapshot(state.parameters.get(id))
        }
        PatchOperation::AddAssertion { assertion } => {
            snapshot(state.assertions.iter().find(|item| item.id == assertion.id))
        }
        PatchOperation::RemoveAssertion { id } => {
            snapshot(state.assertions.iter().find(|item| item.id == *id))
        }
        PatchOperation::AddSketch { id, .. } | PatchOperation::RemoveSketch { id } => {
            snapshot(find_sketch(state, id))
        }
        PatchOperation::AddFeature { node, .. } => {
            snapshot(feature_with_position(state, &node.id).as_ref())
        }
        PatchOperation::RemoveFeature { id }
        | PatchOperation::MoveFeature { id, .. }
        | PatchOperation::SetFeatureSuppressed { id, .. }
        | PatchOperation::ReplaceFeatureDefinition { id, .. } => {
            snapshot(feature_with_position(state, id).as_ref())
        }
        PatchOperation::AddSemanticRef { topo_ref } => snapshot(
            state
                .semantic_refs
                .iter()
                .find(|item| item.ref_id == topo_ref.ref_id),
        ),
        PatchOperation::RemoveSemanticRef { ref_id } => snapshot(
            state
                .semantic_refs
                .iter()
                .find(|item| item.ref_id.as_str() == ref_id),
        ),
        PatchOperation::AddSketchEntity { sketch_id, entity } => snapshot(
            find_sketch(state, sketch_id)
                .and_then(|sketch| sketch.find_entity(entity.id().as_str())),
        ),
        PatchOperation::RemoveSketchEntity {
            sketch_id,
            entity_id,
        } => {
            snapshot(find_sketch(state, sketch_id).and_then(|sketch| sketch.find_entity(entity_id)))
        }
        PatchOperation::AddSketchConstraint {
            sketch_id,
            constraint,
        } => snapshot(find_sketch(state, sketch_id).and_then(|sketch| {
            sketch
                .constraints
                .iter()
                .find(|item| item.id() == constraint.id())
        })),
        PatchOperation::RemoveSketchConstraint {
            sketch_id,
            constraint_id,
        } => snapshot(find_sketch(state, sketch_id).and_then(|sketch| {
            sketch
                .constraints
                .iter()
                .find(|item| item.id().as_str() == constraint_id)
        })),
        PatchOperation::SetFeatureExpr { feature_id, .. }
        | PatchOperation::SetFeatureRef { feature_id, .. } => snapshot(
            state
                .feature_nodes
                .iter()
                .find(|node| node.id == *feature_id),
        ),
        PatchOperation::AssignFaceRef { ref_id, .. } => snapshot(
            state
                .semantic_refs
                .iter()
                .find(|topo_ref| topo_ref.ref_id.as_str() == ref_id),
        ),
        PatchOperation::SetInstancePlacement { instance_id, .. } => {
            snapshot(state.assembly.as_ref().and_then(|assembly| {
                assembly
                    .instances
                    .iter()
                    .find(|instance| instance.id.as_str() == instance_id)
            }))
        }
        PatchOperation::SetMateDistance { mate_id, .. } => {
            snapshot(state.assembly.as_ref().and_then(|assembly| {
                assembly
                    .mates
                    .iter()
                    .find(|mate| mate.id.as_str() == mate_id)
            }))
        }
        PatchOperation::AddConnector { id, .. } => {
            snapshot(state.assembly.as_ref().and_then(|assembly| {
                assembly
                    .connectors
                    .iter()
                    .find(|connector| connector.id.as_str() == id)
            }))
        }
        PatchOperation::SetDrawingViewScale { view_id, .. }
        | PatchOperation::SetDrawingViewOrigin { view_id, .. } => {
            snapshot(state.drawing.as_ref().and_then(|drawing| {
                drawing
                    .sheets
                    .iter()
                    .flat_map(|sheet| sheet.views.iter())
                    .find(|view| view.id.as_str() == view_id)
            }))
        }
        // Assembly and drawing structure is snapshotted by
        // `model_object_snapshot` above.
        _ => snapshot::<()>(None),
    }
}

fn snapshot<T: Serialize>(value: Option<&T>) -> TargetSnapshot {
    match value {
        Some(value) => {
            let identity = canonical_json_bytes(value).unwrap_or_default();
            let display = String::from_utf8(identity.clone()).ok();
            TargetSnapshot { identity, display }
        }
        None => TargetSnapshot {
            identity: b"null".to_vec(),
            display: None,
        },
    }
}

fn snapshot_parameter(value: Option<&String>) -> TargetSnapshot {
    match value {
        Some(value) => TargetSnapshot {
            identity: canonical_json_bytes(value).unwrap_or_default(),
            display: Some(value.clone()),
        },
        None => TargetSnapshot {
            identity: b"null".to_vec(),
            display: None,
        },
    }
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
            reason: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use opencad_assembly::{AssemblyModel, Component, Instance, Placement};
    use opencad_core::{ComponentId, DocumentId, InstanceId, SheetId, ViewId};
    use opencad_drawing::{DrawingModel, DrawingView, ModelReference, ProjectionKind, Sheet};
    use opencad_feature::bracket_with_hole;
    use opencad_geometry::RigidTransform;
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

    fn feature_state(width: &str, height: &str) -> DesignState {
        let part = bracket_with_hole().expect("part");
        DesignState::with_models(
            state(width, height).parameters,
            part.nodes.into_values().collect(),
            Vec::new(),
            None,
            None,
        )
    }

    fn assembly_and_drawing_state() -> DesignState {
        let component_id = ComponentId::new("component:bracket").expect("component");
        let document_id = DocumentId::new("doc:bracket").expect("document");
        let assembly = AssemblyModel {
            components: vec![Component::new(
                component_id.clone(),
                "parts/bracket.ocad.d",
                document_id.clone(),
            )],
            instances: vec![
                Instance::new(
                    InstanceId::new("instance:left").expect("instance"),
                    component_id.clone(),
                    Placement::identity(),
                    "Left",
                ),
                Instance::new(
                    InstanceId::new("instance:right").expect("instance"),
                    component_id,
                    Placement::identity(),
                    "Right",
                ),
            ],
            ..AssemblyModel::default()
        };
        let mut sheet = Sheet::a4_portrait(SheetId::new("sheet:main").expect("sheet"), "Main");
        for (id, name) in [("view:front", "Front"), ("view:back", "Back")] {
            sheet.views.push(DrawingView::new(
                ViewId::new(id).expect("view"),
                name,
                ModelReference::new("parts/bracket.ocad.d", document_id.clone()),
                ProjectionKind::Front,
                1.0,
                [0.05, 0.06],
            ));
        }
        DesignState::with_models(
            state("80 mm", "60 mm").parameters,
            Vec::new(),
            Vec::new(),
            Some(assembly),
            Some(DrawingModel {
                sheets: vec![sheet],
            }),
        )
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

    #[test]
    fn rebase_conflicts_are_deduplicated_sorted_and_show_patch_desired_ours() {
        let old = feature_state("80 mm", "60 mm");
        let patch = DesignPatch::new(vec![
            PatchOperation::SetFeatureExpr {
                feature_id: "feature:extrude_base".into(),
                field: "length_expr".into(),
                expr: "thickness * 2".into(),
            },
            PatchOperation::SetParameter {
                id: "param:width".into(),
                expr: "100 mm".into(),
            },
            // Same target twice: only one deterministic conflict is reported.
            PatchOperation::SetParameter {
                id: "param:width".into(),
                expr: "100 mm".into(),
            },
        ]);
        let mut new = old.clone();
        new.parameters
            .set_expr("param:width", "90 mm")
            .expect("set");
        DesignPatch::set_feature_expr(
            "feature:extrude_base",
            crate::FeatureExprField::LengthExpr,
            "thickness * 3",
        )
        .apply_to_features(&mut new.feature_nodes)
        .expect("feature change");

        let conflicts = rebase_patch(&patch, &old, &new).expect_err("same targets conflict");
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].kind, ConflictKind::Parameter);
        assert_eq!(conflicts[0].id, "param:width");
        assert_eq!(conflicts[0].base.as_deref(), Some("80 mm"));
        assert_eq!(conflicts[0].ours.as_deref(), Some("100 mm"));
        assert_eq!(conflicts[0].theirs.as_deref(), Some("90 mm"));
        assert_eq!(conflicts[1].kind, ConflictKind::Feature);
        assert_eq!(conflicts[1].id, "feature:extrude_base");
        assert!(conflicts[1]
            .ours
            .as_deref()
            .expect("feature ours")
            .contains("thickness * 2"));

        let reversed = DesignPatch {
            operations: patch.operations.iter().cloned().rev().collect(),
            ..patch
        };
        let reversed_conflicts = rebase_patch(&reversed, &old, &new).expect_err("conflict");
        assert_eq!(conflicts, reversed_conflicts);
    }

    #[test]
    fn rebase_handles_independent_assembly_and_drawing_targets_and_updates_revision() {
        let old = assembly_and_drawing_state();
        let patch = DesignPatch::new(vec![
            PatchOperation::SetParameter {
                id: "param:width".into(),
                expr: "100 mm".into(),
            },
            PatchOperation::SetInstancePlacement {
                instance_id: "instance:left".into(),
                translation_m: [0.1, 0.0, 0.0],
                rotation: RigidTransform::identity_rotation(),
            },
            PatchOperation::SetDrawingViewScale {
                view_id: "view:front".into(),
                scale: 2.0,
            },
        ])
        .with_revision_precondition(&old)
        .expect("revision");
        let mut new = old.clone();
        new.parameters
            .set_expr("param:height", "70 mm")
            .expect("set");
        new.assembly.as_mut().expect("assembly").instances[1]
            .placement
            .transform
            .translation_m[0] = 0.2;
        new.drawing.as_mut().expect("drawing").sheets[0].views[1].scale = 3.0;

        let rebased = rebase_patch(&patch, &old, &new).expect("independent targets");
        let revision = rebased
            .preconditions
            .iter()
            .find_map(|precondition| match precondition {
                PatchPrecondition::RevisionEquals { digest, .. } => Some(digest),
                _ => None,
            })
            .expect("revision precondition");
        assert_eq!(
            revision,
            &crate::design_state_revision(&new).expect("new revision")
        );

        let mut changed_target = old.clone();
        changed_target
            .assembly
            .as_mut()
            .expect("assembly")
            .instances[0]
            .placement
            .transform
            .translation_m[0] = 0.3;
        changed_target.drawing.as_mut().expect("drawing").sheets[0].views[0].scale = 4.0;
        let conflicts = rebase_patch(&patch, &old, &changed_target).expect_err("same targets");
        assert_eq!(
            conflicts
                .iter()
                .map(|conflict| (&conflict.kind, conflict.id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (&ConflictKind::Assembly, "instance:left"),
                (&ConflictKind::Drawing, "view:front"),
            ]
        );
    }
}
