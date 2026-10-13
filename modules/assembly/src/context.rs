//! Scoped, analytic parent-context connector constraints (ADR-061).
use crate::{AssemblyModel, ComponentSourceKind, Connector, JointKind, JointValue, ResolvedChild};
use opencad_core::{ConnectorId, DocumentId, InstanceId, JointId, MateId, OpenCadError, Result};
use opencad_geometry::RigidTransform;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopedConnectorRef {
    pub instance_path: Vec<InstanceId>,
    /// Assembly document owning the named connector definition.
    pub source_doc: DocumentId,
    pub connector_id: ConnectorId,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextRelation {
    SignedDistance { distance: String },
    SignedAngle { angle: String, turns: i32 },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextConstraint {
    pub id: MateId,
    pub joint_id: JointId,
    pub moving: ScopedConnectorRef,
    pub target: ScopedConnectorRef,
    pub relation: ContextRelation,
}
fn error(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("parent-context: {message}"))
}

pub fn validate_context_source(model: &AssemblyModel) -> Result<()> {
    if model.has_context_constraints()
        && (model.instances.iter().any(|i| !i.fixed)
            || !model.patterns.is_empty()
            || !model.couplings.is_empty())
    {
        return Err(error(
            "outer instances must be fixed; root patterns/couplings unsupported",
        ));
    }
    let mut ids: BTreeSet<_> = model.mates.iter().map(|m| m.id.as_str()).collect();
    for instance in &model.instances {
        let Some(config) = &instance.articulation else {
            continue;
        };
        if config.context_constraints.len() + config.positions.len() > 64 {
            return Err(error("constraint budget exceeded"));
        }
        let mut drivers: BTreeSet<_> = config
            .positions
            .iter()
            .map(|p| p.joint_id.as_str())
            .collect();
        for constraint in &config.context_constraints {
            MateId::new(constraint.id.as_str())?;
            JointId::new(constraint.joint_id.as_str())?;
            if !ids.insert(constraint.id.as_str()) || !drivers.insert(constraint.joint_id.as_str())
            {
                return Err(error(
                    "duplicate constraint or saved/solved driver conflict",
                ));
            }
            if constraint.moving.instance_path.len() != 2
                || constraint.moving.instance_path[0] != instance.id
                || constraint.moving.source_doc != config.source_doc
                || constraint.target.instance_path.len() != 1
                || constraint.target.instance_path[0] == instance.id
            {
                return Err(error("unsupported or wrong-owner connector scope"));
            }
            match &constraint.relation {
                ContextRelation::SignedDistance { distance } => {
                    if distance.len() > 128
                        || !matches!(JointValue::parse(distance)?, JointValue::LengthM(_))
                    {
                        return Err(error("distance needs a finite explicit length"));
                    }
                }
                ContextRelation::SignedAngle { angle, turns } => {
                    let JointValue::AngleRad(v) = JointValue::parse(angle)? else {
                        return Err(error("angle needs an explicit angle unit"));
                    };
                    if angle.len() > 128
                        || v.abs() > std::f64::consts::PI
                        || !(-32..=32).contains(turns)
                    {
                        return Err(error("angle/branch budget invalid"));
                    }
                }
            }
        }
    }
    Ok(())
}
fn find_instance<'a>(model: &'a AssemblyModel, id: &InstanceId) -> Option<&'a crate::Instance> {
    model.instances.iter().find(|i| i.id == *id)
}
fn connector<'a>(
    model: &'a AssemblyModel,
    reference: &ScopedConnectorRef,
    owner: &DocumentId,
) -> Result<&'a Connector> {
    if reference.source_doc != *owner {
        return Err(error("connector source owner mismatch"));
    }
    let instance = reference
        .instance_path
        .last()
        .ok_or_else(|| error("empty connector scope"))?;
    let candidates: Vec<_> = model
        .connectors
        .iter()
        .filter(|c| c.id == reference.connector_id && c.instance == *instance)
        .collect();
    if candidates.len() != 1 {
        return Err(error("missing or ambiguous exact connector"));
    }
    validate_frame(candidates[0].transform)?;
    Ok(candidates[0])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn vector(t: RigidTransform, v: [f64; 3]) -> [f64; 3] {
    sub(t.transform_point(v), t.translation_m)
}
fn normal(t: RigidTransform) -> [f64; 3] {
    vector(t, [0., 0., 1.])
}
fn validate_frame(t: RigidTransform) -> Result<()> {
    if t.translation_m
        .iter()
        .chain(t.rotation.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(error("nonfinite connector frame"));
    }
    let inverse = t.inverse()?;
    if !RigidTransform::matrices_near(
        &t.compose(inverse).rotation,
        &RigidTransform::identity_rotation(),
        1e-9,
    ) {
        return Err(error("connector requires an orthonormal frame"));
    }
    if (dot(
        cross(vector(t, [1., 0., 0.]), vector(t, [0., 1., 0.])),
        normal(t),
    ) - 1.)
        .abs()
        > 1e-9
    {
        return Err(error("connector requires a proper rigid frame"));
    }
    Ok(())
}
fn signed_angle(axis: [f64; 3], target: [f64; 3], moving: [f64; 3]) -> Result<f64> {
    if dot(axis, target).abs() > 1e-9 || dot(axis, moving).abs() > 1e-9 {
        return Err(error(
            "angle connector normals must be perpendicular to joint axis",
        ));
    }
    Ok(dot(axis, cross(target, moving)).atan2(dot(target, moving)))
}

