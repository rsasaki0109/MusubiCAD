//! Kinematic tree derived from assembly mates (ADR-028).
//!
//! Robot description formats such as URDF need a tree of rigid links joined
//! by joints.  This module derives one from the Design Graph instead of
//! storing a second, hand-written description:
//!
//! - the grounded instance is the root link;
//! - mates join instances; a pair joined by a `concentric` mate becomes a
//!   continuous (unlimited revolute) joint about that mate's axis, and any
//!   other mated pair becomes a fixed joint;
//! - instances that no mate reaches are fixed to the root, with a warning.
//!
//! The zero position of every joint is the assembly's current pose.  Axial
//! sliding that a lone concentric mate would also allow is not modelled.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use opencad_core::{InstanceId, MateId, OpenCadError, Result};
use opencad_geometry::RigidTransform;

use crate::connector::resolve_mate_entity_frame;
use crate::instance::Instance;
use crate::mate::{MateEntity, MateKind};
use crate::model::AssemblyModel;

/// Axis directions shorter than this are rejected (unitless, normalised input).
const MIN_AXIS_LENGTH: f64 = 1e-9;

/// How a child link moves relative to its parent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JointKind {
    /// Unlimited rotation about `axis` (expressed in the child link frame).
    Continuous {
        axis: [f64; 3],
    },
    Fixed,
}

/// Joint that attaches a child link to its parent.
#[derive(Debug, Clone, PartialEq)]
pub struct KinematicJoint {
    /// Mate the joint was derived from; `None` for unmated instances fixed to the root.
    pub mate: Option<MateId>,
    pub parent: InstanceId,
    pub kind: JointKind,
    /// Child link frame in the parent link frame at the zero position.
    pub origin: RigidTransform,
}

/// One rigid body of the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct KinematicLink {
    pub instance: InstanceId,
    /// Link frame expressed in the instance (part) frame: geometry, mass, and
    /// inertia given in part coordinates are moved into the link frame by the
    /// inverse of this transform.  It is a pure translation.
    pub frame_in_part: RigidTransform,
    /// World pose of the link frame at the zero position.
    pub world: RigidTransform,
    /// `None` for the root link.
    pub joint: Option<KinematicJoint>,
}

/// Links in parent-before-child order, root first.
#[derive(Debug, Clone, PartialEq)]
pub struct KinematicTree {
    pub links: Vec<KinematicLink>,
    /// Non-fatal interpretation notes, for example unmated instances.
    pub warnings: Vec<String>,
}

