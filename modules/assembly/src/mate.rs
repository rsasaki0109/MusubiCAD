//! Assembly mate constraints (M3.2).

use opencad_core::{InstanceId, MateId, OpenCadError, Result};
use opencad_geometry::TopoRef;
use serde::{Deserialize, Serialize};

use crate::connector::{find_connector, Connector};

/// Topological attachment on an instance used by mates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MateEntity {
    pub instance: InstanceId,
    pub topo_ref: TopoRef,
    /// Attachment origin in the instance/part local frame (meters).
    #[serde(default)]
    pub local_origin_m: [f64; 3],
    /// Primary axis direction in the instance/part local frame.
    #[serde(default = "default_local_axis")]
    pub local_axis_m: [f64; 3],
    /// When set, overrides `local_origin_m` / `local_axis_m` from a named connector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector: Option<String>,
}

fn default_local_axis() -> [f64; 3] {
    [0.0, 0.0, 1.0]
}

impl MateEntity {
    pub fn at_origin(instance: InstanceId, topo_ref: TopoRef) -> Self {
        Self {
            instance,
            topo_ref,
            local_origin_m: [0.0, 0.0, 0.0],
            local_axis_m: default_local_axis(),
            connector: None,
        }
    }

    pub fn with_connector(
        instance: InstanceId,
        topo_ref: TopoRef,
        connector: impl Into<String>,
    ) -> Self {
        Self {
            instance,
            topo_ref,
            local_origin_m: [0.0, 0.0, 0.0],
            local_axis_m: default_local_axis(),
            connector: Some(connector.into()),
        }
    }

    pub fn with_local_frame(
        instance: InstanceId,
        topo_ref: TopoRef,
        local_origin_m: [f64; 3],
        local_axis_m: [f64; 3],
    ) -> Self {
        Self {
            instance,
            topo_ref,
            local_origin_m,
            local_axis_m,
            connector: None,
        }
    }
}

/// Mate constraint between assembly instances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mate {
    pub id: MateId,
    #[serde(flatten)]
    pub kind: MateKind,
    #[serde(default)]
    pub suppressed: bool,
}

/// Supported mate kinds for the assembly solver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MateKind {
    Coincident {
        a: MateEntity,
        b: MateEntity,
    },
    Concentric {
        a: MateEntity,
        b: MateEntity,
    },
    Distance {
        a: MateEntity,
        b: MateEntity,
        distance_m: f64,
    },
    Angle {
        a: MateEntity,
        b: MateEntity,
        angle_rad: f64,
    },
    Parallel {
        a: MateEntity,
        b: MateEntity,
    },
    Ground {
        instance: InstanceId,
    },
}

impl Mate {
    pub fn new(id: MateId, kind: MateKind) -> Self {
        Self {
            id,
            kind,
            suppressed: false,
        }
    }

    pub fn validate(&self, instances: &[InstanceId], connectors: &[Connector]) -> Result<()> {
        if self.suppressed {
            return Ok(());
        }
        match &self.kind {
            MateKind::Ground { instance } => validate_instance_ref(instance, instances),
            MateKind::Coincident { a, b }
            | MateKind::Concentric { a, b }
            | MateKind::Parallel { a, b } => {
                validate_entity(a, instances, connectors)?;
                validate_entity(b, instances, connectors)?;
                if a.instance == b.instance {
                    return Err(OpenCadError::validation(format!(
                        "mate '{}' must reference two different instances",
                        self.id
                    )));
                }
                Ok(())
            }
            MateKind::Distance { a, b, distance_m } => {
                validate_entity(a, instances, connectors)?;
                validate_entity(b, instances, connectors)?;
                if a.instance == b.instance {
                    return Err(OpenCadError::validation(format!(
                        "mate '{}' must reference two different instances",
                        self.id
                    )));
                }
                if *distance_m < 0.0 {
                    return Err(OpenCadError::validation(format!(
                        "mate '{}' distance must be non-negative",
                        self.id
                    )));
                }
                Ok(())
            }
            MateKind::Angle { a, b, .. } => {
                validate_entity(a, instances, connectors)?;
                validate_entity(b, instances, connectors)?;
                if a.instance == b.instance {
                    return Err(OpenCadError::validation(format!(
                        "mate '{}' must reference two different instances",
                        self.id
                    )));
                }
                Ok(())
            }
        }
    }
}

fn validate_entity(
    entity: &MateEntity,
    instances: &[InstanceId],
    connectors: &[Connector],
) -> Result<()> {
    validate_instance_ref(&entity.instance, instances)?;
    if let Some(name) = entity.connector.as_deref() {
        if find_connector(connectors, &entity.instance, name).is_none() {
            return Err(OpenCadError::validation(format!(
                "mate references unknown connector '{name}' on instance '{}'",
                entity.instance
            )));
        }
    }
    Ok(())
}

fn validate_instance_ref(instance: &InstanceId, instances: &[InstanceId]) -> Result<()> {
    if instances.iter().any(|id| id == instance) {
        Ok(())
    } else {
        Err(OpenCadError::validation(format!(
            "mate references unknown instance '{instance}'"
        )))
    }
}

/// Validate all mates against the assembly instance list.
pub fn validate_mates(
    mates: &[Mate],
    instances: &[InstanceId],
    connectors: &[Connector],
) -> Result<()> {
    let ids: Vec<_> = instances.to_vec();
    for mate in mates {
        mate.validate(&ids, connectors)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::TopoRefId;

    fn sample_entity(name: &str) -> Result<MateEntity> {
        Ok(MateEntity::at_origin(
            InstanceId::new(format!("instance:{name}"))?,
            TopoRef::face(
                TopoRefId::new(format!("ref:face:{name}"))?,
                "feature:extrude_base",
                "top",
            ),
        ))
    }

    #[test]
    fn mate_round_trip() -> Result<()> {
        let mate = Mate::new(
            MateId::new("mate:ground_left")?,
            MateKind::Ground {
                instance: InstanceId::new("instance:left")?,
            },
        );
        let json = serde_json::to_string(&mate).expect("serialize");
        let restored: Mate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mate, restored);
        Ok(())
    }

    #[test]
    fn distance_mate_validates_instances() -> Result<()> {
        let mate = Mate::new(
            MateId::new("mate:spacing")?,
            MateKind::Distance {
                a: sample_entity("left")?,
                b: sample_entity("right")?,
                distance_m: 0.12,
            },
        );
        let instances = vec![
            InstanceId::new("instance:left")?,
            InstanceId::new("instance:right")?,
        ];
        assert!(mate.validate(&instances, &[]).is_ok());
        assert!(mate
            .validate(&[InstanceId::new("instance:left")?], &[])
            .is_err());
        Ok(())
    }
}
