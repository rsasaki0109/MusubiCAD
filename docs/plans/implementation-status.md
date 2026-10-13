# MusubiCAD implementation status

Last reviewed: 2026-10-12

This inventory is evidence-based against the current repository. **Implemented**
means the capability has code and focused tests; **Partial** means the primary
path exists but an explicit contract or integration is still missing; **Stub**
means the public area is reserved but not implemented. The active follow-up IDs
are defined in the [canonical roadmap](roadmap.md).

| Area | Status | Current evidence | Remaining work |
|---|---|---|---|
| Parent-context flexible constraints (MCAD-P13-012) | Complete upon PR merge | ADR-061; exact connector scopes/owners, analytic distance/angle solve, explicit angular branch, bounded unique coordinates/coupled followers, fully bound DOF and shared physical leaves; DesignPatch/schema/migration/history, Mock/OCCT analytic fixtures, actual MCP, examples/guide/skill/eval | Fixed outer instances and immediate root-child drivers only; unsupported nonlinear/deeper/free scopes and motion/reach/URDF/HTML refuse; Mock exact mesh intersection/inertia refuse |
| Independent articulated copies (MCAD-P13-011) | Complete upon PR merge | ADR-059/060; strict parent-owned coordinates, source inspection/snapshots, scoped physical leaves, consumer gates, DesignPatch/history/schema round trips, both-kernel analytic example, guide/skill/eval | Parent-context constraints remain P13-012; full hierarchical DOF/motion/reach/URDF/HTML and nested articulated/pattern scopes refuse; Mock exact mesh intersection/inertia refuse; no maturity credit |
| Coupled gear/rack joints (MCAD-P13-007) | Implemented | ADR-056; source joint ID relations, signed dimensionless/SI coefficients, deterministic DAG and range/owner validation, DesignPatch/authoring/history/schema migration with legacy-reader wire refusal; shared read-only unit-bearing pose API, derived follower controls, both-kernel analytic bounds/centres/volume and native internal-moving-pair collisions, gear URDF mimic, example/guide/skill/eval | Sampled independent-driver grid only; no inferred phase/tooth contact; rack URDF and coupled HTML refused; Mock exact imported-solid Boolean support remains absent (bounded member compounds added by ADR-060); generic mate DOF estimate unchanged |
| Exploded assembly presentations (MCAD-P13-006) | Implemented | ADR-055; stable named views and explicit SI offsets, DesignPatch add/set/remove, schemas/migration/history, both-kernel analytic translated mesh bounds and volume, read-only deterministic PNG preview/export via CLI/Agent/MCP and desktop backend, rigid-child physical report invariance, example/guide/skill/eval | Top-level declared instances only; child assembly moves as one compound; internal paths, pattern targets, rotation/spacing, exploded drawings and mechanical-format exports deferred; f32 render precision |
| Core IDs, units, validation, and document metadata | Implemented | `modules/core/src/id.rs`, `units.rs`, `validation.rs`, `document.rs` | Keep public values unit-explicit; extend only through the roadmap |
| Core transaction primitive | Implemented | `modules/core/src/transaction.rs` tracks successfully applied actions, rolls them back deterministically on apply/commit failure, and covers failure tests; `modules/file/src/history.rs` adds serializable full-document backend history with stale-snapshot validation | Keep public transaction/history contracts deterministic |
| Design Graph, ParamGraph, Feature Graph, and semantic diff | Implemented | `modules/graph/src/` and graph tests | Preserve deterministic traversal while adding consumers |
| Kernel-neutral geometry and Mock backend | Implemented | `modules/geometry/src/kernel.rs`, `transform.rs`, `topology.rs`, `refs.rs`, `topo_sync.rs`, `provenance.rs`, and tessellation modules provide `TopoRefIdentity`, explicit unit-labelled fallback policy, deterministic tie-breaking, and migration-compatible reference resolution; `ReferenceProvenance` classifies every face/edge resolution as exact/derived/fingerprint/ambiguous/missing with scored candidates and fails closed for required ambiguous/missing references (MCAD-P6-003); focused geometry tests cover identity, fallback, policy validation, ordering, provenance statuses, and ambiguity determinism; ADR-012 gates future NURBS editing and kernel operations | Preserve the reference contract; require the future-geometry admission gate before expanding editable geometry |
| OCCT backend integration | Implemented | `modules/kernel-occt/src/backend.rs`, cadrum-backed integration tests | Track ABI and topology behavior in Phase 1/5 evidence |
| Sketch data model and basic solver | Implemented | `modules/sketch/src/constraint.rs`, `solve.rs`; Coincident, Horizontal, Vertical, distance/radius, Equal line/circle/arc targets, Parallel, and Perpendicular paths with normalized direction residuals; deterministic rank-based DOF/redundancy, contradiction, and non-convergence diagnostics in `modules/solver/src/diagnostics.rs`; canonical mixed-constraint fixture and repeated-solve coverage in `examples/sketch_constraints_regression.ocad.d` and `modules/file/tests/sketch_regression.rs` | Preserve fixture/checksum stability while extending supported constraints |
| Construction geometry editing (MCAD-P13-001) | Implemented | Explicit `set_sketch_construction` preserves stored entity/constraint IDs, solver participation, profile exclusion, semantic diff/change impact and rebase conflicts; consumer alias identity refuses implicit rebinding; Mock/native analytic evidence, history/round trips, parameter regression, example/guide/skill/eval; ADR-050 | Existing empty/construction-only sketch admission rules remain; no new kernel operation or GUI surface |
| Analytic sketch trim/extend (MCAD-P13-002) | Implemented | Pure solved-copy planner for line/arc targets and finite line/circle/arc cutters; fresh endpoint IDs, explicit selection/constraint removal, revision-guarded DesignPatch with solved-point postcondition; native sector volume, Mock neutral-profile evidence, history/round trips, example/guide/skill/eval; ADR-051 | No circle targets, tangent/coincident cuts, rectangle-derived edits or inferred cutter association; closed boundaries require explicit closing/rebinding, and retained constraints can refuse edits |
| Feature regeneration | Implemented | `modules/feature/src/` covers sketch, extrude, hole, revolve, fillet, chamfer, and patterns; `RegenerationTrace` records deterministic order, solver/kernel calls, explicit millisecond timing, and logical output hashes; `RegenReport.reference_provenance` classifies every semantic reference against the final topology; `PartModel::regenerate_with_cache` reuses unchanged outputs via a versioned content-addressed `RegenerationCache` with zero kernel calls for cached nodes and failed-run output restoration; OCCT regressions prove `upper_hub_height` and `bolt_circle_radius` edits re-execute only the affected downstream suffix; `robot_joint_actuator_housing()` composes 22 deterministic nodes | Keep the content key contract in sync with kernel contract changes |
| `.ocad` and expanded `.ocad.d` format | Implemented | `modules/file/src/`, schemas, canonical JSON, checksums, migration and round-trip tests | Keep schema changes synchronized with migrations and fixtures |
| DesignPatch and Agent API | Implemented | `ChangeImpact` is returned by dry-run/apply; document-backed prediction maps changed parameter/feature/reference input to an exact topological dirty suffix. Revision guards, atomic candidate validation, history, rebase, and CLI/Desktop/Agent transport remain shared; the design review surfaces fail-closed `ReferenceProvenance` per reference with scored candidates, resolution reasons, and executable assertion results | Keep the shared dry-run/apply/review evidence contract in sync with P6-005/006; extend structural operations (ADR-013, MCAD-P7-001) with further object kinds only through ADR-013's rules; MCAD-P7-001 structural authoring, typed structural rebase conflicts, and whole-state semantic merge are delivered |
| Executable design assertions | Implemented | `modules/core/src/assertions.rs` defines serializable, unit-explicit `Assertion`s (parameter range, mass range, bounding-box limit, body count, required semantic reference, assembly DOF, interference limit) with `required`/`advisory` severity; `.ocad.d` persists them as `graph/assertions.json`; `opencad_ai::evaluate_assertions` runs them against regenerated evidence and `RequiredReference` consumes P6-003 provenance; `musubicad regen` and the design review reject a change when a required assertion fails; actuator acceptance is covered by OCCT regression tests (MCAD-P6-004); assembly documents evaluate the same assertions from their regeneration (`assembly_assertion_context`: DOF, exact interference, mass, bounds, body count) in `musubicad regen` and `verify_patch`, and refusals name the failed assertion (MCAD-P10-001); `motion_interference_at_most` sweeps each joint through its limits and names the worst pose (MCAD-P10-002, ADR-033) | Combined multi-joint poses and pair exclusions for intentional contact; extend assertion kinds through the roadmap |
| Assembly model and regeneration | Implemented | `modules/assembly/src/`, ADR-003, `examples/assembly_two_brackets.ocad.d`; canonical root containment, document kind/identity checks, indirect-cycle detection, localized retryable failures, deterministic explicit-unit interference tolerances, and the OCCT assembly result in `fixtures/golden/mcad_p5_005_end_to_end.json` | Preserve the cross-artifact manifest when assembly evidence changes |
| Parametric construction datums | Implemented | `datum_plane`/`datum_axis` non-body features, parameter expressions, datum sketch extrusion placement and revolve axis references; deterministic dependencies/cache invalidation, OCCT analytic dimensions/volumes and Mock contracts; example, DesignPatch/file round trips, MCP guide/skill/eval (MCAD-P11-001, ADR-035) | Advanced constructors and datum/custom revolve profile planes |
| Drawing model and SVG export | Implemented | `modules/drawing/src/`, ADR-004, and `examples/bracket_front_view.ocad.d`; hidden-line extraction subdivides edges at projected occluder boundaries and depth crossings with explicit tolerances and deterministic ordering; `modules/drawing/tests/golden/partial-occlusion.svg` and the P5-005 cross-artifact test pin visible/hidden SVG output; semantic linear edge dimensions resolve required P6-003 provenance on regenerated final bodies, follow parameter edits, and refuse missing/ambiguous refs on every sheet in CLI/Agent/MCP regen, verified dry-run/apply/review and SVG output (MCAD-P10-003, ADR-034); exclusive wire forms retain legacy point dimensions; Mock/OCCT/MCP and migration tests plus the semantic bracket drawing example cover the contract | Add angle/radius/diameter dimensions and instance-qualified assembly bindings through the roadmap; preserve SVG/HLR goldens |
| Render and desktop preview | Implemented | `modules/render`, `modules/desktop`, reusable `run_desktop_smoke` with serializable summary, `modules/desktop/tests/desktop_smoke.rs`, `modules/desktop/tests/command_parity.rs`, Tauri shell under `apps/desktop`, unsigned `.github/workflows/desktop.yml` native matrix plus versioned bundle/checksum contract, and credential-gated `.github/workflows/desktop-signed-release.yml` with ADR-005 trust scope; local Windows/Linux checks and hosted runs `32612751044` and tagged `32616853187` prove all four native builds, versioned artifacts, independent checksums, and packaged Linux smoke as recorded in `docs/plans/desktop-release-evidence.md`; parameter edit/undo/redo use backend history and keep viewport state separate; the Intent Inspector shows where regeneration fails and what a parameter or picked reference drives, through the same backend queries as `musubicad intent` (MCAD-P6-006); CLI Feature-build animation renders non-tool body milestones against one final-shape camera for the 22-node robot-joint flagship; `musubicad animate-joints` renders assemblies through their joints on the CPU with one fixed framing (MCAD-P9-001), and `musubicad animate-sweep` regenerates a design across a parameter range with captioned frames (MCAD-P9-002); `studio` and `dark` presentation styles with a soft ground shadow and aspect presets (MCAD-P9-003); kernel-free reach and workspace queries with `musubicad reach` (MCAD-P9-004); a self-contained HTML joint viewer from `export *.html` (MCAD-P9-005); Agent API and MCP access to reach and both animations (MCAD-P9-006, ADR-032); a six-axis arm example with OCCT interference checks (MCAD-P9-007), narrowed joint limits and a required 78-pose motion assertion with regenerated URDF/STL, GIF and web viewer (MCAD-P10-005); MCP eval includes `robot_add_wrist_joint`, graded by assembly joint IDs, regenerated URDF, and reach residual (MCAD-P10-004) | Credentialed Authenticode/macOS publication is explicitly deferred; preserve the fail-closed path |
| CLI and release workflow | Implemented | `.github/workflows/ci.yml`, `release.yml`, desktop headless smoke gate, and separate desktop trust/release workflow; tagged run `32616853192` published four CLI archives plus checksums, and the downloaded Windows archive passed an independent version/OCCT smoke; tagged Desktop run `32616853187` published all four workflow artifacts with verified installer checksums | Credentialed desktop publication is explicitly deferred; unsigned verified artifacts remain supported |
| Plugin API | Implemented | `modules/plugin-api` provides versioned data-only contracts and a deterministic registry; `examples/plugin-example` is a separately buildable feature plugin with manifest/version/golden tests; CLI and Agent invocation use host-owned validation, transactions, history, and persistence; importer/exporter goldens and returned-error document-isolation tests are checked in | Dynamic loading and untrusted-code process isolation are explicitly deferred |
| Documentation and examples | Implemented | Architecture, ADRs, API docs, examples, the P5-005 evidence guide, and this roadmap exist | Keep status/evidence synchronized in future feature changes |
| Neutral-plane draft (MCAD-P11-002) | Implemented | `modules/feature/src/draft.rs`, Mock kernel contract and `modules/kernel-occt/src/draft.rs`; signed parametric angles, datum neutral plane, semantic side faces; OCCT analytic wedge volume/bounds and cache/rollback tests; patch-authored `examples/draft_plate.ocad.d`; ADR-036 | Convex planar solids only, initially perpendicular selected sides; curved/concave draft requires a future native kernel operation |
| Centerline ribs (MCAD-P11-003) | Implemented | `modules/feature/src/rib.rs` adds a straight-line rib feature that expands a sketch centerline into a rectangular profile, extrudes by distance, and unions with the target; focused Mock/OCCT tests and `examples/rib_plate.ocad.d` cover analytic volume and rollback; MCP guide, skill and eval task are updated | Curved ribs, taper, draft, and end conditions beyond distance extrusion remain future work |
| Standard holes and simplified internal threads (MCAD-P11-004) | Implemented | `standard_hole` validates circular profile diameter against M3–M12 ISO 273 clearance series or metric coarse nominal-minus-pitch tap drill; required explicit representation, derived SI nominal/pitch/cut dimensions, Mock/OCCT cylinder volume and refusal tests, DesignPatch/file round trips, example, guide/skill and eval; ADR-037 | Simplified drill cylinders only; physical/external/fine/inch threads, tolerance classes and countersinks/counterbores remain future work |
| Variable and semantic face fillets (MCAD-P11-005) | Implemented | Optional linear radius law with endpoint expressions, strict face/edge selection, kernel-neutral Mock/counting paths and private OCCT bridge; 65 fixed radius samples, analytic prism volumes and endpoint contact dimensions; example, DesignPatch/file compatibility, guide/skill/eval; ADR-038 | Straight single-edge variable fillets only; native approximation and geometry-provenance fallback after BREP transfer; physical host evidence outside Linux remains future work |
| Existing-body Booleans (MCAD-P11-006) | Implemented | Explicit source/target body features, derived dependency/type validation; union/subtract/intersect through kernel-neutral Mock/OCCT and counting paths; analytic box volumes/bounds, both-branch cache edits, graph/suppression rollback and empty-output verified Agent refusal; example, schemas/migration/round trips, guide/skill/eval; ADR-039 | Empty OCCT outputs refused; target operand exact face history only, source-derived geometry uses verified provenance; disconnected results follow existing kernel compounds; physical hosts beyond Linux unverified |
| Planar DXF input (MCAD-P12-009) | Implemented | Bounded explicit-unit/layer text parser into ordinary analytic sketch entities, verified DesignPatch/history with undo/redo; Mock/OCCT analytic profiles/volumes, refusal/isolation, example, guide/skill/eval; ADR-048 | Planar model-space LINE/CIRCLE/ARC/straight LWPOLYLINE only; fixed 1 nm point grid, literal coordinates without inferred constraints, no styles/blocks/splines/ellipses/binary format or recovered drawing semantics |
| Connected STL solid input (MCAD-P12-010) | Implemented | Explicit-unit ASCII/binary immutable asset feature through verified DesignPatch/history; bounded manifold/intersection admission, accurate Mock/OCCT mass/bounds/placement, cache source validation, opaque-asset undo/redo, schemas/migration/round trips, example, guide/skill/eval; ADR-049 | One connected closed outward shell only; 4 MiB/10k triangles/1M candidate pairs, 1 nm weld/10 nm OCCT sewing; no open scans, disconnected solids, cavities, repair or surface editing; Mock mesh edits/CSG/inertia refuse; bounded member compounds added by ADR-060 |
| Verified assembly BOM (MCAD-P12-008) | Implemented | Snapshot-backed recursive/pattern leaf counts and structured provenance, strict regeneration at every source, canonical JSON/CSV and protected atomic output; bounded OCCT compound transforms/flattening preserve separate members; Mock/OCCT analytic tests, patch-authored nested example, guide/skill/eval; ADR-047 | Quantities only; no material/cost/exclusion inference or live source subscriptions; copies at different canonical paths remain separate; 64-depth/10000-source/100000-occurrence bounds |
| Millimetre drawing DXF (MCAD-P12-007) | Implemented | Single-sheet AC1027 paper coordinates and UTF-8 primitives, explicit units/layers, analytic even-odd hatch clipping; Mock/OCCT dimensions/volume, deterministic/source-preservation tests, independent ezdxf audit, artifacts/guide/skill/eval; ADR-046 | Drawing output only; view scale applied, no DXF input, associative DIMENSION records or multi-sheet layouts; consumer fonts, existing mesh wireframe approximation and bounded text syntax remain |
| Vector PDF output (MCAD-P12-006) | Implemented | Canonical multi-page drawing PDF, precise SI paper sizes, embedded fixed-font vector text/shapes/hatches, strict missing-glyph/resource/numeric refusal; Mock/OCCT volume and page tests, process determinism, atomic output/source preservation, printable example, guide/skill/eval; ADR-045 | Generated drawing scenes only; bounded embedded DejaVu font coverage, no arbitrary images/filters/system font lookup or PDF/A certification; existing drawing layout limitations remain |
| Explicit title blocks (MCAD-P12-005) | Implemented | Authored title/number/revision/date/author/material/scale note, deterministic 180 × 48 mm frame; DesignPatch set/explicit clear, XML-safe SVG, analytic dimensions and Mock/OCCT unchanged geometry tests, schema/migration/round trips, example/guide/skill/eval; ADR-044 | Fixed template/font sizes and field budgets; no inferred metadata, wrapping, collision avoidance or template library; SVG font appearance depends on consumer |
| Bounded detail views (MCAD-P12-004) | Implemented | Same-sheet ordinary parent, rectangular projected-model crop, explicit magnification, validated parent/child scales; analytic clipping preserving visibility, parent source marker and derived SVG label; Mock/OCCT dimensions/volume, dependency and atomic edit failures, schemas/migration/round trip, example/guide/skill/eval; ADR-043 | Existing mesh wireframe approximation; no nested/section details, detail dimension semantics, circular regions or arbitrary camera projections; empty crops refused |
| Polygonal section views (MCAD-P12-003) | Implemented | Authored SI cutting plane on a view; exact OCCT line intersection and Mock box slices; canonical closed loops, derived material area and even-odd SVG hatch; analytic holes/islands and volume tests, schema/migration/round trips, Agent success/atomic failures, example/guide/skill/eval; ADR-042 | Slice outlines only; no projected geometry beyond plane, parent cut arrows or section dimensions; curves, assemblies, coplanar-face cuts and foreshortened/isometric views refused; 4096-edge limit |
| Radial drawing dimensions (MCAD-P12-002) | Implemented | Semantic complete-circle radius/diameter; exact native circle query, Mock witness, derived SI results and SVG leaders; analytic radii/diameters/cylinder volume, edited/translated bodies, verified Agent success and atomic failures, schema/migration/round trip, example and eval; ADR-041 | Top-Z perimeter semantic discovery only; no arcs, ellipses, assembly-instance refs, foreshortened/isometric layout, arrowheads or label collision avoidance |
| Angular drawing dimensions (MCAD-P12-001) | Implemented | Semantic straight-edge rays and explicit point rays; derived SI radians, orthographic SVG arcs, native exact line query and Mock witnesses; analytic angles/triangle volume and transformed-body tests, failure/rollback, schemas/migration/round trips, example, guide/skill/eval; ADR-040 | Semantic discovery limited to top-Z perimeter; exact native lines only, non-reflex nondegenerate angles, no assembly-instance semantic bindings or foreshortened/isometric layout; other planes support explicit non-associative rays |

