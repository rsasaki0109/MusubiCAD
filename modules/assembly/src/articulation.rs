//! Parent-owned independent child coordinates (ADR-059).
use crate::{AssemblyModel, ComponentSourceKind, JointMotion};
use opencad_core::{DocumentId, JointId, OpenCadError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The source revision binds local joint IDs to the exact owning child graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceArticulation {
    pub source_doc: DocumentId,
    pub source_revision: String,
    pub positions: Vec<ChildJointCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildJointCoordinate {
    pub joint_id: JointId,
    pub position: String,
}

/// A unit-bearing coordinate shared with CLI pose/animation admission.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JointValue {
    AngleRad(f64),
    LengthM(f64),
}
impl JointValue {
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim();
        let (number, value): (&str, fn(f64) -> Self) = if let Some(n) = text.strip_suffix("deg") {
            (n, |v| Self::AngleRad(v.to_radians()))
        } else if let Some(n) = text.strip_suffix("rad") {
            (n, Self::AngleRad)
        } else if let Some(n) = text.strip_suffix("mm") {
            (n, |v| Self::LengthM(v / 1000.))
        } else if let Some(n) = text.strip_suffix('m') {
            (n, Self::LengthM)
        } else {
            return Err(OpenCadError::validation(format!(
                "joint position '{text}' needs a unit: deg, rad, mm, or m"
            )));
        };
        let n: f64 = number.trim().parse().map_err(|_| {
            OpenCadError::validation(format!("joint position '{text}' is not a number"))
        })?;
        if !n.is_finite() {
            return Err(OpenCadError::validation(format!(
                "joint position '{text}' must be finite"
            )));
        }
        Ok(value(n))
    }
}

pub fn validate_articulations(model: &AssemblyModel) -> Result<()> {
    if model.has_articulations() && model.instances.len() > 256 {
        return Err(OpenCadError::validation(
            "articulated root instance budget exceeded",
        ));
    }
    for instance in &model.instances {
        let Some(articulation) = &instance.articulation else {
            continue;
        };
        let component = model
            .component(&instance.component)
            .ok_or_else(|| OpenCadError::validation("articulation has unknown component"))?;
        if component.source_kind != ComponentSourceKind::ArticulatedAssembly
            || component.source_doc != articulation.source_doc
        {
            return Err(OpenCadError::validation(
                "articulation source owner or mandatory wire guard mismatch",
            ));
        }
        if articulation.source_revision.is_empty()
            || articulation.source_revision.len() > 256
            || articulation.positions.len() > 64
        {
            return Err(OpenCadError::validation(
                "articulation revision/coordinate budget invalid",
            ));
        }
        let mut ids = BTreeSet::new();
        for coordinate in &articulation.positions {
            let id = coordinate.joint_id.as_str();
            let expr = &coordinate.position;
            JointId::new(id)?;
            if !ids.insert(id) {
                return Err(OpenCadError::validation("duplicate child coordinate"));
            }
            if expr.len() > 128 {
                return Err(OpenCadError::validation(
                    "articulation coordinate expression too long",
                ));
            }
            JointValue::parse(expr)?;
        }
    }
    for component in &model.components {
        let has = model
            .instances
            .iter()
            .any(|i| i.component == component.id && i.articulation.is_some());
        if has != (component.source_kind == ComponentSourceKind::ArticulatedAssembly) {
            return Err(OpenCadError::validation(
                "orphan or missing articulated assembly wire guard",
            ));
        }
        if has && model.patterns.iter().any(|p| p.component == component.id) {
            return Err(OpenCadError::validation(
                "articulated pattern targets are unsupported",
            ));
        }
    }
    Ok(())
}

impl AssemblyModel {
    pub fn has_articulations(&self) -> bool {
        self.instances.iter().any(|i| i.articulation.is_some())
    }
    pub fn require_rigid_kinematics(&self) -> Result<()> {
        if self.has_articulations() {
            return Err(OpenCadError::validation("hierarchical articulated motion/pose/URDF/reach/viewer is unsupported; use static regeneration"));
        }
        Ok(())
    }
}

/// Solve authored rest frames once, then pose a disposable child model.
pub(crate) fn posed_child(
    model: &AssemblyModel,
    articulation: &InstanceArticulation,
) -> Result<AssemblyModel> {
    model.require_rigid_kinematics()?;
    if model.instances.len() > 256 {
        return Err(OpenCadError::validation(
            "articulated child link budget exceeded",
        ));
    }
    if !model.patterns.is_empty() {
        return Err(OpenCadError::validation(
            "articulated child patterns are unsupported",
        ));
    }
    let mut solved = model.clone();
    let tree = crate::kinematic_tree(&solved)?;
    tree.pose(&BTreeMap::new())?; // Authored zero must be within all child limits.
    let mut positions = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for coordinate in &articulation.positions {
        let id = coordinate.joint_id.as_str();
        let expr = &coordinate.position;
        let declared = solved
            .joints
            .iter()
            .find(|j| j.id.as_str() == id)
            .ok_or_else(|| OpenCadError::validation(format!("unknown child joint '{id}'")))?;
        let links: Vec<_> = tree
            .links
            .iter()
            .filter(|l| {
                l.joint
                    .as_ref()
                    .is_some_and(|j| j.mate.as_ref() == Some(&declared.mate) && j.kind.is_movable())
            })
            .collect();
        if links.len() != 1 {
            return Err(OpenCadError::validation(
                "child joint does not resolve to exactly one movable link",
            ));
        }
        let link = links[0];
        if tree.is_follower(link.instance.as_str())
            || !seen.insert(link.instance.as_str().to_owned())
        {
            return Err(OpenCadError::validation(
                "articulation requires unique independent child drivers",
            ));
        }
        let value = match (&declared.motion, JointValue::parse(expr)?) {
            (JointMotion::Revolute { .. } | JointMotion::Continuous, JointValue::AngleRad(v)) => v,
            (JointMotion::Prismatic { .. }, JointValue::LengthM(v)) => v,
            _ => {
                return Err(OpenCadError::validation(
                    "child joint coordinate has wrong unit",
                ))
            }
        };
        positions.insert(link.instance.as_str().to_owned(), value);
    }
    // pose validates omitted zero coordinates and derived coupled limits too.
    let placements: BTreeMap<_, _> = tree
        .pose(&positions)?
        .into_iter()
        .map(|(id, p)| (id.as_str().to_owned(), p))
        .collect();
    for instance in &mut solved.instances {
        instance.placement.transform = *placements
            .get(instance.id.as_str())
            .ok_or_else(|| OpenCadError::validation("posed child link missing"))?;
    }
    Ok(solved)
}
