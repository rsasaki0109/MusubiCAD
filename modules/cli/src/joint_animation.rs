//! `musubicad animate-joints`: a GPU-free GIF of an assembly moving through
//! its robot joints.
//!
//! Poses come from the kinematic tree the URDF export uses (ADR-028,
//! ADR-030), so the motion shown is the motion a simulator gets: the same
//! axes, the same limits, the same zero pose.  Parts are regenerated once;
//! each frame only moves their meshes.  Frames are drawn by the CPU preview
//! renderer (ADR-031) with one framing for the whole clip, so the camera
//! stays still while the arm moves.

use std::collections::BTreeMap;

use opencad_assembly::{kinematic_tree, AssemblyModel, JointKind, KinematicTree};
use opencad_core::{OpenCadError, Result};
use opencad_file::read_ocad;
use opencad_geometry::{MeshSet, RigidTransform};
use opencad_render::{
    render_preview_framed, write_gif_frames, PreviewFraming, PreviewMesh, PreviewView, RenderImage,
};
use serde::Serialize;

use crate::preview::PALETTE;

/// Upper bound on frames in one clip, to keep GIFs shareable.
const MAX_FRAMES: usize = 600;
/// Default keyframes move each joint to this fraction of its limits.
const DEFAULT_REACH: f64 = 0.6;
/// Continuous joints have no limits; the default keyframes swing them ±90°.
const CONTINUOUS_SWING_RAD: f64 = std::f64::consts::FRAC_PI_2;

const USAGE: &str = "usage: musubicad animate-joints <assembly> <output.gif> \
[--pose JOINT=VALUE[,JOINT=VALUE...]]... [--frames-per-move N] [--fps N] \
[--view iso|front|top|right] [--width N] [--height N]
  VALUE carries a unit: deg or rad for revolute and continuous joints, mm or m for prismatic ones.
  JOINT is a joint ID (joint:shoulder), its short name (shoulder), its mate, or the moving instance.";

/// Parsed `animate-joints` options.
#[derive(Debug, Clone, PartialEq)]
pub struct JointAnimationOptions {
    pub view: PreviewView,
    pub width_px: u32,
    pub height_px: u32,
    pub frames_per_second: u32,
    /// Frames spent moving from one keyframe to the next.
    pub frames_per_move: u32,
    /// Keyframes as written on the command line, unresolved.
    pub poses: Vec<Vec<(String, JointValue)>>,
}

impl Default for JointAnimationOptions {
    fn default() -> Self {
        Self {
            view: PreviewView::Iso,
            width_px: 720,
            height_px: 540,
            frames_per_second: 15,
            frames_per_move: 14,
            poses: Vec::new(),
        }
    }
}

/// A joint position with its unit, converted to SI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JointValue {
    AngleRad(f64),
    LengthM(f64),
}

impl JointValue {
    /// Parse `45deg`, `0.5rad`, `20mm`, or `0.02m`.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim();
        let (number, value): (&str, fn(f64) -> Self) =
            if let Some(number) = text.strip_suffix("deg") {
                (number, |v| Self::AngleRad(v.to_radians()))
            } else if let Some(number) = text.strip_suffix("rad") {
                (number, Self::AngleRad)
            } else if let Some(number) = text.strip_suffix("mm") {
                (number, |v| Self::LengthM(v / 1000.0))
            } else if let Some(number) = text.strip_suffix('m') {
                (number, Self::LengthM)
            } else {
                return Err(OpenCadError::validation(format!(
                    "joint position '{text}' needs a unit: deg, rad, mm, or m"
                )));
            };
        let number: f64 = number.trim().parse().map_err(|_| {
            OpenCadError::validation(format!("joint position '{text}' is not a number"))
        })?;
        if !number.is_finite() {
            return Err(OpenCadError::validation(format!(
                "joint position '{text}' must be finite"
            )));
        }
        Ok(value(number))
    }
}

/// One joint that moves in the clip.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnimatedJoint {
    /// Joint ID, or the mate ID for an undeclared concentric joint.
    pub joint: String,
    /// Instance the joint moves.
    pub instance: String,
    /// `deg` or `mm`.
    pub unit: String,
    /// Smallest and largest position reached, in `unit`.
    pub range: [f64; 2],
}

