//! Reach and workspace of a tool point on a kinematic tree.
//!
//! Kernel-free: only the joint frames, axes, and limits of the
//! [`KinematicTree`] are used, so an agent can ask "can the gripper reach
//! this point?" before regenerating any geometry.  Joint space is sampled on
//! a deterministic grid within the declared limits (continuous joints use one
//! turn, −π to π); the best samples are refined with damped least squares on
//! a finite-difference Jacobian, clamped to the limits.  The same inputs give
//! the same answer on every run.  A target counts as reachable when the
//! refined tool point lies within `tolerance_m` of it.  Reachable targets
//! converge to about a micrometre; for a target out of reach, the reported
//! gap is accurate to about 0.1 mm, because the fully stretched pose it ends
//! in is singular.

use std::collections::BTreeMap;

use opencad_core::{InstanceId, OpenCadError, Result};

use crate::kinematics::KinematicTree;

/// Most grid samples taken; the per-joint count is lowered to stay under it.
const MAX_SAMPLES: usize = 20_000;
/// Grid samples refined by least squares.
const SEEDS: usize = 6;
const MAX_ITERATIONS: usize = 200;
/// Finite-difference step, radians or metres.
const JACOBIAN_STEP: f64 = 1e-6;
/// Damping for least squares, metres.
const DAMPING_M: f64 = 1e-3;
/// Step halvings tried before refinement gives up.
const BACKTRACK_HALVINGS: usize = 24;
/// Refinement stops when the error is below this, metres.
const CONVERGED_M: f64 = 1e-9;

/// A point fixed to one link, given in that instance's part frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolPoint {
    pub instance: InstanceId,
    pub point_m: [f64; 3],
}

/// One movable joint as the reach solver sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachJoint {
    /// Instance the joint moves.
    pub instance: InstanceId,
    /// Search range: the declared limits, or −π to π for continuous joints.
    pub range: (f64, f64),
}

/// Where a tool point can go.
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub joints: Vec<ReachJoint>,
    pub samples: usize,
    /// Axis-aligned bounds of every sampled tool position, metres.
    pub bounds_m: [[f64; 3]; 2],
    /// Smallest and largest sampled distance from the first moving joint's
    /// origin, metres.
    pub reach_m: [f64; 2],
    /// World origin of the first moving joint, metres.
    pub base_m: [f64; 3],
}

/// Best joint positions found for a target.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachSolution {
    pub reachable: bool,
    /// Remaining distance from the tool point to the target, metres.
    pub distance_m: f64,
    /// Tool point at `positions`, metres.
    pub tool_m: [f64; 3],
    /// Joint positions keyed by moving instance ID, radians or metres.
    pub positions: BTreeMap<String, f64>,
}

/// Movable joints of `tree` in link order, with their search ranges.
pub fn reach_joints(tree: &KinematicTree) -> Vec<ReachJoint> {
    tree.links
        .iter()
        .filter_map(|link| {
            let joint = link.joint.as_ref()?;
            if !joint.kind.is_movable() {
                return None;
            }
            let range = joint
                .kind
                .limits()
                .unwrap_or((-std::f64::consts::PI, std::f64::consts::PI));
            Some(ReachJoint {
                instance: link.instance.clone(),
                range,
            })
        })
        .collect()
}

/// World position of `tool` with the joints at `values` (in `joints` order).
fn tool_position(
    tree: &KinematicTree,
    joints: &[ReachJoint],
    tool: &ToolPoint,
    values: &[f64],
) -> Result<[f64; 3]> {
    let positions = joints
        .iter()
        .zip(values)
        .map(|(joint, value)| (joint.instance.as_str().to_string(), *value))
        .collect();
    let posed = tree.pose(&positions)?;
    let world = posed
        .iter()
        .find(|(instance, _)| *instance == tool.instance)
        .map(|(_, world)| *world)
        .ok_or_else(|| {
            OpenCadError::validation(format!(
                "tool instance '{}' is not in the kinematic tree",
                tool.instance
            ))
        })?;
    Ok(world.transform_point(tool.point_m))
}

