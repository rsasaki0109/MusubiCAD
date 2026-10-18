# ADR-109: Reviewed native trim and extend controls

Status: Accepted for MCAD-P17-010; completion requires qualifying main merge
Date: 2026-10-17
Roadmap: MCAD-P17-010, after P17-021 / PR #159

## Decision

Expose the existing exact analytic trim/extend planner through the shared
revision-guarded SketchEdit gateway. Catalogue only stored authored lines/arcs
as targets and lines/circles/arcs as cutters in an admitted single-owner sketch.
Exclude rectangle helpers/derived edges, projected geometry, cubic controls,
ellipses, foreign sketches and unsupported owners/workplanes. Use current strict
SI source resolution established by ADR-108. Geometry/intersection admission and
the existing 1 nm contact tolerance stay unchanged.

Add read-only intersection discovery, available through Tauri, Agent and MCP.
It validates source identity/revision/ownership and returns all backend analytic
intersections and a deterministic globally free endpoint ID. An internal planner
probe may request index zero solely to obtain the intersection list; its patch
and selected index are discarded. Discovery neither chooses an accepted branch
nor solves/applies a candidate. No frontend intersection solver or nearest-click
heuristic may choose a cut.

The user explicitly selects target, cutter, mode, target end and intersection,
and any existing constraints to remove. Offer accessible source dropdowns,
canvas target/cutter selection and backend-position intersection markers. Never
infer constraint removal or delete old shared endpoints. A curve edit is the
sole operation in its typed form batch; the existing planner expands it into
explicit removals, a fresh endpoint and a same-kind target replacement.

Review uses the existing dry-run, solved candidate, assertions, profile/consumer
binding and native solid/display verification. Additionally verify that the
actual solved new endpoint is within 1 nm of the chosen original intersection:
the generic editor verification alone does not evaluate arbitrary patch expected
effects. Retained constraints must not silently move the cut. Present the actual
solved endpoint, before/after closed loops and consumers, and verified solid
consequences (including no solid). A consumed loop may not disappear or rebind.
No implicit consumer repair, suppression, topology rewire or old-point deletion.

Apply rebuilds/rechecks from current source and uses the same atomic DesignPatch,
transaction, file re-read/publication and opaque Undo/Redo boundary. Source/file,
target/cutter/mode/end/removal/branch, tool/view and upstream changes, Escape,
discard and late real IPC invalidate old discovery/review. Candidate-only IDs
never enter source target catalogues. Preserve all unrelated expressions, IDs,
constraints, flags, owners, parameters and workplane associations.

## Acceptance

- Explicit line/circle and line/arc branches, trim/extend and either end; strict
  source expressions, globally fresh endpoint IDs and exact expanded operations.
  Discovery/review are read-only. Retained constraints that move the new endpoint
  refuse, while explicitly removing the offending constraint permits the cut.
  Reject missing/same/unsupported/helper/foreign targets, bad indices/removals,
  stale identity/revision, collisions and contradictory/assertion failures.
- Genuine analytic Mock and native OCCT: an expression-backed construction guide
  crossing a 10 mm circle at y=3 mm has x=-6/10 mm intersections. The consumed
  6 mm cylinder remains V=600*pi mm3, A=320*pi mm2 with independent XY/YZ/XZ
  and tilted-datum bounds/COM and oriented closed meshes. An unconsumed closed
  rectangle may become an open draft (one to zero profiles); the same edit of a
  consumed loop refuses without source/history/publication changes. Do not use
  synthetic Mock mass or admit arbitrary new trimmed polygons as physical solids.
- Packed/expanded source round trips, canonical Undo/Redo, no prefix publication,
  actual MCP/Agent parity, exact endpoint/old shared-point/source intent plus
  independently graded fresh native SI output. Equal-volume wrong placement,
  wrong branch or an equivalent unconsumed source rewrite must fail grading.
- Six or more new unconditional native Linux Tauri and Windows WebView2 groups
  after the existing 50: source picks, discovery/explicit branch and removals,
  read-only review/apply/history, profile/solid consequences/refusal and complete
  cancellation/stale/late-IPC/upstream lifecycle. No mocked IPC or manufactured
  geometry/results. Example, API/architecture, MCP guide/skill and feasible eval.
- All four mandatory pre-push local checks, locked Tauri build and the complete
  applicable exact-head CI inventory, then one merge-commit PR from clean main.

Analytic physical tolerances: volume 1e-12 m3, area 1e-10 m2, positions 3e-7 m;
display meshes 1% volume, 1e-4 m bounds/COM, 1 nm oriented closure. No new source
schema, migration, sampler, kernel admission, solver rule or golden changes.

Only a qualifying main merge may earn GUI 55 to 56, weighted CAD 54.10 to 54.25
(about 54%); agent operation stays 95. Credit covers the new usable selected
trim/extend and explicit branch/removal/review lifecycle. Existing analytic
planner, raw patches, geometry/solver/workplanes and host coverage receive no
repeated credit. The original fixed rubric/weights remain unchanged.
