//! Deterministic engineering policy evaluation for CI and agent approval gates.

use opencad_graph::ParamGraph;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineeringPolicy {
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyRule {
    ParameterExprEquals { id: String, expr: String },
    MaxMassKg { value: f64 },
    BoundingBoxWithinM { max: [f64; 3] },
    NoAssemblyInterference,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineeringMetrics {
    pub mass_kg: Option<f64>,
    pub bounding_box_size_m: Option<[f64; 3]>,
    pub assembly_interference_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyFinding {
    pub rule_index: usize,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyReport {
    pub passed: bool,
    pub findings: Vec<PolicyFinding>,
}

pub fn evaluate_policy(
    policy: &EngineeringPolicy,
    parameters: &ParamGraph,
    metrics: &EngineeringMetrics,
) -> PolicyReport {
    let findings = policy
        .rules
        .iter()
        .enumerate()
        .map(|(index, rule)| evaluate_rule(index, rule, parameters, metrics))
        .collect::<Vec<_>>();
    PolicyReport {
        passed: findings.iter().all(|finding| finding.passed),
        findings,
    }
}

fn evaluate_rule(
    index: usize,
    rule: &PolicyRule,
    parameters: &ParamGraph,
    metrics: &EngineeringMetrics,
) -> PolicyFinding {
    let (passed, message) = match rule {
        PolicyRule::ParameterExprEquals { id, expr } => {
            let actual = parameters.get(id).map(|p| p.expr.as_str());
            (
                actual == Some(expr),
                format!("{id}: expected '{expr}', actual {actual:?}"),
            )
        }
        PolicyRule::MaxMassKg { value } => match metrics.mass_kg {
            Some(actual) => (actual <= *value, format!("mass {actual} kg <= {value} kg")),
            None => (false, "mass metric is unavailable".into()),
        },
        PolicyRule::BoundingBoxWithinM { max } => match metrics.bounding_box_size_m {
            Some(actual) => (
                actual
                    .iter()
                    .zip(max)
                    .all(|(actual, limit)| actual <= limit),
                format!("bounding box {actual:?} m within {max:?} m"),
            ),
            None => (false, "bounding-box metric is unavailable".into()),
        },
        PolicyRule::NoAssemblyInterference => match metrics.assembly_interference_count {
            Some(count) => (
                count == 0,
                format!("assembly interference count is {count}"),
            ),
            None => (false, "assembly interference metric is unavailable".into()),
        },
    };
    PolicyFinding {
        rule_index: index,
        passed,
        message,
    }
}

#[cfg(test)]
mod tests {
    use opencad_graph::{ParamGraph, ParameterEntry};

    use super::*;

    #[test]
    fn reports_all_policy_failures_deterministically() {
        let mut parameters = ParamGraph::new();
        parameters
            .add_parameter(ParameterEntry::new("param:width", "width", "80 mm"))
            .unwrap();
        let policy = EngineeringPolicy {
            rules: vec![
                PolicyRule::ParameterExprEquals {
                    id: "param:width".into(),
                    expr: "100 mm".into(),
                },
                PolicyRule::MaxMassKg { value: 0.08 },
                PolicyRule::NoAssemblyInterference,
            ],
        };
        let report = evaluate_policy(
            &policy,
            &parameters,
            &EngineeringMetrics {
                mass_kg: Some(0.09),
                bounding_box_size_m: None,
                assembly_interference_count: Some(0),
            },
        );
        assert!(!report.passed);
        assert_eq!(
            report
                .findings
                .iter()
                .filter(|finding| !finding.passed)
                .count(),
            2
        );
    }
}
