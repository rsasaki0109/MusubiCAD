//! Ready-made flagship assembly models.

use opencad_core::{
    ComponentId, ConnectorId, DocumentId, InstanceId, JointId, MateId, Result, TopoRefId,
};
use opencad_geometry::{RigidTransform, TopoRef};

use crate::component::Component;
use crate::connector::Connector;
use crate::instance::{Instance, Placement};
use crate::joint::{AssemblyJoint, JointMotion};
use crate::mate::{Mate, MateEntity, MateKind};
use crate::model::AssemblyModel;

/// Robot-arm child part document IDs and relative paths.
pub mod robot_arm {
    pub const BASE_PATH: &str = "parts/base.ocad.d";
    pub const UPPER_ARM_PATH: &str = "parts/upper_arm.ocad.d";
    pub const FOREARM_PATH: &str = "parts/forearm.ocad.d";
    pub const GRIPPER_PATH: &str = "parts/gripper.ocad.d";

    pub const BASE_DOC: &str = "doc:robot_arm_base_001";
    pub const UPPER_ARM_DOC: &str = "doc:robot_arm_upper_arm_001";
    pub const FOREARM_DOC: &str = "doc:robot_arm_forearm_001";
    pub const GRIPPER_DOC: &str = "doc:robot_arm_gripper_001";
}

fn rotation_z(angle_rad: f64) -> [[f64; 3]; 3] {
    [
        [angle_rad.cos(), -angle_rad.sin(), 0.0],
        [angle_rad.sin(), angle_rad.cos(), 0.0],
        [0.0, 0.0, 1.0],
    ]
}