/// Derive a kinematic tree from the assembly's instances and mates.
pub fn kinematic_tree(model: &AssemblyModel) -> Result<KinematicTree> {
    let instances: BTreeMap<&str, &Instance> = model
        .instances
        .iter()
        .map(|instance| (instance.id.as_str(), instance))
        .collect();
    let root = root_instance(model)?;

    // Mated pairs, keyed by the unordered instance pair, in mate-ID order.
    let mut mates: Vec<_> = model.mates.iter().filter(|mate| !mate.suppressed).collect();
    mates.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let mut pairs: BTreeMap<(String, String), Vec<(&MateId, &MateKind)>> = BTreeMap::new();
    for mate in &mates {
        let Some((a, b)) = mate_entities(&mate.kind) else {
            continue;
        };
        if a.instance == b.instance {
            continue;
        }
        let key = ordered_pair(a.instance.as_str(), b.instance.as_str());
        pairs.entry(key).or_default().push((&mate.id, &mate.kind));
    }

    let root_world = instances
        .get(root.as_str())
        .ok_or_else(|| OpenCadError::validation(format!("ground instance '{root}' not found")))?
        .placement
        .transform;
    let mut links = vec![KinematicLink {
        instance: root.clone(),
        frame_in_part: RigidTransform::identity(),
        world: root_world,
        joint: None,
    }];
    let mut world_of: BTreeMap<String, RigidTransform> = BTreeMap::new();
    world_of.insert(root.as_str().to_string(), root_world);
    let mut queue = VecDeque::from([root.as_str().to_string()]);

    // Breadth-first from the root.  Each mated pair is one tree edge; a pair
    // that reaches an already placed instance closes a loop.
    let mut used: BTreeSet<&(String, String)> = BTreeSet::new();
    while let Some(parent) = queue.pop_front() {
        for (pair, pair_mates) in &pairs {
            let child = if pair.0 == parent {
                &pair.1
            } else if pair.1 == parent {
                &pair.0
            } else {
                continue;
            };
            if !used.insert(pair) {
                continue;
            }
            if world_of.contains_key(child) {
                return Err(OpenCadError::validation(format!(
                    "mate '{}' closes a kinematic loop between '{parent}' and '{child}'; \
                     a robot description needs a tree",
                    pair_mates[0].0
                )));
            }
            let child_instance = instances.get(child.as_str()).ok_or_else(|| {
                OpenCadError::validation(format!("mated instance '{child}' not found"))
            })?;
            let parent_world = world_of[&parent];
            let (frame_in_part, kind, mate) = joint_frame(model, child_instance, pair_mates)?;
            let world = child_instance.placement.transform.compose(frame_in_part);
            let origin = parent_world.inverse()?.compose(world);
            links.push(KinematicLink {
                instance: child_instance.id.clone(),
                frame_in_part,
                world,
                joint: Some(KinematicJoint {
                    mate: Some(mate.clone()),
                    parent: InstanceId::new(parent.as_str())?,
                    kind,
                    origin,
                }),
            });
            world_of.insert(child.clone(), world);
            queue.push_back(child.clone());
        }
    }

    let mut warnings = Vec::new();
    for instance in &model.instances {
        if world_of.contains_key(instance.id.as_str()) {
            continue;
        }
        warnings.push(format!(
            "instance '{}' is not connected to '{root}' by mates; it is fixed to the root link",
            instance.id
        ));
        let world = instance.placement.transform;
        links.push(KinematicLink {
            instance: instance.id.clone(),
            frame_in_part: RigidTransform::identity(),
            world,
            joint: Some(KinematicJoint {
                mate: None,
                parent: root.clone(),
                kind: JointKind::Fixed,
                origin: root_world.inverse()?.compose(world),
            }),
        });
        world_of.insert(instance.id.as_str().to_string(), world);
    }
    Ok(KinematicTree { links, warnings })
}

fn root_instance(model: &AssemblyModel) -> Result<InstanceId> {
    let grounded: BTreeMap<&str, &InstanceId> = model
        .mates
        .iter()
        .filter(|mate| !mate.suppressed)
        .filter_map(|mate| match &mate.kind {
            MateKind::Ground { instance } => Some((instance.as_str(), instance)),
            _ => None,
        })
        .collect();
    let mut ids = grounded.values();
    match (ids.next(), ids.next()) {
        (Some(root), None) => Ok((*root).clone()),
        (None, _) => Err(OpenCadError::validation(
            "a robot description needs exactly one grounded instance; add a ground mate",
        )),
        (Some(_), Some(_)) => Err(OpenCadError::validation(format!(
            "a robot description needs exactly one grounded instance; found {}",
            grounded.keys().copied().collect::<Vec<_>>().join(", ")
        ))),
    }
}

fn mate_entities(kind: &MateKind) -> Option<(&MateEntity, &MateEntity)> {
    match kind {
        MateKind::Coincident { a, b }
        | MateKind::Concentric { a, b }
        | MateKind::Distance { a, b, .. }
        | MateKind::Angle { a, b, .. }
        | MateKind::Parallel { a, b } => Some((a, b)),
        MateKind::Ground { .. } => None,
    }
}

