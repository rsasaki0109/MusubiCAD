# ADR-026: Verified patch apply

Status: Accepted  
Date: 2026-10-06  
Roadmap: MCAD-P8-001

## Context

`patch_dry_run_document` and `patch_apply_document` validated a DesignPatch
against the Design Graph but did not regenerate geometry. A valid parameter
edit can still leave no solid: on `examples/bearing_carrier.ocad.d`, setting
`param:bore_diameter` from `18 mm` to `200 mm` passed dry-run and apply, and
only the next `regen_document` failed with
`OCCT error: Expected exactly one resulting Solid, got 0`. The error did not
name a feature. The graph stayed consistent and undo worked, but an agent
could commit a change that destroys the part without noticing.

`musubicad review` already regenerated both sides and checked declared
`expected_effects`, but it also rendered images and therefore needed a GPU,
so it was not a usable gate for every edit.

## Decision

1. Split the render-free half of `review` into `verify_patch`: validate,
   apply to a copy, regenerate before and after, check required assertions,
   assembly interference counts, and declared `expected_effects`, and build
   the geometric diff. It needs no GPU and never writes.
2. `patch_dry_run_document` adds a `verification` object when validation
   reports no errors.
3. `patch_apply_document` runs the same verification first and refuses the
   patch, leaving the document unchanged, unless it passes. The error starts
   with `patch rejected; the document was not changed:`.
4. Regeneration errors name the failing feature
   (`feature '<id>': <kernel message>`) through
   `OpenCadError::with_context`, which keeps the error kind.
5. A document that did not regenerate before the patch is reported in
   `geometry.before_regen_error`, not rejected, so a patch can repair it.
   A part that regenerates without a solid yet (only sketches) verifies
   against an empty scene, so staged authoring keeps working.
6. `verify: false` (default `true`) skips the geometry gate for a staged step
   known not to regenerate yet. Validation still runs.

MCP inherits the behaviour because its `patch_dry_run` and `patch_apply`
tools delegate to these Agent API methods (ADR-014). `musubicad patch`
(without `--dry-run`) runs the same gate before writing, so agents that use
the CLI instead of MCP get the same guarantee; `--no-verify` opts out.

CAD plugin invocations (`plugin_invoke`, ADR-010) are not gated yet.

## Consequences

- Dry-run and apply cost two regenerations plus tessellation. On the bearing
  carrier the full MCP session of dry-run and apply took about 0.5 s.
- Agents can declare intent as `expected_effects` (for example
  `mass_delta_kg`) and rely on apply to enforce it.
- Callers that relied on applying a non-regenerating patch must pass
  `verify: false`.
- Golden review artifacts are unchanged: `before_regen_error` is omitted when
  absent.
