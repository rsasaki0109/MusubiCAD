//! Placed component instance.

use opencad_core::{ComponentId, InstanceId};
use opencad_geometry::RigidTransform;
use serde::{Deserialize, Serialize};

/// Rigid placement of a component instance in world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub transform: RigidTransform,
}

impl Placement {
    pub fn new(transform: RigidTransform) -> Self {
        Self { transform }
    }

    pub fn identity() -> Self {
        Self {
            transform: RigidTransform::identity(),
        }
    }
}

/// A placed copy of a child part component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub id: InstanceId,
    pub component: ComponentId,
    pub placement: Placement,
    pub fixed: bool,
    pub name: String,
}

impl Instance {
    pub fn new(
        id: InstanceId,
        component: ComponentId,
        placement: Placement,
        name: impl Into<String>,
    ) -> Self {
        Self {
            id,
            component,
            placement,
            fixed: false,
            name: name.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::Result;

    #[test]
    fn instance_round_trip() -> Result<()> {
        let instance = Instance::new(
            InstanceId::new("instance:left")?,
            ComponentId::new("component:bracket")?,
            Placement::identity(),
            "Left Bracket",
        );
        let json = serde_json::to_string(&instance).expect("serialize");
        let restored: Instance = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(instance, restored);
        Ok(())
    }
}