## Historical plan crosswalk

The older plans use implementation milestone names rather than the active
roadmap IDs:

| Historical document | Current interpretation |
|---|---|
| `assembly-phase3-plan.md` | Phase 3 Assembly milestone is complete; follow-up quality work is `MCAD-P5-004` and `MCAD-P5-005` |
| `drawing-phase4-plan.md` | Phase 4 Drawing milestone is complete; follow-up HLR/golden work is `MCAD-P5-003` and `MCAD-P5-005` |
| `visual-showcase-plan.md` | Showcase baseline is complete; future release and regression work follows `MCAD-P1-003` and `MCAD-P5-005` |

Historical `Task-###` references are retained only where they identify the
original implementation change. New work must use `MCAD-P…` IDs.

P13-003 / ADR-052: bounded analytic convex line/tangent-arc and circle offsets
are independent authored copies through CLI/Agent/MCP planning and DesignPatch.
Caller IDs, explicit SI distance, closure/convexity/inset refusals, both-kernel
neutral wire dimensions and native analytical volumes, history and file round
trips are covered. Concave repair and persistent source association remain
unimplemented. serde_json float_roundtrip preserves source f64/history identity.

P13-004 / ADR-053: analytic parametric closed ellipses with centre constraints,
source axes/angle expressions and strict units, exact native normal extrusion
(distance/centred symmetric, native feature cut/join), analytic Mock volume/
bounds/COM/AGM area, source/patch/schema/expanded/packed/history evidence and
example/guide/skill/eval. Elliptical arcs, axis/tangent constraints, through-all,
ellipse loft/sweep/helix/revolve/trim/offset, Mock topology/booleans/rendering and
ellipse sketch-overlay picking remain unsupported and fail closed.

