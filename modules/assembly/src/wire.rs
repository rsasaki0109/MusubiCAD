//! ADR-056 forward-reader refusal: old JointMotion enums must reject followers.
use crate::{
    AssemblyJoint, AssemblyModel, AssemblyPattern, Component, Connector, ExplodedView, Instance,
    JointCoupling, JointMotion, Mate,
};
use opencad_core::{CouplingId, JointId, MateId, OpenCadError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum JointWire {
    Guard(CoupledJointWire),
    Legacy(AssemblyJoint),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoupledJointWire {
    pub id: JointId,
    pub mate: MateId,
    #[serde(rename = "type")]
    pub tag: CoupledTag,
    pub coupling_id: CouplingId,
    pub motion: JointMotion,
}
#[derive(Serialize, Deserialize)]
pub enum CoupledTag {
    #[serde(rename = "coupled")]
    Coupled,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct AssemblyWire {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    components: Vec<Component>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    instances: Vec<Instance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    mates: Vec<Mate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    connectors: Vec<Connector>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    patterns: Vec<AssemblyPattern>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    joints: Vec<JointWire>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exploded_views: Vec<ExplodedView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    couplings: Vec<JointCoupling>,
}
impl From<AssemblyModel> for AssemblyWire {
    fn from(model: AssemblyModel) -> Self {
        let joints = model
            .joints
            .into_iter()
            .map(|joint| {
                if let Some(coupling) = model.couplings.iter().find(|c| c.follower() == &joint.id) {
                    JointWire::Guard(CoupledJointWire {
                        id: joint.id,
                        mate: joint.mate,
                        tag: CoupledTag::Coupled,
                        coupling_id: coupling.id().clone(),
                        motion: joint.motion,
                    })
                } else {
                    JointWire::Legacy(joint)
                }
            })
            .collect();
        Self {
            components: model.components,
            instances: model.instances,
            mates: model.mates,
            connectors: model.connectors,
            patterns: model.patterns,
            joints,
            exploded_views: model.exploded_views,
            couplings: model.couplings,
        }
    }
}
impl TryFrom<AssemblyWire> for AssemblyModel {
    type Error = OpenCadError;
    fn try_from(wire: AssemblyWire) -> Result<Self, Self::Error> {
        let mut joints = Vec::new();
        let mut guards = std::collections::BTreeSet::new();
        for item in wire.joints {
            match item {
                JointWire::Guard(guard) => {
                    let matching: Vec<_> = wire
                        .couplings
                        .iter()
                        .filter(|c| c.follower() == &guard.id && c.id() == &guard.coupling_id)
                        .collect();
                    if matching.len() != 1 || !guards.insert(guard.id.as_str().to_owned()) {
                        return Err(OpenCadError::validation(
                            "orphan, duplicate or mismatched coupled joint wire guard",
                        ));
                    }
                    joints.push(AssemblyJoint::new(guard.id, guard.mate, guard.motion));
                }
                JointWire::Legacy(joint) => {
                    if wire.couplings.iter().any(|c| c.follower() == &joint.id) {
                        return Err(OpenCadError::validation(
                            "coupled follower lacks mandatory wire guard",
                        ));
                    }
                    joints.push(joint);
                }
            }
        }
        if wire
            .couplings
            .iter()
            .any(|c| !guards.contains(c.follower().as_str()))
        {
            return Err(OpenCadError::validation(
                "coupling references unguarded or missing follower",
            ));
        }
        let model = Self {
            components: wire.components,
            instances: wire.instances,
            mates: wire.mates,
            connectors: wire.connectors,
            patterns: wire.patterns,
            joints,
            exploded_views: wire.exploded_views,
            couplings: wire.couplings,
        };
        if !model.couplings.is_empty() {
            crate::ordered_couplings(&model)?;
        }
        Ok(model)
    }
}