fn ordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// Link frame (in the child's part frame), joint kind, and source mate for a
/// child joined to its parent by `pair_mates`.
fn joint_frame<'a>(
    model: &AssemblyModel,
    child: &Instance,
    pair_mates: &[(&'a MateId, &MateKind)],
) -> Result<(RigidTransform, JointKind, &'a MateId)> {
    let concentric = pair_mates
        .iter()
        .find(|(_, kind)| matches!(kind, MateKind::Concentric { .. }));
    let Some((mate_id, MateKind::Concentric { a, b })) = concentric.copied() else {
        return Ok((
            RigidTransform::identity(),
            JointKind::Fixed,
            pair_mates[0].0,
        ));
    };
    let entity = if a.instance == child.id { a } else { b };
    let (origin, axis) = resolve_mate_entity_frame(entity, &model.connectors)?;
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if length < MIN_AXIS_LENGTH {
        return Err(OpenCadError::validation(format!(
            "mate '{mate_id}' has a zero-length axis on '{}'",
            child.id
        )));
    }
    let axis = [axis[0] / length, axis[1] / length, axis[2] / length];
    Ok((
        RigidTransform::from_translation(origin),
        JointKind::Continuous { axis },
        mate_id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mate::Mate;
    use crate::models::robot_arm_assembly_model;

    const TOLERANCE_M: f64 = 1e-9;

    fn link<'a>(tree: &'a KinematicTree, id: &str) -> &'a KinematicLink {
        tree.links
            .iter()
            .find(|link| link.instance.as_str() == id)
            .unwrap_or_else(|| panic!("link {id}"))
    }

    #[test]
    fn robot_arm_is_a_chain_of_three_continuous_joints() {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        assert!(tree.warnings.is_empty(), "{:?}", tree.warnings);
        let order: Vec<&str> = tree.links.iter().map(|l| l.instance.as_str()).collect();
        assert_eq!(
            order,
            [
                "instance:base",
                "instance:upper_arm",
                "instance:forearm",
                "instance:gripper"
            ]
        );
        for (child, parent, mate) in [
            ("instance:upper_arm", "instance:base", "mate:shoulder"),
            ("instance:forearm", "instance:upper_arm", "mate:elbow"),
            ("instance:gripper", "instance:forearm", "mate:wrist"),
        ] {
            let joint = link(&tree, child).joint.as_ref().expect("joint");
            assert_eq!(joint.parent.as_str(), parent);
            assert_eq!(joint.mate.as_ref().map(|id| id.as_str()), Some(mate));
            assert_eq!(
                joint.kind,
                JointKind::Continuous {
                    axis: [0.0, 0.0, 1.0]
                }
            );
        }
    }

    #[test]
    fn joint_origins_reproduce_world_poses() {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        for link in &tree.links {
            let Some(joint) = &link.joint else {
                continue;
            };
            let parent = tree
                .links
                .iter()
                .find(|candidate| candidate.instance == joint.parent)
                .expect("parent");
            let world = parent.world.compose(joint.origin);
            assert!(
                RigidTransform::points_near(
                    world.translation_m,
                    link.world.translation_m,
                    TOLERANCE_M
                ),
                "{}",
                link.instance
            );
            assert!(RigidTransform::matrices_near(
                &world.rotation,
                &link.world.rotation,
                1e-9
            ));
            // The link frame sits on the joint axis in the child part.
            let placed = model
                .instances
                .iter()
                .find(|instance| instance.id == link.instance)
                .expect("instance")
                .placement
                .transform
                .compose(link.frame_in_part);
            assert!(RigidTransform::points_near(
                placed.translation_m,
                link.world.translation_m,
                TOLERANCE_M
            ));
        }
    }

    #[test]
    fn a_closed_loop_is_rejected_by_mate() {
        let mut model = robot_arm_assembly_model().expect("model");
        let shoulder = model
            .mates
            .iter()
            .find(|mate| mate.id.as_str() == "mate:shoulder")
            .expect("shoulder")
            .clone();
        let MateKind::Concentric { a, mut b } = shoulder.kind else {
            panic!("concentric");
        };
        b.instance = InstanceId::new("instance:gripper").expect("id");
        b.connector = None;
        model.mates.push(Mate::new(
            MateId::new("mate:loop").expect("id"),
            MateKind::Concentric { a, b },
        ));
        let error = kinematic_tree(&model).expect_err("loop");
        assert!(error.to_string().contains("kinematic loop"), "{error}");
    }

    #[test]
    fn a_missing_ground_is_rejected() {
        let mut model = robot_arm_assembly_model().expect("model");
        model
            .mates
            .retain(|mate| !matches!(mate.kind, MateKind::Ground { .. }));
        let error = kinematic_tree(&model).expect_err("no ground");
        assert!(error.to_string().contains("exactly one grounded instance"));
    }

    #[test]
    fn unmated_instances_are_fixed_to_the_root_with_a_warning() {
        let mut model = robot_arm_assembly_model().expect("model");
        model.mates.retain(|mate| mate.id.as_str() != "mate:wrist");
        let tree = kinematic_tree(&model).expect("tree");
        let gripper = link(&tree, "instance:gripper")
            .joint
            .as_ref()
            .expect("joint");
        assert_eq!(gripper.kind, JointKind::Fixed);
        assert_eq!(gripper.parent.as_str(), "instance:base");
        assert_eq!(gripper.mate, None);
        assert_eq!(tree.warnings.len(), 1);
    }
}