P13-005 / ADR-054: graph-owned semantic cross-sketch projection of lines/full
circles/ellipses into explicit planes, derived sketch outputs excluded from
authored files, stable destination IDs, dependency/warm-cache updates, reference/
cycle/plane/degeneracy refusal, pure inspection via CLI/Agent/MCP, both-kernel
analytic ellipse volume and source/history/schema examples. Body-edge and
Datum/FaceRef/arc/spline projection and editable generated sketches remain
deferred; legacy Mock line/parallel-circle measures remain synthetic.

P13-008 / ADR-057: scoped closed single-span non-rational cubic sketch curves
with stable control-point IDs and constraints, strict units/unknown-field
refusal, exact normal distance/centred symmetric NewBody extrusion in Mock and
OCCT, analytic volume/centroid/extrema, capped GL8 perimeter, native AddOptimal
bounds without triangulation, source expressions and resolved-input cache
identity, atomic patch/history and source/file round trips. Example/guide/skill/
eval and real MCP stdio control editing are included. General NURBS, open/mixed/
multispan/rational curves, knot/weight editing, trim/offset/projection, loft/sweep/
revolve/through-all and cubic cut/join remain deferred. Native closed-curve edge
roles are left unbound; Mock shape/topology/inertia/rendering refuse rather than
inventing geometry. Sketch overlay picking and external-host validation remain
unverified.