/// Articulated planar robot arm: base, upper arm, forearm, and wrist gripper.
///
/// Links are stacked along `+Z` so adjacent joint hubs touch face-to-face with
/// zero interference. The upper arm sits on the base turret; the forearm and
/// gripper rotate `-45°` about `+Z` at the elbow and wrist joints, so the
/// initial concentric mate residuals are exactly zero and the solver leaves the
/// pose unchanged.
pub fn robot_arm_assembly_model() -> Result<AssemblyModel> {
    use robot_arm::{
        BASE_DOC, BASE_PATH, FOREARM_DOC, FOREARM_PATH, GRIPPER_DOC, GRIPPER_PATH, UPPER_ARM_DOC,
        UPPER_ARM_PATH,
    };

    const ELBOW_ANGLE_RAD: f64 = -std::f64::consts::FRAC_PI_4;
    const TURRET_HEIGHT_M: f64 = 0.030;
    const UPPER_ARM_LENGTH_M: f64 = 0.160;
    const UPPER_ARM_THICKNESS_M: f64 = 0.014;
    const FOREARM_LENGTH_M: f64 = 0.110;
    const FOREARM_THICKNESS_M: f64 = 0.012;

    // Links are stacked along +Z so adjacent joint hubs touch face-to-face
    // instead of occupying the same volume (zero interference).
    let upper_arm_z = TURRET_HEIGHT_M;
    let forearm_z = upper_arm_z + UPPER_ARM_THICKNESS_M;
    let gripper_z = forearm_z + FOREARM_THICKNESS_M;

    let forearm_rotation = rotation_z(ELBOW_ANGLE_RAD);
    let forearm_tip = [
        forearm_rotation[0][1] * FOREARM_LENGTH_M,
        forearm_rotation[1][1] * FOREARM_LENGTH_M,
        0.0,
    ];
    let upper_arm_tip_y = UPPER_ARM_LENGTH_M;
    let gripper_origin = [forearm_tip[0], upper_arm_tip_y + forearm_tip[1], gripper_z];

    let placement = |translation: [f64; 3], rotation: [[f64; 3]; 3]| {
        Placement::new(RigidTransform {
            translation_m: translation,
            rotation,
        })
    };

    let components = vec![
        Component::new(
            ComponentId::new("component:base")?,
            BASE_PATH,
            DocumentId::new(BASE_DOC)?,
        ),
        Component::new(
            ComponentId::new("component:upper_arm")?,
            UPPER_ARM_PATH,
            DocumentId::new(UPPER_ARM_DOC)?,
        ),
        Component::new(
            ComponentId::new("component:forearm")?,
            FOREARM_PATH,
            DocumentId::new(FOREARM_DOC)?,
        ),
        Component::new(
            ComponentId::new("component:gripper")?,
            GRIPPER_PATH,
            DocumentId::new(GRIPPER_DOC)?,
        ),
    ];

    let instances = vec![
        Instance::new(
            InstanceId::new("instance:base")?,
            ComponentId::new("component:base")?,
            placement([0.0, 0.0, 0.0], RigidTransform::identity_rotation()),
            "Base",
        ),
        Instance::new(
            InstanceId::new("instance:upper_arm")?,
            ComponentId::new("component:upper_arm")?,
            placement([0.0, 0.0, upper_arm_z], RigidTransform::identity_rotation()),
            "Upper Arm",
        ),
        Instance::new(
            InstanceId::new("instance:forearm")?,
            ComponentId::new("component:forearm")?,
            placement([0.0, UPPER_ARM_LENGTH_M, forearm_z], forearm_rotation),
            "Forearm",
        ),
        Instance::new(
            InstanceId::new("instance:gripper")?,
            ComponentId::new("component:gripper")?,
            placement(gripper_origin, forearm_rotation),
            "Gripper",
        ),
    ];

    let shoulder_connector = Connector::new(
        ConnectorId::new("connector:shoulder_pin")?,
        "shoulder_pin",
        InstanceId::new("instance:base")?,
        RigidTransform::from_translation([0.0, 0.0, TURRET_HEIGHT_M]),
    );
    let upper_shoulder = Connector::new(
        ConnectorId::new("connector:upper_arm_shoulder")?,
        "upper_arm_shoulder",
        InstanceId::new("instance:upper_arm")?,
        RigidTransform::identity(),
    );
    let upper_elbow = Connector::new(
        ConnectorId::new("connector:upper_arm_elbow")?,
        "upper_arm_elbow",
        InstanceId::new("instance:upper_arm")?,
        RigidTransform::from_translation([0.0, UPPER_ARM_LENGTH_M, UPPER_ARM_THICKNESS_M]),
    );
    let forearm_elbow = Connector::new(
        ConnectorId::new("connector:forearm_elbow")?,
        "forearm_elbow",
        InstanceId::new("instance:forearm")?,
        RigidTransform::identity(),
    );
    let forearm_wrist = Connector::new(
        ConnectorId::new("connector:forearm_wrist")?,
        "forearm_wrist",
        InstanceId::new("instance:forearm")?,
        RigidTransform::from_translation([0.0, FOREARM_LENGTH_M, FOREARM_THICKNESS_M]),
    );
    let gripper_wrist = Connector::new(
        ConnectorId::new("connector:gripper_wrist")?,
        "gripper_wrist",
        InstanceId::new("instance:gripper")?,
        RigidTransform::identity(),
    );
    let connectors = vec![
        shoulder_connector,
        upper_shoulder,
        upper_elbow,
        forearm_elbow,
        forearm_wrist,
        gripper_wrist,
    ];

    let mates = vec![
        Mate::new(
            MateId::new("mate:ground_base")?,
            MateKind::Ground {
                instance: InstanceId::new("instance:base")?,
            },
        ),
        Mate::new(
            MateId::new("mate:shoulder")?,
            MateKind::Concentric {
                a: MateEntity::with_connector(
                    InstanceId::new("instance:upper_arm")?,
                    TopoRef::face(
                        TopoRefId::new("ref:face:upper_arm_shoulder")?,
                        "feature:upper_arm_proximal_hub",
                        "proximal",
                    ),
                    "upper_arm_shoulder",
                ),
                b: MateEntity::with_connector(
                    InstanceId::new("instance:base")?,
                    TopoRef::face(
                        TopoRefId::new("ref:face:base_turret")?,
                        "feature:turret",
                        "top",
                    ),
                    "shoulder_pin",
                ),
            },
        ),
        Mate::new(
            MateId::new("mate:elbow")?,
            MateKind::Concentric {
                a: MateEntity::with_connector(
                    InstanceId::new("instance:forearm")?,
                    TopoRef::face(
                        TopoRefId::new("ref:face:forearm_elbow")?,
                        "feature:forearm_proximal_hub",
                        "proximal",
                    ),
                    "forearm_elbow",
                ),
                b: MateEntity::with_connector(
                    InstanceId::new("instance:upper_arm")?,
                    TopoRef::face(
                        TopoRefId::new("ref:face:upper_arm_elbow")?,
                        "feature:upper_arm_distal_hub",
                        "distal",
                    ),
                    "upper_arm_elbow",
                ),
            },
        ),
        Mate::new(
            MateId::new("mate:wrist")?,
            MateKind::Concentric {
                a: MateEntity::with_connector(
                    InstanceId::new("instance:gripper")?,
                    TopoRef::face(
                        TopoRefId::new("ref:face:gripper_wrist")?,
                        "feature:wrist_bore",
                        "wrist",
                    ),
                    "gripper_wrist",
                ),
                b: MateEntity::with_connector(
                    InstanceId::new("instance:forearm")?,
                    TopoRef::face(
                        TopoRefId::new("ref:face:forearm_wrist")?,
                        "feature:forearm_distal_hub",
                        "distal",
                    ),
                    "forearm_wrist",
                ),
            },
        ),
    ];

    // Joint zero is the authored pose; limits are measured from it.
    let revolute = |lower_deg: f64, upper_deg: f64, effort_n_m: f64, velocity_rad_s: f64| {
        JointMotion::Revolute {
            lower_rad: lower_deg.to_radians(),
            upper_rad: upper_deg.to_radians(),
            effort_n_m,
            velocity_rad_s,
        }
    };
    let joints = vec![
        AssemblyJoint::new(
            JointId::new("joint:shoulder")?,
            MateId::new("mate:shoulder")?,
            revolute(-150.0, 150.0, 1.5, 3.0),
        ),
        AssemblyJoint::new(
            JointId::new("joint:elbow")?,
            MateId::new("mate:elbow")?,
            revolute(-100.0, 140.0, 1.0, 3.0),
        ),
        AssemblyJoint::new(
            JointId::new("joint:wrist")?,
            MateId::new("mate:wrist")?,
            revolute(-90.0, 90.0, 0.4, 4.0),
        ),
    ];

    Ok(AssemblyModel {
        components,
        instances,
        mates,
        connectors,
        patterns: Vec::new(),
        joints,
    }
    .sorted_deterministic())
}