/// Returns a disposable coordinate-resolved model; original graph stays intact.
pub(crate) fn solve_parent_context(
    model: &AssemblyModel,
    owner: &DocumentId,
    root: &Path,
    load: &mut dyn FnMut(&Path) -> Result<ResolvedChild>,
) -> Result<AssemblyModel> {
    validate_context_source(model)?;
    let mut result = model.clone();
    for instance in &model.instances {
        let Some(config) = instance
            .articulation
            .as_ref()
            .filter(|a| !a.context_constraints.is_empty())
        else {
            continue;
        };
        let component = model
            .component(&instance.component)
            .ok_or_else(|| error("unknown component"))?;
        let path = crate::validate_component_path(root, &component.source_path)?;
        let ResolvedChild::Versioned { revision, child } = load(&path)? else {
            return Err(error("versioned child required"));
        };
        if revision != config.source_revision {
            return Err(error("stale child revision"));
        }
        let ResolvedChild::Assembly {
            model: child,
            doc_id,
        } = *child
        else {
            return Err(error("child assembly required"));
        };
        if doc_id != config.source_doc {
            return Err(error("child owner mismatch"));
        }
        child.require_rigid_kinematics()?;
        if !child.patterns.is_empty() || child.instances.len() > 256 {
            return Err(error("child patterns/link budget unsupported"));
        }
        let (placements, _) = crate::solve_assembly_mates(&child)?;
        let mut child = *child;
        child.instances = placements;
        let tree = crate::kinematic_tree(&child)?;
        tree.pose(&BTreeMap::new())?;
        let mut independent = BTreeMap::new();
        for link in &tree.links {
            let Some(joint) = link.joint.as_ref().filter(|j| j.kind.is_movable()) else {
                continue;
            };
            let declared: Vec<_> = child
                .joints
                .iter()
                .filter(|j| joint.mate.as_ref() == Some(&j.mate))
                .collect();
            if declared.len() != 1 {
                return Err(error("all movable links require unique declared joints"));
            }
            if !tree.is_follower(link.instance.as_str()) {
                independent.insert(declared[0].id.as_str(), link);
            }
        }
        let bound: BTreeSet<_> = config
            .positions
            .iter()
            .map(|p| p.joint_id.as_str())
            .chain(
                config
                    .context_constraints
                    .iter()
                    .map(|c| c.joint_id.as_str()),
            )
            .collect();
        if bound != independent.keys().copied().collect() {
            return Err(error(
                "unknown/follower or unconstrained independent coordinates",
            ));
        }
        let mut evaluation = config.clone();
        evaluation.context_constraints.clear();
        let rest = crate::articulation::posed_child(&child, &evaluation)?;
        let mut constraints: Vec<_> = config.context_constraints.iter().collect();
        constraints.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        let mut verification = Vec::new();
        for constraint in constraints {
            let link = independent[constraint.joint_id.as_str()];
            let joint = link.joint.as_ref().ok_or_else(|| error("missing driver"))?;
            if joint.parent != tree.links[0].instance
                || constraint.moving.instance_path[1] != link.instance
            {
                return Err(error(
                    "context driver must be an immediate child of grounded root",
                ));
            }
            let moving = connector(&child, &constraint.moving, &doc_id)?;
            let moving_instance = find_instance(&child, &moving.instance)
                .ok_or_else(|| error("moving link missing"))?;
            if child
                .component(&moving_instance.component)
                .map(|c| c.source_kind)
                != Some(ComponentSourceKind::Part)
            {
                return Err(error("moving connector must identify a direct part leaf"));
            }
            let target = connector(model, &constraint.target, owner)?;
            let target_instance = find_instance(model, &target.instance)
                .ok_or_else(|| error("target instance missing"))?;
            if model
                .component(&target_instance.component)
                .map(|c| c.source_kind)
                != Some(ComponentSourceKind::Part)
            {
                return Err(error("target must be a fixed parent part leaf"));
            }
            validate_frame(instance.placement.transform)?;
            validate_frame(target_instance.placement.transform)?;
            let target_world = target_instance
                .placement
                .transform
                .compose(target.transform);
            let moving_world = instance
                .placement
                .transform
                .compose(
                    find_instance(&rest, &link.instance)
                        .ok_or_else(|| error("posed link missing"))?
                        .placement
                        .transform,
                )
                .compose(moving.transform);
            let axis_local = match joint.kind {
                JointKind::Prismatic { axis, .. } | JointKind::Revolute { axis, .. } => axis,
                _ => {
                    return Err(error(
                        "only bounded prismatic/revolute context drivers supported",
                    ))
                }
            };
            let axis = vector(instance.placement.transform.compose(link.world), axis_local);
            let value = match (&constraint.relation, joint.kind) {
                (ContextRelation::SignedDistance { distance }, JointKind::Prismatic { .. }) => {
                    let JointValue::LengthM(want) = JointValue::parse(distance)? else {
                        return Err(error("distance unit"));
                    };
                    let slope = dot(axis, normal(target_world));
                    if slope.abs() <= 1e-9 {
                        return Err(error("singular distance constraint"));
                    }
                    (want
                        - dot(
                            sub(moving_world.translation_m, target_world.translation_m),
                            normal(target_world),
                        ))
                        / slope
                }
                (ContextRelation::SignedAngle { angle, turns }, JointKind::Revolute { .. }) => {
                    let JointValue::AngleRad(want) = JointValue::parse(angle)? else {
                        return Err(error("angle unit"));
                    };
                    want - signed_angle(axis, normal(target_world), normal(moving_world))?
                        + std::f64::consts::TAU * f64::from(*turns)
                }
                _ => return Err(error("constraint/joint kind mismatch")),
            };
            if !value.is_finite() {
                return Err(error("nonfinite solution"));
            }
            evaluation.positions.push(crate::ChildJointCoordinate {
                joint_id: constraint.joint_id.clone(),
                position: format!(
                    "{value} {}",
                    if matches!(joint.kind, JointKind::Prismatic { .. }) {
                        "m"
                    } else {
                        "rad"
                    }
                ),
            });
            verification.push((constraint, moving.transform, target_world, axis));
        }
        let posed = crate::articulation::posed_child(&child, &evaluation)?;
        for (constraint, local, target, axis) in verification {
            let world = instance
                .placement
                .transform
                .compose(
                    find_instance(&posed, &constraint.moving.instance_path[1])
                        .ok_or_else(|| error("solved leaf missing"))?
                        .placement
                        .transform,
                )
                .compose(local);
            let residual = match &constraint.relation {
                ContextRelation::SignedDistance { distance } => {
                    let JointValue::LengthM(want) = JointValue::parse(distance)? else {
                        return Err(error("length unit"));
                    };
                    dot(
                        sub(world.translation_m, target.translation_m),
                        normal(target),
                    ) - want
                }
                ContextRelation::SignedAngle { angle, .. } => {
                    let JointValue::AngleRad(want) = JointValue::parse(angle)? else {
                        return Err(error("angle unit"));
                    };
                    let delta = signed_angle(axis, normal(target), normal(world))? - want;
                    delta.sin().atan2(delta.cos())
                }
            };
            if !residual.is_finite() || residual.abs() > 1e-9 {
                return Err(error("solved connector residual exceeds tolerance"));
            }
        }
        result
            .instances
            .iter_mut()
            .find(|i| i.id == instance.id)
            .ok_or_else(|| error("copy missing"))?
            .articulation = Some(evaluation);
    }
    for component in &mut result.components {
        if component.source_kind == ComponentSourceKind::FlexibleAssembly {
            component.source_kind = ComponentSourceKind::ArticulatedAssembly;
        }
    }
    Ok(result)
}
