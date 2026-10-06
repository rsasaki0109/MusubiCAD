# FreeCAD differentiation plan: Intent Integrity

Status: active; MCAD-P6-001 complete

Research date: 2026-08-23

## Product thesis

MusubiCAD should not try to outgrow FreeCAD's workbench count, mature geometry
surface, or add-on ecosystem. Its defensible wedge is **Intent Integrity**:

> Every accepted edit has a deterministic, reviewable proof of what changed,
> why it changed, what regenerated, which references were rebound, and whether
> the engineering intent still holds.

FreeCAD 1.0 introduced important topological-naming improvements. The official
documentation is explicit, however, that the algorithm cannot resolve every
ambiguity and that datum-based modelling practices remain necessary. MusubiCAD
must therefore avoid claiming that OCCT topology can be made intrinsically
stable. It should make ambiguity observable and fail closed before an incorrect
result becomes accepted document state.

## Evidence from FreeCAD primary sources

| Observed problem | Primary evidence | Opportunity for MusubiCAD |
|---|---|---|
| Topological names can change after booleans, fillets, and other operations, breaking or incorrectly recomputing dependent PartDesign and TechDraw objects. FreeCAD 1.0 mitigates but does not eliminate this class. | [FreeCAD topological naming documentation](https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/Topological_naming_problem.md) | Treat semantic identity, provenance, ambiguity, and accepted rebinds as first-class Design Graph data. Never silently accept an ambiguous match. |
| TechDraw dimensions remain vulnerable to topology changes, and official guidance recommends linking dimensions late in the drawing process. | [TechDraw LinkDimension limitations](https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/TechDraw_LinkDimension.md) | Bind drawing annotations to semantic design intent and validate them on every regeneration, not to projected edge indexes. |
| A current Assembly report shows a cold recompute collapsing previously saved slider positions even when no joint property changed. | [FreeCAD issue #31855](https://github.com/FreeCAD/FreeCAD/issues/31855) | Separate authored assembly state from solved output, compare pre/post solve intent, and reject unexplained state movement transactionally. |
| A current Sketcher report describes 5–10 minute delays for unchanged UI actions on a roughly 4,000-constraint sketch. | [FreeCAD issue #27319](https://github.com/FreeCAD/FreeCAD/issues/27319) | Make dirty-subgraph evaluation real, prove no-op edits make zero solver/kernel calls, and publish reproducible latency budgets. |
| Expressions are tracked at object granularity, cyclic cases need workarounds, document-wide variables are spreadsheet aliases, and there is no built-in expression manager. | [FreeCAD expressions documentation](https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/Expressions.md) | Expose a typed ParamGraph with property-level dependencies, units, cycles, impact preview, and one inspector shared by Desktop, CLI, and Agent API. |
| FCStd is a compressed compound file containing XML, GUI state, thumbnails, and optional B-Rep payloads. | [FreeCAD FCStd format documentation](https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/File_Format_FCStd.md) | Keep `.ocad.d` canonical, deterministic, split by semantic concern, and mergeable without committing disposable B-Rep or viewport state. |

These examples are not a claim that every FreeCAD model exhibits every problem.
They identify architectural seams where MusubiCAD can offer a stronger,
testable contract.

## North-star user contract

For any proposed parameter, sketch, feature, drawing, or assembly edit, the same
Desktop, CLI, and Agent API operation must answer:

1. What semantic inputs changed, with explicit units?
2. Which exact Design Graph nodes are affected, and why?
3. Which nodes were actually solved or regenerated?
4. Which semantic topology references remained exact, were rebound, became
   ambiguous, or were lost?
5. Which authored design assertions passed or failed?
6. What mass, bounds, DOF, interference, and drawing outputs changed?
7. Can the edit be committed, and can it be reproduced byte-for-byte?

The operation must not replace accepted document state when any required answer
is unavailable or violates a declared assertion.

## Phase 6 work packages

### MCAD-P6-001 — Regeneration trace and impact preview

Status: complete (2026-08-23)

Add a serializable `ChangeImpact` and `RegenerationTrace` contract shared by
dry-run and apply. It records changed inputs, dirty nodes, execution order,
solver/kernel call counts, elapsed time in milliseconds, output hashes, and
skipped nodes. CLI and Agent API expose `impact` and `trace`; Desktop consumes
the same DTO.

Acceptance:

- the 22-node actuator tower edit predicts the exact affected suffix before
  execution;
- dry-run and apply return the same ordered trace apart from explicitly labelled
  cache/timing fields;
- a no-op patch reports no dirty nodes and performs zero geometry-kernel calls;
- trace ordering and output hashes are deterministic in repeated tests.

### MCAD-P6-002 — Incremental, content-addressed regeneration

Connect existing dirty propagation to the feature pipeline. Reuse an output only
when a versioned key over canonical feature input, upstream output identity,
parameter values, tolerances, kernel backend, and kernel contract version is an
exact match. Cache data remains disposable and outside `.ocad` source truth.

Acceptance:

- editing `upper_hub_height` in the flagship model does not execute unrelated
  upstream nodes;
- changing `bolt_circle_radius` invalidates the PCD and every true downstream
  consumer, without re-executing the base and hubs;
- cached and clean full regeneration produce equivalent mass, bounds, semantic
  references, and canonical trace hashes within named tolerances;
- failed incremental regeneration preserves the previous document and cache
  generation; cold regeneration remains an always-available correctness oracle;
- benchmark fixtures cover 22, 100, and 250 feature nodes with checked-in latency
  and call-count budgets. CI gates call counts and determinism, not noisy wall
  time alone.

### MCAD-P6-003 — Fail-closed semantic reference provenance

Extend semantic reference resolution with a review DTO that distinguishes
`exact`, `derived`, `fingerprint`, `ambiguous`, and `missing`. Record the source
feature, intended semantic role, candidate set, tolerance policy, and reason for
the selected result. An ambiguous or missing required reference blocks commit;
automatic repair is a proposed DesignPatch, never hidden mutation.

Acceptance:

- adversarial reorder, split, merge, fillet, chamfer, and pattern fixtures cover
  exact, recoverable, ambiguous, and missing outcomes;
- equal candidates fail deterministically rather than choosing by incidental
  kernel order;
- drawing dimensions and assembly mates consume the same reference status;
- review artifacts visually highlight every rebound candidate and show the
  resolution reason.

### MCAD-P6-004 — Executable design assertions

Add unit-explicit, serializable assertions to the Design Graph. Initial bounded
types are parameter range, mass range, bounding-box range, minimum wall/clearance,
expected body count, required semantic reference, assembly DOF, and interference
limit. Assertions run during patch dry-run and regeneration and are never hidden
inside UI code or external scripts.

Acceptance:

- the actuator example checks mass, shaft/bearing references, mounting-hole
  count, and overall bounds;
- an edit that regenerates valid B-Rep but violates an assertion is rejected
  without changing the document or history;
- every assertion carries units, tolerance, severity, and a stable ID;
- CLI, Desktop, Agent API, review HTML, and GitHub summary show identical results.

### MCAD-P6-005 — Git-native semantic merge productization

Productize the existing semantic three-way merge and patch rebase primitives as
`musubicad merge-driver`, `musubicad conflicts`, and a documented Git attributes
workflow. Conflicts are stable-ID parameter/feature/sketch/assembly/drawing
conflicts, not JSON line conflicts. Geometry is regenerated only after the
merged Design Graph validates.

Acceptance:

- independent edits to different parameters and features merge automatically;
- same-target edits produce deterministic base/ours/theirs conflict records;
- conflict resolution is itself a DesignPatch with a revision precondition;
- merge order does not affect canonical `.ocad.d` bytes;
- a GitHub Actions fixture demonstrates branch, merge, semantic review, and
  regenerated evidence without committed B-Rep.

### MCAD-P6-006 — Unified intent and dependency inspector

Build one backend query surface for parameters, expressions, sketches, features,
semantic references, assertions, dirty propagation, regeneration trace, and
failure causes. Desktop presents it as an Intent Inspector rather than creating
separate modelling workbenches. CLI and Agent API retain complete parity.

Acceptance:

- selecting a parameter answers both "what drives this?" and "what will this
  change?";
- selecting a face, drawing dimension, or assembly mate reveals its semantic
  reference provenance and consumers;
- a failed regeneration points to the first failing node and preserves usable
  upstream results for inspection while leaving document state unchanged;
- the flagship example has a scripted 60-second inspection and repair scenario.

## Delivery order and gates

| Order | Work package | Dependency | Exit gate |
|---|---|---|---|
| 1 | P6-001 trace and impact | Existing graphs and dry-run/apply parity | One authoritative observability DTO |
| 2 | P6-003 reference provenance | P6-001 trace; existing TopoRef | Ambiguity is visible and fail-closed |
| 3 | P6-004 design assertions | P6-001; P6-003 for reference assertions | Valid geometry can still be rejected for violated intent |
| 4 | P6-002 incremental regeneration | P6-001 provides correctness/call-count oracle | Faster evaluation with cold-regeneration equivalence |
| 5 | P6-005 semantic merge | P6-004 makes merged intent verifiable | Git collaboration works at design semantics |
| 6 | P6-006 intent inspector | All shared DTOs above | One coherent human/CLI/agent workflow |

P6-001 should be the next implementation task. P6-002 is deliberately after the
trace and fail-closed contracts: optimizing regeneration before observability
would make stale-cache errors difficult to prove or diagnose.

## Competitive scorecard

The phase is successful only if MusubiCAD can demonstrate all of the following
on committed fixtures:

| Metric | Phase 6 target |
|---|---|
| Silent incorrect reference acceptance | Zero in the adversarial reference suite |
| No-op solver/kernel executions | Zero |
| Dirty-node prediction accuracy | Exact node-set match on all golden patches |
| Failed edit persistence | Source and history byte-for-byte unchanged |
| Semantic merge determinism | Identical canonical bytes for equivalent merge order |
| Surface parity | Every new mutation/query available through Desktop, CLI, and Agent API |
| Review completeness | Intent, impact, references, assertions, geometry effects, and trace in one artifact |

## Explicit non-goals

- Matching FreeCAD's complete Part, BIM, CAM, FEM, Surface, and add-on breadth.
- Claiming that any naming algorithm eliminates OCCT topology ambiguity.
- Persisting cached B-Rep, meshes, solver state, or viewport state as source truth.
- Letting an AI agent automatically accept ambiguous reference repair.
- Introducing NURBS, loft, sweep, shell, or draft under this phase; each remains
  subject to MCAD-P5-006 and its own ADR.

## Risks

- Content-addressed cache keys can be incomplete. Cold regeneration and trace
  equivalence are mandatory correctness oracles.
- Fail-closed reference handling may initially reject edits a heuristic system
  would accept. The review/repair workflow must make rejection actionable.
- Assertion design can become an unrestricted scripting language. Phase 6 starts
  with a closed, serializable set of typed assertions.
- Benchmarks are hardware-sensitive. CI gates deterministic call counts and
  complexity bounds; wall-time numbers are informational unless run on a pinned
  environment.