P14-001 / ADR-058: accepted backend-only desktop editing architecture and Phase
14 task/acceptance plan. Frontend drafts/presentation, typed Tauri adapters,
shared source/semantic/unit/revision queries and DesignPatch commands, verified
candidate regeneration/history/persistence and failure checkpoint behavior are
defined. The existing source-only parameter/history publication gap is scheduled
for the editor gateway. Feature-tree, semantic selection patch and interactive
sketcher implementations remain planned (P14-002/003/004). This architecture-only
change adds no runtime editor, schema field or maturity credit. Static handler,
headless/backend, IPC harness, actual Tauri and actual external-host evidence
must be reported separately.

P14-002: bounded part feature-tree editing through the shared verified backend.
Tauri product forms create NewBody distance extrusions from authored closed
profiles, edit six admitted length fields, reorder and suppress with dependency
admission; source revisions, explicit units and required assertions fail closed.
Parameter/history commands verify geometry and native tessellation before file
publication; no-op keeps history and skips regeneration. Both-kernel analytic
cubic tests, staged-failure source/history/preview retention, actual MCP stdio
source/history parity, real Linux Tauri form interactions, examples/guide/skill
and an independently graded eval are included. Native interaction coverage is
local Linux only. General feature/assembly/sketch editing and selection-to-patch
remain next tasks; nonempty physical BodyCount is unavailable and refuses;
GPU rasterization and cross-process file locking remain separate boundaries.
No schema, kernel feature or golden change is introduced.

