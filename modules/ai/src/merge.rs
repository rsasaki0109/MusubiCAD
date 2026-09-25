//! Semantic three-way merge and patch rebase for Design Graph state.

use std::collections::{BTreeMap, BTreeSet};

use opencad_feature::FeatureNode;
use serde::{Deserialize, Serialize};

use crate::state::canonical_json_bytes;
use crate::validation::build_patch_candidate;
use crate::{design_state_revision, DesignPatch, DesignState, PatchOperation, PatchPrecondition};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    Parameter,
    Feature,
    Assembly,
    Drawing,
    Assertion,
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
    for (target, operation) in targets {
        let base = target_snapshot(old_base, &operation);
        let ours = target_snapshot(&desired, &operation);
        let theirs = target_snapshot(new_base, &operation);
        if base.identity != theirs.identity && theirs.identity != ours.identity {
            conflicts.push(SemanticConflict {
                kind: target.kind,
                id: target.id,
                base: base.display,
                ours: ours.display,
                theirs: theirs.display,
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
                    }]
                })?;
            }
            PatchPrecondition::ParameterExprEquals { id, expr } => {
                if let Some(current) = new_base.parameters.get(id) {
                    *expr = current.expr.clone();
                }
            }
            PatchPrecondition::FeatureExists { .. } | PatchPrecondition::TopoRefExists { .. } => {}
        }
    }
    Ok(rebased)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PatchTarget {
    kind: ConflictKind,
    id: String,
}

fn patch_target(operation: &PatchOperation) -> Option<PatchTarget> {
    let (kind, id) = match operation {
        PatchOperation::SetParameter { id, .. }
        | PatchOperation::AddParameter { id, .. }
        | PatchOperation::RemoveParameter { id } => (ConflictKind::Parameter, id.clone()),
        PatchOperation::AddAssertion { assertion } => {
            (ConflictKind::Assertion, assertion.id.clone())
        }
        PatchOperation::RemoveAssertion { id } => (ConflictKind::Assertion, id.clone()),
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
    };
    Some(PatchTarget { kind, id })
}

#[derive(Debug, Clone)]
struct TargetSnapshot {
    identity: Vec<u8>,
    display: Option<String>,
}

fn target_snapshot(state: &DesignState, operation: &PatchOperation) -> TargetSnapshot {
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
        });
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
