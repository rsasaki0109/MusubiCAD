//! Semantic coupled joint displacements (ADR-056).
use crate::{AssemblyJoint, AssemblyModel, JointMotion};
use opencad_core::{CouplingId, JointId, OpenCadError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Coordinates are displacements from authored zero, in radians/metres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum JointCoupling {
    Gear {
        id: CouplingId,
        driver_joint: JointId,
        follower_joint: JointId,
        ratio: f64,
    },
    Rack {
        id: CouplingId,
        driver_joint: JointId,
        follower_joint: JointId,
        metres_per_radian: f64,
    },
}
impl JointCoupling {
    pub fn id(&self) -> &CouplingId {
        match self {
            Self::Gear { id, .. } | Self::Rack { id, .. } => id,
        }
    }
    pub fn driver(&self) -> &JointId {
        match self {
            Self::Gear { driver_joint, .. } | Self::Rack { driver_joint, .. } => driver_joint,
        }
    }
    pub fn follower(&self) -> &JointId {
        match self {
            Self::Gear { follower_joint, .. } | Self::Rack { follower_joint, .. } => follower_joint,
        }
    }
    pub fn multiplier(&self) -> f64 {
        match self {
            Self::Gear { ratio, .. } => *ratio,
            Self::Rack {
                metres_per_radian, ..
            } => *metres_per_radian,
        }
    }
    pub fn follower_is_linear(&self) -> bool {
        matches!(self, Self::Rack { .. })
    }
}
fn angular(motion: &JointMotion) -> bool {
    matches!(
        motion,
        JointMotion::Revolute { .. } | JointMotion::Continuous
    )
}
fn range(motion: &JointMotion) -> Option<(f64, f64)> {
    match *motion {
        JointMotion::Revolute {
            lower_rad,
            upper_rad,
            ..
        } => Some((lower_rad, upper_rad)),
        JointMotion::Prismatic {
            lower_m, upper_m, ..
        } => Some((lower_m, upper_m)),
        _ => None,
    }
}
fn unique_joint<'a>(model: &'a AssemblyModel, id: &JointId) -> Result<&'a AssemblyJoint> {
    let mut found = model.joints.iter().filter(|j| j.id == *id);
    let joint = found.next().ok_or_else(|| {
        OpenCadError::validation(format!("coupling references missing joint '{id}'"))
    })?;
    if found.next().is_some() {
        return Err(OpenCadError::validation(format!(
            "coupling references ambiguous joint '{id}'"
        )));
    }
    joint.validate(&model.mates)?;
    if model
        .mates
        .iter()
        .filter(|m| m.id == joint.mate && !m.suppressed)
        .count()
        != 1
    {
        return Err(OpenCadError::validation(format!(
            "coupled joint '{id}' needs one active mate"
        )));
    }
    Ok(joint)
}
/// Validate source semantics and return deterministic dependency order.
pub fn ordered_couplings(model: &AssemblyModel) -> Result<Vec<&JointCoupling>> {
    if model.couplings.len() > 256 {
        return Err(OpenCadError::validation(
            "at most 256 joint couplings are admitted",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut followers = BTreeSet::new();
    for coupling in &model.couplings {
        CouplingId::new(coupling.id().as_str())?;
        JointId::new(coupling.driver().as_str())?;
        JointId::new(coupling.follower().as_str())?;
        if !ids.insert(coupling.id().as_str()) || !followers.insert(coupling.follower().as_str()) {
            return Err(OpenCadError::validation(
                "coupling IDs and follower ownership must be unique",
            ));
        }
        let driver = unique_joint(model, coupling.driver())?;
        let follower = unique_joint(model, coupling.follower())?;
        let valid = angular(&driver.motion)
            && if coupling.follower_is_linear() {
                matches!(follower.motion, JointMotion::Prismatic { .. })
            } else {
                angular(&follower.motion)
            };
        let multiplier = coupling.multiplier();
        // Positive lower magnitudes are labelled in the accepted ADR: ratio or m/rad.
        let minimum = if coupling.follower_is_linear() {
            1e-9
        } else {
            1e-12
        };
        if !valid
            || !multiplier.is_finite()
            || multiplier.abs() <= minimum
            || multiplier.abs() > 1e6
        {
            return Err(OpenCadError::validation(
                "coupling types or finite bounded nonzero coefficient are invalid",
            ));
        }
        for joint in [driver, follower] {
            if let Some((lower, upper)) = range(&joint.motion) {
                if lower > 0. || upper < 0. {
                    return Err(OpenCadError::validation(
                        "coupled finite ranges must admit authored zero",
                    ));
                }
            }
        }
        match (range(&driver.motion), range(&follower.motion)) {
            (None, Some(_)) => {
                return Err(OpenCadError::validation(
                    "unlimited driver cannot feed limited follower",
                ))
            }
            (Some((a, b)), Some((lower, upper))) => {
                let (x, y) = (a * multiplier, b * multiplier);
                // Explicit displacement tolerances: 1e-9 radians or metres.
                if !x.is_finite()
                    || !y.is_finite()
                    || x.min(y) < lower - 1e-9
                    || x.max(y) > upper + 1e-9
                {
                    return Err(OpenCadError::validation(
                        "driver range does not map inside follower range",
                    ));
                }
            }
            _ => {}
        }
    }
    let mut pending: BTreeMap<&str, &JointCoupling> = model
        .couplings
        .iter()
        .map(|c| (c.id().as_str(), c))
        .collect();
    let mut completed = BTreeSet::new();
    let mut ordered = Vec::new();
    while !pending.is_empty() {
        let ready: Vec<_> = pending
            .iter()
            .filter(|(_, c)| {
                !followers.contains(c.driver().as_str()) || completed.contains(c.driver().as_str())
            })
            .map(|(id, _)| *id)
            .collect();
        if ready.is_empty() {
            return Err(OpenCadError::validation("joint coupling dependency cycle"));
        }
        for id in ready {
            let coupling = pending
                .remove(id)
                .ok_or_else(|| OpenCadError::validation("coupling ordering invariant"))?;
            completed.insert(coupling.follower().as_str());
            ordered.push(coupling);
        }
    }
    Ok(ordered)
}

/// Derived stable association between moving instance coordinates. Not source data.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCoupling {
    pub id: CouplingId,
    pub driver_instance: String,
    pub follower_instance: String,
    pub multiplier: f64,
    pub follower_is_linear: bool,
}
/// Resolve followers in validated dependency order; explicit conflicts refuse.
pub fn coupled_positions(
    couplings: &[ResolvedCoupling],
    input: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>> {
    let mut positions = input.clone();
    if input.values().any(|v| !v.is_finite()) {
        return Err(OpenCadError::validation("joint positions must be finite"));
    }
    for coupling in couplings {
        let value = positions
            .get(&coupling.driver_instance)
            .copied()
            .unwrap_or(0.)
            * coupling.multiplier;
        if !value.is_finite() {
            return Err(OpenCadError::validation(
                "derived coupled position is non-finite",
            ));
        }
        if input
            .get(&coupling.follower_instance)
            .is_some_and(|explicit| (explicit - value).abs() > 1e-9)
        {
            return Err(OpenCadError::validation(format!(
                "explicit follower '{}' conflicts with coupling '{}'",
                coupling.follower_instance, coupling.id
            )));
        }
        positions.insert(coupling.follower_instance.clone(), value);
    }
    Ok(positions)
}
