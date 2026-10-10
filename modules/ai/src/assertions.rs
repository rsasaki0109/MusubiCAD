//! Executable design assertion evaluation (MCAD-P6-004).

use std::collections::BTreeMap;

use opencad_assembly::{AssemblyRegenReport, InstanceRegenStatus};
use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
use opencad_geometry::ReferenceProvenance;
use opencad_graph::{evaluate_param_graph, ParamGraph};
use serde::{Deserialize, Serialize};

/// Evidence available when evaluating assertions after regeneration.
#[derive(Debug, Clone, Default)]
pub struct AssertionContext {
    /// Evaluated parameter values keyed by parameter name (meters).
    pub parameter_values: BTreeMap<String, f64>,
    /// Regenerated body mass in kilograms.
    pub mass_kg: Option<f64>,
    /// Regenerated bounding-box size in meters.
    pub bounding_box_size_m: Option<[f64; 3]>,
    /// Number of solid bodies in the regenerated result.
    pub body_count: Option<u32>,
    /// Fail-closed reference provenance (MCAD-P6-003).
    pub reference_provenance: Vec<ReferenceProvenance>,
    /// Solved assembly DOF.
    pub assembly_dof: Option<i32>,
    /// Detected assembly interference count.
    pub interference_count: Option<usize>,
}

/// Assertion evidence for a regenerated assembly (MCAD-P10-001).
///
/// Mass and bounds come from the placed instance bodies, the body count is
/// the number of instances that regenerated to a body, and DOF is the
/// report's remaining assembly DOF.  `interference_count` is the caller's
/// exact count under its explicit interference tolerances.  Parameters that
/// do not evaluate leave the map empty, so parameter assertions fail closed.
/// Semantic references are part-level evidence and stay empty here.
pub fn assembly_assertion_context(
    parameters: &ParamGraph,
    report: &AssemblyRegenReport,
    interference_count: usize,
) -> AssertionContext {
    let body_count = report
        .instances
        .iter()
        .filter(|instance| {
            matches!(instance.status, InstanceRegenStatus::Ok) && instance.body.is_some()
        })
        .count();
    AssertionContext {
        parameter_values: evaluate_param_graph(parameters)
            .unwrap_or_default()
            .into_iter()
            .collect(),
        mass_kg: report.scene.mass.map(|mass| mass.mass_kg),
        bounding_box_size_m: report.scene.bounding_box.map(|bounds| {
            [
                bounds.max[0] - bounds.min[0],
                bounds.max[1] - bounds.min[1],
                bounds.max[2] - bounds.min[2],
            ]
        }),
        body_count: Some(u32::try_from(body_count).unwrap_or(u32::MAX)),
        reference_provenance: Vec::new(),
        assembly_dof: Some(report.dof),
        interference_count: Some(interference_count),
    }
}

/// Outcome of one assertion evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionResult {
    pub id: String,
    pub name: String,
    pub severity: AssertionSeverity,
    pub passed: bool,
    pub message: String,
}

/// Evaluate every assertion against the regeneration evidence.
///
/// Results are sorted by assertion id so the report is deterministic. Invalid
/// assertions fail closed so a malformed rule cannot silently pass.
pub fn evaluate_assertions(
    assertions: &[Assertion],
    context: &AssertionContext,
) -> Vec<AssertionResult> {
    let mut results = assertions
        .iter()
        .map(|assertion| evaluate_assertion(assertion, context))
        .collect::<Vec<_>>();
    results.sort_by(|left, right| left.id.cmp(&right.id));
    results
}

/// Whether every `Required` assertion passed. Advisory failures never block.
pub fn required_assertions_pass(results: &[AssertionResult]) -> bool {
    results
        .iter()
        .all(|result| result.severity != AssertionSeverity::Required || result.passed)
}