/// Grid of joint values, `per_joint` per joint, every combination in order.
fn grid(joints: &[ReachJoint], max_samples: usize) -> (usize, Vec<Vec<f64>>) {
    let count = joints.len().max(1) as u32;
    let mut per_joint = 2usize;
    while (per_joint + 1)
        .checked_pow(count)
        .is_some_and(|n| n <= max_samples)
        && per_joint < 64
    {
        per_joint += 1;
    }
    let total = per_joint.pow(count);
    let samples = (0..total)
        .map(|mut index| {
            joints
                .iter()
                .map(|joint| {
                    let step = index % per_joint;
                    index /= per_joint;
                    let (lower, upper) = joint.range;
                    lower + (upper - lower) * step as f64 / (per_joint - 1) as f64
                })
                .collect()
        })
        .collect();
    (per_joint, samples)
}

fn first_joint_origin(tree: &KinematicTree, joints: &[ReachJoint]) -> Result<[f64; 3]> {
    let first = joints
        .first()
        .ok_or_else(|| OpenCadError::validation("the assembly has no movable joints"))?;
    let link = tree
        .links
        .iter()
        .find(|link| link.instance == first.instance)
        .ok_or_else(|| OpenCadError::validation("joint link is missing from the tree"))?;
    Ok(link.world.translation_m)
}

/// Sample where `tool` can go within the joint limits.
pub fn workspace(tree: &KinematicTree, tool: &ToolPoint) -> Result<Workspace> {
    let joints = reach_joints(tree);
    let base = first_joint_origin(tree, &joints)?;
    let (_, samples) = grid(&joints, MAX_SAMPLES);
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut reach = [f64::INFINITY, 0.0f64];
    for values in &samples {
        let point = tool_position(tree, &joints, tool, values)?;
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
        let distance = distance(point, base);
        reach = [reach[0].min(distance), reach[1].max(distance)];
    }
    Ok(Workspace {
        joints,
        samples: samples.len(),
        bounds_m: [min, max],
        reach_m: reach,
        base_m: base,
    })
}

/// Find joint positions that bring `tool` closest to `target_m`.
pub fn solve_reach(
    tree: &KinematicTree,
    tool: &ToolPoint,
    target_m: [f64; 3],
    tolerance_m: f64,
) -> Result<ReachSolution> {
    if !(tolerance_m.is_finite() && tolerance_m > 0.0) || !target_m.iter().all(|v| v.is_finite()) {
        return Err(OpenCadError::validation(
            "reach target and tolerance must be finite, tolerance positive",
        ));
    }
    let joints = reach_joints(tree);
    if joints.is_empty() {
        return Err(OpenCadError::validation(
            "the assembly has no movable joints",
        ));
    }
    let (_, samples) = grid(&joints, MAX_SAMPLES);
    let mut scored = Vec::with_capacity(samples.len());
    for values in samples {
        let error = distance(tool_position(tree, &joints, tool, &values)?, target_m);
        scored.push((error, values));
    }
    // Stable sort: ties keep grid order, so seeds are deterministic.
    scored.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut best: Option<(f64, Vec<f64>)> = None;
    for (_, seed) in scored.into_iter().take(SEEDS) {
        let (error, values) = refine(tree, &joints, tool, target_m, seed)?;
        let better = match &best {
            None => true,
            Some((best_error, _)) => error < *best_error - 1e-12,
        };
        if better {
            best = Some((error, values));
        }
    }
    let (distance_m, values) = best.ok_or_else(|| OpenCadError::validation("no reach samples"))?;
    let tool_m = tool_position(tree, &joints, tool, &values)?;
    Ok(ReachSolution {
        reachable: distance_m <= tolerance_m,
        distance_m,
        tool_m,
        positions: joints
            .iter()
            .zip(values)
            .map(|(joint, value)| (joint.instance.as_str().to_string(), value))
            .collect(),
    })
}

