//! Interference across joint motion (MCAD-P10-002).
//!
//! A static interference check only proves the parts clear each other in the
//! authored pose.  This module sweeps every movable joint of the kinematic
//! tree through its range and counts interfering instance pairs at each
//! sampled pose, using the same explicit tolerances as the static check.
//!
//! Sampling is per joint: one joint moves through `samples_per_joint` evenly
//! spaced positions while every other joint stays at zero (the authored
//! pose).  The cost is linear in the number of joints rather than
//! exponential, and combined positions of several joints are not sampled.
//! Limited joints sample both limits; continuous joints sample a full turn.
//!
//! Moving one joint moves only its subtree rigidly, so pairs on the same side
//! keep their authored-pose result and only pairs that straddle the subtree
//! boundary need an exact Boolean.

use std::collections::{BTreeMap, BTreeSet};

use opencad_core::{
    OpenCadError, Result, MAX_MOTION_SAMPLES_PER_JOINT, MIN_MOTION_SAMPLES_PER_JOINT,
};
use opencad_geometry::{GeometryKernel, KernelBody, RigidTransform};
use serde::{Deserialize, Serialize};

use crate::kinematics::{kinematic_tree, JointKind, KinematicTree};
use crate::model::AssemblyModel;
use crate::pattern::expand_patterns;
use crate::regen::{interference_volume, AssemblyInterferenceTolerance, AssemblyScene};

/// One sampled pose: a single joint (named by the instance it moves) away
/// from zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MotionPose {
    /// Instance whose joint is moved.
    pub joint_instance: String,
    /// Joint position in radians (`revolute`, `continuous`) or metres
    /// (`prismatic`).
    pub position: f64,
    pub prismatic: bool,
}

impl MotionPose {
    /// Position with its unit, in degrees or millimetres.
    pub fn describe(&self) -> String {
        // Round first so a tiny negative position does not print as -0.0.
        let tenth = |value: f64| (value * 10.0).round() / 10.0 + 0.0;
        if self.prismatic {
            format!(
                "{} at {:.1} mm",
                self.joint_instance,
                tenth(self.position * 1000.0)
            )
        } else {
            format!(
                "{} at {:.1} deg",
                self.joint_instance,
                tenth(self.position.to_degrees())
            )
        }
    }
}

/// Interfering pairs at one sampled pose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MotionInterferencePose {
    pub pose: MotionPose,
    /// Interfering instance pairs, each ordered and the list sorted by ID.
    pub pairs: Vec<[String; 2]>,
}

/// Worst interference found while sweeping every joint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MotionInterferenceReport {
    pub samples_per_joint: u32,
    /// Movable joints swept.
    pub joints: usize,
    /// Poses checked (joints × samples).
    pub poses_checked: usize,
    /// Interfering pairs in the authored pose.
    pub rest_count: usize,
    /// Largest pair count over every sampled pose (and the authored pose).
    pub max_count: usize,
    /// First sampled pose, in sweep order, with more interfering pairs than
    /// the authored pose; `None` when no pose is worse.
    pub worst: Option<MotionInterferencePose>,
}

/// Evenly spaced positions for one joint: both limits for a limited joint,
/// a full turn starting at -180° for a continuous joint, `None` when fixed.
pub fn joint_sample_positions(kind: &JointKind, samples: u32) -> Option<Vec<f64>> {
    let count = samples as usize;
    match *kind {
        JointKind::Fixed => None,
        JointKind::Continuous { .. } => {
            let step = std::f64::consts::TAU / samples as f64;
            Some(
                (0..count)
                    .map(|index| -std::f64::consts::PI + step * index as f64)
                    .collect(),
            )
        }
        JointKind::Revolute { lower, upper, .. } | JointKind::Prismatic { lower, upper, .. } => {
            let step = (upper - lower) / (samples - 1) as f64;
            Some(
                (0..count)
                    .map(|index| {
                        if index + 1 == count {
                            upper
                        } else {
                            lower + step * index as f64
                        }
                    })
                    .collect(),
            )
        }
    }
}

