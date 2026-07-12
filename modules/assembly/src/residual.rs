//! Mate residual equations for `gauss_newton_solve`.

use opencad_solver::{ResidualEquation, VarId, VarSet};

use crate::connector::{resolve_mate_entity_frame, Connector};
use crate::dof::AssemblyDofModel;
use crate::mate::{Mate, MateEntity, MateKind};
use crate::pose::{self, normalize, transform_direction, transform_point};

/// Scalar residual used by the assembly solver.
#[derive(Debug, Clone)]
pub enum MateResidual {
    PointDelta {
        vars_a: [VarId; 6],
        local_a: [f64; 3],
        vars_b: [VarId; 6],
        local_b: [f64; 3],
        axis: usize,
    },
    Translation {
        vars: [VarId; 6],
        local: [f64; 3],
        other: [f64; 3],
        axis: usize,
    },
    Distance {
        vars_a: [VarId; 6],
        local_a: [f64; 3],
        vars_b: [VarId; 6],
        local_b: [f64; 3],
        target_m: f64,
    },
    DistanceToFixed {
        vars: [VarId; 6],
        local: [f64; 3],
        fixed_world: [f64; 3],
        target_m: f64,
    },
    AxisAlignment {
        vars_a: [VarId; 6],
        local_axis_a: [f64; 3],
        vars_b: [VarId; 6],
        local_axis_b: [f64; 3],
    },
    AxisAngle {
        vars_a: [VarId; 6],
        local_axis_a: [f64; 3],
        vars_b: [VarId; 6],
        local_axis_b: [f64; 3],
        target_cos: f64,
    },
}

impl MateResidual {
    fn pose_from_vars(vars: [VarId; 6], values: &VarSet) -> opencad_geometry::RigidTransform {
        let translation_m = [
            values.get(vars[0]),
            values.get(vars[1]),
            values.get(vars[2]),
        ];
        let rotation = pose::rotation_matrix_from_vector([
            values.get(vars[3]),
            values.get(vars[4]),
            values.get(vars[5]),
        ]);
        opencad_geometry::RigidTransform {
            translation_m,
            rotation,
        }
    }

    fn world_origin(vars: [VarId; 6], local: [f64; 3], values: &VarSet) -> [f64; 3] {
        let pose = Self::pose_from_vars(vars, values);
        transform_point(pose, local)
    }

    fn world_axis(vars: [VarId; 6], local_axis: [f64; 3], values: &VarSet) -> [f64; 3] {
        let pose = Self::pose_from_vars(vars, values);
        normalize(transform_direction(pose, local_axis))
    }
}

impl ResidualEquation for MateResidual {
    fn involved_vars(&self) -> Vec<VarId> {
        match self {
            Self::PointDelta { vars_a, vars_b, .. } | Self::Distance { vars_a, vars_b, .. } => {
                vars_a.iter().chain(vars_b).copied().collect()
            }
            Self::Translation { vars, .. } | Self::DistanceToFixed { vars, .. } => vars.to_vec(),
            Self::AxisAlignment { vars_a, vars_b, .. } | Self::AxisAngle { vars_a, vars_b, .. } => {
                vars_a.iter().chain(vars_b).copied().collect()
            }
        }
    }