/// What `animate-joints` wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JointAnimationSummary {
    pub output: String,
    pub frames: usize,
    pub frames_per_second: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub view: String,
    /// Keyframes after the zero pose; the clip loops back to zero.
    pub keyframes: usize,
    pub joints: Vec<AnimatedJoint>,
}

/// Parse the flags after `<assembly> <output.gif>`.
pub fn parse_joint_animation_args(args: &[String]) -> Result<JointAnimationOptions> {
    let mut options = JointAnimationOptions::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| OpenCadError::validation(format!("{flag} needs a value\n{USAGE}")))?;
        let number = || -> Result<u32> {
            value
                .parse()
                .map_err(|_| OpenCadError::validation(format!("{flag} needs an integer")))
        };
        match flag {
            "--pose" => options.poses.push(parse_pose(value)?),
            "--frames-per-move" => options.frames_per_move = number()?,
            "--fps" => options.frames_per_second = number()?,
            "--width" => options.width_px = number()?,
            "--height" => options.height_px = number()?,
            "--view" => options.view = PreviewView::parse(value)?,
            _ => {
                return Err(OpenCadError::validation(format!(
                    "unknown option '{flag}'\n{USAGE}"
                )))
            }
        }
        index += 2;
    }
    if options.frames_per_move == 0 || options.frames_per_second == 0 {
        return Err(OpenCadError::validation(
            "--frames-per-move and --fps must be positive",
        ));
    }
    Ok(options)
}

fn parse_pose(text: &str) -> Result<Vec<(String, JointValue)>> {
    text.split(',')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let (name, value) = entry.split_once('=').ok_or_else(|| {
                OpenCadError::validation(format!("pose entry '{entry}' must be JOINT=VALUE"))
            })?;
            Ok((name.trim().to_string(), JointValue::parse(value)?))
        })
        .collect()
}

/// Usage text for `musubicad animate-joints`.
pub fn usage() -> &'static str {
    USAGE
}

/// A movable joint and the names it answers to.
struct NamedJoint {
    id: String,
    mate: Option<String>,
    instance: String,
    kind: JointKind,
}

impl NamedJoint {
    fn answers_to(&self, name: &str) -> bool {
        let short = |id: &str| id.split_once(':').map(|(_, rest)| rest.to_string());
        [
            Some(self.id.clone()),
            self.mate.clone(),
            Some(self.instance.clone()),
        ]
        .into_iter()
        .flatten()
        .any(|id| id == name || short(&id).as_deref() == Some(name))
    }

    fn is_prismatic(&self) -> bool {
        matches!(self.kind, JointKind::Prismatic { .. })
    }

    /// Position the default keyframes move to, toward the upper or lower side.
    fn default_reach(&self, upper: bool) -> f64 {
        let (lower, high) = self
            .kind
            .limits()
            .unwrap_or((-CONTINUOUS_SWING_RAD, CONTINUOUS_SWING_RAD));
        DEFAULT_REACH * if upper { high } else { lower }
    }
}

fn movable_joints(model: &AssemblyModel, tree: &KinematicTree) -> Vec<NamedJoint> {
    tree.links
        .iter()
        .filter_map(|link| {
            let joint = link.joint.as_ref()?;
            if !joint.kind.is_movable() {
                return None;
            }
            let mate = joint.mate.as_ref().map(|mate| mate.as_str().to_string());
            let id = mate
                .as_deref()
                .and_then(|mate| {
                    model
                        .joints
                        .iter()
                        .find(|declared| declared.mate.as_str() == mate)
                        .map(|declared| declared.id.as_str().to_string())
                })
                .or_else(|| mate.clone())
                .unwrap_or_else(|| link.instance.as_str().to_string());
            Some(NamedJoint {
                id,
                mate,
                instance: link.instance.as_str().to_string(),
                kind: joint.kind,
            })
        })
        .collect()
}

