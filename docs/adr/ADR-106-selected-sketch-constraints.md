# ADR-106: Selected sketch constraints and backend solve diagnostics

Status: Accepted with MCAD-P17-008 qualifying main merge
Date: 2026-10-17
Roadmap: MCAD-P17-008, after P17-007 / PR #156

## Decision

Add usable native controls for existing horizontal/vertical line constraints,
coincident/distance point pairs, circle radius and explicit constraint removal.
Expose a backend-owned, source-bound target catalogue and fresh suggested
`con:` ID in each disposable sketch editor row. Points exclude cubic controls;
lines and circles use the existing authored-target admission. All actual
mutations still use the existing typed SketchEdit, guarded DesignPatch,
verified plan/apply, file publication and opaque history. No new mutation tool,
source entity, constraint kind, schema, geometry admission or sampler.

The canvas selects declared source targets; controls offer the backend catalogue
and unit-bearing length expressions. Two point choices are explicit. The UI
does not infer an intersection, edit derived helpers, compute DOF, solve geometry
or write source. Labels and a selection overlay are disposable presentation.
Show actual backend DOF in inspection and successful candidate review. Preserve
legacy raw forms and all existing pointer tools.

Cancelled reviews, Escape, target/control/sketch/tool/view/file/upstream changes
and late real IPC must not restore an old review. New controls share the existing
review sequence and apply handler, so they cannot bypass assertions, consumer
profiles, native geometry verification or publication/history guards.

Keep the solver's existing implicit first-point anchor and rank, tolerance,
redundancy, contradiction and convergence policy. In the shared solved-copy
boundary retain the actual runtime SolveStatus long enough to explain a
refusal as contradictory, redundant or nonconvergent. Keep the existing error
prefix and validation class. Do not expose unlabeled mixed residual numbers,
infer a reason in JavaScript, admit a failed candidate, or change persisted
SolveState. Underconstrained states can coexist with redundant equations under
existing policy; do not relabel them or invent a redundancy count.

Solve all admitted disposable sketch copies before resolving regenerated datum
frames. This preserves actual contradiction/redundancy diagnostics when a
dependent solid cannot regenerate; it does not admit failed candidates or
change solver classification.

Use fresh IDs for additions and exact existing IDs for removal. The current
DesignPatch tombstone forbids removing and reusing a constraint ID in one
patch. Preserve it. These controls add/remove constraints; they do not promise
in-place dimensional-value replacement or associative dimension dragging.

## Acceptance

- A source circle and construction guide share first point A=(2,-3) mm;
  B=(26,15) mm, circle initial radius 5 mm. Backend remaining DOF starts at 3.
  Horizontal, point distance 20 mm and radius 10 mm reduce it to zero;
  positive-branch solved B=(22,-3) mm. Preserve original source coordinates,
  radius expression, owner/datum and unrelated source. Test vertical and
  distinct-point coincidence separately without collapsing a consumed profile.
- Genuine analytic Mock (`with_analytic_primitives`) and native OCCT: circle
  prism depth 6 mm, radius 10 mm, volume 600*pi mm3, surface 320*pi mm2.
  XY COM=(2,-3,3) mm, bounds=(-8,-13,0)..(12,7,6) mm; +5 mm datum adds 5 mm
  to z. Test absolute XY/YZ/XZ and tilted placements. Native volume tolerance
  1e-12 m3, area 1e-10 m2, analytic position 3e-7 m; real meshes 1% volume,
  1e-4 m position/COM and 1 nm oriented geometric closure. Default synthetic
  Mock IDs are not physical evidence. No default Mock admission expansion.
- Read-only plan, atomic add/remove/Undo/Redo and packed/expanded round trips;
  unit/range/type/cubic/helper/owner/identity/revision/late-ID/assertion/profile
  refusal; full source/history/publication preservation. Contradictory radii
  refuse; redundant constraints on a fully constrained fixture refuse with
  the actual backend reason. Never loosen a previous acceptance test.
- Actual new Linux Tauri and Windows WebView2 selection/control/review/apply,
  DOF/diagnostic, cancel/late real IPC/upstream stale and history acceptance.
  Both drivers append the new checks unconditionally after existing 38 groups;
  workflow path policy includes their helper. No fake IPC, input properties,
  solve states, catalogue, mass or exported geometry.
- Actual same-intent MCP/Agent parity and independent intended-source plus
  fresh native SI physical grading. A direct radius overwrite can produce the
  right volume but the wrong constraint source and must fail. Equal-volume
  translated outputs must fail. Example, API/architecture, authoring guide,
  skill and feasible eval updates; all four local mandatory checks and complete
  applicable exact-head CI inventory.

Only the qualifying main merge earns partial GUI 53 to 54, weighted CAD
53.80 to 53.95 (about 54%); agent operation remains 95. Credit is the usable
source-selected constraint lifecycle and backend diagnostic presentation.
Existing solver/constraint/geometry/raw-form/workplane/host capabilities receive
no repeat credit. Original fixed rubric and weights remain unchanged.
