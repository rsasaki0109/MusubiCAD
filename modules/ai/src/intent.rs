//! Provider-separated, selection-aware agent proposal and approval pipeline.

use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};

use crate::{dry_run_patch_state, ensure_patch_valid, DesignPatch, DesignState, PatchDryRunReport};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSelection {
    #[serde(default)]
    pub semantic_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentIntent {
    pub text: String,
    pub selection: AgentSelection,
}

/// Pluggable intent provider. Implementations may be local, remote, scripted, or test doubles;
/// they can only propose serializable patches and never receive mutable document state.
pub trait IntentProvider {
    fn name(&self) -> &str;
    fn propose(&self, intent: &AgentIntent, snapshot: &DesignState) -> Result<DesignPatch>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentProposal {
    pub approval_id: String,
    pub provider: String,
    pub intent: AgentIntent,
    pub patch: DesignPatch,
    pub dry_run: PatchDryRunReport,
}

pub fn create_proposal(
    provider: &dyn IntentProvider,
    intent: AgentIntent,
    snapshot: &DesignState,
) -> Result<AgentProposal> {
    validate_selection(&intent.selection, snapshot)?;
    let patch = provider.propose(&intent, snapshot)?;
    let dry_run = dry_run_patch_state(snapshot, &patch);
    ensure_patch_valid(&dry_run)?;
    let approval_id = approval_id(provider.name(), &intent, &patch)?;
    Ok(AgentProposal {
        approval_id,
        provider: provider.name().to_string(),
        intent,
        patch,
        dry_run,
    })
}

/// Apply only the exact reviewed proposal after an explicit approval ID round-trip.
pub fn apply_approved_proposal(
    proposal: &AgentProposal,
    supplied_approval_id: &str,
    state: &mut DesignState,
) -> Result<()> {
    if supplied_approval_id != proposal.approval_id {
        return Err(OpenCadError::validation(
            "approval ID does not match proposal",
        ));
    }
    let current = approval_id(&proposal.provider, &proposal.intent, &proposal.patch)?;
    if current != proposal.approval_id {
        return Err(OpenCadError::validation("proposal changed after review"));
    }
    ensure_patch_valid(&dry_run_patch_state(state, &proposal.patch))?;
    proposal.patch.apply_to_document(
        &mut state.parameters,
        &mut state.feature_nodes,
        &mut state.semantic_refs,
        state.assembly.as_mut(),
        state.drawing.as_mut(),
    )
}

fn validate_selection(selection: &AgentSelection, state: &DesignState) -> Result<()> {
    for id in &selection.semantic_ids {
        let exists = state.parameters.get(id).is_some()
            || state.feature_nodes.iter().any(|node| &node.id == id)
            || state
                .semantic_refs
                .iter()
                .any(|reference| reference.ref_id.as_str() == id);
        if !exists {
            return Err(OpenCadError::not_found(format!(
                "selected semantic ID '{id}'"
            )));
        }
    }
    Ok(())
}

fn approval_id(provider: &str, intent: &AgentIntent, patch: &DesignPatch) -> Result<String> {
    let bytes = serde_json::to_vec(&(provider, intent, patch))?;
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    Ok(format!("proposal:{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use opencad_graph::bracket_parameters;

    use super::*;

    struct WidthProvider;
    impl IntentProvider for WidthProvider {
        fn name(&self) -> &str {
            "test-width"
        }
        fn propose(&self, _: &AgentIntent, _: &DesignState) -> Result<DesignPatch> {
            Ok(DesignPatch::set_parameter("param:width", "100 mm"))
        }
    }

    #[test]
    fn requires_exact_approval_before_mutation() {
        let mut state = DesignState::new(bracket_parameters(), vec![]);
        let proposal = create_proposal(
            &WidthProvider,
            AgentIntent {
                text: "make it wider".into(),
                selection: AgentSelection {
                    semantic_ids: vec!["param:width".into()],
                },
            },
            &state,
        )
        .unwrap();
        apply_approved_proposal(&proposal, "wrong", &mut state).expect_err("approval");
        assert_eq!(state.parameters.get("param:width").unwrap().expr, "80 mm");
        apply_approved_proposal(&proposal, &proposal.approval_id, &mut state).unwrap();
        assert_eq!(state.parameters.get("param:width").unwrap().expr, "100 mm");
    }
}
