//! Assembly mate solver (M3.2).

use opencad_core::{OpenCadError, Result};
use opencad_solver::{solve_with_diagnostics_generic, SolveStatus, SolverOptions, VarSet};

use crate::dof::AssemblyDofModel;
use crate::instance::Instance;
use crate::mate::validate_mates;
use crate::model::AssemblyModel;
use crate::residual::build_mate_residuals;

/// Outcome of solving assembly mates.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblySolveReport {
    pub status: SolveStatus,
    pub dof: i32,
    pub equation_count: usize,
    pub variable_count: usize,
    pub grounded_instances: usize,
    pub iterations: usize,
    pub max_error: f64,
}

/// Solve mate constraints and write poses back into `instances`.
pub fn solve_assembly_mates(model: &AssemblyModel) -> Result<(Vec<Instance>, AssemblySolveReport)> {
    let instance_ids: Vec<_> = model.instances.iter().map(|i| i.id.clone()).collect();
    validate_mates(&model.mates, &instance_ids, &model.connectors)?;

    let dof_model = AssemblyDofModel::build(&model.instances, &model.mates);
    let equations = build_mate_residuals(&dof_model, &model.mates, &model.connectors)?;
    let vars = dof_model.initial_var_set();

    if dof_model.movable.is_empty() {
        return Ok((
            model.instances.clone(),
            AssemblySolveReport {
                status: SolveStatus::Solved {
                    iterations: 0,
                    max_error: 0.0,
                },
                dof: 0,
                equation_count: equations.len(),
                variable_count: 0,
                grounded_instances: dof_model.grounded.len(),
                iterations: 0,
                max_error: 0.0,
            },
        ));
    }

    if equations.is_empty() && !dof_model.movable.is_empty() {
        return Ok((
            model.instances.clone(),
            AssemblySolveReport {
                status: SolveStatus::UnderConstrained {
                    dof: dof_model.total_instance_dof() as i32,
                    iterations: 0,
                    max_error: 0.0,
                },
                dof: dof_model.total_instance_dof() as i32,
                equation_count: 0,
                variable_count: dof_model.registry.len(),
                grounded_instances: dof_model.grounded.len(),
                iterations: 0,
                max_error: 0.0,
            },
        ));
    }

    let (output, status) =
        solve_with_diagnostics_generic(&equations, vars, &SolverOptions::default());
    let dof = opencad_solver::estimate_dof_generic(&equations, &output.vars);

    if matches!(status, SolveStatus::Failed { .. }) {
        return Err(OpenCadError::validation(format!(
            "assembly mate solver failed: max_error={}",
            output.max_error
        )));
    }

    let instances = apply_solved_poses(&model.instances, &dof_model, &output.vars);
    let (iterations, max_error) = match &status {
        SolveStatus::Solved {
            iterations,
            max_error,
        }
        | SolveStatus::UnderConstrained {
            iterations,
            max_error,
            ..
        }
        | SolveStatus::OverConstrained {
            iterations,
            max_error,
            ..
        }
        | SolveStatus::Failed {
            iterations,
            max_error,
            ..
        } => (*iterations, *max_error),
    };

    Ok((
        instances,
        AssemblySolveReport {
            status,
            dof,
            equation_count: equations.len(),
            variable_count: dof_model.registry.len(),
            grounded_instances: dof_model.grounded.len(),
            iterations,
            max_error,
        },
    ))
}

fn apply_solved_poses(
    instances: &[Instance],
    dof_model: &AssemblyDofModel,
    vars: &VarSet,
) -> Vec<Instance> {
    instances
        .iter()
        .map(|instance| {
            let mut updated = instance.clone();
            if dof_model.pose_vars(&instance.id).is_some() {
                updated.placement.transform = dof_model.transform_for_instance(&instance.id, vars);
            }
            updated
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::{Instance, Placement};
    use crate::mate::{Mate, MateEntity, MateKind};
    use opencad_core::{ComponentId, InstanceId, MateId, TopoRefId};
    use opencad_geometry::{RigidTransform, TopoRef};

    fn two_bracket_model(distance_m: f64) -> Result<AssemblyModel> {
        Ok(AssemblyModel {
            components: vec![crate::component::Component::new(
                ComponentId::new("component:bracket")?,
                "parts/bracket.ocad.d",
                opencad_core::DocumentId::new("doc:bracket_001")?,
            )],
            instances: vec![
                Instance {
                    id: InstanceId::new("instance:left")?,
                    component: ComponentId::new("component:bracket")?,
                    placement: Placement::identity(),
                    fixed: false,
                    name: "Left".into(),
                },
                Instance::new(
                    InstanceId::new("instance:right")?,
                    ComponentId::new("component:bracket")?,
                    Placement::new(RigidTransform::from_translation([0.05, 0.0, 0.0])),
                    "Right",
                ),
            ],
            mates: vec![
                Mate::new(
                    MateId::new("mate:ground_left")?,
                    MateKind::Ground {
                        instance: InstanceId::new("instance:left")?,
                    },
                ),
                Mate::new(
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
                        distance_m,
                    },
                ),
            ],
            connectors: Vec::new(),
            patterns: Vec::new(),
        })
    }

    #[test]
    fn solves_grounded_distance_mate() -> Result<()> {
        let model = two_bracket_model(0.12)?;
        let (instances, report) = solve_assembly_mates(&model)?;
        assert!(report.max_error < 1e-4);
        let right = instances
            .iter()
            .find(|i| i.id.as_str() == "instance:right")
            .expect("right");
        assert!((right.placement.transform.translation_m[0] - 0.12).abs() < 1e-3);
        Ok(())
    }
}
