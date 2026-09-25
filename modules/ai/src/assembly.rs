//! Assembly query, patch, and diff helpers for the Agent API (M3.3).

use std::collections::BTreeSet;

use opencad_assembly::{
    validate_connectors, validate_mates, validate_patterns, AssemblyModel, Component, Connector,
    Instance, Mate, MateEntity, MateKind,
};
use opencad_core::{InstanceId, OpenCadError, Result};
use opencad_geometry::RigidTransform;
use opencad_graph::{build_summary, DesignDiff, SemanticChange};
use serde::{Deserialize, Serialize};

use crate::patch::{validate_stable_id, PatchOperation};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyInstanceInfo {
    pub id: String,
    pub name: String,
    pub component: String,
    pub fixed: bool,
    pub translation_m: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyMateInfo {
    pub id: String,
    pub kind: String,
    pub suppressed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectorInfo {
    pub id: String,
    pub name: String,
    pub instance: String,
    pub translation_m: [f64; 3],
}

pub fn list_assembly_instances(model: &AssemblyModel) -> Vec<AssemblyInstanceInfo> {
    model
        .instances
        .iter()
        .map(|instance| AssemblyInstanceInfo {
            id: instance.id.as_str().to_string(),
            name: instance.name.clone(),
            component: instance.component.as_str().to_string(),
            fixed: instance.fixed,
            translation_m: instance.placement.transform.translation_m,
        })
        .collect()
}

pub fn get_assembly_instance(model: &AssemblyModel, id: &str) -> Result<AssemblyInstanceInfo> {
    let instance = model
        .instances
        .iter()
        .find(|instance| instance.id.as_str() == id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown assembly instance '{id}'")))?;
    Ok(AssemblyInstanceInfo {
        id: instance.id.as_str().to_string(),
        name: instance.name.clone(),
        component: instance.component.as_str().to_string(),
        fixed: instance.fixed,
        translation_m: instance.placement.transform.translation_m,
    })
}

pub fn list_assembly_mates(model: &AssemblyModel) -> Vec<AssemblyMateInfo> {
    model
        .mates
        .iter()
        .map(|mate| AssemblyMateInfo {
            id: mate.id.as_str().to_string(),
            kind: mate_kind_name(&mate.kind),
            suppressed: mate.suppressed,
        })
        .collect()
}

pub fn list_connectors(model: &AssemblyModel) -> Vec<ConnectorInfo> {
    model
        .connectors
        .iter()
        .map(|connector| ConnectorInfo {
            id: connector.id.as_str().to_string(),
            name: connector.name.clone(),
            instance: connector.instance.as_str().to_string(),
            translation_m: connector.transform.translation_m,
        })
        .collect()
}

fn mate_kind_name(kind: &opencad_assembly::MateKind) -> String {
    match kind {
        opencad_assembly::MateKind::Coincident { .. } => "coincident".into(),
        opencad_assembly::MateKind::Concentric { .. } => "concentric".into(),
        opencad_assembly::MateKind::Distance { .. } => "distance".into(),
        opencad_assembly::MateKind::Angle { .. } => "angle".into(),
        opencad_assembly::MateKind::Parallel { .. } => "parallel".into(),
        opencad_assembly::MateKind::Ground { .. } => "ground".into(),
    }
}

/// Reject creating an ID that already exists or was removed earlier in the
/// same patch (ADR-013 §2).
fn check_new_id(
    id: &str,
    prefix: &str,
    exists: bool,
    removed: &BTreeSet<String>,
    kind: &str,
) -> Result<()> {
    validate_stable_id(id, prefix)?;
    if removed.contains(id) {
        return Err(OpenCadError::validation(format!(
            "{kind} id '{id}' was removed earlier in this patch and cannot be reused"
        )));
    }
    if exists {
        return Err(OpenCadError::validation(format!(
            "{kind} '{id}' already exists"
        )));
    }
    Ok(())
}

fn remove_by_id<T>(
    items: &mut Vec<T>,
    id: &str,
    item_id: impl Fn(&T) -> &str,
    kind: &str,
    removed: &mut BTreeSet<String>,
) -> Result<()> {
    let index = items
        .iter()
        .position(|item| item_id(item) == id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown {kind} '{id}'")))?;
    items.remove(index);
    removed.insert(id.to_string());
    Ok(())
}

/// The part-attached entities a mate constrains.
fn mate_entities(kind: &MateKind) -> Vec<&MateEntity> {
    match kind {
        MateKind::Coincident { a, b }
        | MateKind::Concentric { a, b }
        | MateKind::Distance { a, b, .. }
        | MateKind::Angle { a, b, .. }
        | MateKind::Parallel { a, b } => vec![a, b],
        MateKind::Ground { .. } => Vec::new(),
    }
}

/// Instances a mate references.
fn mate_instances(kind: &MateKind) -> Vec<&InstanceId> {
    match kind {
        MateKind::Ground { instance } => vec![instance],
        _ => mate_entities(kind)
            .into_iter()
            .map(|entity| &entity.instance)
            .collect(),
    }
}

/// Final-state checks for structural assembly operations: every reference
/// resolves, and removed objects have no remaining consumers.
fn structural_assembly_failures(
    model: &AssemblyModel,
    operations: &[PatchOperation],
) -> Vec<String> {
    let mut failures = BTreeSet::new();
    let components: BTreeSet<&str> = model.components.iter().map(|c| c.id.as_str()).collect();
    for instance in &model.instances {
        if !components.contains(instance.component.as_str()) {
            failures.insert(format!(
                "instance '{}' references unknown component '{}'",
                instance.id, instance.component
            ));
        }
    }
    for pattern in &model.patterns {
        if !components.contains(pattern.component.as_str()) {
            failures.insert(format!(
                "pattern '{}' references unknown component '{}'",
                pattern.id, pattern.component
            ));
        }
    }
    for operation in operations {
        let (id, dependents): (&str, BTreeSet<String>) = match operation {
            PatchOperation::RemoveComponent { id } => (
                id,
                model
                    .instances
                    .iter()
                    .filter(|instance| instance.component.as_str() == id)
                    .map(|instance| format!("instance {}", instance.id))
                    .chain(
                        model
                            .patterns
                            .iter()
                            .filter(|pattern| pattern.component.as_str() == id)
                            .map(|pattern| format!("pattern {}", pattern.id)),
                    )
                    .collect(),
            ),
            PatchOperation::RemoveInstance { id } => (
                id,
                model
                    .mates
                    .iter()
                    .filter(|mate| {
                        mate_instances(&mate.kind)
                            .iter()
                            .any(|instance| instance.as_str() == id)
                    })
                    .map(|mate| format!("mate {}", mate.id))
                    .chain(
                        model
                            .connectors
                            .iter()
                            .filter(|connector| connector.instance.as_str() == id)
                            .map(|connector| format!("connector {}", connector.id)),
                    )
                    .collect(),
            ),
            _ => continue,
        };
        if !dependents.is_empty() {
            failures.insert(format!(
                "cannot remove '{id}': still used by {}",
                dependents.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    failures.into_iter().collect()
}

pub fn apply_assembly_patch(
    model: &mut AssemblyModel,
    operations: &[PatchOperation],
) -> Result<()> {
    let mut removed = BTreeSet::new();
    // Connectors are referenced by mates through (instance, name); capture
    // the pairs of removed connectors before they disappear.
    let mut removed_connectors = Vec::new();
    for operation in operations {
        match operation {
            PatchOperation::AddComponent { component } => {
                let id = component.id.as_str();
                let exists = model.components.iter().any(|item| item.id == component.id);
                check_new_id(id, "component", exists, &removed, "component")?;
                Component::validate_source_path(&component.source_path)?;
                model.components.push(component.clone());
            }
            PatchOperation::RemoveComponent { id } => {
                remove_by_id(
                    &mut model.components,
                    id,
                    |item| item.id.as_str(),
                    "assembly component",
                    &mut removed,
                )?;
            }
            PatchOperation::AddInstance { instance } => {
                let id = instance.id.as_str();
                let exists = model.instances.iter().any(|item| item.id == instance.id);
                check_new_id(id, "instance", exists, &removed, "instance")?;
                if instance.name.trim().is_empty() {
                    return Err(OpenCadError::validation(format!(
                        "instance '{id}' must have a non-empty name"
                    )));
                }
                model.instances.push(instance.clone());
            }
            PatchOperation::RemoveInstance { id } => {
                remove_by_id(
                    &mut model.instances,
                    id,
                    |item| item.id.as_str(),
                    "assembly instance",
                    &mut removed,
                )?;
            }
            PatchOperation::AddMate { mate } => {
                let id = mate.id.as_str();
                let exists = model.mates.iter().any(|item| item.id == mate.id);
                check_new_id(id, "mate", exists, &removed, "mate")?;
                model.mates.push(mate.as_ref().clone());
            }
            PatchOperation::RemoveMate { id } => {
                remove_by_id(
                    &mut model.mates,
                    id,
                    |item| item.id.as_str(),
                    "assembly mate",
                    &mut removed,
                )?;
            }
            PatchOperation::RemoveConnector { id } => {
                if let Some(connector) = model.connectors.iter().find(|item| item.id.as_str() == id)
                {
                    removed_connectors.push((
                        connector.instance.clone(),
                        connector.name.clone(),
                        id.clone(),
                    ));
                }
                remove_by_id(
                    &mut model.connectors,
                    id,
                    |item| item.id.as_str(),
                    "connector",
                    &mut removed,
                )?;
            }
            PatchOperation::AddAssemblyPattern { pattern } => {
                let id = pattern.id.as_str();
                let exists = model.patterns.iter().any(|item| item.id == pattern.id);
                check_new_id(id, "pattern", exists, &removed, "assembly pattern")?;
                model.patterns.push(pattern.clone());
            }
            PatchOperation::RemoveAssemblyPattern { id } => {
                remove_by_id(
                    &mut model.patterns,
                    id,
                    |item| item.id.as_str(),
                    "assembly pattern",
                    &mut removed,
                )?;
            }
            PatchOperation::SetInstancePlacement {
                instance_id,
                translation_m,
                rotation,
            } => {
                let instance = find_instance_mut(model, instance_id)?;
                instance.placement.transform = RigidTransform {
                    translation_m: *translation_m,
                    rotation: *rotation,
                };
            }
            PatchOperation::SetMateDistance {
                mate_id,
                distance_m,
            } => {
                let mate = find_mate_mut(model, mate_id)?;
                match &mut mate.kind {
                    opencad_assembly::MateKind::Distance {
                        distance_m: target, ..
                    } => {
                        *target = *distance_m;
                    }
                    _ => {
                        return Err(OpenCadError::validation(format!(
                            "mate '{mate_id}' is not a distance mate"
                        )));
                    }
                }
            }
            PatchOperation::AddConnector {
                id,
                name,
                instance_id,
                transform,
            } => {
                model.connectors.push(Connector {
                    id: opencad_core::ConnectorId::new(id)?,
                    name: name.clone(),
                    instance: InstanceId::new(instance_id)?,
                    transform: *transform,
                });
            }
            _ => {}
        }
    }

    // Structural checks run before the model validators so a removal is
    // reported with its complete consumer list.
    let mut failures = structural_assembly_failures(model, operations);
    for (instance, name, id) in removed_connectors {
        if model
            .connectors
            .iter()
            .any(|item| item.instance == instance && item.name == name)
        {
            continue;
        }
        let users: BTreeSet<String> = model
            .mates
            .iter()
            .filter(|mate| {
                mate_entities(&mate.kind).iter().any(|entity| {
                    entity.instance == instance
                        && entity.connector.as_deref() == Some(name.as_str())
                })
            })
            .map(|mate| format!("mate {}", mate.id))
            .collect();
        if !users.is_empty() {
            failures.push(format!(
                "cannot remove '{id}': still used by {}",
                users.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    if !failures.is_empty() {
        failures.sort();
        return Err(OpenCadError::validation(failures.join("; ")));
    }

    let instance_ids: Vec<_> = model.instances.iter().map(|i| i.id.clone()).collect();
    validate_mates(&model.mates, &instance_ids, &model.connectors)?;
    validate_connectors(&model.connectors, &instance_ids)?;
    validate_patterns(model)?;
    Ok(())
}

fn find_instance_mut<'a>(
    model: &'a mut AssemblyModel,
    instance_id: &str,
) -> Result<&'a mut Instance> {
    model
        .instances
        .iter_mut()
        .find(|instance| instance.id.as_str() == instance_id)
        .ok_or_else(|| {
            OpenCadError::validation(format!("unknown assembly instance '{instance_id}'"))
        })
}

fn find_mate_mut<'a>(model: &'a mut AssemblyModel, mate_id: &str) -> Result<&'a mut Mate> {
    model
        .mates
        .iter_mut()
        .find(|mate| mate.id.as_str() == mate_id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown assembly mate '{mate_id}'")))
}

pub fn diff_assembly_models(before: &AssemblyModel, after: &AssemblyModel) -> DesignDiff {
    let mut changes = Vec::new();

    let before_instances: std::collections::BTreeMap<_, _> = before
        .instances
        .iter()
        .map(|instance| (instance.id.as_str().to_string(), instance))
        .collect();
    let after_instances: std::collections::BTreeMap<_, _> = after
        .instances
        .iter()
        .map(|instance| (instance.id.as_str().to_string(), instance))
        .collect();

    for id in before_instances
        .keys()
        .chain(after_instances.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (before_instances.get(id), after_instances.get(id)) {
            (Some(before_instance), Some(after_instance)) => {
                if before_instance.placement != after_instance.placement {
                    changes.push(SemanticChange::AssemblyInstanceChanged {
                        id: id.clone(),
                        field: "placement".into(),
                        before: serde_json::to_string(&before_instance.placement)
                            .unwrap_or_default(),
                        after: serde_json::to_string(&after_instance.placement).unwrap_or_default(),
                    });
                }
                if before_instance.fixed != after_instance.fixed {
                    changes.push(SemanticChange::AssemblyInstanceChanged {
                        id: id.clone(),
                        field: "fixed".into(),
                        before: before_instance.fixed.to_string(),
                        after: after_instance.fixed.to_string(),
                    });
                }
            }
            (Some(_), None) => {
                changes.push(SemanticChange::AssemblyInstanceRemoved { id: id.clone() })
            }
            (None, Some(_)) => {
                changes.push(SemanticChange::AssemblyInstanceAdded { id: id.clone() })
            }
            (None, None) => {}
        }
    }

    let before_mates: std::collections::BTreeMap<_, _> = before
        .mates
        .iter()
        .map(|mate| (mate.id.as_str().to_string(), mate))
        .collect();
    let after_mates: std::collections::BTreeMap<_, _> = after
        .mates
        .iter()
        .map(|mate| (mate.id.as_str().to_string(), mate))
        .collect();

    for id in before_mates
        .keys()
        .chain(after_mates.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (before_mates.get(id), after_mates.get(id)) {
            (Some(before_mate), Some(after_mate)) if before_mate != after_mate => {
                changes.push(SemanticChange::AssemblyMateChanged {
                    id: id.clone(),
                    before: serde_json::to_string(before_mate).unwrap_or_default(),
                    after: serde_json::to_string(after_mate).unwrap_or_default(),
                });
            }
            (Some(_), None) => changes.push(SemanticChange::AssemblyMateRemoved { id: id.clone() }),
            (None, Some(_)) => changes.push(SemanticChange::AssemblyMateAdded { id: id.clone() }),
            _ => {}
        }
    }

    let before_connectors: std::collections::BTreeMap<_, _> = before
        .connectors
        .iter()
        .map(|connector| (connector.id.as_str().to_string(), connector))
        .collect();
    let after_connectors: std::collections::BTreeMap<_, _> = after
        .connectors
        .iter()
        .map(|connector| (connector.id.as_str().to_string(), connector))
        .collect();

    for id in before_connectors
        .keys()
        .chain(after_connectors.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (before_connectors.get(id), after_connectors.get(id)) {
            (Some(before_connector), Some(after_connector))
                if before_connector != after_connector =>
            {
                changes.push(SemanticChange::AssemblyConnectorChanged {
                    id: id.clone(),
                    before: serde_json::to_string(before_connector).unwrap_or_default(),
                    after: serde_json::to_string(after_connector).unwrap_or_default(),
                });
            }
            (Some(_), None) => {
                changes.push(SemanticChange::AssemblyConnectorRemoved { id: id.clone() })
            }
            (None, Some(_)) => {
                changes.push(SemanticChange::AssemblyConnectorAdded { id: id.clone() })
            }
            _ => {}
        }
    }

    diff_items_by_id(
        &before.components,
        &after.components,
        |item| item.id.as_str(),
        |id| SemanticChange::AssemblyComponentAdded { id },
        |id| SemanticChange::AssemblyComponentRemoved { id },
        |id, before, after| SemanticChange::AssemblyComponentChanged { id, before, after },
        &mut changes,
    );
    diff_items_by_id(
        &before.patterns,
        &after.patterns,
        |item| item.id.as_str(),
        |id| SemanticChange::AssemblyPatternAdded { id },
        |id| SemanticChange::AssemblyPatternRemoved { id },
        |id, before, after| SemanticChange::AssemblyPatternChanged { id, before, after },
        &mut changes,
    );

    DesignDiff::semantic(build_summary(&changes), changes)
}

/// Report added, removed, and changed items by stable ID, in ID order.
fn diff_items_by_id<T: serde::Serialize + PartialEq>(
    before: &[T],
    after: &[T],
    id_of: impl Fn(&T) -> &str,
    added: impl Fn(String) -> SemanticChange,
    removed: impl Fn(String) -> SemanticChange,
    changed: impl Fn(String, String, String) -> SemanticChange,
    changes: &mut Vec<SemanticChange>,
) {
    let before: std::collections::BTreeMap<&str, &T> =
        before.iter().map(|item| (id_of(item), item)).collect();
    let after: std::collections::BTreeMap<&str, &T> =
        after.iter().map(|item| (id_of(item), item)).collect();
    let ids: std::collections::BTreeSet<&str> =
        before.keys().chain(after.keys()).copied().collect();
    for id in ids {
        match (before.get(id), after.get(id)) {
            (Some(_), None) => changes.push(removed(id.to_string())),
            (None, Some(_)) => changes.push(added(id.to_string())),
            (Some(old), Some(new)) if old != new => changes.push(changed(
                id.to_string(),
                serde_json::to_string(old).unwrap_or_default(),
                serde_json::to_string(new).unwrap_or_default(),
            )),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_assembly::{Component, Instance, Placement};
    use opencad_core::{ComponentId, DocumentId};

    fn sample_model() -> opencad_core::Result<AssemblyModel> {
        Ok(AssemblyModel {
            components: vec![Component::new(
                ComponentId::new("component:bracket")?,
                "parts/bracket.ocad.d",
                DocumentId::new("doc:bracket_001")?,
            )],
            instances: vec![Instance::new(
                InstanceId::new("instance:left")?,
                ComponentId::new("component:bracket")?,
                Placement::identity(),
                "Left",
            )],
            mates: Vec::new(),
            connectors: Vec::new(),
            patterns: Vec::new(),
        })
    }

    #[test]
    fn patch_moves_instance_placement() -> opencad_core::Result<()> {
        let mut model = sample_model()?;
        let patch = vec![PatchOperation::SetInstancePlacement {
            instance_id: "instance:left".into(),
            translation_m: [0.1, 0.0, 0.0],
            rotation: RigidTransform::identity_rotation(),
        }];
        apply_assembly_patch(&mut model, &patch)?;
        assert!((model.instances[0].placement.transform.translation_m[0] - 0.1).abs() < 1e-9);
        Ok(())
    }

    #[test]
    fn diff_reports_placement_change() -> opencad_core::Result<()> {
        let before = sample_model()?;
        let mut after = before.clone();
        after.instances[0].placement.transform.translation_m[0] = 0.2;
        let diff = diff_assembly_models(&before, &after);
        assert_eq!(diff.changes.len(), 1);
        assert!(matches!(
            diff.changes[0],
            SemanticChange::AssemblyInstanceChanged { .. }
        ));
        Ok(())
    }
}
