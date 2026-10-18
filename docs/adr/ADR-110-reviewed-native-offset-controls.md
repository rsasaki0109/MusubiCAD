# ADR-110: Reviewed native independent sketch offsets

Status: Accepted for MCAD-P17-011; completion requires qualifying main merge
Date: 2026-10-18
Roadmap: MCAD-P17-011, after P17-010 / PR #160

## Decision

Expose ADR-052's existing analytic offset planner through a separate shared
desktop command because an offset creates a new sketch and its authored owner.
Require current document identity/revision, one admitted single-owner source
sketch, an explicitly ordered list of stored ordinary line/arc IDs or one circle,
a positive constant length with explicit units, an explicit outward/inward side
and a nonempty destination name. No inferred contour order, reversal, join repair,
constraint copying, new geometry or associative offset dependency is admitted.
Reuse strict current source evaluation from ADR-108 and the existing planner's
256-curve, convex CCW/tangent-join and 1 nm contact/collapse admission.

The backend catalogues eligible authored sources, excludes helpers/projected/
cubic/ellipse/construction geometry and assigns deterministic globally free
destination sketch, owner, point and curve identities. Review returns these IDs,
the concrete revision-guarded AddSketch/AddSketchEntity/AddFeature DesignPatch,
solved local SI curves, retained symbolic workplane, closed profiles and verified
whole-document solid consequences. The original sketch, constraints, expressions,
flags, consumers and parameters remain untouched. Copy coordinates are literals;
later source-parameter edits do not change them. A shared datum association still
moves both sketches with its source datum; this is not curve association.

Review is read-only. Apply recomputes deterministic IDs and the complete candidate
from the same current revision, verifies solves/assertions/profile bindings/native
solids/display, applies through transactions and opaque history, and re-reads the
file before publication. Refusals preserve source, history and files. No client
supplied patch or generated identity may replace the reviewed intent.

Native controls offer accessible ordered source selection and canvas picks,
explicit distance/side/name, removal/reordering of the selected list, candidate
preview and a separate Apply. Keep source catalogues separate from the candidate.
Source/file/tool/view/selection/parameter/form changes, another command, Escape,
discard and late IPC invalidate the review through the common sketch sequence.
Tauri, Agent and MCP call the same gateway; UI owns disposable intent/view only.

## Acceptance

- Independent outward/inward circle and ordered rectangle dimensions, strict
  expressions resolved from current source, generated IDs/owner and symbolic
  global/datum planes. Original source/constraints/consumer bindings are exact.
  Literal copies remain unchanged after source parameter edits.
- Genuine analytic Mock and native OCCT independent placed circle/prism volume,
  area, COM, bounds and oriented closed meshes; no synthetic Mock mass credit.
  Existing tangent line/arc admission is reused without introducing a sampler.
  Reject open/reordered/reversed/concave/unsupported/construction/helper/foreign
  selections, zero/unitless/negative/nonfinite/collapsing distance, stale identity
  or revision, bad owners/workplanes, contradictory source and required assertions.
- Read-only review, packed/expanded source round trips, canonical Undo/Redo,
  failure atomicity, actual MCP/Agent parity and independent fresh SI/source eval
  with equal-volume wrong-side/placement/association/source-rewrite adversaries.
- Six or more new unconditional actual Linux Tauri and Windows WebView2 groups
  after the existing 56: source/canvas ordering, explicit sides, candidate IDs,
  review/apply/history, independent copy, refusals and stale/late/cancel lifecycle.
  No mocked IPC or geometry. Runnable example, API/architecture, MCP guide/skill
  and a feasible independently graded eval are mandatory.
- All four pre-push checks, locked Tauri build and complete applicable exact-head
  CI inventory precede one merge-commit PR. No schema/migration/golden change.

Physical tolerances: volume 1e-12 m3, area 1e-10 m2, positions 3e-7 m;
display meshes 1% volume, 1e-4 m bounds/COM and 1 nm oriented closure.
Only the qualifying main merge earns GUI 56 to 57 and weighted CAD 54.25 to 54.40
(about 54%) for usable selected offset controls and their reviewed lifecycle.
Agent operation remains 95. Existing planner/kernel/solver/workplane/host coverage
receives no repeated credit; the original fixed rubric and weights are unchanged.