/// Sweep every movable joint of `model` through its range and count the
/// interfering instance pairs of `scene` at each sampled pose.
///
/// `scene` must come from regenerating `model`: its instance bodies are
/// placed where the mate solver put them.  An instance without a body fails
/// the sweep instead of being skipped, so a missing part cannot hide a
/// collision.  Pose order, pair order, and the reported worst pose are
/// deterministic.
pub fn sample_motion_interference<K: GeometryKernel>(
    kernel: &K,
    model: &AssemblyModel,
    scene: &AssemblyScene,
    samples_per_joint: u32,
    tolerance: AssemblyInterferenceTolerance,
) -> Result<MotionInterferenceReport> {
    if !(MIN_MOTION_SAMPLES_PER_JOINT..=MAX_MOTION_SAMPLES_PER_JOINT).contains(&samples_per_joint) {
        return Err(OpenCadError::validation(format!(
            "samples per joint must be between {MIN_MOTION_SAMPLES_PER_JOINT} and \
             {MAX_MOTION_SAMPLES_PER_JOINT}, got {samples_per_joint}"
        )));
    }
    tolerance.validate()?;

    let solved = solved_model(model)?;
    let tree = kinematic_tree(&solved)?;

    let mut bodies: BTreeMap<String, &KernelBody> = BTreeMap::new();
    for instance in &scene.instances {
        let body = instance.body.as_ref().ok_or_else(|| {
            OpenCadError::validation(format!(
                "instance '{}' did not regenerate, so its motion cannot be checked",
                instance.instance_id
            ))
        })?;
        bodies.insert(instance.instance_id.as_str().to_string(), body);
    }
    let mut inverse_placements: BTreeMap<String, RigidTransform> = BTreeMap::new();
    for instance in &solved.instances {
        inverse_placements.insert(
            instance.id.as_str().to_string(),
            instance.placement.transform.inverse()?,
        );
    }

    let ids: Vec<&String> = bodies.keys().collect();
    let mut rest_pairs: BTreeSet<[String; 2]> = BTreeSet::new();
    for (index, first) in ids.iter().enumerate() {
        for second in &ids[index + 1..] {
            if interference_volume(kernel, bodies[*first], bodies[*second], tolerance)?.is_some() {
                rest_pairs.insert([(*first).clone(), (*second).clone()]);
            }
        }
    }

    let mut report = MotionInterferenceReport {
        samples_per_joint,
        joints: 0,
        poses_checked: 0,
        rest_count: rest_pairs.len(),
        max_count: rest_pairs.len(),
        worst: None,
    };
    for link in &tree.links {
        let Some(joint) = &link.joint else { continue };
        let Some(positions) = joint_sample_positions(&joint.kind, samples_per_joint) else {
            continue;
        };
        report.joints += 1;
        let moving = subtree(&tree, link.instance.as_str());
        for position in positions {
            let pose = MotionPose {
                joint_instance: link.instance.as_str().to_string(),
                position,
                prismatic: matches!(joint.kind, JointKind::Prismatic { .. }),
            };
            let pairs = pose_pairs(
                kernel,
                &tree,
                &pose,
                &moving,
                &bodies,
                &inverse_placements,
                &rest_pairs,
                tolerance,
            )?;
            report.poses_checked += 1;
            if pairs.len() > report.max_count {
                report.max_count = pairs.len();
                report.worst = Some(MotionInterferencePose { pose, pairs });
            }
        }
    }
    Ok(report)
}

/// `model` with patterns expanded and instances placed by the mate solver,
/// as assembly regeneration places them.
fn solved_model(model: &AssemblyModel) -> Result<AssemblyModel> {
    let expanded = expand_patterns(model)?;
    if expanded.mates.is_empty() {
        return Ok(expanded);
    }
    let (instances, _) = crate::solve::solve_assembly_mates(&expanded)?;
    let mut solved = expanded;
    solved.instances = instances;
    Ok(solved)
}

