//! Ready-made flagship assembly models.

use opencad_core::{ComponentId, ConnectorId, DocumentId, InstanceId, MateId, Result, TopoRefId};
use opencad_geometry::{RigidTransform, TopoRef};

use crate::component::Component;
use crate::connector::Connector;
use crate::instance::{Instance, Placement};
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

    Ok(AssemblyModel {
        components,
        instances,
        mates,
        connectors,
        patterns: Vec::new(),
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
}