/// Damped least squares from `values`, clamped to the joint ranges.
fn refine(
    tree: &KinematicTree,
    joints: &[ReachJoint],
    tool: &ToolPoint,
    target: [f64; 3],
    mut values: Vec<f64>,
) -> Result<(f64, Vec<f64>)> {
    let clamp = |values: &mut [f64]| {
        for (value, joint) in values.iter_mut().zip(joints) {
            *value = value.clamp(joint.range.0, joint.range.1);
        }
    };
    let mut point = tool_position(tree, joints, tool, &values)?;
    let mut error = distance(point, target);
    for _ in 0..MAX_ITERATIONS {
        if error < CONVERGED_M {
            break;
        }
        let residual = sub(target, point);
        // Jacobian columns: d(point)/d(value_j), one-sided inside the range.
        let mut jacobian = Vec::with_capacity(joints.len());
        for (index, joint) in joints.iter().enumerate() {
            let mut moved = values.clone();
            let step = if values[index] + JACOBIAN_STEP <= joint.range.1 {
                JACOBIAN_STEP
            } else {
                -JACOBIAN_STEP
            };
            moved[index] += step;
            let shifted = tool_position(tree, joints, tool, &moved)?;
            jacobian.push(sub(shifted, point).map(|delta| delta / step));
        }
        // Δq = Jᵀ (J Jᵀ + λ² I)⁻¹ r
        let mut normal = [[0.0; 3]; 3];
        for (row, normal_row) in normal.iter_mut().enumerate() {
            for (column, value) in normal_row.iter_mut().enumerate() {
                *value = jacobian.iter().map(|c| c[row] * c[column]).sum::<f64>();
            }
            normal_row[row] += DAMPING_M * DAMPING_M;
        }
        let Some(weights) = solve3(normal, residual) else {
            break;
        };
        let step: Vec<f64> = jacobian
            .iter()
            .map(|column| dot(*column, weights))
            .collect();
        // Backtrack: halve the step until it improves, which keeps progress
        // near singular (fully stretched) poses.
        let mut improved = false;
        let mut scale = 1.0;
        for _ in 0..BACKTRACK_HALVINGS {
            let mut candidate: Vec<f64> = values
                .iter()
                .zip(&step)
                .map(|(value, delta)| value + delta * scale)
                .collect();
            clamp(&mut candidate);
            let candidate_point = tool_position(tree, joints, tool, &candidate)?;
            let candidate_error = distance(candidate_point, target);
            if candidate_error < error {
                values = candidate;
                point = candidate_point;
                error = candidate_error;
                improved = true;
                break;
            }
            scale *= 0.5;
        }
        if !improved {
            break;
        }
    }
    Ok((error, values))
}