Upon this task's main merge, the implemented feature-tree create/edit/reorder/
suppress and verified Undo/Redo justify GUI 10 → 20 under the fixed rubric
(weight 15%, +1.5 total): general weighted 43.95 → 45.45 (about 45%) and
agent operation 86 → 87. Before the merge, the baseline remains about 44% / 86%.
This credit does not claim interactive sketching, semantic selection-to-patch
or external host verification.


P14-003: source/scene-bound selection to reviewed dimension patch. Declared face
semantic refs use current unique Exact/Derived/Fingerprint provenance; authored
overlay IDs require a unique active supported profile consumer. Missing,
ambiguous, consumed, aliased, generated, suppressed, unsupported and stale
bindings refuse. Scene evidence captures the exact source document and hashes
its identity plus actual mesh/overlay data; camera/kernel IDs cannot bind source.
Tauri pointer selection and dimension forms share native rebinding/planning/
verified application with Agent/MCP. Name hints no longer focus editable
parameters. Both-kernel analytic plans, native repeatability/document-scope and
source/history refusal, real MCP parity, actual Linux Tauri interactions and
independent eval evidence accompany the example/guide/skill updates. No source
schema, sampler, dependency, kernel operation or golden changes. Declared native
face refs and supported authored line/circle/arc overlays only; ellipse/cubic
strokes, edge/vertex/assembly editing, sketch manipulation and physical nonempty
BodyCount remain explicit limitations.

