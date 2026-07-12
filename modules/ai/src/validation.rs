//! Patch dry-run validation (Task-146+).

use opencad_core::{OpenCadError, Result, ValidationMessage, ValidationReport};
use opencad_graph::{build_summary, evaluate_param_graph, DesignDiff, ParamGraph};
use serde::{Deserialize, Serialize};

use crate::state::{diff_design_state, DesignState};
use crate::DesignPatch;

/// Result of validating a patch without mutating the source document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchDryRunReport {
    pub validation: ValidationReport,
    pub diff: DesignDiff,
}

/// Validate that a patch can be applied against a full design state.
pub fn dry_run_patch_state(before: &DesignState, patch: &DesignPatch) -> PatchDryRunReport {
    let mut validation = ValidationReport::new();
    let mut after = before.clone();

    if let Err(err) = patch.apply_to_document(
        &mut after.parameters,
        &mut after.feature_nodes,
        &mut after.semantic_refs,
        after.assembly.as_mut(),
        after.drawing.as_mut(),
    ) {
        validation.push(
            ValidationMessage::error("patch_apply_failed", err.to_string()).with_target("patch"),
        );
        return PatchDryRunReport {
            validation,
            diff: DesignDiff::semantic("Patch rejected", Vec::new()),
        };
    }

    if let Err(err) = evaluate_param_graph(&after.parameters) {
        validation.push(
            ValidationMessage::error("param_eval_failed", err.to_string()).with_target("patch"),
        );
    }

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

    PatchDryRunReport {
        validation,
        diff: DesignDiff::semantic(summary, diff.changes),
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