pub fn evaluate_assertion(assertion: &Assertion, context: &AssertionContext) -> AssertionResult {
    if let Err(error) = assertion.validate() {
        return AssertionResult {
            id: assertion.id.clone(),
            name: assertion.name.clone(),
            severity: assertion.severity,
            passed: false,
            message: error.to_string(),
        };
    }
    let (passed, message) = match &assertion.kind {
        AssertionKind::ParameterRange {
            parameter_name,
            min_m,
            max_m,
        } => match context.parameter_values.get(parameter_name) {
            Some(value) => (
                *value >= *min_m && *value <= *max_m,
                format!("parameter '{parameter_name}' = {value} m within [{min_m}, {max_m}] m"),
            ),
            None => (
                false,
                format!("parameter '{parameter_name}' is not defined"),
            ),
        },
        AssertionKind::MassRange { min_kg, max_kg } => match context.mass_kg {
            Some(actual) => (
                actual >= *min_kg && actual <= *max_kg,
                format!("mass {actual} kg within [{min_kg}, {max_kg}] kg"),
            ),
            None => (false, "mass metric is unavailable".into()),
        },
        AssertionKind::BoundingBoxWithin { max_m } => match context.bounding_box_size_m {
            Some(actual) => (
                actual
                    .iter()
                    .zip(max_m)
                    .all(|(actual, limit)| actual <= limit),
                format!("bounding box {actual:?} m within {max_m:?} m"),
            ),
            None => (false, "bounding-box metric is unavailable".into()),
        },
        AssertionKind::BodyCount { expected } => match context.body_count {
            Some(actual) => (
                actual == *expected,
                format!("body count {actual} == {expected}"),
            ),
            None => (false, "body count metric is unavailable".into()),
        },
        AssertionKind::RequiredReference { ref_id } => {
            match context
                .reference_provenance
                .iter()
                .find(|provenance| provenance.ref_id == *ref_id)
            {
                Some(provenance) => (
                    !provenance.status.is_unresolved(),
                    format!(
                        "reference '{ref_id}' resolved as {:?}: {}",
                        provenance.status, provenance.reason
                    ),
                ),
                None => (
                    false,
                    format!("reference '{ref_id}' has no provenance record"),
                ),
            }
        }
        AssertionKind::AssemblyDofAtMost { max_dof } => match context.assembly_dof {
            Some(actual) => (
                actual <= *max_dof,
                format!("assembly DOF {actual} <= {max_dof}"),
            ),
            None => (false, "assembly DOF metric is unavailable".into()),
        },
        AssertionKind::InterferenceAtMost { max_count } => match context.interference_count {
            Some(actual) => (
                actual <= *max_count as usize,
                format!("assembly interference count {actual} <= {max_count}"),
            ),
            None => (false, "assembly interference metric is unavailable".into()),
        },
    };
    AssertionResult {
        id: assertion.id.clone(),
        name: assertion.name.clone(),
        severity: assertion.severity,
        passed,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{AssertionKind, AssertionSeverity};
    use opencad_geometry::ReferenceStatus;

    fn context() -> AssertionContext {
        let mut parameter_values = BTreeMap::new();
        parameter_values.insert("upper_hub_height".to_string(), 0.032);
        AssertionContext {
            parameter_values,
            mass_kg: Some(0.61),
            bounding_box_size_m: Some([0.14, 0.11, 0.04]),
            body_count: Some(1),
            reference_provenance: vec![opencad_geometry::ReferenceProvenance {
                ref_id: "ref:face:shaft_bore".into(),
                kind: opencad_geometry::TopoRefKind::Face,
                status: ReferenceStatus::Exact,
                created_by: "feature:shaft_bore".into(),
                role: Some("bore".into()),
                resolved_kernel_id: Some(42),
                candidate_count: 1,
                reason: "stored kernel face present".into(),
                candidates: Vec::new(),
                required: false,
            }],
            assembly_dof: Some(9),
            interference_count: Some(0),
        }
    }

    #[test]
    fn evaluates_every_assertion_kind() {
        let assertions = vec![
            Assertion::new(
                "assertion:param",
                "Hub height",
                AssertionSeverity::Required,
                AssertionKind::ParameterRange {
                    parameter_name: "upper_hub_height".into(),
                    min_m: 0.020,
                    max_m: 0.050,
                },
            ),
            Assertion::new(
                "assertion:mass",
                "Mass",
                AssertionSeverity::Required,
                AssertionKind::MassRange {
                    min_kg: 0.5,
                    max_kg: 0.7,
                },
            ),
            Assertion::new(
                "assertion:bbox",
                "Bounds",
                AssertionSeverity::Required,
                AssertionKind::BoundingBoxWithin {
                    max_m: [0.2, 0.2, 0.1],
                },
            ),
            Assertion::new(
                "assertion:body_count",
                "Body count",
                AssertionSeverity::Required,
                AssertionKind::BodyCount { expected: 1 },
            ),
            Assertion::new(
                "assertion:reference",
                "Shaft reference",
                AssertionSeverity::Required,
                AssertionKind::RequiredReference {
                    ref_id: "ref:face:shaft_bore".into(),
                },
            ),
            Assertion::new(
                "assertion:dof",
                "Assembly DOF",
                AssertionSeverity::Required,
                AssertionKind::AssemblyDofAtMost { max_dof: 12 },
            ),
            Assertion::new(
                "assertion:interference",
                "Zero interference",
                AssertionSeverity::Required,
                AssertionKind::InterferenceAtMost { max_count: 0 },
            ),
        ];
        let results = evaluate_assertions(&assertions, &context());
        assert_eq!(results.len(), 7);
        assert!(results.iter().all(|result| result.passed));
        assert!(required_assertions_pass(&results));
    }

    #[test]
    fn violated_required_assertion_fails_and_blocks() {
        let mut ctx = context();
        ctx.mass_kg = Some(1.2);
        let assertions = vec![Assertion::new(
            "assertion:mass",
            "Mass",
            AssertionSeverity::Required,
            AssertionKind::MassRange {
                min_kg: 0.5,
                max_kg: 0.7,
            },
        )];
        let results = evaluate_assertions(&assertions, &ctx);
        assert!(!results[0].passed);
        assert!(!required_assertions_pass(&results));
    }

    #[test]
    fn advisory_failure_never_blocks() {
        let mut ctx = context();
        ctx.body_count = Some(3);
        let assertions = vec![Assertion::new(
            "assertion:body_count",
            "Body count",
            AssertionSeverity::Advisory,
            AssertionKind::BodyCount { expected: 1 },
        )];
        let results = evaluate_assertions(&assertions, &ctx);
        assert!(!results[0].passed);
        assert!(required_assertions_pass(&results));
    }

    fn assembly_report(dof: i32) -> AssemblyRegenReport {
        use opencad_assembly::{AssemblyScene, InstanceRegenResult};
        use opencad_core::InstanceId;
        use opencad_geometry::{BoundingBox, KernelBody, MassProperties};

        let instances = vec![
            InstanceRegenResult {
                instance_id: InstanceId::new("instance:base").expect("instance id"),
                status: InstanceRegenStatus::Ok,
                body: Some(KernelBody(1)),
            },
            InstanceRegenResult {
                instance_id: InstanceId::new("instance:arm").expect("instance id"),
                status: InstanceRegenStatus::Ok,
                body: Some(KernelBody(2)),
            },
            InstanceRegenResult {
                instance_id: InstanceId::new("instance:missing").expect("instance id"),
                status: InstanceRegenStatus::Failed("unknown component".into()),
                body: None,
            },
        ];
        AssemblyRegenReport {
            instance_count: instances.len(),
            successful_instances: 2,
            scene: AssemblyScene {
                instances: instances.clone(),
                compound_body: None,
                bounding_box: Some(BoundingBox {
                    min: [0.0, 0.0, 0.0],
                    max: [0.3, 0.1, 0.5],
                }),
                mass: Some(MassProperties {
                    volume_m3: 1.0e-3,
                    area_m2: 0.1,
                    mass_kg: 2.7,
                    center_of_mass: [0.0, 0.0, 0.0],
                }),
            },
            instances,
            mate_solve: None,
            dof,
        }
    }

    #[test]
    fn assembly_context_carries_assembly_evidence() {
        use opencad_graph::ParameterEntry;

        let mut parameters = ParamGraph::new();
        parameters
            .add_parameter(ParameterEntry::new("param:reach", "reach", "250 mm"))
            .expect("parameter");
        let context = assembly_assertion_context(&parameters, &assembly_report(1), 2);
        assert_eq!(context.parameter_values.get("reach").copied(), Some(0.25));
        assert_eq!(context.mass_kg, Some(2.7));
        let size = context.bounding_box_size_m.expect("bounds");
        assert!((size[0] - 0.3).abs() < 1e-12 && (size[2] - 0.5).abs() < 1e-12);
        assert_eq!(context.body_count, Some(2), "failed instances have no body");
        assert_eq!(context.assembly_dof, Some(1));
        assert_eq!(context.interference_count, Some(2));
        assert!(context.reference_provenance.is_empty());
    }

    #[test]
    fn assembly_assertions_block_on_interference_and_dof() {
        let assertions = vec![
            Assertion::new(
                "assertion:clearance",
                "No collisions",
                AssertionSeverity::Required,
                AssertionKind::InterferenceAtMost { max_count: 0 },
            ),
            Assertion::new(
                "assertion:dof",
                "One joint free",
                AssertionSeverity::Required,
                AssertionKind::AssemblyDofAtMost { max_dof: 1 },
            ),
        ];
        let clear = assembly_assertion_context(&ParamGraph::new(), &assembly_report(1), 0);
        assert!(required_assertions_pass(&evaluate_assertions(
            &assertions,
            &clear
        )));

        let colliding = assembly_assertion_context(&ParamGraph::new(), &assembly_report(1), 1);
        let results = evaluate_assertions(&assertions, &colliding);
        assert!(!required_assertions_pass(&results));
        assert_eq!(results[0].id, "assertion:clearance");
        assert!(!results[0].passed);
        assert!(results[0].message.contains("interference count 1"));

        let loose = assembly_assertion_context(&ParamGraph::new(), &assembly_report(7), 0);
        let results = evaluate_assertions(&assertions, &loose);
        assert!(!results[1].passed, "{}", results[1].message);
    }

    #[test]
    fn ambiguous_reference_fails_a_required_reference_assertion() {
        let mut ctx = context();
        ctx.reference_provenance[0].status = ReferenceStatus::Ambiguous;
        ctx.reference_provenance[0].resolved_kernel_id = None;
        let assertions = vec![Assertion::new(
            "assertion:reference",
            "Shaft reference",
            AssertionSeverity::Required,
            AssertionKind::RequiredReference {
                ref_id: "ref:face:shaft_bore".into(),
            },
        )];
        let results = evaluate_assertions(&assertions, &ctx);
        assert!(!results[0].passed);
        assert!(results[0].message.to_lowercase().contains("ambiguous"));
    }
}
