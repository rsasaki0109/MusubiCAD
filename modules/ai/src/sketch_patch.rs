//! Structural sketch operations for `DesignPatch` (ADR-013 slice 2).
//!
//! Sketch operations create or remove whole sketches, sketch entities, and
//! sketch constraints.  Local checks run per operation; referential checks
//! run once on the final candidate.  Profiles of every touched sketch are
//! re-detected from its entities and its solve state is reset, because both
//! are derived data that regeneration recomputes.

use std::collections::{BTreeMap, BTreeSet};

use opencad_core::{OpenCadError, Result, SketchId};
use opencad_feature::{prepare_sketch, resolve_sketch_profile, FeatureDefinition, FeatureNode};
use opencad_graph::{parameter_names_in_expr, SemanticChange};
use opencad_sketch::{Sketch, SketchEntity, SolveState, Workplane};

use crate::patch::{validate_stable_id, PatchOperation};
use crate::DesignState;

/// Minimum length of a custom workplane normal or x axis (dimensionless
/// direction vectors).  Shorter vectors have no reliable direction.
pub const WORKPLANE_AXIS_MIN_NORM: f64 = 1e-9;

/// Maximum `|cos θ|` between a custom workplane normal and x axis
/// (dimensionless).  Larger values mean the axes are not perpendicular.
pub const WORKPLANE_ORTHOGONALITY_TOLERANCE: f64 = 1e-6;

/// Sketch ID targeted by a sketch operation, if the operation is one.
pub(crate) fn operation_sketch_id(operation: &PatchOperation) -> Option<&str> {
    match operation {
        PatchOperation::AddSketch { id, .. } | PatchOperation::RemoveSketch { id } => Some(id),
        PatchOperation::AddSketchEntity { sketch_id, .. }
        | PatchOperation::RemoveSketchEntity { sketch_id, .. }
        | PatchOperation::AddSketchConstraint { sketch_id, .. }
        | PatchOperation::RemoveSketchConstraint { sketch_id, .. } => Some(sketch_id),
        _ => None,
    }
}

fn find_sketch_mut<'a>(sketches: &'a mut [Sketch], id: &str) -> Result<&'a mut Sketch> {
    sketches
        .iter_mut()
        .find(|sketch| sketch.id.as_str() == id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown sketch '{id}'")))
}

fn validate_workplane(workplane: &Workplane) -> Result<()> {
    let Workplane::Custom {
        origin,
        normal,
        x_axis,
    } = workplane
    else {
        return Ok(());
    };
    let finite = |values: &[f64; 3]| values.iter().all(|value| value.is_finite());
    if !finite(origin) || !finite(normal) || !finite(x_axis) {
        return Err(OpenCadError::validation(
            "custom workplane origin (m), normal, and x_axis must be finite",
        ));
    }
    let norm = |values: &[f64; 3]| values.iter().map(|value| value * value).sum::<f64>().sqrt();
    let (normal_norm, x_norm) = (norm(normal), norm(x_axis));
    if normal_norm < WORKPLANE_AXIS_MIN_NORM || x_norm < WORKPLANE_AXIS_MIN_NORM {
        return Err(OpenCadError::validation(format!(
            "custom workplane normal and x_axis must have length >= {WORKPLANE_AXIS_MIN_NORM} (dimensionless)"
        )));
    }
    let dot: f64 = normal.iter().zip(x_axis).map(|(a, b)| a * b).sum();
    let cosine = (dot / (normal_norm * x_norm)).abs();
    if cosine > WORKPLANE_ORTHOGONALITY_TOLERANCE {
        return Err(OpenCadError::validation(format!(
            "custom workplane x_axis must be perpendicular to normal (|cos| = {cosine:e} exceeds {WORKPLANE_ORTHOGONALITY_TOLERANCE:e})"
        )));
    }
    Ok(())
}