    fn residual(&self, values: &VarSet) -> f64 {
        match self {
            Self::PointDelta {
                vars_a,
                local_a,
                vars_b,
                local_b,
                axis,
            } => {
                let a = Self::world_origin(*vars_a, *local_a, values);
                let b = Self::world_origin(*vars_b, *local_b, values);
                a[*axis] - b[*axis]
            }
            Self::Translation {
                vars,
                local,
                other,
                axis,
            } => {
                let pose = Self::pose_from_vars(*vars, values);
                let world = transform_point(pose, *local);
                world[*axis] - other[*axis]
            }
            Self::Distance {
                vars_a,
                local_a,
                vars_b,
                local_b,
                target_m,
            } => {
                let a = Self::world_origin(*vars_a, *local_a, values);
                let b = Self::world_origin(*vars_b, *local_b, values);
                pose::distance(a, b) - target_m
            }
            Self::DistanceToFixed {
                vars,
                local,
                fixed_world,
                target_m,
            } => {
                let world = Self::world_origin(*vars, *local, values);
                pose::distance(world, *fixed_world) - target_m
            }
            Self::AxisAlignment {
                vars_a,
                local_axis_a,
                vars_b,
                local_axis_b,
            } => {
                let a = Self::world_axis(*vars_a, *local_axis_a, values);
                let b = Self::world_axis(*vars_b, *local_axis_b, values);
                let c = pose::cross(a, b);
                c[0] * c[0] + c[1] * c[1] + c[2] * c[2]
            }
            Self::AxisAngle {
                vars_a,
                local_axis_a,
                vars_b,
                local_axis_b,
                target_cos,
            } => {
                let a = Self::world_axis(*vars_a, *local_axis_a, values);
                let b = Self::world_axis(*vars_b, *local_axis_b, values);
                pose::dot(a, b) - target_cos
            }
        }
    }
}

fn fixed_world_origin(
    dof: &AssemblyDofModel,
    entity: &MateEntity,
    connectors: &[Connector],
) -> [f64; 3] {
    let (local_origin, _) = resolve_mate_entity_frame(entity, connectors)
        .unwrap_or((entity.local_origin_m, entity.local_axis_m));
    let transform = dof
        .initial_transforms
        .get(entity.instance.as_str())
        .copied()
        .unwrap_or_else(opencad_geometry::RigidTransform::identity);
    transform_point(transform, local_origin)
}

fn resolved_local_frame(entity: &MateEntity, connectors: &[Connector]) -> ([f64; 3], [f64; 3]) {
    resolve_mate_entity_frame(entity, connectors)
        .unwrap_or((entity.local_origin_m, entity.local_axis_m))
}

pub fn build_mate_residuals(
    dof: &AssemblyDofModel,
    mates: &[Mate],
    connectors: &[Connector],
) -> opencad_core::Result<Vec<MateResidual>> {
    let mut equations = Vec::new();
    for mate in mates {
        if mate.suppressed {
            continue;
        }
        match &mate.kind {
            MateKind::Ground { .. } => {}
            MateKind::Coincident { a, b } => push_coincident(&mut equations, dof, a, b, connectors),
            MateKind::Concentric { a, b } => {
                push_coincident(&mut equations, dof, a, b, connectors);
                push_parallel(&mut equations, dof, a, b, connectors);
            }
            MateKind::Distance { a, b, distance_m } => {
                push_distance(&mut equations, dof, a, b, *distance_m, connectors)
            }
            MateKind::Angle { a, b, angle_rad } => {
                push_angle(&mut equations, dof, a, b, *angle_rad, connectors)
            }
            MateKind::Parallel { a, b } => push_parallel(&mut equations, dof, a, b, connectors),
        }
    }
    Ok(equations)
}

fn push_coincident(
    equations: &mut Vec<MateResidual>,
    dof: &AssemblyDofModel,
    a: &MateEntity,
    b: &MateEntity,
    connectors: &[Connector],
) {
    let (local_a, _) = resolved_local_frame(a, connectors);
    let (local_b, _) = resolved_local_frame(b, connectors);
    match (dof.pose_vars(&a.instance), dof.pose_vars(&b.instance)) {
        (Some(pose_a), Some(pose_b)) => {
            for axis in 0..3 {
                equations.push(MateResidual::PointDelta {
                    vars_a: pose_a.all(),
                    local_a,
                    vars_b: pose_b.all(),
                    local_b,
                    axis,
                });
            }
        }
        (Some(pose), None) => {
            let fixed = fixed_world_origin(dof, b, connectors);
            for axis in 0..3 {
                equations.push(MateResidual::Translation {
                    vars: pose.all(),
                    local: local_a,
                    other: fixed,
                    axis,
                });
            }
        }
        (None, Some(pose)) => {
            let fixed = fixed_world_origin(dof, a, connectors);
            for axis in 0..3 {
                equations.push(MateResidual::Translation {
                    vars: pose.all(),
                    local: local_b,
                    other: fixed,
                    axis,
                });
            }
        }
        (None, None) => {}
    }
}