/// Keyframes keyed by moving instance, SI positions.  Without `--pose`, every
/// joint swings toward its upper limits together, then toward its lower ones.
fn resolve_keyframes(
    joints: &[NamedJoint],
    poses: &[Vec<(String, JointValue)>],
) -> Result<Vec<BTreeMap<String, f64>>> {
    if joints.is_empty() {
        return Err(OpenCadError::validation(
            "the assembly has no movable joints; declare one with add_joint or a concentric mate",
        ));
    }
    if poses.is_empty() {
        return Ok([true, false]
            .into_iter()
            .map(|upper| {
                joints
                    .iter()
                    .map(|joint| (joint.instance.clone(), joint.default_reach(upper)))
                    .collect()
            })
            .collect());
    }
    poses
        .iter()
        .map(|pose| {
            let mut keyframe = BTreeMap::new();
            for (name, value) in pose {
                let joint = joints
                    .iter()
                    .find(|joint| joint.answers_to(name))
                    .ok_or_else(|| {
                        OpenCadError::validation(format!(
                            "no movable joint named '{name}'; movable joints: {}",
                            joints
                                .iter()
                                .map(|joint| joint.id.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    })?;
                let position = match (value, joint.is_prismatic()) {
                    (JointValue::AngleRad(rad), false) => *rad,
                    (JointValue::LengthM(m), true) => *m,
                    (JointValue::AngleRad(_), true) => {
                        return Err(OpenCadError::validation(format!(
                            "joint '{}' is prismatic; give its position in mm or m",
                            joint.id
                        )))
                    }
                    (JointValue::LengthM(_), false) => {
                        return Err(OpenCadError::validation(format!(
                            "joint '{}' rotates; give its position in deg or rad",
                            joint.id
                        )))
                    }
                };
                keyframe.insert(joint.instance.clone(), position);
            }
            Ok(keyframe)
        })
        .collect()
}

/// Joint positions for every frame: zero → each keyframe → zero, eased in
/// and out, `frames_per_move` frames per move, ending one frame before the
/// loop restarts.
fn frame_positions(
    keyframes: &[BTreeMap<String, f64>],
    frames_per_move: u32,
) -> Result<Vec<BTreeMap<String, f64>>> {
    let mut stops = vec![BTreeMap::new()];
    stops.extend(keyframes.iter().cloned());
    stops.push(BTreeMap::new());
    let moves = stops.len() - 1;
    if moves * frames_per_move as usize > MAX_FRAMES {
        return Err(OpenCadError::validation(format!(
            "{moves} moves × {frames_per_move} frames exceeds {MAX_FRAMES} frames; \
             lower --frames-per-move or use fewer poses"
        )));
    }
    let mut frames = Vec::with_capacity(moves * frames_per_move as usize);
    for pair in stops.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);
        let instances: Vec<&String> = from.keys().chain(to.keys()).collect();
        for step in 0..frames_per_move {
            let t = f64::from(step) / f64::from(frames_per_move);
            let eased = t * t * (3.0 - 2.0 * t);
            let mut positions = BTreeMap::new();
            for instance in &instances {
                let a = from.get(*instance).copied().unwrap_or(0.0);
                let b = to.get(*instance).copied().unwrap_or(0.0);
                positions.insert((*instance).clone(), a + (b - a) * eased);
            }
            frames.push(positions);
        }
    }
    Ok(frames)
}

/// `mesh` moved by `delta` (positions and normals).
fn moved(mesh: &MeshSet, delta: RigidTransform) -> MeshSet {
    let rotation = RigidTransform {
        translation_m: [0.0; 3],
        rotation: delta.rotation,
    };
    let apply = |transform: RigidTransform, point: [f32; 3]| {
        transform
            .transform_point(point.map(f64::from))
            .map(|value| value as f32)
    };
    MeshSet {
        positions: mesh
            .positions
            .iter()
            .map(|position| apply(delta, *position))
            .collect(),
        normals: mesh
            .normals
            .iter()
            .map(|normal| apply(rotation, *normal))
            .collect(),
        indices: mesh.indices.clone(),
        triangle_face_ids: mesh.triangle_face_ids.clone(),
    }
}

/// Instance meshes in palette order, matching `musubicad preview` colours.
fn colored(frame: &[MeshSet]) -> Vec<PreviewMesh<'_>> {
    frame
        .iter()
        .enumerate()
        .map(|(index, mesh)| PreviewMesh {
            mesh,
            color: PALETTE[index % PALETTE.len()],
        })
        .collect()
}

