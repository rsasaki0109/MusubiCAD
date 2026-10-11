# ADR-039: Existing-body Boolean features require nonempty output

Status: Accepted  
Date: 2026-10-11  
Roadmap: MCAD-P11-006

## Decision

A `body_boolean` Feature Graph definition explicitly names `source_feature`,
`target_feature` and operation (`union`, `subtract`, `intersect`). Subtraction
means target minus source. Operands must be distinct existing solid-producing
features; graph derivation rejects sketches/datums, missing IDs and cycles.
Suppressed inputs have no available body and fail regeneration. No new body
ownership or implicit tool selection is introduced; upstream outputs remain
part of their existing graph branches.

The existing low-level Boolean kernel operation deliberately permits empty
intersections for interference queries. Introduce `body_boolean` at the same
kernel-neutral boundary: it delegates to that operation, and OCCT additionally
requires finite positive output volume in m³. Complete subtraction and empty
intersection cannot become a solid feature. No fixed minimum volume or sampling
method is introduced. Mock delegates deterministically without claiming physical
geometry evidence. Counting kernels forward the stricter operation, so feature
regeneration and verified Agent paths cannot lose the nonempty-output check.

All edits use DesignPatch and transactions. Failed verified execution does not
persist the candidate; failed model regeneration restores previous outputs.
Exact OCCT derivation history currently follows the target operand. References
to source-derived geometry must use existing verified geometric provenance;
missing/ambiguous references remain failures. Disconnected nonempty results
retain the existing kernel compound representation, without body splitting UI.

## Compatibility and evidence

This is an additive tagged definition in both document/patch schemas. Migration
never combines old bodies or infers operands, and all legacy examples retain
round-trip/authoring rebuild coverage. No existing golden changes are needed.
An 80 × 60 × 6 mm plate and 30 × 20 × 8 mm tool at (20,10,0) mm have overlap
3600 mm³; union/subtract/intersect volumes are 30000/25200/3600 mm³.
OCCT tests check those volumes to 1e-11 m³ and bounds to 1e-6 m. Mock tests check
operation/dependency plumbing. Additional evidence covers operand edits and
cache reuse (after initial expanded-sketch diagnostic normalization), graph
validation, suppression rollback, empty results, verified Agent failure without
persistence, a patch-authored example, documentation/skill and eval checker.
Physical host validation beyond Linux remains separate work.