Upon P14-003's main merge, semantic selection to reviewed verified edits
justifies GUI 20 → 30 (+1.5 weighted): general 45.45 → 46.95 (about 47%) and
agent operation 87 → 88. Before merge the baseline remains about 45% / 87%.
P14-004 interactive sketching and Phase 15 host/ecosystem work remain.


P14-003 merged as PR #88 (`42a318ed`), after all applicable CI succeeded:
GUI 30, general weighted 46.95 (about 47%), agent operation 88.

P14-004: interactive local-mm SVG drafts for existing global-plane authored
sketches. Point selection/move, two-click line drafts, circle/ellipse forms,
construction flags and horizontal/vertical/coincident/distance/radius constraints
are typed shared backend commands. Read-only review solves all admitted sketches,
verifies graph/profile/geometry/assertions and returns the concrete DesignPatch;
apply rechecks source revision and verifies tessellation before atomic write.
Source expressions and IDs survive; unsupported/failed/overconstrained/stale
candidates retain source/history/display. Undo/Redo stays backend-owned. Mock and
OCCT analytic volume/dimensions, constraint residuals, native history/refusal,
real MCP stdio parity and actual Linux Tauri interaction accompany the example,
API/architecture/guide/skill and independent eval task. No schema, dependency,
new kernel operation, sampler or golden changes. Initial scope excludes new
sketches, custom/face workplanes, arc/rectangle/cubic/NURBS editing, trim/offset,
constraint replacement and assembly sketch editing; external host evidence and
physical nonempty BodyCount remain unavailable.

Upon P14-004's main merge, interactive sketch editing completes the three
implemented GUI rubric capabilities and justifies GUI 30 → 45 (+2.25 weighted):
general 46.95 → 49.20 (about 49%), agent operation 88 → 89. Before merge,
the baseline remains about 47% / 88%. Phase 15 ecosystem evidence remains to
reach the fixed approximate 50% target; do not infer external host verification
from Linux Tauri or local MCP checks.


P14-004 merged as PR #89 (`6fce031f`), after all applicable CI succeeded:
GUI 45, general weighted 49.20 (about 49%), agent operation 89.