/// Six-axis arm child part document IDs and relative paths.
pub mod six_axis_arm {
    pub const BASE_PATH: &str = "parts/base.ocad.d";
    pub const TURNTABLE_PATH: &str = "parts/turntable.ocad.d";
    pub const UPPER_ARM_PATH: &str = "parts/upper_arm.ocad.d";
    pub const FOREARM_PATH: &str = "parts/forearm.ocad.d";
    pub const WRIST_PATH: &str = "parts/wrist.ocad.d";
    pub const HAND_PATH: &str = "parts/hand.ocad.d";
    pub const GRIPPER_PATH: &str = "parts/gripper.ocad.d";

    pub const BASE_DOC: &str = "doc:robot_arm_base_001";
    pub const TURNTABLE_DOC: &str = "doc:six_axis_turntable_001";
    pub const UPPER_ARM_DOC: &str = "doc:robot_arm_upper_arm_001";
    pub const FOREARM_DOC: &str = "doc:robot_arm_forearm_001";
    pub const WRIST_DOC: &str = "doc:six_axis_wrist_001";
    pub const HAND_DOC: &str = "doc:six_axis_hand_001";
    pub const GRIPPER_DOC: &str = "doc:robot_arm_gripper_001";
}

/// Connector frame whose axis (third row, and third column) is the part's
/// `+Y`: the frame of a roll joint along a link.  The matrix is a symmetric
/// rotation (180° about `(0, 1, 1)/√2`), so its row and column agree.
const ROLL_FRAME: [[f64; 3]; 3] = [[-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]];

/// One joint of the six-axis arm: the parent connector, the child connector
/// (both in part frames, axis = third row), and the authored zero angle.
struct ArmJoint {
    name: &'static str,
    parent: &'static str,
    child: &'static str,
    parent_frame: RigidTransform,
    child_frame: RigidTransform,
    zero_angle_rad: f64,
    limits_deg: (f64, f64),
    effort_n_m: f64,
    velocity_rad_s: f64,
    parent_feature: (&'static str, &'static str),
    child_feature: (&'static str, &'static str),
}

