//! `musubicad reach`: where a tool point on an assembly can go, and whether
//! it reaches a target, without regenerating geometry.
//!
//! The answer comes from the same kinematic tree the URDF export and
//! `animate-joints` use, after solving the assembly's mates the way
//! regeneration does, so a connector or link-length edit is reflected
//! immediately.  With `--gif`, the arm is drawn moving to its best pose for
//! the target, with the target marked.

use opencad_assembly::{kinematic_tree, solve_reach, workspace, ToolPoint};
use opencad_core::{InstanceId, OpenCadError, Result};
use opencad_file::read_ocad;
use serde::Serialize;

use crate::joint_animation::{
    animate_joints, movable_joints, parse_joint_animation_args, solved_assembly, JointValue,
};

const USAGE: &str = "usage: musubicad reach <assembly> --tool INSTANCE [--point X,Y,Z<mm|m>] \
[--target X,Y,Z<mm|m>] [--tolerance VALUE<mm|m>] [--gif out.gif [animate-joints options]]
  INSTANCE is the instance carrying the tool (instance:gripper or gripper);
  --point is the tool point in that part's frame (default its origin);
  --target is a world point; --tolerance defaults to 0.5mm.";

/// Parsed `reach` options.
#[derive(Debug, Clone, PartialEq)]
pub struct ReachOptions {
    pub tool: String,
    pub point_m: [f64; 3],
    pub target_m: Option<[f64; 3]>,
    pub tolerance_m: f64,
    pub gif: Option<String>,
    /// Flags after `--gif out.gif`, passed to `animate-joints`.
    pub gif_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReachWorkspace {
    pub samples: usize,
    pub bounds_mm: [[f64; 3]; 2],
    /// Closest and farthest sampled tool distance from the first joint, mm.
    pub reach_mm: [f64; 2],
    /// World origin of the first moving joint, mm.
    pub base_mm: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReachJointValue {
    pub joint: String,
    pub instance: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReachTarget {
    pub target_mm: [f64; 3],
    pub tolerance_mm: f64,
    pub reachable: bool,
    /// Remaining distance from the tool point to the target, mm.
    pub gap_mm: f64,
    pub tool_mm: [f64; 3],
    pub joints: Vec<ReachJointValue>,
    /// The pose as an `animate-joints --pose` argument.
    pub pose: String,
}

/// What `reach` reports.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReachSummary {
    pub tool: String,
    pub point_mm: [f64; 3],
    pub workspace: ReachWorkspace,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ReachTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gif: Option<String>,
}

pub fn usage() -> &'static str {
    USAGE
}

/// Parse `X,Y,Zmm` or `X,Y,Zm` into metres.
fn parse_point(text: &str) -> Result<[f64; 3]> {
    let text = text.trim();
    let (numbers, scale) = if let Some(numbers) = text.strip_suffix("mm") {
        (numbers, 1e-3)
    } else if let Some(numbers) = text.strip_suffix('m') {
        (numbers, 1.0)
    } else {
        return Err(OpenCadError::validation(format!(
            "point '{text}' needs a unit: X,Y,Zmm or X,Y,Zm"
        )));
    };
    let values: Vec<f64> = numbers
        .split(',')
        .map(|value| value.trim().parse::<f64>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| OpenCadError::validation(format!("point '{text}' is not X,Y,Z")))?;
    let [x, y, z] = <[f64; 3]>::try_from(values)
        .map_err(|_| OpenCadError::validation(format!("point '{text}' needs three values")))?;
    if ![x, y, z].iter().all(|value| value.is_finite()) {
        return Err(OpenCadError::validation(format!(
            "point '{text}' must be finite"
        )));
    }
    Ok([x * scale, y * scale, z * scale])
}

fn parse_length(text: &str) -> Result<f64> {
    match JointValue::parse(text)? {
        JointValue::LengthM(metres) if metres > 0.0 => Ok(metres),
        _ => Err(OpenCadError::validation(format!(
            "'{text}' must be a positive length in mm or m"
        ))),
    }
}

/// Parse the flags after `<assembly>`.
pub fn parse_reach_args(args: &[String]) -> Result<ReachOptions> {
    let mut options = ReachOptions {
        tool: String::new(),
        point_m: [0.0; 3],
        target_m: None,
        tolerance_m: 5e-4,
        gif: None,
        gif_args: Vec::new(),
    };
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| OpenCadError::validation(format!("{flag} needs a value\n{USAGE}")))?;
        match flag {
            "--tool" => options.tool = value.clone(),
            "--point" => options.point_m = parse_point(value)?,
            "--target" => options.target_m = Some(parse_point(value)?),
            "--tolerance" => options.tolerance_m = parse_length(value)?,
            "--gif" => {
                options.gif = Some(value.clone());
                options.gif_args = args[index + 2..].to_vec();
                break;
            }
            _ => {
                return Err(OpenCadError::validation(format!(
                    "unknown option '{flag}'\n{USAGE}"
                )))
            }
        }
        index += 2;
    }
    if options.tool.is_empty() {
        return Err(OpenCadError::validation(format!(
            "--tool is required\n{USAGE}"
        )));
    }
    if options.gif.is_some() && options.target_m.is_none() {
        return Err(OpenCadError::validation(
            "--gif needs a --target to reach for",
        ));
    }
    if options.gif_args.iter().any(|arg| arg == "--pose") {
        return Err(OpenCadError::validation(
            "--gif animates the reach pose; --pose is not allowed after it",
        ));
    }
    Ok(options)
}

