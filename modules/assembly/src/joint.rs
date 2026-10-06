//! Robot joints: motion type and limits for a mated pair (ADR-030).
//!
//! A joint refines one mate. The mate still positions the parts; the joint
//! says how the child may move about that mate's axis and how far, which a
//! robot description (URDF) needs and a mate alone cannot express.

use std::collections::BTreeSet;

use opencad_core::{JointId, MateId, OpenCadError, Result};
use serde::{Deserialize, Serialize};

use crate::mate::{Mate, MateKind};

/// How the child moves relative to the parent along or about the mate axis.
/// Limits are SI: radians for rotation, metres for translation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JointMotion {
    /// Rotation about the mate axis between `lower_rad` and `upper_rad`.
    Revolute {
        lower_rad: f64,
        upper_rad: f64,
        /// Maximum actuator torque, N·m.
        effort_n_m: f64,
        /// Maximum angular speed, rad/s.
        velocity_rad_s: f64,
    },
    /// Unlimited rotation about the mate axis.
    Continuous,
    /// Translation along the mate axis between `lower_m` and `upper_m`.
    Prismatic {
        lower_m: f64,
        upper_m: f64,
        /// Maximum actuator force, N.
        effort_n: f64,
        /// Maximum linear speed, m/s.
        velocity_m_s: f64,
    },
    /// No relative motion.
    Fixed,
}

/// Joint over one mate of the assembly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssemblyJoint {
    pub id: JointId,
    /// Mate whose instances and axis this joint uses.
    pub mate: MateId,
    #[serde(flatten)]
    pub motion: JointMotion,
}

impl AssemblyJoint {
    pub fn new(id: JointId, mate: MateId, motion: JointMotion) -> Self {
        Self { id, mate, motion }
    }

    pub fn validate(&self, mates: &[Mate]) -> Result<()> {
        let mate = mates
            .iter()
            .find(|mate| mate.id == self.mate)
            .ok_or_else(|| {
                OpenCadError::validation(format!(
                    "joint '{}' references unknown mate '{}'",
                    self.id, self.mate
                ))
            })?;
        let moving = !matches!(self.motion, JointMotion::Fixed);
        if moving && !matches!(mate.kind, MateKind::Concentric { .. }) {
            return Err(OpenCadError::validation(format!(
                "joint '{}' moves about an axis, so mate '{}' must be concentric",
                self.id, self.mate
            )));
        }
        if matches!(mate.kind, MateKind::Ground { .. }) {
            return Err(OpenCadError::validation(format!(
                "joint '{}' cannot use ground mate '{}'",
                self.id, self.mate
            )));
        }
        let check_range = |lower: f64, upper: f64, unit: &str| -> Result<()> {
            if !lower.is_finite() || !upper.is_finite() || lower > upper {
                return Err(OpenCadError::validation(format!(
                    "joint '{}' needs finite limits with lower <= upper (got {lower} {unit} to {upper} {unit})",
                    self.id
                )));
            }
            Ok(())
        };
        let check_positive = |value: f64, name: &str| -> Result<()> {
            if !value.is_finite() || value <= 0.0 {
                return Err(OpenCadError::validation(format!(
                    "joint '{}' needs a positive {name} (got {value})",
                    self.id
                )));
            }
            Ok(())
        };
        match self.motion {
            JointMotion::Revolute {
                lower_rad,
                upper_rad,
                effort_n_m,
                velocity_rad_s,
            } => {
                check_range(lower_rad, upper_rad, "rad")?;
                check_positive(effort_n_m, "effort_n_m")?;
                check_positive(velocity_rad_s, "velocity_rad_s")
            }
            JointMotion::Prismatic {
                lower_m,
                upper_m,
                effort_n,
                velocity_m_s,
            } => {
                check_range(lower_m, upper_m, "m")?;
                check_positive(effort_n, "effort_n")?;
                check_positive(velocity_m_s, "velocity_m_s")
            }
            JointMotion::Continuous | JointMotion::Fixed => Ok(()),
        }
    }
}

/// Validate every joint: unique IDs, known mates, at most one joint per mate.
pub fn validate_joints(joints: &[AssemblyJoint], mates: &[Mate]) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut used_mates = BTreeSet::new();
    for joint in joints {
        if !ids.insert(joint.id.as_str()) {
            return Err(OpenCadError::validation(format!(
                "duplicate assembly joint '{}'",
                joint.id
            )));
        }
        if !used_mates.insert(joint.mate.as_str()) {
            return Err(OpenCadError::validation(format!(
                "mate '{}' already has a joint; '{}' would be a second one",
                joint.mate, joint.id
            )));
        }
        joint.validate(mates)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::robot_arm_assembly_model;

    fn elbow(motion: JointMotion) -> AssemblyJoint {
        AssemblyJoint::new(
            JointId::new("joint:elbow").expect("id"),
            MateId::new("mate:elbow").expect("mate"),
            motion,
        )
    }

    fn revolute(lower_rad: f64, upper_rad: f64) -> JointMotion {
        JointMotion::Revolute {
            lower_rad,
            upper_rad,
            effort_n_m: 2.0,
            velocity_rad_s: 3.0,
        }
    }

    #[test]
    fn joint_round_trips_with_flat_type_tag() {
        let joint = elbow(revolute(-2.0, 2.0));
        let json = serde_json::to_value(&joint).expect("json");
        assert_eq!(json["type"], "revolute");
        assert_eq!(json["mate"], "mate:elbow");
        assert_eq!(json["lower_rad"], -2.0);
        let back: AssemblyJoint = serde_json::from_value(json).expect("parse");
        assert_eq!(back, joint);
    }

    #[test]
    fn validation_accepts_limits_on_a_concentric_mate() {
        let model = robot_arm_assembly_model().expect("model");
        validate_joints(&[elbow(revolute(-2.0, 2.0))], &model.mates).expect("valid");
    }

    #[test]
    fn validation_rejects_bad_limits_mates_and_duplicates() {
        let model = robot_arm_assembly_model().expect("model");
        let mates = &model.mates;
        let error = |joints: &[AssemblyJoint]| {
            validate_joints(joints, mates)
                .expect_err("invalid")
                .to_string()
        };
        assert!(error(&[elbow(revolute(1.0, -1.0))]).contains("lower <= upper"));
        assert!(error(&[elbow(JointMotion::Revolute {
            lower_rad: -1.0,
            upper_rad: 1.0,
            effort_n_m: 0.0,
            velocity_rad_s: 1.0
        })])
        .contains("effort_n_m"));
        let mut ground = elbow(JointMotion::Fixed);
        ground.mate = MateId::new("mate:ground_base").expect("mate");
        assert!(error(&[ground]).contains("ground mate"));
        let mut unknown = elbow(JointMotion::Continuous);
        unknown.mate = MateId::new("mate:missing").expect("mate");
        assert!(error(&[unknown]).contains("unknown mate"));
        let mut second = elbow(JointMotion::Continuous);
        second.id = JointId::new("joint:elbow_again").expect("id");
        assert!(error(&[elbow(JointMotion::Continuous), second]).contains("already has a joint"));
    }
}