/// Regenerate `input` once and write a GIF of it moving through its joints.
pub fn animate_joints(
    input: &str,
    output: &str,
    options: &JointAnimationOptions,
) -> Result<JointAnimationSummary> {
    if !output.to_ascii_lowercase().ends_with(".gif") {
        return Err(OpenCadError::validation(
            "joint animation output must use .gif",
        ));
    }
    let doc = read_ocad(input)?;
    let assembly = doc.assembly.clone().ok_or_else(|| {
        OpenCadError::validation("animate-joints needs an assembly document with joints")
    })?;
    let tree = kinematic_tree(&assembly)?;
    let joints = movable_joints(&assembly, &tree);
    let keyframes = resolve_keyframes(&joints, &options.poses)?;
    let frames = frame_positions(&keyframes, options.frames_per_move)?;
    // Check every keyframe against the limits before regenerating anything.
    for keyframe in &keyframes {
        tree.pose(keyframe)?;
    }

    let meshes = crate::export::assembly_instance_meshes(input, &doc)?;
    let mut inverse_placements = BTreeMap::new();
    for instance in &assembly.instances {
        inverse_placements.insert(
            instance.id.as_str().to_string(),
            instance.placement.transform.inverse()?,
        );
    }

    let mut posed_frames: Vec<Vec<MeshSet>> = Vec::with_capacity(frames.len());
    for positions in &frames {
        let posed: BTreeMap<String, RigidTransform> = tree
            .pose(positions)?
            .into_iter()
            .map(|(instance, world)| (instance.as_str().to_string(), world))
            .collect();
        posed_frames.push(
            meshes
                .iter()
                .map(|(instance, _, mesh)| {
                    let key = instance.as_str();
                    match (posed.get(key), inverse_placements.get(key)) {
                        (Some(world), Some(inverse)) => moved(mesh, world.compose(*inverse)),
                        _ => mesh.clone(),
                    }
                })
                .collect(),
        );
    }

    let mut framing = PreviewFraming::new(options.view);
    for frame in &posed_frames {
        framing.include(&colored(frame));
    }
    let mut images = Vec::with_capacity(posed_frames.len());
    for frame in &posed_frames {
        let image = render_preview_framed(
            &colored(frame),
            &framing,
            options.width_px,
            options.height_px,
        )?;
        images.push(RenderImage {
            width: image.width,
            height: image.height,
            non_background_pixels: 0,
            rgba: image.rgba,
        });
    }
    write_gif_frames(&images, options.frames_per_second, output)?;

    let animated = joints
        .iter()
        .filter_map(|joint| {
            let reached: Vec<f64> = frames
                .iter()
                .filter_map(|positions| positions.get(&joint.instance).copied())
                .collect();
            if reached.iter().all(|value| value.abs() < 1e-12) {
                return None;
            }
            let (scale, unit) = if joint.is_prismatic() {
                (1000.0, "mm")
            } else {
                (180.0 / std::f64::consts::PI, "deg")
            };
            let round = |value: f64| (value * scale * 1000.0).round() / 1000.0;
            let low = reached.iter().copied().fold(0.0, f64::min);
            let high = reached.iter().copied().fold(0.0, f64::max);
            Some(AnimatedJoint {
                joint: joint.id.clone(),
                instance: joint.instance.clone(),
                unit: unit.into(),
                range: [round(low), round(high)],
            })
        })
        .collect();
    Ok(JointAnimationSummary {
        output: output.to_string(),
        frames: images.len(),
        frames_per_second: options.frames_per_second,
        width_px: options.width_px,
        height_px: options.height_px,
        view: options.view.name().into(),
        keyframes: keyframes.len(),
        joints: animated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_assembly::models::robot_arm_assembly_model;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn joint_values_need_units() {
        assert_eq!(
            JointValue::parse("90deg").expect("deg"),
            JointValue::AngleRad(std::f64::consts::FRAC_PI_2)
        );
        assert_eq!(
            JointValue::parse("-0.5rad").expect("rad"),
            JointValue::AngleRad(-0.5)
        );
        assert_eq!(
            JointValue::parse("20mm").expect("mm"),
            JointValue::LengthM(0.02)
        );
        assert_eq!(
            JointValue::parse("0.1m").expect("m"),
            JointValue::LengthM(0.1)
        );
        assert!(JointValue::parse("45").is_err());
        assert!(JointValue::parse("fastdeg").is_err());
        assert!(JointValue::parse("infdeg").is_err());
    }

    #[test]
    fn parses_poses_and_options() {
        let options = parse_joint_animation_args(&strings(&[
            "--pose",
            "shoulder=45deg,elbow=-30deg",
            "--pose",
            "wrist=1rad",
            "--frames-per-move",
            "6",
            "--view",
            "top",
            "--width",
            "320",
        ]))
        .expect("options");
        assert_eq!(options.poses.len(), 2);
        assert_eq!(options.poses[0][1].0, "elbow");
        assert_eq!(options.frames_per_move, 6);
        assert_eq!(options.view, PreviewView::Top);
        assert_eq!((options.width_px, options.height_px), (320, 540));
        assert!(parse_joint_animation_args(&strings(&["--pose", "shoulder"])).is_err());
        assert!(parse_joint_animation_args(&strings(&["--speed", "2"])).is_err());
        assert!(parse_joint_animation_args(&strings(&["--fps", "0"])).is_err());
    }

    #[test]
    fn default_keyframes_swing_every_robot_arm_joint_within_its_limits() {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        let joints = movable_joints(&model, &tree);
        let ids: Vec<&str> = joints.iter().map(|joint| joint.id.as_str()).collect();
        assert_eq!(ids, ["joint:shoulder", "joint:elbow", "joint:wrist"]);
        let keyframes = resolve_keyframes(&joints, &[]).expect("keyframes");
        assert_eq!(keyframes.len(), 2);
        // Elbow limits are [-100°, 140°].
        let elbow = |keyframe: &BTreeMap<String, f64>| keyframe["instance:forearm"].to_degrees();
        assert!((elbow(&keyframes[0]) - 84.0).abs() < 1e-9);
        assert!((elbow(&keyframes[1]) + 60.0).abs() < 1e-9);
        for positions in frame_positions(&keyframes, 8).expect("frames") {
            tree.pose(&positions).expect("within limits");
        }
    }

    #[test]
    fn named_poses_resolve_by_joint_mate_or_instance() {
        let model = robot_arm_assembly_model().expect("model");
        let tree = kinematic_tree(&model).expect("tree");
        let joints = movable_joints(&model, &tree);
        let pose = |text: &str| resolve_keyframes(&joints, &[parse_pose(text).expect("pose")]);
        for name in ["shoulder", "joint:shoulder", "mate:shoulder", "upper_arm"] {
            let keyframes = pose(&format!("{name}=30deg")).expect(name);
            assert!((keyframes[0]["instance:upper_arm"].to_degrees() - 30.0).abs() < 1e-9);
        }
        let error = pose("knee=30deg").expect_err("unknown joint");
        assert!(error.to_string().contains("joint:elbow"), "{error}");
        assert!(pose("elbow=10mm").is_err());
    }

    #[test]
    fn frames_loop_back_to_zero_and_are_capped() {
        let keyframes = vec![BTreeMap::from([("instance:a".to_string(), 1.0)])];
        let frames = frame_positions(&keyframes, 4).expect("frames");
        assert_eq!(frames.len(), 8);
        assert_eq!(frames[0]["instance:a"], 0.0);
        assert_eq!(frames[4]["instance:a"], 1.0);
        assert!(frames[7]["instance:a"] > 0.0 && frames[7]["instance:a"] < 0.2);
        let many = vec![BTreeMap::new(); 100];
        assert!(frame_positions(&many, 10).is_err());
    }

    #[test]
    fn moving_a_mesh_turns_its_normals_too() {
        let mesh = MeshSet {
            positions: vec![[0.01, 0.0, 0.0]],
            normals: vec![[1.0, 0.0, 0.0]],
            indices: Vec::new(),
            triangle_face_ids: Vec::new(),
        };
        let quarter = RigidTransform {
            translation_m: [0.0, 0.0, 0.5],
            rotation: [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        };
        let result = moved(&mesh, quarter);
        let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
        assert!(near(result.positions[0], [0.0, 0.01, 0.5]));
        assert!(near(result.normals[0], [0.0, 1.0, 0.0]));
    }
}
