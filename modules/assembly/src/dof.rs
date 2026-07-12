//! Instance pose variables (6 DOF) for the assembly solver.

use std::collections::{BTreeMap, HashSet};

use opencad_core::InstanceId;
use opencad_geometry::RigidTransform;
use opencad_solver::{VarId, VariableRegistry};

use crate::instance::Instance;
use crate::mate::{Mate, MateKind};

/// Solver variables for one movable instance pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstancePoseVars {
    pub tx: VarId,
    pub ty: VarId,
    pub tz: VarId,
    pub rx: VarId,
    pub ry: VarId,
    pub rz: VarId,
}

impl InstancePoseVars {
    pub fn all(&self) -> [VarId; 6] {
        [self.tx, self.ty, self.tz, self.rx, self.ry, self.rz]
    }
}

/// Assembly DOF model: grounded instances and pose variables for movers.
#[derive(Debug)]
pub struct AssemblyDofModel {
    pub registry: VariableRegistry,
    pub movable: BTreeMap<String, InstancePoseVars>,
    pub grounded: HashSet<String>,
    pub initial_transforms: BTreeMap<String, RigidTransform>,
}

impl AssemblyDofModel {
    pub fn build(instances: &[Instance], mates: &[Mate]) -> Self {
        let mut grounded = HashSet::new();
        for instance in instances {
            if instance.fixed {
                grounded.insert(instance.id.as_str().to_string());
            }
        }
        for mate in mates {
            if mate.suppressed {
                continue;
            }
            if let MateKind::Ground { instance } = &mate.kind {
                grounded.insert(instance.as_str().to_string());
            }
        }

        let mut registry = VariableRegistry::new();
        let mut movable = BTreeMap::new();
        let mut initial_transforms = BTreeMap::new();

        for instance in instances {
            let key = instance.id.as_str().to_string();
            initial_transforms.insert(key.clone(), instance.placement.transform);
            if grounded.contains(&key) {
                continue;
            }
            let prefix = key.clone();
            let vars = InstancePoseVars {
                tx: registry.register(format!("{prefix}.tx")),
                ty: registry.register(format!("{prefix}.ty")),
                tz: registry.register(format!("{prefix}.tz")),
                rx: registry.register(format!("{prefix}.rx")),
                ry: registry.register(format!("{prefix}.ry")),
                rz: registry.register(format!("{prefix}.rz")),
            };
            movable.insert(key, vars);
        }

        Self {
            registry,
            movable,
            grounded,
            initial_transforms,
        }
    }

    pub fn initial_var_set(&self) -> opencad_solver::VarSet {
        let mut values = self.registry.initial_values();
        for (instance_id, vars) in &self.movable {
            let transform = self
                .initial_transforms
                .get(instance_id)
                .copied()
                .unwrap_or_else(RigidTransform::identity);
            let rot_vec = crate::pose::rotation_vector_from_matrix(transform.rotation);
            values[vars.tx.index()] = transform.translation_m[0];
            values[vars.ty.index()] = transform.translation_m[1];
            values[vars.tz.index()] = transform.translation_m[2];
            values[vars.rx.index()] = rot_vec[0];
            values[vars.ry.index()] = rot_vec[1];
            values[vars.rz.index()] = rot_vec[2];
        }
        opencad_solver::VarSet::new(values)
    }

    pub fn total_instance_dof(&self) -> usize {
        self.movable.len() * 6
    }

    pub fn is_grounded(&self, instance: &InstanceId) -> bool {
        self.grounded.contains(instance.as_str())
    }

    pub fn pose_vars(&self, instance: &InstanceId) -> Option<InstancePoseVars> {
        self.movable.get(instance.as_str()).copied()
    }

    pub fn transform_for_instance(
        &self,
        instance: &InstanceId,
        vars: &opencad_solver::VarSet,
    ) -> RigidTransform {
        if let Some(pose) = self.pose_vars(instance) {
            let translation_m = [vars.get(pose.tx), vars.get(pose.ty), vars.get(pose.tz)];
            let rotation = crate::pose::rotation_matrix_from_vector([
                vars.get(pose.rx),
                vars.get(pose.ry),
                vars.get(pose.rz),
            ]);
            RigidTransform {
                translation_m,
                rotation,
            }
        } else {
            self.initial_transforms
                .get(instance.as_str())
                .copied()
                .unwrap_or_else(RigidTransform::identity)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::{Instance, Placement};
    use opencad_core::{ComponentId, Result};

    #[test]
    fn grounded_instance_has_no_pose_vars() -> Result<()> {
        let left = Instance {
            id: InstanceId::new("instance:left")?,
            component: ComponentId::new("component:bracket")?,
            placement: Placement::identity(),
            fixed: true,
            name: "Left".into(),
        };
        let right = Instance::new(
            InstanceId::new("instance:right")?,
            ComponentId::new("component:bracket")?,
            Placement::new(RigidTransform::from_translation([0.1, 0.0, 0.0])),
            "Right",
        );
        let model = AssemblyDofModel::build(&[left, right], &[]);
        assert!(model.is_grounded(&InstanceId::new("instance:left")?));
        assert_eq!(model.movable.len(), 1);
        assert_eq!(model.total_instance_dof(), 6);
        Ok(())
    }
}