/// Apply sketch operations in order with local checks only.
pub(crate) fn apply_sketch_operations(
    operations: &[PatchOperation],
    sketches: &mut Vec<Sketch>,
) -> Result<()> {
    let mut removed_sketches = BTreeSet::new();
    // Entity and constraint IDs have distinct prefixes, so one set of
    // (sketch, member) pairs covers both.
    let mut removed_members: BTreeSet<(String, String)> = BTreeSet::new();
    let mut touched = BTreeSet::new();

    for operation in operations {
        match operation {
            PatchOperation::AddSketch {
                id,
                name,
                workplane,
            } => {
                validate_stable_id(id, "sketch")?;
                if name.trim().is_empty() {
                    return Err(OpenCadError::validation(format!(
                        "sketch '{id}' must have a non-empty name"
                    )));
                }
                validate_workplane(workplane)?;
                if removed_sketches.contains(id.as_str()) {
                    return Err(OpenCadError::validation(format!(
                        "sketch id '{id}' was removed earlier in this patch and cannot be reused"
                    )));
                }
                if sketches.iter().any(|sketch| sketch.id.as_str() == id) {
                    return Err(OpenCadError::validation(format!(
                        "sketch '{id}' already exists"
                    )));
                }
                sketches.push(Sketch::new(SketchId::new(id)?, name, workplane.clone()));
                touched.insert(id.clone());
            }
            PatchOperation::RemoveSketch { id } => {
                let index = sketches
                    .iter()
                    .position(|sketch| sketch.id.as_str() == id)
                    .ok_or_else(|| OpenCadError::validation(format!("unknown sketch '{id}'")))?;
                sketches.remove(index);
                removed_sketches.insert(id.as_str());
                touched.remove(id);
            }
            PatchOperation::AddSketchEntity { sketch_id, entity } => {
                let sketch = find_sketch_mut(sketches, sketch_id)?;
                let mut existing: BTreeSet<&str> = sketch
                    .entities
                    .iter()
                    .flat_map(SketchEntity::defined_ids)
                    .map(|id| id.as_str())
                    .collect();
                for defined in entity.defined_ids() {
                    let defined = defined.as_str();
                    validate_stable_id(defined, "ent")?;
                    if removed_members.contains(&(sketch_id.clone(), defined.to_string())) {
                        return Err(OpenCadError::validation(format!(
                            "entity id '{defined}' was removed from '{sketch_id}' earlier in this patch and cannot be reused"
                        )));
                    }
                    if !existing.insert(defined) {
                        return Err(OpenCadError::validation(format!(
                            "entity '{defined}' already exists in sketch '{sketch_id}'"
                        )));
                    }
                }
                sketch.entities.push(entity.clone());
                touched.insert(sketch_id.clone());
            }
            PatchOperation::RemoveSketchEntity {
                sketch_id,
                entity_id,
            } => {
                let sketch = find_sketch_mut(sketches, sketch_id)?;
                let index = sketch
                    .entities
                    .iter()
                    .position(|entity| entity.id().as_str() == entity_id)
                    .ok_or_else(|| {
                        OpenCadError::validation(format!(
                            "unknown entity '{entity_id}' in sketch '{sketch_id}'"
                        ))
                    })?;
                let removed = sketch.entities.remove(index);
                for defined in removed.defined_ids() {
                    removed_members.insert((sketch_id.clone(), defined.as_str().to_string()));
                }
                touched.insert(sketch_id.clone());
            }
            PatchOperation::AddSketchConstraint {
                sketch_id,
                constraint,
            } => {
                let constraint_id = constraint.id().as_str();
                validate_stable_id(constraint_id, "con")?;
                if removed_members.contains(&(sketch_id.clone(), constraint_id.to_string())) {
                    return Err(OpenCadError::validation(format!(
                        "constraint id '{constraint_id}' was removed from '{sketch_id}' earlier in this patch and cannot be reused"
                    )));
                }
                let sketch = find_sketch_mut(sketches, sketch_id)?;
                sketch.add_constraint(constraint.clone()).map_err(|_| {
                    OpenCadError::validation(format!(
                        "constraint '{constraint_id}' already exists in sketch '{sketch_id}'"
                    ))
                })?;
                touched.insert(sketch_id.clone());
            }
            PatchOperation::RemoveSketchConstraint {
                sketch_id,
                constraint_id,
            } => {
                let sketch = find_sketch_mut(sketches, sketch_id)?;
                let index = sketch
                    .constraints
                    .iter()
                    .position(|constraint| constraint.id().as_str() == constraint_id)
                    .ok_or_else(|| {
                        OpenCadError::validation(format!(
                            "unknown constraint '{constraint_id}' in sketch '{sketch_id}'"
                        ))
                    })?;
                sketch.constraints.remove(index);
                removed_members.insert((sketch_id.clone(), constraint_id.clone()));
                touched.insert(sketch_id.clone());
            }
            _ => {}
        }
    }

    for id in touched {
        let sketch = find_sketch_mut(sketches, &id)?;
        sketch.update_profiles()?;
        sketch.solve_state = SolveState::Unknown;
    }
    Ok(())
}