P15-001: an executable editing tutorial discovers real MCP tools, reviews and
applies typed sketch/feature commands, checks stale/unit refusals against
byte-identical files, exercises opaque Undo/Redo through real Agent processes,
and independently verifies native volume/dimensions and source identity. It
leaves the original fixture unchanged. The guide, scenario and runner ship in
CLI archives; release CI executes the tutorial from the extracted native
archive before uploading it. A new independently graded eval covers the same
source/volume contract. The current published latest is v0.1.1; these commands
require current main or the next v0.2.0 release. This is protocol/native evidence,
not actual Codex/Cursor/Gemini client integration. Windows/client sessions remain
P15-003; actual release publication and downloaded verification remain P15-002.

Upon P15-001's main merge, the reproducible tutorial, example/eval and packaged
execution gate justify ecosystem 10 → 11 (+0.15 weighted), general 49.20 →
49.35 (about 49%). Agent operation remains 89; no new CAD capability is claimed.
Before merge the baseline remains general 49.20 / agent 89. No weights/targets,
source schema, kernel operations, dependencies, golden files or desktop signing
policy change.


P15-001 merged as PR #90 (`c2b7ffbf`), after all applicable CI succeeded:
ecosystem 11, general weighted 49.35 (about 49%), agent operation 89.

P15-002: official v0.2.0 publication from the exact tested main/tag; all four
native CLI archives run their included tutorial before upload, complete
main/tag CI inventories pass, and the independently downloaded Linux archive
matches official SHA-256 and passes fresh native tutorial/source/history checks.
The downloaded binary also passes actual Codex CLI 0.159.2 app-server startup,
35-tool discovery and typed review/apply/refusal/regeneration dispatch in an
ephemeral session with zero model turns. This is not autonomous model selection,
persistent plugin installation, Cursor/Gemini integration or Windows desktop
interaction. See [release evidence](cli-release-v0.2.0-evidence.md) for exact
commits, runs, assets, official hashes and observed scope. Unsigned CLI/desktop
trust scope is unchanged; no schema, kernel, dependency or golden changes.

After this evidence task's main merge, ecosystem 11 → 13 (+0.30 weighted)
justifies general 49.35 → 49.65 (about 50%), agent operation unchanged at 89.
Before merge the baseline remains about 49% / 89%. The fixed ecosystem target
15 remains pending further client/GUI host evidence; no unexecuted host check
is credited and rubric weights/targets are unchanged.


P15-003 partial evidence: `tools/run_codex_host_smoke.py` exercises actual Codex
CLI 0.159.2 app-server dispatch with an ephemeral thread and zero model turns.
Both the current native development binary and independently checksum-verified
v0.2.0 Linux archive discover 35 tools and pass typed review/apply, byte-exact
stale/unit refusal, authored source identity and independent native volume/
dimension checks. Transport failure regression tests run in CI without a host
or credentials. See [host evidence](host-integration-evidence.md). Windows
desktop, Cursor and Gemini sessions remain unavailable; model-driven selection
and persistent host-plugin installation are untested. P15-003 remains partial
and earns no additional maturity credit: general 49.65 (about 50%), ecosystem
13, agent operation 89. The existing Windows native archive/tutorial CI is
packaging evidence, not a Windows client/desktop session.


P15-004 is the bounded Gemini startup follow-up to P15-003: official pinned
Gemini CLI 0.63.0 links/loads the distributed extension and reports the native
MCP server Connected in a disposable profile. Actual ACP initialization passes,
but session/new returns -32000 (missing API key); no model prompt/login occurs.
The install/startup configuration draft retains prior CAD/GUI setup and adds
verified Gemini steps; publication/restoration are not inferred from saving.
See [host evidence](host-integration-evidence.md) for integrity and reproduction.
Gemini discovery/edit/refusal/regeneration, Windows desktop and Cursor sessions
remain P15-003 gaps. No additional maturity credit: general 49.65 (about 50%),
ecosystem 13, agent operation 89. No source, schema, dependency or golden changes.


P15-005 adds an actual Windows native Tauri/WebView2 editing gate to desktop CI.
An isolated fixture/profile and loopback CDP drive existing product forms through
real Tauri IPC; create/length/reorder/suppress, independent 2400 mm³ preview,
Undo/Redo canonical identity, byte-exact review/refusal and original fixture
retention are checked. Desktop PR builds now include Windows and Linux; all
four platforms remain on main/tags/manual runs. This is automated native GUI
evidence, not a manual host session, installer or signed distribution claim.

Only after the actual Windows CI gate succeeds and P15-005 merges main, its
new Windows GUI evidence justifies ecosystem 13 → 14 (+0.15 weighted), general
49.65 → 49.80 (about 50%); agent operation stays 89. Before that evidence/merge,
the baseline remains 49.65 / ecosystem 13 / agent 89. Cursor and authenticated
Gemini editing remain P15-003 gaps and no unavailable-host credit is assigned.
No source schema, production UI/backend, kernel, dependencies or golden changes.


