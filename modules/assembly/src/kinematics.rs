//! Kinematic tree derived from assembly mates (ADR-028).
//!
//! Robot description formats such as URDF need a tree of rigid links joined
//! by joints.  This module derives one from the Design Graph instead of
//! storing a second, hand-written description:
//!
//! - the grounded instance is the root link;
//! - mates join instances; a declared joint over one of the pair's mates
//!   (ADR-030) sets the motion and limits; otherwise a pair joined by a
//!   `concentric` mate becomes a continuous (unlimited revolute) joint about
//!   that mate's axis, and any other mated pair becomes a fixed joint;
//! - instances that no mate reaches are fixed to the root, with a warning.
//!
//! The zero position of every joint is the assembly's current pose.  Axial
//! sliding that a lone concentric mate would also allow is not modelled.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use opencad_core::{InstanceId, MateId, OpenCadError, Result};
use opencad_geometry::RigidTransform;

use crate::connector::resolve_mate_entity_frame;
use crate::instance::Instance;
use crate::joint::JointMotion;
use crate::mate::{MateEntity, MateKind};
use crate::model::AssemblyModel;

/// Axis directions shorter than this are rejected (unitless, normalised input).
const MIN_AXIS_LENGTH: f64 = 1e-9;

/// How a child link moves relative to its parent.  Axes are unit vectors in
/// the child link frame; limits are SI (radians, metres; N·m or N; rad/s or m/s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JointKind {
    /// Unlimited rotation about `axis`.
    Continuous {
        axis: [f64; 3],
    },
    /// Limited rotation about `axis`.
    Revolute {
        axis: [f64; 3],
        lower: f64,
        upper: f64,
        effort: f64,
        velocity: f64,
    },
    /// Limited translation along `axis`.
    Prismatic {
        axis: [f64; 3],
        lower: f64,
        upper: f64,
        effort: f64,
        velocity: f64,
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

/// Tolerance on joint positions checked against declared limits (radians or
/// metres): positions this far past a limit still count as within it.
const LIMIT_TOLERANCE: f64 = 1e-9;

impl JointKind {
    /// `true` for joints that can move.
    pub fn is_movable(&self) -> bool {
        !matches!(self, Self::Fixed)
    }

    /// `(lower, upper)` limits in radians or metres; `None` for continuous
    /// and fixed joints.
    pub fn limits(&self) -> Option<(f64, f64)> {
        match *self {
            Self::Revolute { lower, upper, .. } | Self::Prismatic { lower, upper, .. } => {
                Some((lower, upper))
            }
            Self::Continuous { .. } | Self::Fixed => None,
        }
    }

    /// Child link frame relative to its zero position for joint position
    /// `position` (radians for rotation, metres for translation).  Positions
    /// outside declared limits are rejected, not clamped.
    pub fn motion(&self, position: f64) -> Result<RigidTransform> {
        if !position.is_finite() {
            return Err(OpenCadError::validation("joint position must be finite"));
        }
        if let Some((lower, upper)) = self.limits() {
            if position < lower - LIMIT_TOLERANCE || position > upper + LIMIT_TOLERANCE {
                let (scale, unit) = match self {
                    Self::Prismatic { .. } => (1000.0, "mm"),
                    _ => (180.0 / std::f64::consts::PI, "deg"),
                };
                return Err(OpenCadError::validation(format!(
                    "joint position {:.3} {unit} is outside its limits [{:.3}, {:.3}] {unit}",
                    position * scale,
                    lower * scale,
                    upper * scale
                )));
            }
        }
        Ok(match *self {
            Self::Continuous { axis } | Self::Revolute { axis, .. } => RigidTransform {
                translation_m: [0.0; 3],
                rotation: axis_angle_rotation(axis, position),
            },
            Self::Prismatic { axis, .. } => {
                RigidTransform::from_translation(axis.map(|a| a * position))
            }
            Self::Fixed if position.abs() <= LIMIT_TOLERANCE => RigidTransform::identity(),
            Self::Fixed => {
                return Err(OpenCadError::validation(
                    "a fixed joint has no position other than zero",
                ))
            }
        })
    }
}

impl KinematicTree {
    /// World transform of every instance (part frame) with the joints of the
    /// child links (keyed by instance ID) at the given positions, in link
    /// order.  Joints not
    /// named stay at zero, which is the assembly's current pose.
    pub fn pose(
        &self,
        positions: &BTreeMap<String, f64>,
    ) -> Result<Vec<(InstanceId, RigidTransform)>> {
        for instance in positions.keys() {
            if !self
                .links
                .iter()
                .any(|link| link.instance.as_str() == instance)
            {
                return Err(OpenCadError::validation(format!(
                    "no joint moves instance '{instance}'"
                )));
            }
        }
        let mut link_world: BTreeMap<&str, RigidTransform> = BTreeMap::new();
        let mut placed = Vec::with_capacity(self.links.len());
        for link in &self.links {
            let world = match &link.joint {
                None => link.world,
                Some(joint) => {
                    let parent = link_world.get(joint.parent.as_str()).ok_or_else(|| {
                        OpenCadError::validation(format!(
                            "parent '{}' of '{}' is not posed before it",
                            joint.parent, link.instance
                        ))
                    })?;
                    let position = positions
                        .get(link.instance.as_str())
                        .copied()
                        .unwrap_or(0.0);
                    let motion = joint.kind.motion(position).map_err(|error| {
                        error.with_context(format!("joint of '{}'", link.instance))
                    })?;
                    parent.compose(joint.origin).compose(motion)
                }
            };
            link_world.insert(link.instance.as_str(), world);
            placed.push((
                link.instance.clone(),
                world.compose(link.frame_in_part.inverse()?),
            ));
        }
        Ok(placed)
    }
}

/// Rotation matrix for `angle_rad` about the unit vector `axis` (Rodrigues).
fn axis_angle_rotation(axis: [f64; 3], angle_rad: f64) -> [[f64; 3]; 3] {
    let [x, y, z] = axis;
    let (sin, cos) = angle_rad.sin_cos();
    let t = 1.0 - cos;
    [
        [t * x * x + cos, t * x * y - sin * z, t * x * z + sin * y],
        [t * x * y + sin * z, t * y * y + cos, t * y * z - sin * x],
        [t * x * z - sin * y, t * y * z + sin * x, t * z * z + cos],
    ]
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
    // A declared joint (ADR-030) decides the motion; otherwise a concentric
    // mate is an unlimited rotation and anything else is fixed (ADR-028).
    let declared = pair_mates.iter().find_map(|(mate_id, kind)| {
        model
            .joints
            .iter()
            .find(|joint| &joint.mate == *mate_id)
            .map(|joint| (*mate_id, *kind, &joint.motion))
    });
    let (mate_id, kind, motion) = match declared {
        Some((mate_id, kind, motion)) => (mate_id, kind, Some(motion)),
        None => match pair_mates
            .iter()
            .find(|(_, kind)| matches!(kind, MateKind::Concentric { .. }))
        {
            Some((mate_id, kind)) => (*mate_id, *kind, None),
            None => {
                return Ok((
                    RigidTransform::identity(),
                    JointKind::Fixed,
                    pair_mates[0].0,
                ))
            }
        },
    };
    if matches!(motion, Some(JointMotion::Fixed)) {
        return Ok((RigidTransform::identity(), JointKind::Fixed, mate_id));
    }
    let MateKind::Concentric { a, b } = kind else {
        return Err(OpenCadError::validation(format!(
            "mate '{mate_id}' must be concentric to carry a moving joint"
        )));
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
    let joint = match motion {
        None | Some(JointMotion::Continuous) => JointKind::Continuous { axis },
        Some(JointMotion::Revolute {
            lower_rad,
            upper_rad,
            effort_n_m,
            velocity_rad_s,
        }) => JointKind::Revolute {
            axis,
            lower: *lower_rad,
            upper: *upper_rad,
            effort: *effort_n_m,
            velocity: *velocity_rad_s,
        },
        Some(JointMotion::Prismatic {
            lower_m,
            upper_m,
            effort_n,
            velocity_m_s,
        }) => JointKind::Prismatic {
            axis,
            lower: *lower_m,
            upper: *upper_m,
            effort: *effort_n,
            velocity: *velocity_m_s,
        },
        Some(JointMotion::Fixed) => JointKind::Fixed,
    };
    Ok((RigidTransform::from_translation(origin), joint, mate_id))
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
    fn robot_arm_joints_carry_their_declared_limits() {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        let JointKind::Revolute {
            axis,
            lower,
            upper,
            effort,
            velocity,
        } = link(&tree, "instance:forearm")
            .joint
            .as_ref()
            .expect("joint")
            .kind
        else {
            panic!("elbow should be revolute");
        };
        assert_eq!(axis, [0.0, 0.0, 1.0]);
        assert!((lower - (-100.0_f64).to_radians()).abs() < 1e-12);
        assert!((upper - 140.0_f64.to_radians()).abs() < 1e-12);
        assert_eq!((effort, velocity), (1.0, 3.0));
    }

    #[test]
    fn a_declared_fixed_joint_locks_a_concentric_pair() {
        let mut model = robot_arm_assembly_model().expect("model");
        let wrist = model
            .joints
            .iter_mut()
            .find(|joint| joint.mate.as_str() == "mate:wrist")
            .expect("wrist");
        wrist.motion = JointMotion::Fixed;
        let tree = kinematic_tree(&model).expect("tree");
        let joint = link(&tree, "instance:gripper")
            .joint
            .as_ref()
            .expect("joint");
        assert_eq!(joint.kind, JointKind::Fixed);
        assert_eq!(
            joint.mate.as_ref().map(|id| id.as_str()),
            Some("mate:wrist")
        );
    }

    #[test]
    fn concentric_mates_without_joints_are_continuous() {
        let mut model = robot_arm_assembly_model().expect("model");
        model.joints.clear();
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
    fn the_zero_pose_reproduces_the_placements() {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        let posed = tree.pose(&BTreeMap::new()).expect("pose");
        assert_eq!(posed.len(), model.instances.len());
        for (instance, world) in posed {
            let placement = model
                .instances
                .iter()
                .find(|candidate| candidate.id == instance)
                .expect("instance")
                .placement
                .transform;
            assert!(RigidTransform::points_near(
                world.translation_m,
                placement.translation_m,
                TOLERANCE_M
            ));
            assert!(RigidTransform::matrices_near(
                &world.rotation,
                &placement.rotation,
                1e-12
            ));
        }
    }

    #[test]
    fn turning_the_shoulder_swings_the_whole_arm_about_its_axis() {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        let shoulder = link(&tree, "instance:upper_arm").world.translation_m;
        let zero = tree.pose(&BTreeMap::new()).expect("zero");
        let quarter = tree
            .pose(&BTreeMap::from([(
                "instance:upper_arm".to_string(),
                90.0_f64.to_radians(),
            )]))
            .expect("quarter turn");
        let at = |posed: &[(InstanceId, RigidTransform)], id: &str| {
            posed
                .iter()
                .find(|(instance, _)| instance.as_str() == id)
                .expect("posed")
                .1
        };
        // Base stays put; every link downstream turns 90° about +Z through the shoulder.
        assert_eq!(at(&zero, "instance:base"), at(&quarter, "instance:base"));
        for id in ["instance:forearm", "instance:gripper"] {
            let before = at(&zero, id).translation_m;
            let after = at(&quarter, id).translation_m;
            let (dx, dy) = (before[0] - shoulder[0], before[1] - shoulder[1]);
            let expected = [shoulder[0] - dy, shoulder[1] + dx, before[2]];
            assert!(
                RigidTransform::points_near(after, expected, TOLERANCE_M),
                "{id}: {after:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn positions_outside_limits_and_unknown_links_are_rejected() {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        // Elbow is limited to [-100°, 140°].
        let error = tree
            .pose(&BTreeMap::from([(
                "instance:forearm".to_string(),
                150.0_f64.to_radians(),
            )]))
            .expect_err("past the limit");
        assert!(error.to_string().contains("instance:forearm"), "{error}");
        assert!(tree
            .pose(&BTreeMap::from([("instance:tail".to_string(), 0.1)]))
            .is_err());
        assert!(tree
            .pose(&BTreeMap::from([(
                "instance:forearm".to_string(),
                f64::NAN
            )]))
            .is_err());
    }

    #[test]
    fn prismatic_and_fixed_motions() {
        let slide = JointKind::Prismatic {
            axis: [0.0, 1.0, 0.0],
            lower: 0.0,
            upper: 0.05,
            effort: 10.0,
            velocity: 0.1,
        };
        assert_eq!(
            slide.motion(0.02).expect("slide"),
            RigidTransform::from_translation([0.0, 0.02, 0.0])
        );
        assert!(slide.motion(0.06).is_err());
        assert_eq!(slide.limits(), Some((0.0, 0.05)));
        assert!(!JointKind::Fixed.is_movable());
        assert!(JointKind::Fixed.motion(0.0).is_ok());
        assert!(JointKind::Fixed.motion(0.1).is_err());
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
