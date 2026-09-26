//! Patch dry-run validation (Task-146+).

use opencad_core::{OpenCadError, Result, ValidationMessage, ValidationReport};
use opencad_graph::{build_summary, DesignDiff, ParamGraph};
use serde::{Deserialize, Serialize};

use crate::state::{diff_design_state, DesignState};
use crate::{predict_change_impact, ChangeImpact, DesignPatch, ImpactContext};

/// Result of validating a patch without mutating the source document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchDryRunReport {
    pub validation: ValidationReport,
    pub diff: DesignDiff,
    pub impact: ChangeImpact,
}

/// Build and validate the candidate state used by both dry-run and apply.
pub fn build_patch_candidate(before: &DesignState, patch: &DesignPatch) -> Result<DesignState> {
    let mut after = before.clone();
    patch.apply_to_state(&mut after)?;
    Ok(after)
}

/// Validate every structural invariant of a complete design state.
///
/// This is the whole-state form of the final-candidate checks that
/// structural patches run on the objects they touch (ADR-013 §3).  It is used
/// where no patch describes the change, such as a semantic merge result.
pub fn validate_design_state(state: &DesignState) -> Result<()> {
    use std::collections::BTreeSet;

    let mut failures = BTreeSet::new();
    match opencad_graph::evaluate_param_graph(&state.parameters) {
        Ok(scope) => {
            if let Err(error) = crate::patch::validate_feature_values(state, &scope) {
                failures.insert(error.to_string());
            }
        }
        Err(error) => {
            failures.insert(error.to_string());
        }
    }
    for sketch in &state.sketches {
        crate::sketch_patch::validate_sketch_references(sketch, state, &mut failures);
    }
    crate::sketch_patch::validate_profile_consumers(state, |_, _| true, &mut failures);
    for node in &state.feature_nodes {
        crate::feature_patch::check_feature_inputs(node, state, &mut failures);
    }
    for topo_ref in &state.semantic_refs {
        let created_by = topo_ref.semantic.created_by.as_str();
        if !state.feature_nodes.iter().any(|node| node.id == created_by) {
            failures.insert(format!(
                "semantic reference '{}' is created by unknown feature '{created_by}'",
                topo_ref.ref_id
            ));
        }
    }
    if !state.feature_nodes.is_empty() || !state.feature_order.is_empty() {
        if let Err(error) = state.derive_feature_graph() {
            failures.insert(error.to_string());
        }
    }
    if let Some(assembly) = &state.assembly {
        if let Err(error) = crate::assembly::apply_assembly_patch(&mut assembly.clone(), &[]) {
            failures.insert(error.to_string());
        }
    }
    if let Some(drawing) = &state.drawing {
        if let Err(error) = crate::drawing::apply_drawing_patch(&mut drawing.clone(), &[]) {
            failures.insert(error.to_string());
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(OpenCadError::validation(
            failures.into_iter().collect::<Vec<_>>().join("; "),
        ))
    }
}

/// Validate that a patch can be applied against a full design state.
pub fn dry_run_patch_state(before: &DesignState, patch: &DesignPatch) -> PatchDryRunReport {
    dry_run_patch_state_with_context(before, patch, ImpactContext::default())
}

/// Validate a patch and predict its affected Feature Graph nodes.
pub fn dry_run_patch_state_with_context(
    before: &DesignState,
    patch: &DesignPatch,
    context: ImpactContext<'_>,
) -> PatchDryRunReport {
    let mut validation = ValidationReport::new();

    let after = match build_patch_candidate(before, patch) {
        Ok(after) => after,
        Err(err) => {
            validation.push(
                ValidationMessage::error("patch_apply_failed", err.to_string())
                    .with_target("patch"),
            );
            return PatchDryRunReport {
                validation,
                diff: DesignDiff::semantic("Patch rejected", Vec::new()),
                impact: ChangeImpact::default(),
            };
        }
    };

    let diff = diff_design_state(before, &after);
    let summary = if validation.is_ok() {
        if diff.changes.is_empty() {
            "No changes".into()
        } else {
            build_summary(&diff.changes)
        }
    } else {
        "Patch rejected".into()
    };

    let diff = DesignDiff::semantic(summary, diff.changes);
    // A patch that changes Feature Graph inputs is predicted against the
    // derived candidate graph, so added features appear in the dirty suffix.
    let derived_graph = patch
        .changes_feature_graph()
        .then(|| after.derive_feature_graph().ok())
        .flatten();
    let context = ImpactContext {
        feature_graph: derived_graph.as_ref().or(context.feature_graph),
        sketches: context.sketches,
    };
    let impact = predict_change_impact(before, &after, &diff, context);
    PatchDryRunReport {
        validation,
        diff,
        impact,
    }
}

/// Validate that a patch can be applied and evaluated against a parameter graph.
pub fn dry_run_patch(before: &ParamGraph, patch: &DesignPatch) -> PatchDryRunReport {
    dry_run_patch_state(&DesignState::new(before.clone(), Vec::new()), patch)
}

/// Fail fast when dry-run validation contains errors.
pub fn ensure_patch_valid(report: &PatchDryRunReport) -> Result<()> {
    if report.validation.is_ok() {
        return Ok(());
    }
    let messages: Vec<String> = report
        .validation
        .messages
        .iter()
        .filter(|message| message.level == opencad_core::ValidationLevel::Error)
        .map(|message| message.message.clone())
        .collect();
    Err(OpenCadError::validation(messages.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DesignPatch;
    use opencad_graph::{bracket_parameters, SemanticChange};

    #[test]
    fn dry_run_accepts_valid_parameter_patch() {
        let before = bracket_parameters();
        let patch = DesignPatch::set_parameter("param:width", "100 mm");
        let report = dry_run_patch(&before, &patch);
        assert!(report.validation.is_ok());
        assert_eq!(report.diff.changes.len(), 1);
        assert_eq!(
            report.diff.changes[0],
            SemanticChange::ParameterChanged {
                id: "param:width".into(),
                before: "80 mm".into(),
                after: "100 mm".into(),
            }
        );
    }

    #[test]
    fn dry_run_rejects_unknown_parameter() {
        let before = bracket_parameters();
        let patch = DesignPatch::set_parameter("param:missing", "10 mm");
        let report = dry_run_patch(&before, &patch);
        assert!(!report.validation.is_ok());
        ensure_patch_valid(&report).expect_err("invalid patch");
    }

    #[test]
    fn dry_run_rejects_invalid_expression() {
        let before = bracket_parameters();
        let patch = DesignPatch::set_parameter("param:width", "not_a_length");
        let report = dry_run_patch(&before, &patch);
        assert!(!report.validation.is_ok());
    }
}