P15-005 Windows native GUI passed at `a408e015` in Desktop run 38346081515,
job 115127806676 (`Exercise actual Windows native Tauri forms`: success).
The initial test-only Windows path representation mismatch was corrected using
filesystem-resolved identity; all product form, native preview, source, refusal
and history checks then passed. Failed runs were never merged or credited.
PR #94's final-head applicable CI remains required before merge. After that
merge, the current baseline is ecosystem 14, general 49.80 (about 50%), agent 89,
based on this actual Windows evidence; P15-003 stays partial for Cursor and
authenticated Gemini editing. Manual desktop interaction/signing is not claimed.


P13-010 / ADR-059 defines the flexible-subassembly implementation gate. It is
design only. P13-011 implements parent-owned independent child coordinates and
a shared scoped physical leaf scene; P13-012 will add admitted parent-context
constraints. P13-011 completion is conditional on its implementation PR merge. A posed
copy alone is not full flexibility. Both must address source snapshots/cache
ownership, internal collision pairs, consumer refusal, reader migration and the
five required feature deliverables. No maturity credit from the design PR:
general 49.80 (about 50%), assembly 68, ecosystem 14, agent operation 89.
Cursor/authenticated Gemini verification remains P15-003, independently pending.


P13-011 / ADR-059 adds strict `set_instance_articulation`, inspected child
revision/driver ownership, mandatory old-reader refusal, additive migration,
source snapshots before verification and before publication, canonical history
and structured leaf regeneration. Static geometry/review/BOM use the posed
scene; hierarchical motion/reach/URDF/HTML and nested articulated/pattern scopes
refuse. ADR-060 supplies bounded actual mesh compounds for Mock, preserving
explicit exact-intersection/inertia refusals. Mock/OCCT independent box volume,
centre, bounds, 90° rotation and 25 mm translation evidence, native internal,
copy/copy and parent/child collision tests, expanded/packed round trips, real
MCP apply/review/Agent Undo/Redo, example, guide, skill and independently graded
eval accompany the task. No parent-context constraint or full flexibility
credit: assembly remains 68; general 49.80 (about 50%), agent 89.


P13-012 / ADR-061 completes an admitted parent-context flexible-subassembly
workflow: parent connector constraints solve internal bounded prismatic/revolute
drivers in one copy, preserving child bytes and other copies. Independent
Mock/OCCT transforms/centres/dimensions/volumes, derived coupling ownership and
unique zero DOF, full leaf collisions, strict reader/schema/migration round trips,
source snapshots, verified actual MCP/review/history, examples, guide, skill and
independent eval accompany the implementation. Existing unsupported consumers
explicitly refuse; no nonlinear free-outer or continuous collision proof claimed.
Only after this PR merges, assembly 68 → 70, weighted general 49.80 → 50.10
(about 50%), agent operation 89 → 90. Before merge the baseline remains 50%/89%.
The fixed rubric weights and all other component scores remain unchanged.


P15-006 packages the P13-011/012 flexible-assembly workflow with a runnable
real-MCP/Agent tutorial, exact source/revision/connector inspection, read-only
review, parent-only edits, canonical Undo/Redo, refused stale/unit/range/owner/
missing-reference inputs and protected URDF/HTML outputs. Independent SI frames,
native volumes/bounds/zero DOF and exported closed five-member STL volume/centre/
bounds are checked; a changed internal pose retaining total volume/outer bounds
fails the checker. Both complete mechanisms and every runner input are included
and exercised from the extracted native archive before release artifact upload.

Local actual PowerShell packaging and extracted-archive execution passed on
current-main native debug CLI (0.2.0 development version, 36 discovered tools).
This is local package evidence, not an official new release; published v0.2.0
predates these APIs. Future tag execution remains required on all four platforms.
Cursor/authenticated Gemini remain P15-003 gaps. No maturity increment: general
50.10 (about 50%), assembly 70, ecosystem 14 and agent operation 90. Source
schema, kernels, dependencies, lockfiles, versions and goldens are unchanged.


P15-007 prepares v0.3.0 with coherent workspace, desktop, plugin and lockfile
versions and reviewed release notes for the accumulated flexible-assembly APIs.
Publication is a separate tag-build/asset-verification step; a merged preparation
PR alone is not a published release. No maturity increment is assigned.
Local pre-push checks passed (1042 Rust tests, zero failures, 31 ignored). Actual
PowerShell packaging with the native development v0.3.0 binary and both tutorials
from its extracted Linux archive passed; these are local checks, not official
release assets or validation of all four native tag builds.


P15-008 recovers the failed v0.3.0 tag: exact-main gates passed and the annotated
tag points to PR #99 main commit 75ac288fb97402433310d1d513041821deb7a434,
but Windows failed while executing the extracted archive tutorial. No v0.3.0
official release is claimed. Recovery exposes real failure diagnostics in Actions
annotations and exercises Windows packaged tutorials in PRs. The v0.3.1 candidate
keeps workspace/desktop/plugin/locks coherent; publication remains P15-009 after
all gates. No capability, host, signing or maturity credit is assigned here.
