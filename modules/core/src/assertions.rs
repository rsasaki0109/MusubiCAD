//! Executable design assertions embedded in the Design Graph (MCAD-P6-004).
//!
//! Assertions are serializable, unit-explicit design intent that runs during
//! patch dry-run and regeneration. A `Required` assertion that fails blocks
//! the change; an `Advisory` assertion is reported but never blocks.

use serde::{Deserialize, Serialize};

/// Fewest joint positions a motion assertion may sample: both range ends.
pub const MIN_MOTION_SAMPLES_PER_JOINT: u32 = 2;
/// Most joint positions a motion assertion may sample; bounds the number of
/// exact Booleans one evaluation runs.
pub const MAX_MOTION_SAMPLES_PER_JOINT: u32 = 360;

/// Severity of a design assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertionSeverity {
    /// A failed assertion rejects the change.
    Required,
    /// A failed assertion is reported but does not block the change.
    Advisory,
}

/// Typed, unit-explicit assertion rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssertionKind {
    /// Parameter value (by name) inside a closed range in meters.
    ParameterRange {
        parameter_name: String,
        min_m: f64,
        max_m: f64,
    },
    /// Regenerated body mass inside a closed range in kilograms.
    MassRange { min_kg: f64, max_kg: f64 },
    /// Regenerated bounding-box size inside a closed limit in meters.
    BoundingBoxWithin { max_m: [f64; 3] },
    /// Exact expected solid body count.
    BodyCount { expected: u32 },
    /// A semantic reference must resolve to a current face/edge (not ambiguous
    /// or missing); consumed from `ReferenceProvenance` (MCAD-P6-003).
    RequiredReference { ref_id: String },
    /// Solved assembly DOF must not exceed the limit.
    AssemblyDofAtMost { max_dof: i32 },
    /// Assembly interference count must not exceed the limit.
    InterferenceAtMost { max_count: u32 },
    /// Interfering instance pairs must not exceed the limit at any sampled
    /// joint pose (MCAD-P10-002).  Each movable joint sweeps its range in
    /// `samples_per_joint` evenly spaced positions (a full turn for a
    /// continuous joint) while the other joints stay at zero.
    MotionInterferenceAtMost {
        max_count: u32,
        samples_per_joint: u32,
    },
}

impl AssertionKind {
    /// Reject non-finite or inverted numeric bounds before evaluation.
    pub fn validate(&self) -> Result<(), crate::OpenCadError> {
        match self {
            Self::ParameterRange { min_m, max_m, .. } => {
                if !min_m.is_finite() || !max_m.is_finite() || min_m > max_m {
                    return Err(crate::OpenCadError::validation(format!(
                        "assertion range must be finite with min <= max, got [{min_m}, {max_m}]"
                    )));
                }
            }
            Self::MassRange { min_kg, max_kg } => {
                if !min_kg.is_finite() || !max_kg.is_finite() || min_kg > max_kg {
                    return Err(crate::OpenCadError::validation(format!(
                        "assertion range must be finite with min <= max, got [{min_kg}, {max_kg}]"
                    )));
                }
            }
            Self::BoundingBoxWithin { max_m } => {
                if max_m
                    .iter()
                    .any(|value| !value.is_finite() || *value <= 0.0)
                {
                    return Err(crate::OpenCadError::validation(
                        "bounding-box assertion limit must be finite and strictly positive",
                    ));
                }
            }
            Self::BodyCount { .. } | Self::RequiredReference { .. } => {}
            Self::AssemblyDofAtMost { max_dof } => {
                if *max_dof < 0 {
                    return Err(crate::OpenCadError::validation(
                        "assembly DOF assertion must be non-negative",
                    ));
                }
            }
            Self::InterferenceAtMost { max_count } => {
                if *max_count > i32::MAX as u32 {
                    return Err(crate::OpenCadError::validation(
                        "interference limit is unreasonably large",
                    ));
                }
            }
            Self::MotionInterferenceAtMost {
                max_count,
                samples_per_joint,
            } => {
                if *max_count > i32::MAX as u32 {
                    return Err(crate::OpenCadError::validation(
                        "interference limit is unreasonably large",
                    ));
                }
                if !(MIN_MOTION_SAMPLES_PER_JOINT..=MAX_MOTION_SAMPLES_PER_JOINT)
                    .contains(samples_per_joint)
                {
                    return Err(crate::OpenCadError::validation(format!(
                        "motion interference samples per joint must be between \
                         {MIN_MOTION_SAMPLES_PER_JOINT} and {MAX_MOTION_SAMPLES_PER_JOINT}, \
                         got {samples_per_joint}"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// One design assertion: stable id, human name, severity, and a typed rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assertion {
    pub id: String,
    pub name: String,
    pub severity: AssertionSeverity,
    #[serde(flatten)]
    pub kind: AssertionKind,
}

impl Assertion {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        severity: AssertionSeverity,
        kind: AssertionKind,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            severity,
            kind,
        }
    }

    pub fn validate(&self) -> Result<(), crate::OpenCadError> {
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err(crate::OpenCadError::validation(
                "assertion id and name must not be empty",
            ));
        }
        self.kind.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assertion_round_trip() {
        let assertion = Assertion::new(
            "assertion:upper_hub_height",
            "Upper hub height range",
            AssertionSeverity::Required,
            AssertionKind::ParameterRange {
                parameter_name: "upper_hub_height".into(),
                min_m: 0.020,
                max_m: 0.050,
            },
        );
        let json = serde_json::to_string(&assertion).expect("serialize");
        let restored: Assertion = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(assertion, restored);
        assert!(restored.validate().is_ok());
    }

    #[test]
    fn inverted_range_is_rejected() {
        let assertion = Assertion::new(
            "assertion:mass",
            "Mass range",
            AssertionSeverity::Required,
            AssertionKind::MassRange {
                min_kg: 2.0,
                max_kg: 1.0,
            },
        );
        assert!(assertion.validate().is_err());
    }

    #[test]
    fn motion_interference_round_trips_and_bounds_its_samples() {
        let assertion = Assertion::new(
            "assertion:motion_clearance",
            "Clear through every joint range",
            AssertionSeverity::Required,
            AssertionKind::MotionInterferenceAtMost {
                max_count: 0,
                samples_per_joint: 13,
            },
        );
        let json = serde_json::to_value(&assertion).expect("serialize");
        assert_eq!(json["type"], "motion_interference_at_most");
        assert_eq!(json["samples_per_joint"], 13);
        let restored: Assertion = serde_json::from_value(json).expect("deserialize");
        assert_eq!(assertion, restored);
        assert!(restored.validate().is_ok());

        for samples_per_joint in [0, 1, MAX_MOTION_SAMPLES_PER_JOINT + 1] {
            let invalid = Assertion::new(
                "assertion:motion_clearance",
                "Clear through every joint range",
                AssertionSeverity::Required,
                AssertionKind::MotionInterferenceAtMost {
                    max_count: 0,
                    samples_per_joint,
                },
            );
            assert!(invalid.validate().is_err(), "{samples_per_joint} samples");
        }
    }

    #[test]
    fn severity_and_kind_are_explicit() {
        let assertion = Assertion::new(
            "assertion:no_interference",
            "Zero interference",
            AssertionSeverity::Required,
            AssertionKind::InterferenceAtMost { max_count: 0 },
        );
        assert_eq!(assertion.severity, AssertionSeverity::Required);
        assert!(matches!(
            assertion.kind,
            AssertionKind::InterferenceAtMost { max_count: 0 }
        ));
    }
}