/// Referential checks for every sketch touched by the patch, run on the
/// final candidate.  Failures are collected in sorted order.
pub(crate) fn validate_sketch_candidate(
    operations: &[PatchOperation],
    after: &DesignState,
    failures: &mut BTreeSet<String>,
) {
    let targeted: BTreeSet<&str> = operations.iter().filter_map(operation_sketch_id).collect();
    if targeted.is_empty() {
        return;
    }
    let touched: Vec<&Sketch> = after
        .sketches
        .iter()
        .filter(|sketch| targeted.contains(sketch.id.as_str()))
        .collect();

    for sketch in &touched {
        validate_sketch_references(sketch, after, failures);
    }

    // Removed sketches must not be used by any remaining sketch feature.
    for operation in operations {
        let PatchOperation::RemoveSketch { id } = operation else {
            continue;
        };
        if after.sketches.iter().any(|sketch| sketch.id.as_str() == id) {
            continue;
        }
        let users: BTreeSet<&str> = after
            .feature_nodes
            .iter()
            .filter(|node| {
                matches!(&node.definition, FeatureDefinition::Sketch(def) if def.sketch_id == *id)
            })
            .map(|node| node.id.as_str())
            .collect();
        if !users.is_empty() {
            failures.insert(format!(
                "cannot remove sketch '{id}': still used by feature {}",
                users.into_iter().collect::<Vec<_>>().join(", feature ")
            ));
        }
    }

    // Every profile consumer of a touched sketch must resolve to a closed
    // profile, using the same lookup regeneration uses.
    let touched_ids: BTreeSet<&str> = touched.iter().map(|sketch| sketch.id.as_str()).collect();
    validate_profile_consumers(
        after,
        |_, sketch_id| touched_ids.contains(sketch_id),
        failures,
    );
}

/// Check that every extrude, hole, and revolve selected by `select` (given
/// the feature and the sketch it consumes) resolves its `profile_ref` to a
/// closed profile on a prepared copy of that sketch.
pub(crate) fn validate_profile_consumers(
    after: &DesignState,
    select: impl Fn(&FeatureNode, &str) -> bool,
    failures: &mut BTreeSet<String>,
) {
    let sketch_of_feature: BTreeMap<&str, &str> = after
        .feature_nodes
        .iter()
        .filter_map(|node| match &node.definition {
            FeatureDefinition::Sketch(def) => Some((node.id.as_str(), def.sketch_id.as_str())),
            _ => None,
        })
        .collect();
    let mut prepared: BTreeMap<&str, std::result::Result<Sketch, String>> = BTreeMap::new();
    for node in &after.feature_nodes {
        let (sketch_feature, profile_ref) = match &node.definition {
            FeatureDefinition::Extrude(def) => (&def.sketch_feature, &def.profile_ref),
            FeatureDefinition::Hole(def) => (&def.sketch_feature, &def.profile_ref),
            FeatureDefinition::Revolve(def) => (&def.sketch_feature, &def.profile_ref),
            _ => continue,
        };
        let Some(sketch_id) = sketch_of_feature.get(sketch_feature.as_str()).copied() else {
            continue;
        };
        if !select(node, sketch_id) {
            continue;
        }
        let Some(sketch) = after
            .sketches
            .iter()
            .find(|sketch| sketch.id.as_str() == sketch_id)
        else {
            continue;
        };
        let prepared = prepared.entry(sketch_id).or_insert_with(|| {
            let mut copy = sketch.clone();
            prepare_sketch(&mut copy)
                .map(|()| copy)
                .map_err(|error| error.to_string())
        });
        match prepared {
            Err(error) => {
                failures.insert(format!("sketch '{sketch_id}' cannot be prepared: {error}"));
            }
            Ok(prepared) => match resolve_sketch_profile(prepared, profile_ref) {
                Some(profile) if profile.is_closed() => {}
                Some(_) => {
                    failures.insert(format!(
                        "feature '{}' profile '{profile_ref}' in sketch '{sketch_id}' is not closed",
                        node.id
                    ));
                }
                None => {
                    failures.insert(format!(
                        "feature '{}' profile '{profile_ref}' does not resolve in sketch '{sketch_id}'",
                        node.id
                    ));
                }
            },
        }
    }
}

