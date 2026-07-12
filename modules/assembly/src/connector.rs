//! Named coordinate frames for reusable mate anchors (M3.3).

use opencad_core::{ConnectorId, InstanceId, OpenCadError, Result};
use opencad_geometry::RigidTransform;
use serde::{Deserialize, Serialize};

use crate::mate::MateEntity;

/// Named connector frame on an instance (mate anchor).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    pub id: ConnectorId,
    pub name: String,
    pub instance: InstanceId,
    pub transform: RigidTransform,
}

impl Connector {
    pub fn new(
        id: ConnectorId,
        name: impl Into<String>,
        instance: InstanceId,
        transform: RigidTransform,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            instance,
            transform,
        }
    }

    pub fn local_origin_m(&self) -> [f64; 3] {
        self.transform.translation_m
    }

    pub fn local_axis_m(&self) -> [f64; 3] {
        self.transform.rotation[2]
    }
}

/// Resolve a mate entity's local frame, preferring a named connector when set.
pub fn resolve_mate_entity_frame(
    entity: &MateEntity,
    connectors: &[Connector],
) -> Result<([f64; 3], [f64; 3])> {
    if let Some(name) = entity.connector.as_deref() {
        let connector = find_connector(connectors, &entity.instance, name).ok_or_else(|| {
            OpenCadError::validation(format!(
                "unknown connector '{name}' on instance '{}'",
                entity.instance
            ))
        })?;
        return Ok((connector.local_origin_m(), connector.local_axis_m()));
    }
    Ok((entity.local_origin_m, entity.local_axis_m))
}

pub fn find_connector<'a>(
    connectors: &'a [Connector],
    instance: &InstanceId,
    name: &str,
) -> Option<&'a Connector> {
    connectors
        .iter()
        .find(|connector| connector.instance == *instance && connector.name == name)
}

pub fn validate_connectors(connectors: &[Connector], instances: &[InstanceId]) -> Result<()> {
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut seen_names = std::collections::BTreeSet::new();

    for connector in connectors {
        if !seen_ids.insert(connector.id.as_str().to_string()) {
            return Err(OpenCadError::validation(format!(
                "duplicate connector id '{}'",
                connector.id
            )));
        }

        let name_key = format!("{}:{}", connector.instance, connector.name);
        if !seen_names.insert(name_key.clone()) {
            return Err(OpenCadError::validation(format!(
                "duplicate connector name '{name_key}'"
            )));
        }

        if connector.name.trim().is_empty() {
            return Err(OpenCadError::validation("connector name must not be empty"));
        }

        if !instances.iter().any(|id| id == &connector.instance) {
            return Err(OpenCadError::validation(format!(
                "connector '{}' references unknown instance '{}'",
                connector.id, connector.instance
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{ConnectorId, InstanceId, Result, TopoRefId};
    use opencad_geometry::TopoRef;

    #[test]
    fn connector_resolves_mate_entity_frame() -> Result<()> {
        let instance = InstanceId::new("instance:left")?;
        let connector = Connector::new(
            ConnectorId::new("connector:left_mount")?,
            "mount",
            instance.clone(),
            RigidTransform::from_translation([0.01, 0.02, 0.0]),
        );
        let entity = MateEntity {
            instance,
            topo_ref: TopoRef::face(
                TopoRefId::new("ref:face:left")?,
                "feature:extrude_base",
                "top",
            ),
            local_origin_m: [9.0, 9.0, 9.0],
            local_axis_m: [0.0, 1.0, 0.0],
            connector: Some("mount".into()),
        };
        let (origin, axis) = resolve_mate_entity_frame(&entity, &[connector]).expect("resolve");
        assert!((origin[0] - 0.01).abs() < 1e-9);
        assert!((origin[1] - 0.02).abs() < 1e-9);
        assert!((axis[2] - 1.0).abs() < 1e-9);
        Ok(())
    }
}