/// Six-axis articulated arm (ADR-032): turntable yaw, shoulder and elbow
/// pitch, wrist roll and pitch, and a flange roll, built from the planar
/// arm's base, upper arm, forearm, and gripper plus a turntable, wrist, and
/// hand.
///
/// Every placement is composed from its parent's: parent placement ∘ parent
/// connector ∘ rotation about the joint axis by the zero angle ∘ child
/// connector⁻¹.  Connector frames therefore coincide, every concentric mate
/// starts with zero residual, and adjacent links meet face to face or
/// tangent, without overlapping.
pub fn six_axis_arm_model() -> Result<AssemblyModel> {
    use six_axis_arm as paths;

    let at = |translation: [f64; 3]| RigidTransform::from_translation(translation);
    let roll_at = |translation: [f64; 3]| RigidTransform {
        translation_m: translation,
        rotation: ROLL_FRAME,
    };
    let joints = [
        ArmJoint {
            name: "base_yaw",
            parent: "base",
            child: "turntable",
            parent_frame: at([0.0, 0.0, 0.030]),
            child_frame: RigidTransform::identity(),
            zero_angle_rad: 0.0,
            limits_deg: (-170.0, 170.0),
            effort_n_m: 4.0,
            velocity_rad_s: 2.0,
            parent_feature: ("feature:turret", "top"),
            child_feature: ("feature:turntable_disc", "bottom"),
        },
        ArmJoint {
            name: "shoulder",
            parent: "turntable",
            child: "upper_arm",
            // Shoulder axis along the turntable's +Y, one hub radius above its
            // top face, with the 14 mm link centred on the turntable.
            parent_frame: roll_at([0.0, -0.007, 0.022 + 0.018]),
            child_frame: RigidTransform::identity(),
            zero_angle_rad: (-30.0_f64).to_radians(),
            limits_deg: (-80.0, 80.0),
            effort_n_m: 6.0,
            velocity_rad_s: 2.0,
            parent_feature: ("feature:turntable_disc", "top"),
            child_feature: ("feature:upper_arm_proximal_hub", "proximal"),
        },
        ArmJoint {
            name: "elbow",
            parent: "upper_arm",
            child: "forearm",
            parent_frame: at([0.0, 0.160, 0.014]),
            child_frame: RigidTransform::identity(),
            zero_angle_rad: 115.0_f64.to_radians(),
            limits_deg: (-130.0, 100.0),
            effort_n_m: 3.0,
            velocity_rad_s: 2.5,
            parent_feature: ("feature:upper_arm_distal_hub", "distal"),
            child_feature: ("feature:forearm_proximal_hub", "proximal"),
        },
        ArmJoint {
            name: "wrist_roll",
            parent: "forearm",
            child: "wrist",
            // Along the forearm's centre line, the wrist hub meeting the end
            // of the forearm's 30 mm wrist hub.
            parent_frame: roll_at([0.0, 0.110 + 0.015, 0.006]),
            child_frame: roll_at([0.0, -0.013, 0.006]),
            zero_angle_rad: 0.0,
            limits_deg: (-170.0, 170.0),
            effort_n_m: 1.0,
            velocity_rad_s: 3.0,
            parent_feature: ("feature:forearm_distal_hub", "distal"),
            child_feature: ("feature:wrist_link_proximal_hub", "proximal"),
        },
        ArmJoint {
            name: "wrist_pitch",
            parent: "wrist",
            child: "hand",
            parent_frame: at([0.0, 0.040, 0.012]),
            child_frame: RigidTransform::identity(),
            zero_angle_rad: 70.0_f64.to_radians(),
            limits_deg: (-110.0, 110.0),
            effort_n_m: 0.8,
            velocity_rad_s: 3.0,
            parent_feature: ("feature:wrist_link_distal_hub", "distal"),
            child_feature: ("feature:hand_proximal_hub", "proximal"),
        },
        ArmJoint {
            name: "flange_roll",
            parent: "hand",
            child: "gripper",
            parent_frame: roll_at([0.0, 0.032 + 0.011, 0.005]),
            child_frame: roll_at([0.0, 0.0, 0.007]),
            zero_angle_rad: 0.0,
            limits_deg: (-170.0, 170.0),
            effort_n_m: 0.5,
            velocity_rad_s: 4.0,
            parent_feature: ("feature:hand_distal_hub", "distal"),
            child_feature: ("feature:wrist_bore", "wrist"),
        },
    ];

    let parts = [
        ("base", "Base", paths::BASE_PATH, paths::BASE_DOC),
        (
            "turntable",
            "Turntable",
            paths::TURNTABLE_PATH,
            paths::TURNTABLE_DOC,
        ),
        (
            "upper_arm",
            "Upper Arm",
            paths::UPPER_ARM_PATH,
            paths::UPPER_ARM_DOC,
        ),
        (
            "forearm",
            "Forearm",
            paths::FOREARM_PATH,
            paths::FOREARM_DOC,
        ),
        ("wrist", "Wrist", paths::WRIST_PATH, paths::WRIST_DOC),
        ("hand", "Hand", paths::HAND_PATH, paths::HAND_DOC),
        (
            "gripper",
            "Gripper",
            paths::GRIPPER_PATH,
            paths::GRIPPER_DOC,
        ),
    ];

    // Compose placements down the chain.
    let mut placements = std::collections::BTreeMap::from([("base", RigidTransform::identity())]);
    for joint in &joints {
        let parent = placements[joint.parent];
        let turn = RigidTransform {
            translation_m: [0.0; 3],
            rotation: rotation_z(joint.zero_angle_rad),
        };
        let placement = parent
            .compose(joint.parent_frame)
            .compose(turn)
            .compose(joint.child_frame.inverse()?);
        placements.insert(joint.child, placement);
    }

    let mut components = Vec::new();
    let mut instances = Vec::new();
    for (key, name, path, doc) in parts {
        let component = ComponentId::new(format!("component:{key}"))?;
        components.push(Component::new(
            component.clone(),
            path,
            DocumentId::new(doc)?,
        ));
        instances.push(Instance::new(
            InstanceId::new(format!("instance:{key}"))?,
            component,
            Placement::new(placements[key]),
            name,
        ));
    }

    let mut connectors = Vec::new();
    let mut mates = vec![Mate::new(
        MateId::new("mate:ground_base")?,
        MateKind::Ground {
            instance: InstanceId::new("instance:base")?,
        },
    )];
    let mut arm_joints = Vec::new();
    for joint in &joints {
        let parent_connector = format!("{}_{}", joint.parent, joint.name);
        let child_connector = format!("{}_{}", joint.child, joint.name);
        for (instance, connector, frame) in [
            (joint.parent, &parent_connector, joint.parent_frame),
            (joint.child, &child_connector, joint.child_frame),
        ] {
            connectors.push(Connector::new(
                ConnectorId::new(format!("connector:{connector}"))?,
                connector.as_str(),
                InstanceId::new(format!("instance:{instance}"))?,
                frame,
            ));
        }
        let entity = |instance: &str, connector: &str, (feature, role): (&str, &str)| {
            Ok::<_, opencad_core::OpenCadError>(MateEntity::with_connector(
                InstanceId::new(format!("instance:{instance}"))?,
                TopoRef::face(
                    TopoRefId::new(format!("ref:face:{connector}"))?,
                    feature,
                    role,
                ),
                connector,
            ))
        };
        let mate = MateId::new(format!("mate:{}", joint.name))?;
        mates.push(Mate::new(
            mate.clone(),
            MateKind::Concentric {
                a: entity(joint.child, &child_connector, joint.child_feature)?,
                b: entity(joint.parent, &parent_connector, joint.parent_feature)?,
            },
        ));
        arm_joints.push(AssemblyJoint::new(
            JointId::new(format!("joint:{}", joint.name))?,
            mate,
            JointMotion::Revolute {
                lower_rad: joint.limits_deg.0.to_radians(),
                upper_rad: joint.limits_deg.1.to_radians(),
                effort_n_m: joint.effort_n_m,
                velocity_rad_s: joint.velocity_rad_s,
            },
        ));
    }

    Ok(AssemblyModel {
        components,
        instances,
        mates,
        connectors,
        patterns: Vec::new(),
        joints: arm_joints,
    }
    .sorted_deterministic())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_solver::ResidualEquation;

    #[test]
    fn robot_arm_assembly_validates() {
        let model = robot_arm_assembly_model().expect("model");
        assert_eq!(model.components.len(), 4);
        assert_eq!(model.instances.len(), 4);
        assert_eq!(model.mates.len(), 4);
        assert_eq!(model.connectors.len(), 6);
        model
            .validate(&DocumentId::new("doc:robot_arm_assembly_001").expect("doc id"))
            .expect("validate");
    }

    #[test]
    fn robot_arm_assembly_mates_have_zero_initial_residual() {
        let model = robot_arm_assembly_model().expect("model");
        let dof = crate::dof::AssemblyDofModel::build(&model.instances, &model.mates);
        let equations =
            crate::residual::build_mate_residuals(&dof, &model.mates, &model.connectors)
                .expect("residuals");
        assert!(!equations.is_empty());
        let vars = dof.initial_var_set();
        for equation in &equations {
            assert!(
                equation.residual(&vars).abs() < 1e-9,
                "initial residual {}",
                equation.residual(&vars)
            );
        }
    }

    #[test]
    fn robot_arm_assembly_solve_keeps_the_posed_arm() {
        let model = robot_arm_assembly_model().expect("model");
        let before: Vec<_> = model
            .instances
            .iter()
            .map(|instance| instance.placement.transform)
            .collect();
        let (instances, report) = crate::solve::solve_assembly_mates(&model).expect("solve");
        assert!(
            report.max_error < 1e-6,
            "mate solve max error {}",
            report.max_error
        );
        let after: Vec<_> = instances
            .iter()
            .map(|instance| instance.placement.transform)
            .collect();
        for (a, b) in before.iter().zip(after.iter()) {
            for axis in 0..3 {
                assert!(
                    (a.translation_m[axis] - b.translation_m[axis]).abs() < 1e-9,
                    "translation moved on axis {axis}"
                );
                for row in 0..3 {
                    assert!(
                        (a.rotation[row][axis] - b.rotation[row][axis]).abs() < 1e-9,
                        "rotation moved at [{row}][{axis}]"
                    );
                }
            }
        }
    }

    #[test]
    fn six_axis_arm_validates_with_zero_residuals_and_six_limited_joints() {
        let model = six_axis_arm_model().expect("model");
        assert_eq!(model.instances.len(), 7);
        assert_eq!(model.joints.len(), 6);
        assert_eq!(model.connectors.len(), 12);
        model
            .validate(&DocumentId::new("doc:six_axis_arm_001").expect("doc id"))
            .expect("validate");
        let dof = crate::dof::AssemblyDofModel::build(&model.instances, &model.mates);
        let equations =
            crate::residual::build_mate_residuals(&dof, &model.mates, &model.connectors)
                .expect("residuals");
        let vars = dof.initial_var_set();
        for equation in &equations {
            assert!(equation.residual(&vars).abs() < 1e-9);
        }
        let tree = crate::kinematics::kinematic_tree(&model).expect("tree");
        assert!(tree.warnings.is_empty(), "{:?}", tree.warnings);
        let axes: Vec<[f64; 3]> = tree
            .links
            .iter()
            .filter_map(|link| match link.joint.as_ref()?.kind {
                crate::kinematics::JointKind::Revolute { axis, .. } => Some(
                    link.world
                        .rotation
                        .map(|row| row[0] * axis[0] + row[1] * axis[1] + row[2] * axis[2]),
                ),
                _ => None,
            })
            .collect();
        assert_eq!(axes.len(), 6);
        // Base yaw is vertical; shoulder and elbow are horizontal and parallel.
        assert!((axes[0][2].abs() - 1.0).abs() < 1e-9, "{:?}", axes[0]);
        assert!(axes[1][2].abs() < 1e-9 && axes[2][2].abs() < 1e-9);
        let dot = axes[1][0] * axes[2][0] + axes[1][1] * axes[2][1] + axes[1][2] * axes[2][2];
        assert!((dot.abs() - 1.0).abs() < 1e-9);
    }
}