/// Instances moved by the joint of `root`: the link itself and every link
/// below it.  Links are parent-before-child, so one pass suffices.
fn subtree(tree: &KinematicTree, root: &str) -> BTreeSet<String> {
    let mut moving = BTreeSet::from([root.to_string()]);
    for link in &tree.links {
        if let Some(joint) = &link.joint {
            if moving.contains(joint.parent.as_str()) {
                moving.insert(link.instance.as_str().to_string());
            }
        }
    }
    moving
}

/// Interfering pairs with one joint at `pose`: authored-pose pairs that do
/// not straddle the moving subtree, plus exact checks of the straddling pairs.
#[allow(clippy::too_many_arguments)]
fn pose_pairs<K: GeometryKernel>(
    kernel: &K,
    tree: &KinematicTree,
    pose: &MotionPose,
    moving: &BTreeSet<String>,
    bodies: &BTreeMap<String, &KernelBody>,
    inverse_placements: &BTreeMap<String, RigidTransform>,
    rest_pairs: &BTreeSet<[String; 2]>,
    tolerance: AssemblyInterferenceTolerance,
) -> Result<Vec<[String; 2]>> {
    let straddles = |pair: &[String; 2]| moving.contains(&pair[0]) != moving.contains(&pair[1]);
    let mut pairs: BTreeSet<[String; 2]> = rest_pairs
        .iter()
        .filter(|pair| !straddles(pair))
        .cloned()
        .collect();

    let positions = BTreeMap::from([(pose.joint_instance.clone(), pose.position)]);
    let mut moved: BTreeMap<String, KernelBody> = BTreeMap::new();
    for (instance, world) in tree.pose(&positions)? {
        let key = instance.as_str();
        if !moving.contains(key) {
            continue;
        }
        let (Some(body), Some(inverse)) = (bodies.get(key), inverse_placements.get(key)) else {
            continue;
        };
        moved.insert(
            key.to_string(),
            kernel.transform_body((*body).clone(), world.compose(*inverse))?,
        );
    }
    for (moving_id, moving_body) in &moved {
        for (other_id, other_body) in bodies {
            if moving.contains(other_id) {
                continue;
            }
            if interference_volume(kernel, moving_body, other_body, tolerance)?.is_some() {
                let mut pair = [moving_id.clone(), other_id.clone()];
                pair.sort();
                pairs.insert(pair);
            }
        }
    }
    Ok(pairs.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT_TOLERANCE: f64 = 1e-12;

    #[test]
    fn limited_joints_sample_both_limits_evenly() {
        let kind = JointKind::Revolute {
            axis: [0.0, 0.0, 1.0],
            lower: -1.0,
            upper: 2.0,
            effort: 1.0,
            velocity: 1.0,
        };
        let positions = joint_sample_positions(&kind, 4).expect("movable");
        assert_eq!(positions.len(), 4);
        for (actual, expected) in positions.iter().zip([-1.0, 0.0, 1.0, 2.0]) {
            assert!((actual - expected).abs() < LIMIT_TOLERANCE, "{positions:?}");
        }
        assert_eq!(
            *positions.last().expect("upper"),
            2.0,
            "upper limit is exact"
        );
    }

    #[test]
    fn continuous_joints_sample_one_full_turn() {
        let kind = JointKind::Continuous {
            axis: [0.0, 0.0, 1.0],
        };
        let positions = joint_sample_positions(&kind, 4).expect("movable");
        let quarter = std::f64::consts::FRAC_PI_2;
        for (actual, expected) in positions
            .iter()
            .zip([-2.0 * quarter, -quarter, 0.0, quarter])
        {
            assert!((actual - expected).abs() < LIMIT_TOLERANCE, "{positions:?}");
        }
        assert!(joint_sample_positions(&JointKind::Fixed, 4).is_none());
    }

    #[test]
    fn poses_describe_their_unit() {
        let revolute = MotionPose {
            joint_instance: "instance:elbow".into(),
            position: -std::f64::consts::FRAC_PI_2,
            prismatic: false,
        };
        assert_eq!(revolute.describe(), "instance:elbow at -90.0 deg");
        let prismatic = MotionPose {
            joint_instance: "instance:slide".into(),
            position: 0.0125,
            prismatic: true,
        };
        assert_eq!(prismatic.describe(), "instance:slide at 12.5 mm");
    }
}
