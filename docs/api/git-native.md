# Git-native API

The `opencad-ai` crate exposes:

- `DesignPatch`, `PatchPrecondition`, and `ExpectedEffect` for reviewable design proposals.
- `semantic_three_way_merge` and `rebase_patch` for stable-ID collaboration.
- `EngineeringPolicy`, `EngineeringMetrics`, and `evaluate_policy` for CI gates.
- `IntentProvider`, `create_proposal`, and `apply_approved_proposal` for provider-separated agents.

All inputs and reports are serializable except `DesignState` and `SemanticMergeResult`, whose merged
state is an in-memory document model. File I/O remains in `opencad-file` and the CLI.

The `opencad review` CLI writes the machine-readable report as `review.json` and a deterministic
GitHub Actions summary as `github-summary.md`. It returns a validation error after producing the
review bundle when one or more declared `ExpectedEffect` checks fail, so CI can preserve the
evidence and still block the change.