/// Solve the 3 × 3 system `a x = b` by Gaussian elimination with partial pivoting.
fn solve3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> Option<[f64; 3]> {
    for column in 0..3 {
        let pivot =
            (column..3).max_by(|&i, &j| a[i][column].abs().total_cmp(&a[j][column].abs()))?;
        if a[pivot][column].abs() < 1e-300 {
            return None;
        }
        a.swap(column, pivot);
        b.swap(column, pivot);
        let pivot_row = a[column];
        for row in column + 1..3 {
            let factor = a[row][column] / pivot_row[column];
            for (value, pivot_value) in a[row].iter_mut().zip(pivot_row).skip(column) {
                *value -= factor * pivot_value;
            }
            b[row] -= factor * b[column];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let tail: f64 = (row + 1..3).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - tail) / a[row][row];
    }
    Some(x)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

/// Tool point world position for explicit joint positions; exposed for
/// callers that want to check a solution.
pub fn tool_point_at(
    tree: &KinematicTree,
    tool: &ToolPoint,
    positions: &BTreeMap<String, f64>,
) -> Result<[f64; 3]> {
    let posed = tree.pose(positions)?;
    posed
        .iter()
        .find(|(instance, _)| *instance == tool.instance)
        .map(|(_, world)| world.transform_point(tool.point_m))
        .ok_or_else(|| {
            OpenCadError::validation(format!(
                "tool instance '{}' is not in the kinematic tree",
                tool.instance
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinematics::kinematic_tree;
    use crate::models::robot_arm_assembly_model;

    fn arm() -> (KinematicTree, ToolPoint) {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        let tool = ToolPoint {
            instance: InstanceId::new("instance:gripper").expect("id"),
            point_m: [0.0, 0.04, 0.007],
        };
        (tree, tool)
    }

    #[test]
    fn the_workspace_is_an_annulus_around_the_shoulder() {
        let (tree, tool) = arm();
        let space = workspace(&tree, &tool).expect("workspace");
        assert_eq!(space.joints.len(), 3);
        assert!(space.samples <= MAX_SAMPLES && space.samples >= 1000);
        // Upper arm 160 mm + forearm 110 mm + 40 mm to the tool: at most 310 mm
        // out horizontally; the tool sits above the shoulder joint origin.
        let rise = space.bounds_m[0][2] - space.base_m[2];
        let horizontal = |reach: f64| (reach * reach - rise * rise).sqrt();
        assert!(
            horizontal(space.reach_m[1]) <= 0.310 + 1e-6,
            "{:?}",
            space.reach_m
        );
        assert!(horizontal(space.reach_m[1]) > 0.29, "{:?}", space.reach_m);
        // A planar arm: the tool stays at one height.
        assert!((space.bounds_m[1][2] - space.bounds_m[0][2]).abs() < 1e-9);
    }

    #[test]
    fn a_target_inside_the_workspace_is_reached_within_tolerance() {
        let (tree, tool) = arm();
        let zero = tool_point_at(&tree, &tool, &BTreeMap::new()).expect("zero");
        let target = [0.12, 0.17, zero[2]];
        let solution = solve_reach(&tree, &tool, target, 1e-4).expect("solve");
        assert!(solution.reachable, "{solution:?}");
        assert!(solution.distance_m < 1e-6, "{}", solution.distance_m);
        let check = tool_point_at(&tree, &tool, &solution.positions).expect("check");
        assert!(distance(check, target) < 1e-6);
        // Within limits: posing again does not fail.
        tree.pose(&solution.positions).expect("within limits");
        // Deterministic.
        assert_eq!(
            solution,
            solve_reach(&tree, &tool, target, 1e-4).expect("again")
        );
    }

    #[test]
    fn a_target_beyond_reach_reports_the_gap() {
        let (tree, tool) = arm();
        let zero = tool_point_at(&tree, &tool, &BTreeMap::new()).expect("zero");
        let base = first_joint_origin(&tree, &reach_joints(&tree)).expect("base");
        // 350 mm from the shoulder axis, at tool height: 40 mm past full reach.
        let target = [base[0], base[1] + 0.35, zero[2]];
        let solution = solve_reach(&tree, &tool, target, 1e-3).expect("solve");
        assert!(!solution.reachable);
        // Fully stretched: 350 − 310 mm. Near that singular pose the gap is
        // accurate to about 0.1 mm.
        assert!((solution.distance_m - 0.04).abs() < 1e-4, "{solution:?}");
        let along = |point: [f64; 3]| point[1] - base[1];
        assert!(along(solution.tool_m) > 0.309, "{solution:?}");
        assert!(solve_reach(&tree, &tool, target, 0.0).is_err());
    }

    #[test]
    fn grids_respect_the_sample_budget_and_include_the_limits() {
        let joints = vec![
            ReachJoint {
                instance: InstanceId::new("instance:a").expect("id"),
                range: (-1.0, 1.0),
            };
            3
        ];
        let (per_joint, samples) = grid(&joints, 1000);
        assert_eq!(per_joint, 10);
        assert_eq!(samples.len(), 1000);
        assert_eq!(samples[0], [-1.0, -1.0, -1.0]);
        assert_eq!(samples[999], [1.0, 1.0, 1.0]);
        assert_eq!(
            solve3(
                [[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [1.0, 0.0, 1.0]],
                [2.0, 8.0, 4.0]
            ),
            Some([1.0, 2.0, 3.0])
        );
    }
}