fn mm(point: [f64; 3]) -> [f64; 3] {
    point.map(|value| (value * 1e6).round() / 1e3)
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Answer the reach query for `input`, writing a GIF when asked.
pub fn reach(input: &str, options: &ReachOptions) -> Result<ReachSummary> {
    let doc = read_ocad(input)?;
    let assembly = doc
        .assembly
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("reach needs an assembly document with joints"))?;
    let assembly = solved_assembly(assembly)?;
    let tree = kinematic_tree(&assembly)?;
    let instance = assembly
        .instances
        .iter()
        .find(|instance| {
            let id = instance.id.as_str();
            id == options.tool || id.split_once(':').map(|(_, short)| short) == Some(&options.tool)
        })
        .ok_or_else(|| {
            OpenCadError::validation(format!(
                "no instance named '{}'; instances: {}",
                options.tool,
                assembly
                    .instances
                    .iter()
                    .map(|instance| instance.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
    let tool = ToolPoint {
        instance: InstanceId::new(instance.id.as_str())?,
        point_m: options.point_m,
    };
    let space = workspace(&tree, &tool)?;
    let summary_workspace = ReachWorkspace {
        samples: space.samples,
        bounds_mm: space.bounds_m.map(mm),
        reach_mm: space.reach_m.map(|value| round3(value * 1000.0)),
        base_mm: mm(space.base_m),
    };

    let mut target = None;
    let mut gif = None;
    if let Some(target_m) = options.target_m {
        let solution = solve_reach(&tree, &tool, target_m, options.tolerance_m)?;
        let named = movable_joints(&assembly, &tree);
        let mut joints = Vec::new();
        let mut pose = Vec::new();
        let mut keyframe = Vec::new();
        for joint in &named {
            let Some(value) = solution.positions.get(&joint.instance).copied() else {
                continue;
            };
            let (shown, unit, exact) = if joint.is_prismatic() {
                (value * 1000.0, "mm", JointValue::LengthM(value))
            } else {
                (value.to_degrees(), "deg", JointValue::AngleRad(value))
            };
            joints.push(ReachJointValue {
                joint: joint.id.clone(),
                instance: joint.instance.clone(),
                value: round3(shown),
                unit: unit.into(),
            });
            pose.push(format!("{}={}{unit}", joint.short_name(), round3(shown)));
            keyframe.push((joint.id.clone(), exact));
        }
        let gap_mm = round3(solution.distance_m * 1000.0);
        if let Some(output) = &options.gif {
            let mut animation = parse_joint_animation_args(&options.gif_args)?;
            animation.poses = vec![keyframe];
            animation.marker_m = Some(target_m);
            animation.note = Some(if solution.reachable {
                "target reached".to_string()
            } else {
                format!(
                    "target out of reach: {:.1} mm short",
                    solution.distance_m * 1000.0
                )
            });
            animate_joints(input, output, &animation)?;
            gif = Some(output.clone());
        }
        target = Some(ReachTarget {
            target_mm: mm(target_m),
            tolerance_mm: round3(options.tolerance_m * 1000.0),
            reachable: solution.reachable,
            gap_mm,
            tool_mm: mm(solution.tool_m),
            joints,
            pose: pose.join(","),
        });
    }
    Ok(ReachSummary {
        tool: instance.id.as_str().to_string(),
        point_mm: mm(options.point_m),
        workspace: summary_workspace,
        target,
        gif,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    fn arm() -> String {
        format!(
            "{}/../../examples/robot_arm_assembly.ocad.d",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).expect("mkdir");
        for entry in std::fs::read_dir(from).expect("read dir") {
            let entry = entry.expect("entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("type").is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).expect("copy");
            }
        }
    }

    #[test]
    fn lengthening_the_upper_arm_brings_an_out_of_reach_target_within_reach() {
        let examples = format!("{}/../../examples", env!("CARGO_MANIFEST_DIR"));
        let scratch =
            std::env::temp_dir().join(format!("musubicad-reach-loop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let document = scratch.join("arm.ocad.d");
        copy_dir(
            std::path::Path::new(&format!("{examples}/robot_arm_assembly.ocad.d")),
            &document,
        );
        let document = document.to_str().expect("utf-8").to_string();
        let options = parse_reach_args(&strings(&[
            "--tool",
            "gripper",
            "--point",
            "0,40,7mm",
            "--target",
            "0,330,63mm",
        ]))
        .expect("options");
        let before = reach(&document, &options).expect("before");
        let gap = before.target.as_ref().expect("target").gap_mm;
        assert!((gap - 20.0).abs() < 0.1, "{before:?}");

        // The two checked-in patches: a verified 40 mm longer link (its mass
        // must rise 30–50 g) and the elbow connector moved to its new end.
        let patch = |doc: String, patch: &str| {
            crate::patch::patch_document_with_options(&crate::patch::PatchArgs {
                doc_path: doc,
                patch_path: format!("{examples}/agent/{patch}"),
                options: crate::patch::PatchOptions::default(),
            })
            .expect(patch)
        };
        patch(
            format!("{document}/parts/upper_arm.ocad.d"),
            "reach_upper_arm_length_patch.json",
        );
        patch(document.clone(), "reach_elbow_connector_patch.json");

        let after = reach(&document, &options).expect("after");
        let target = after.target.as_ref().expect("target");
        assert!(target.reachable, "{after:?}");
        assert!(after.workspace.reach_mm[1] > before.workspace.reach_mm[1] + 39.0);
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[test]
    fn parses_points_with_units_and_gif_tail() {
        assert_eq!(parse_point("0,40,7mm").expect("mm"), [0.0, 0.04, 0.007]);
        assert_eq!(parse_point("0.1, 0, 0m").expect("m"), [0.1, 0.0, 0.0]);
        assert!(parse_point("0,40,7").is_err());
        assert!(parse_point("0,40mm").is_err());
        let options = parse_reach_args(&strings(&[
            "--tool",
            "gripper",
            "--target",
            "120,170,63mm",
            "--gif",
            "reach.gif",
            "--width",
            "320",
        ]))
        .expect("options");
        assert_eq!(options.gif.as_deref(), Some("reach.gif"));
        assert_eq!(options.gif_args, strings(&["--width", "320"]));
        assert!((options.tolerance_m - 5e-4).abs() < 1e-15);
        assert!(parse_reach_args(&strings(&["--target", "1,2,3mm"])).is_err());
        assert!(parse_reach_args(&strings(&["--tool", "g", "--gif", "x.gif"])).is_err());
        assert!(parse_reach_args(&strings(&["--tool", "g", "--tolerance", "5deg"])).is_err());
    }

    #[test]
    fn the_robot_arm_reaches_inside_and_reports_the_gap_outside() {
        let base = |target: &str| {
            parse_reach_args(&strings(&[
                "--tool", "gripper", "--point", "0,40,7mm", "--target", target,
            ]))
            .expect("options")
        };
        let inside = reach(&arm(), &base("120,170,63mm")).expect("inside");
        let target = inside.target.as_ref().expect("target");
        assert!(target.reachable, "{inside:?}");
        assert!(target.gap_mm < 0.01);
        assert_eq!(target.joints.len(), 3);
        assert!(target.pose.starts_with("shoulder="), "{}", target.pose);
        // Upper arm 160 + forearm 110 + 40 mm tool offset: 310 mm, plus rise.
        assert!(inside.workspace.reach_mm[1] > 300.0 && inside.workspace.reach_mm[1] < 315.0);

        let outside = reach(&arm(), &base("0,350,63mm")).expect("outside");
        let target = outside.target.as_ref().expect("target");
        assert!(!target.reachable);
        assert!((target.gap_mm - 40.0).abs() < 0.1, "{outside:?}");
        assert!(reach(
            &arm(),
            &parse_reach_args(&strings(&["--tool", "elbow_pad"])).expect("options")
        )
        .is_err());
    }
}