pub(crate) fn validate_sketch_references(
    sketch: &Sketch,
    after: &DesignState,
    failures: &mut BTreeSet<String>,
) {
    let sketch_id = sketch.id.as_str();
    let defined: BTreeSet<&str> = sketch
        .entities
        .iter()
        .flat_map(SketchEntity::defined_ids)
        .map(|id| id.as_str())
        .collect();
    let points: BTreeSet<&str> = sketch
        .entities
        .iter()
        .flat_map(|entity| match entity {
            SketchEntity::Point(point) => vec![point.base.id.as_str()],
            SketchEntity::Rectangle(rectangle) => {
                rectangle.corner_ids.iter().map(|id| id.as_str()).collect()
            }
            _ => Vec::new(),
        })
        .collect();

    let mut check_expression = |owner: String, expr: &str| {
        for name in parameter_names_in_expr(expr) {
            if after.parameters.find_by_name(&name).is_none() {
                failures.insert(format!(
                    "sketch '{sketch_id}': {owner} expression '{expr}' references unknown parameter '{name}'"
                ));
            }
        }
    };
    for entity in &sketch.entities {
        for expr in entity.expressions() {
            check_expression(format!("entity '{}'", entity.id()), expr.as_str());
        }
    }
    for constraint in &sketch.constraints {
        if let Some(expr) = constraint.expression() {
            check_expression(format!("constraint '{}'", constraint.id()), expr.as_str());
        }
    }

    for entity in &sketch.entities {
        for point in entity.point_refs() {
            if !points.contains(point.as_str()) {
                failures.insert(format!(
                    "sketch '{sketch_id}': entity '{}' references missing point '{point}'",
                    entity.id()
                ));
            }
        }
    }
    for constraint in &sketch.constraints {
        for entity in constraint.entity_refs() {
            if !defined.contains(entity.as_str()) {
                failures.insert(format!(
                    "sketch '{sketch_id}': constraint '{}' references missing entity '{entity}'",
                    constraint.id()
                ));
            }
        }
    }
    let constraint_ids: BTreeSet<&str> = sketch
        .constraints
        .iter()
        .map(|constraint| constraint.id().as_str())
        .collect();
    for dimension in &sketch.dimensions {
        if !constraint_ids.contains(dimension.id.as_str()) {
            failures.insert(format!(
                "sketch '{sketch_id}': dimension '{}' has no matching constraint",
                dimension.id
            ));
        }
    }
    if let Workplane::FaceRef { face_ref } = &sketch.workplane {
        if !after
            .semantic_refs
            .iter()
            .any(|topo_ref| topo_ref.ref_id.as_str() == face_ref)
        {
            failures.insert(format!(
                "sketch '{sketch_id}': workplane references unknown semantic reference '{face_ref}'"
            ));
        }
    }
}

/// Report added and removed sketches, entities, and constraints by stable
/// ID.  Derived profile and solve-state changes are not reported.
pub(crate) fn diff_sketches(before: &[Sketch], after: &[Sketch]) -> Vec<SemanticChange> {
    let by_id = |sketches: &'_ [Sketch]| -> BTreeMap<String, Sketch> {
        sketches
            .iter()
            .map(|sketch| (sketch.id.as_str().to_string(), sketch.clone()))
            .collect()
    };
    let (before, after) = (by_id(before), by_id(after));
    let ids: BTreeSet<&String> = before.keys().chain(after.keys()).collect();

    let mut changes = Vec::new();
    for id in ids {
        match (before.get(id), after.get(id)) {
            (Some(_), None) => changes.push(SemanticChange::SketchRemoved { id: id.clone() }),
            (None, Some(_)) => changes.push(SemanticChange::SketchAdded { id: id.clone() }),
            (Some(old), Some(new)) => {
                let entity_ids = |sketch: &Sketch| -> BTreeSet<String> {
                    sketch
                        .entities
                        .iter()
                        .map(|entity| entity.id().as_str().to_string())
                        .collect()
                };
                let constraint_ids = |sketch: &Sketch| -> BTreeSet<String> {
                    sketch
                        .constraints
                        .iter()
                        .map(|constraint| constraint.id().as_str().to_string())
                        .collect()
                };
                let (old_entities, new_entities) = (entity_ids(old), entity_ids(new));
                for entity in new_entities.difference(&old_entities) {
                    changes.push(SemanticChange::SketchEntityAdded {
                        sketch_id: id.clone(),
                        id: entity.clone(),
                    });
                }
                for entity in old_entities.difference(&new_entities) {
                    changes.push(SemanticChange::SketchEntityRemoved {
                        sketch_id: id.clone(),
                        id: entity.clone(),
                    });
                }
                let (old_constraints, new_constraints) = (constraint_ids(old), constraint_ids(new));
                for constraint in new_constraints.difference(&old_constraints) {
                    changes.push(SemanticChange::SketchConstraintAdded {
                        sketch_id: id.clone(),
                        id: constraint.clone(),
                    });
                }
                for constraint in old_constraints.difference(&new_constraints) {
                    changes.push(SemanticChange::SketchConstraintRemoved {
                        sketch_id: id.clone(),
                        id: constraint.clone(),
                    });
                }
            }
            (None, None) => {}
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_workplanes_must_be_finite_non_degenerate_and_orthogonal() {
        let custom = |normal: [f64; 3], x_axis: [f64; 3]| Workplane::Custom {
            origin: [0.0, 0.0, 0.01],
            normal,
            x_axis,
        };
        assert!(validate_workplane(&custom([0.0, 0.0, 1.0], [1.0, 0.0, 0.0])).is_ok());
        assert!(validate_workplane(&custom([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])).is_err());
        assert!(validate_workplane(&custom([0.0, 0.0, 1.0], [1.0, 0.0, 0.5])).is_err());
        assert!(validate_workplane(&custom([0.0, 0.0, f64::NAN], [1.0, 0.0, 0.0])).is_err());
        assert!(validate_workplane(&Workplane::xy()).is_ok());
    }
}
