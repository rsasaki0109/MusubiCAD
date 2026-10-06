# Git-native API

The `opencad-ai` crate exposes:

- `DesignPatch`, `PatchPrecondition`, and `ExpectedEffect` for reviewable design proposals.
- `semantic_three_way_merge` and `rebase_patch` for stable-ID collaboration.
- `EngineeringPolicy`, `EngineeringMetrics`, and `evaluate_policy` for CI gates.
- `IntentProvider`, `create_proposal`, and `apply_approved_proposal` for provider-separated agents.

All inputs and reports are serializable except `DesignState` and `SemanticMergeResult`, whose merged
state is an in-memory document model. File I/O remains in `opencad-file` and the CLI.

`PatchPrecondition::RevisionEquals` carries `algorithm`, `version`, and a
SHA-256 `digest` for the complete canonical patchable `DesignState`. Use
`design_state_revision` to calculate it; `rebase_patch` refreshes an existing
revision guard after moving a patch to a newer base.

The `musubicad review` CLI writes the machine-readable report as `review.json` and a deterministic
GitHub Actions summary as `github-summary.md`. It returns a validation error after producing the
review bundle when one or more declared `ExpectedEffect` checks fail, so CI can preserve the
evidence and still block the change.

`before.png`, `after.png`, and `comparison.gif` need a GPU adapter (Vulkan, DX12, or Metal; Mesa's
software Vulkan works). Without one, the review still regenerates both sides, runs every check, and
writes `review.json`, `review.html`, and `github-summary.md`; the image fields are omitted and
`images_skipped` says why. The checks do not depend on the images.
