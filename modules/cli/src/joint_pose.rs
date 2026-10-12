//! Read-only exact-ID, explicit-unit joint pose inspection (ADR-056).
use crate::joint_animation::{solved_assembly, JointValue};
use opencad_assembly::{kinematic_tree, JointMotion};
use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointPoseRequest {
    pub path: String,
    #[serde(default)]
    pub positions: BTreeMap<String, String>,
}
#[derive(Serialize)]
pub struct JointPosition {
    pub joint_id: String,
    pub instance_id: String,
    pub derived: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_rad: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_m: Option<f64>,
}
#[derive(Serialize)]
pub struct JointPoseInspection {
    pub positions: Vec<JointPosition>,
    pub placements: BTreeMap<String, opencad_geometry::RigidTransform>,
}
pub fn inspect(request: &JointPoseRequest) -> Result<JointPoseInspection> {
    let doc = opencad_file::read_ocad(&request.path)?;
    let assembly = doc
        .assembly
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("joint pose requires an assembly"))?;
    assembly.validate(&doc.metadata.id)?;
    let assembly = solved_assembly(assembly)?;
    let tree = kinematic_tree(&assembly)?;
    let mut source_links = BTreeMap::new();
    for declared in &assembly.joints {
        let mut matches = tree.links.iter().filter(|l| {
            l.joint
                .as_ref()
                .is_some_and(|j| j.mate.as_ref() == Some(&declared.mate) && j.kind.is_movable())
        });
        if let Some(link) = matches.next() {
            if matches.next().is_some() {
                return Err(OpenCadError::validation("ambiguous declared joint link"));
            }
            source_links.insert(declared.id.as_str(), (declared, link));
        }
    }
    let mut input = BTreeMap::new();
    for (id, text) in &request.positions {
        let (declared, link) = source_links.get(id.as_str()).ok_or_else(|| {
            OpenCadError::validation(format!("unknown or non-moving declared joint '{id}'"))
        })?;
        let value = match (&declared.motion, JointValue::parse(text)?) {
            (JointMotion::Revolute { .. } | JointMotion::Continuous, JointValue::AngleRad(v)) => v,
            (JointMotion::Prismatic { .. }, JointValue::LengthM(v)) => v,
            _ => {
                return Err(OpenCadError::validation(
                    "joint coordinate unit does not match motion kind",
                ))
            }
        };
        input.insert(link.instance.as_str().to_owned(), value);
    }
    let values = tree.resolve_positions(&input)?;
    let placements = tree
        .pose(&input)?
        .into_iter()
        .map(|(id, transform)| (id.as_str().to_owned(), transform))
        .collect();
    let positions = source_links
        .into_iter()
        .map(|(id, (declared, link))| {
            let value = values.get(link.instance.as_str()).copied().unwrap_or(0.);
            let linear = matches!(declared.motion, JointMotion::Prismatic { .. });
            JointPosition {
                joint_id: id.to_owned(),
                instance_id: link.instance.as_str().to_owned(),
                derived: tree.is_follower(link.instance.as_str()),
                position_rad: (!linear).then_some(value),
                position_m: linear.then_some(value),
            }
        })
        .collect();
    Ok(JointPoseInspection {
        positions,
        placements,
    })
}
pub fn command(args: Vec<String>) -> Result<()> {
    let path = args.first().ok_or_else(|| {
        OpenCadError::validation(
            "usage: musubicad inspect-joint-pose <assembly> [joint:id=90deg ...]",
        )
    })?;
    let mut positions = BTreeMap::new();
    for item in &args[1..] {
        let (id, value) = item.split_once('=').ok_or_else(|| {
            OpenCadError::validation("pose coordinate must be joint:id=value_with_unit")
        })?;
        if positions.insert(id.to_owned(), value.to_owned()).is_some() {
            return Err(OpenCadError::validation("duplicate joint coordinate"));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&inspect(&JointPoseRequest {
            path: path.clone(),
            positions
        })?)?
    );
    Ok(())
}