fn push_distance(
    equations: &mut Vec<MateResidual>,
    dof: &AssemblyDofModel,
    a: &MateEntity,
    b: &MateEntity,
    target_m: f64,
    connectors: &[Connector],
) {
    let (local_a, _) = resolved_local_frame(a, connectors);
    let (local_b, _) = resolved_local_frame(b, connectors);
    match (dof.pose_vars(&a.instance), dof.pose_vars(&b.instance)) {
        (Some(pose_a), Some(pose_b)) => {
            equations.push(MateResidual::Distance {
                vars_a: pose_a.all(),
                local_a,
                vars_b: pose_b.all(),
                local_b,
                target_m,
            });
        }
        (Some(pose), None) => {
            equations.push(MateResidual::DistanceToFixed {
                vars: pose.all(),
                local: local_a,
                fixed_world: fixed_world_origin(dof, b, connectors),
                target_m,
            });
        }
        (None, Some(pose)) => {
            equations.push(MateResidual::DistanceToFixed {
                vars: pose.all(),
                local: local_b,
                fixed_world: fixed_world_origin(dof, a, connectors),
                target_m,
            });
        }
        (None, None) => {}
    }
}

fn push_parallel(
    equations: &mut Vec<MateResidual>,
    dof: &AssemblyDofModel,
    a: &MateEntity,
    b: &MateEntity,
    connectors: &[Connector],
) {
    if let (Some(pose_a), Some(pose_b)) = (dof.pose_vars(&a.instance), dof.pose_vars(&b.instance)) {
        let (_, local_axis_a) = resolved_local_frame(a, connectors);
        let (_, local_axis_b) = resolved_local_frame(b, connectors);
        equations.push(MateResidual::AxisAlignment {
            vars_a: pose_a.all(),
            local_axis_a,
            vars_b: pose_b.all(),
            local_axis_b,
        });
    }
}

fn push_angle(
    equations: &mut Vec<MateResidual>,
    dof: &AssemblyDofModel,
    a: &MateEntity,
    b: &MateEntity,
    angle_rad: f64,
    connectors: &[Connector],
) {
    if let (Some(pose_a), Some(pose_b)) = (dof.pose_vars(&a.instance), dof.pose_vars(&b.instance)) {
        let (_, local_axis_a) = resolved_local_frame(a, connectors);
        let (_, local_axis_b) = resolved_local_frame(b, connectors);
        equations.push(MateResidual::AxisAngle {
            vars_a: pose_a.all(),
            local_axis_a,
            vars_b: pose_b.all(),
            local_axis_b,
            target_cos: angle_rad.cos(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::{Instance, Placement};
    use opencad_core::{ComponentId, InstanceId, MateId, TopoRefId};
    use opencad_geometry::RigidTransform;
    use opencad_geometry::TopoRef;

    #[test]
    fn distance_residual_at_target() -> opencad_core::Result<()> {
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
            Placement::new(RigidTransform::from_translation([0.12, 0.0, 0.0])),
            "Right",
        );
        let mates = vec![Mate::new(
            MateId::new("mate:spacing")?,
            MateKind::Distance {
                a: MateEntity::at_origin(
                    InstanceId::new("instance:left")?,
                    TopoRef::face(
                        TopoRefId::new("ref:face:left")?,
                        "feature:extrude_base",
                        "top",
                    ),
                ),
                b: MateEntity::at_origin(
                    InstanceId::new("instance:right")?,
                    TopoRef::face(
                        TopoRefId::new("ref:face:right")?,
                        "feature:extrude_base",
                        "top",
                    ),
                ),
                distance_m: 0.12,
            },
        )];
        let dof = AssemblyDofModel::build(&[left, right], &mates);
        let eqs = build_mate_residuals(&dof, &mates, &[])?;
        let vars = dof.initial_var_set();
        assert_eq!(eqs.len(), 1);
        assert!(eqs[0].residual(&vars).abs() < 1e-6);
        Ok(())
    }
}
