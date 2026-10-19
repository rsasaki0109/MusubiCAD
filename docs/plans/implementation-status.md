# MusubiCAD implementation status

Last reviewed: 2026-10-19

This inventory is evidence-based against the current repository. **Implemented**
means the capability has code and focused tests; **Partial** means the primary
path exists but an explicit contract or integration is still missing; **Stub**
means the public area is reserved but not implemented. The active follow-up IDs
are defined in the [canonical roadmap](roadmap.md).

| Area | Status | Current evidence | Remaining work |
|---|---|---|---|
| Physical rectangular revolves (MCAD-P17-022) | Implemented; qualifying main merge pending | ADR-112 shared strict portable admission, exact analytic Mock annular-sector measurements and closed mesh, native radial-start correction and placed normals; independent volume/area/COM/world bounds/contacts, source/patch/file/cache/history and actual MCP/Agent/eval evidence | Positive inner radius, global planes, coplanar edge-parallel axis and NewBody only; P17-013 controls follow this prerequisite; no maturity increment |
| Reviewed native standard holes (MCAD-P17-012) | Complete: PR #162 | ADR-111 source-owned stock/entry/circle catalogue and standard/seat controls; strict final entry provenance including blind bores; local four checks (1364 passed), actual Linux and exact-head Windows native 68 groups, four complete workflows/11 jobs; qualifying main merge 1135d988 | Current active original rectangle/distance/NewBody stock and existing outward custom/face/datum circles only |
| Reviewed native independent offsets (MCAD-P17-011) | Complete: PR #161 | ADR-110 ordinary source catalogue, explicit ordered selection/constant distance/side, backend-generated sketch/owner/entity IDs, solved candidate and protected atomic DesignPatch/history; six both-kernel/source/file regressions, actual MCP/Agent five placements and independent source/physical/adversary eval; actual Linux and exact-head Windows native 62 groups, all four local checks and four workflows/11 jobs; qualifying merge 7e742047 | Existing convex CCW shared-endpoint mitre/tangent contours or one circle only; no automatic repair or persistent curve association |
| Reviewed native trim/extend (MCAD-P17-010) | Complete: PR #160 | ADR-109 source catalogues, backend-only discovery, explicit end/branch/removals and solved endpoint verification, protected DesignPatch/history; analytic Mock/native OCCT independent placed solids, packed/expanded source and actual MCP/Agent/eval evidence; actual Linux and exact-head Windows native 56 groups including six new curve groups; qualifying main merge 8512b229 | Authored line/arc targets and line/circle/arc cutters only; no helper/projected/ellipse/cubic admission or automatic constraint/consumer repair |
| Counterbored standard holes (MCAD-P16-003) | Implemented; complete upon PR merge | ADR-063 strict source/seat and lengths; source/physical stock/entry ownership, actual union volume/first moments/closed faces, producer metadata and warm-cache rechecks; both kernels, schema/migration/round trips, patch-authored example, guide/skill/eval, actual MCP creation/review/edit/refusal and Agent history | Original rectangle/distance/NewBody stock and owned end faces only; no side/oblique/already-cut/imported/fused targets, through-all, inferred head sizes, countersinks or threads; default Mock unsupported |
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

PR #100 diagnosed the actual Windows failure: ASCII STL LF-to-CRLF conversion
produced hash 666cdf8c… while feature:stl_plate required 129172c9…. Git binary
STL attributes and a real core.autocrlf=true checkout regression preserve exact
asset bytes; source hashes, fixtures, units and physical assertions are unchanged.
Actual native Windows archive execution must still pass before this recovery
is complete or v0.3.1 publication can proceed.


P15-009 published official CLI v0.3.1 from PR #100 main commit `a9ec76f8616ecfce146f178c5b6d741b2d083825`
at 2026-10-13T13:10:07Z. All exact-main and exact-tag native gate inventories
passed; all four CLI archives execute both extracted tutorials before upload.
The five official assets and their published hashes, independently downloaded/
hashed Linux binary, both real-MCP/Agent tutorial successes and bounded host/
trust scope are recorded in cli-release-v0.3.1-evidence.md. No maturity increment:
general 50.10 (about 50%), ecosystem 14, agent operation 90. Actual Cursor and
authenticated Gemini editing remain unverified; no signed desktop release claimed.


MCAD-P16-001 registers the additional approximately 60% capability plan while
preserving the fixed FreeCAD rubric, weights and original target. Actual baseline
remains solid 55, sketch 60, assembly 70, drawing 50, I/O 60, GUI 45, ecosystem 14,
weighted 50.10 and agent operation 90. The proposed 60.10 allocation is a planning
estimate, not earned credit. Phase 16 prioritizes bounded analytic Mock support,
seated standard holes, curved draft, tapered ribs, physical external threads and
expanded fillets. Existing capabilities are not credited twice. ADR-062 admits
only the explicit Mock analytic infrastructure prerequisite; feature-specific
source/schema/semantic/tessellation gates remain required before seat implementation.
This documentation-only design task changes no source, schemas or CAD capability
and awards no maturity increment. Future phases remain unregistered candidates.


MCAD-P16-002 implements an explicitly selected bounded analytic Mock mode for
right rectangular stock, exact circular extrusion and contained concentric
subtraction. Ordered exact axial integration avoids overlapping removal twice;
independent Mock/OCCT volume, first moments, surface area, bounds and rigid
placement tests exercise reversed cut order, blind cuts and tilted/symmetric
primitives. The native ordinary symmetric extrusion placement is corrected.
Unsupported consumers and unknown/nonfinite/ambiguous inputs refuse without
changing existing bodies; default Mock behavior is preserved. Runnable SDK
example, API/architecture/ADR, MCP guide and skill identify the bounded scope.
This is runtime kernel infrastructure, not a new document feature or MCP tool:
no schema/migration/history changes or new MCP eval are applicable. Semantic
faces/tessellation/source gates remain necessary for P16-003. No maturity credit:
weighted general CAD 50.10, agent operation 90; fixed rubric unchanged.


MCAD-P16-009 registers ADR-063 and the explicit prerequisite order P16-009 →
P16-010 → P16-003. The seated_standard_hole/closed counterbore seat wire, positive
length expressions, mandatory owned entry reference, original rectangular-stock
source/geometry proof, both-kernel closed mesh/face identities, bounded sampling
and independent error/volume/history/reader tests are specified before coding.
Current analytic Mock still refuses meshes/topology; the legacy box helper is
not a closed box. No source, schema, feature or sampling implementation changes
are included by this design task. New feature/seat fields remain unavailable
until P16-003, and default Mock factories remain unchanged. Actual baseline stays
weighted 50.10, agent operation 90; no planning/infrastructure maturity credit.


MCAD-P16-010 implements ADR-063 runtime prerequisites: actual closed analytic
Mock stock/cylinder/concentric-cut meshes with strict sampling/coordinate/output
budgets, owned face IDs and transform/split-wall history, and a physical uncut
rectangular-stock face witness in both Mock and OCCT. Native exact BREP admission
stays private; default Mock factories and source serialization stay unchanged.
Independent exported-triangle and pick/refusal tests and the runnable SDK example
supply evidence. Completion requires this task PR's checks and merge; this
infrastructure earns no score. MCAD-P16-003 source/schema/semantic/DesignPatch seat
feature work still follows after that merge.


MCAD-P16-003 implements ADR-063 explicit counterbores on the P16-010 runtime
prerequisite (PR #105). Source nodes and actual cuboid/entry geometry agree before
cutting; actual volume/first moments and closed face-bearing output are required.
A stateless feature query supplies physical output producer metadata, distinguishing
retained stock faces from new cylinder/step/blind faces. Cached outputs repeat
ownership and physical checks. New source/seat/length decoding is strict; both
schemas and additive no-inference migration/expanded/packed/old-reader tests ship.
Default Mock is not switched. CLI/Agent geometry gates cannot be disabled for
seated holes. Patch-authored example, guide/skill and independent source-aware eval
are included. Real MCP stdio makes 10 requests and Agent history makes two, with
zero model calls: create, read-only preview/review, parameter edit, physical/source
checker, protected wrong-entry/depth refusal, canonical Undo/Redo and CLI no-verify
refusal. Existing history preserves JSON values/array order and imported bytes;
object member order can change through opaque transport. No host/release credit.

Only after this implementation PR merges, solid modeling 55 → 57 for bounded
parametric counterbores with genuine Mock/OCCT physical evidence and protected
source/patch/history. All other fixed-rubric scores stay sketch 60, assembly 70,
drawing 50, I/O 60, GUI 45, ecosystem 14. Weighted general CAD 50.10 → 50.60
(about 51%), agent operation 90 → 91 for the actual typed source/verified MCP
manufacturing workflow. The bounded original-stock scope earns partial credit,
not general arbitrary-face hole support. Before merge the baseline stays 50.10/90.

MCAD-P16-003 is complete in PR #106 (merge 35a053b): the preceding conditional
counterbore credit is now earned, weighted general CAD 50.60 and agent operation
91. MCAD-P16-011 supplies ADR-064's missing runtime countersink geometry before
P16-004: actual cylinder/frustum union in OCCT, linear-radius analytic Mock
cuts, exact volume/area/first moments, conical closed face-bearing meshes,
retained stock face identities and protected admission failures. Independent
integration tests cover multiple taper depths, blind/through bores, both entry
ends and rigid placements. Default Mock and generic cuts on tapered Mock stock
refuse. Runtime API/docs/SDK example are provided; no .ocad source, MCP tool or
model call is introduced. P16-004 still owns angle-bearing source/patch/schema/
roundtrip/history/eval deliverables. Infrastructure earns no score: all rubric
scores remain solid 57, sketch 60, assembly 70, drawing 50, I/O 60, GUI 45,
ecosystem 14 (weighted 50.60, about 51%; agent operation 91).

MCAD-P16-011 is complete in PR #107 (merge 3fc2817); its runtime-only geometry
prerequisite earns no score. MCAD-P16-004 implements ADR-064's explicit
`countersink` seat: SI diameter/included angle, derived frustum depth, strict
nested decoding/length-angle expressions, typed `seat_included_angle` patch,
source/owned-end/physical/cache admission and mandatory native host gates.
Both schemas preserve the counterbore form in a strict oneOf, with no-inference
migration, packed/expanded/authoring/old-reader refusal tests. Independent
Mock/native formulae, contact/closed-mesh checks and a non-axis-aligned stock
placement prove actual geometry. Patch-authored example, MCP guide/skill and
source-aware `countersink_angle` eval are included. Real stdio makes 11 MCP
requests and two Agent history requests with zero model calls; fresh native
volume, read-only preview/review, protected wrong-ref/range/unit refusal,
canonical Undo/Redo and CLI no-verify refusal pass. No host/release credit.

Only after P16-004 merges, solid 57 → 59 earns partial countersink capability
credit for real conical removal, parametric angle/contact and protected source/
MCP authoring; arbitrary-face hole support or implicit manufacturing standards
are not claimed. Other rubric scores remain sketch 60, assembly 70, drawing 50,
I/O 60, GUI 45, ecosystem 14. Weighted general CAD 50.60 → 51.10 (about 51%),
agent operation 91 → 92 for the verified typed angle-bearing manufacturing
workflow. Before that merge, the earned baseline remains 50.60 / 91.

MCAD-P16-004 is complete in PR #108 (merge 1d84151): its conditional partial
countersink credit is earned, solid 59, weighted CAD 51.10 and agent operation
92. MCAD-P16-012 supplies ADR-065's physical cylindrical-stock witness and
neutral-plane solid draft before P16-005. Native checks exact full cylinder/
circle/cap/seam topology, outward analytic derivatives and positive volume;
Mock queries tracked Cylinder records. Signed plane/angle radial law produces
actual standalone Frustum measurements/closed conical meshes, with no fake
reconstruction ancestry. A private exception-catching BREP bridge protects
native constructor failures. Independent Mock/native tests cover signed angles,
endpoint/interior planes, axis reversals, rigid source/output placements,
contact/volume/area/COM/bounds/face membership and protected invalid/unsupported
inputs. Runtime API/ADR/architecture, SDK example, guide/skill boundary ship;
no .ocad/MCP curved draft, source ownership or new host/release credit is claimed.
P16-005 owns guarded source/semantic/history/schemas/example/eval afterward.
Infrastructure earns no score: solid 59, sketch 60, assembly 70, drawing 50,
I/O 60, GUI 45, ecosystem 14 (weighted 51.10, about 51%; agent operation 92).


MCAD-P16-012 is complete in PR #109 (merge 98b2f8e), with all four exact-head
workflows and applicable Windows/native/desktop jobs passing. Runtime-only
geometry earns no score; earned baseline remains weighted CAD 51.10 / agent 92.

MCAD-P16-005 implements ADR-065's closed `cylindrical_draft` source over that
runtime. Original authored exact-circle/distance/NewBody stock must agree with
the physical witness; an owned full cylindrical side, signed strict SI angle
and explicit neutral plane drive actual solid frustum geometry. Changed side
consumption, physically proved cap/conical producers, cache revalidation and
mandatory native CLI/MCP candidate verification protect source/history/output.
Both schemas, additive no-inference migration, canonical expanded/packed
round trips and genuine planar-draft reader refusal accompany the tag.
Independent Mock/native source cases integrate radial slices and exported closed
triangles, with contact/COM, signed/reversed planes, oblique source placement,
cap ownership, corrupted cache and protected range/unit/reference refusal.
The patch-authored example, API/guide/skill and source-aware signed-angle eval
ship. Actual stdio makes 11 MCP and two Agent history requests, zero model calls;
review/preview is read-only, verify:false/no-verify cannot bypass native checks,
and refused edits plus canonical Undo/Redo preserve source/history.

Only after the P16-005 main merge, solid 59 → 61 earns partial credit for the
full-cylinder curved-face family with neutral-plane semantics and protected
parametric authoring. Generic curved/concave/NURBS draft gets no credit.
Other scores remain sketch 60, assembly 70, drawing 50, I/O 60, GUI 45,
ecosystem 14. Weighted CAD 51.10 → 51.60 (about 52%); agent operation 92 → 93
for the verified signed draft/source/history workflow. Infrastructure P16-012,
new planning, host and release work receive no additional credit. Until merge,
actual maturity remains 51.10 / 92; the additional approximately 60% goal remains
in progress.

MCAD-P16-005 is complete in PR #110 (merge ad09a40), after all exact-head CI,
Design Review, Desktop and Release workflows and applicable Windows/native/
packaged-smoke jobs pass. The conditional full-cylinder credit above is now
earned: solid 61, weighted CAD 51.60 (about 52%), agent operation 93. Other fixed
rubric scores and weights stay unchanged. P16-013 registers the physical tapered-
rib prerequisite for P16-006; infrastructure itself earns no new score.

MCAD-P16-013 implements ADR-066's runtime tapered-rib prerequisite: actual
physical box witness and contained square-ended trapezoid-prism union, signed
base-neutral thickness law, exact mass/area/COM/bounds, proper placements and
protected invalid/unsupported inputs. Mock uses a separate RibbedStock record
and exact planar stock/contact partition (11 faces, 68 triangles), omitting the
internal interface. Native uses exception-catching exact prism/fuse, verifies
one valid solid, 11 planar faces/24 edges/16 vertices, exact surviving stock/
contact/top vertices and independent volume/area. No tolerance-erased addition,
fake reconstruction ancestry or looser mesh retries. Independent tests cover
all six face orientations, axis reversal, signed angles, rigid source/output
placement and negative-draft overhang, with slice/triangle integrals, contact,
area/COM/bounds and protected refusal. SDK example/API/architecture/guide/skill
mark the runtime boundary. P16-006 supplies source/semantic/schema/patch/history/
MCP eval next. No feature/host/release credit: earned solid 61, weighted CAD
51.60 (about 52%), agent operation 93; the additional 60% goal remains active.


MCAD-P16-013 is complete in PR #111 (merge 7328ef3), after all exact-head
workflow/job CI, native Windows, Desktop and packaged-smoke checks pass.
Runtime infrastructure earns no maturity increment: weighted CAD 51.60 / agent 93.

MCAD-P16-006 implements closed `tapered_rib` authoring over ADR-066's runtime.
Original authored rectangle/distance/NewBody stock and complete open line prove
an owned original extrusion end, with stable extrusion_start/end roles independent
of global orientation. Strict SI thickness/height/signed angle/square ends,
source/world witness comparison, physical six-stock/five-rib producers and
cache rediscovery protect the real interface-free union. Profile-free complete
construction lines need an explicit declared consumer; suppression keeps source
and returns stock geometry, while orphan/legacy profile checks remain unchanged.
Known collapsed/resolution-invalid top thickness refuses before kernel work.
Both schemas, no-inference migration, packed/expanded/authoring roundtrips,
genuine old constant-rib reader refusal and canonical history accompany the tag.
Independent Mock/native source tests cover original ends, oblique placement,
height edits, signed widening, contact/volume/COM/closed triangles, real ownership,
corrupted cache, unit/ref/source/range refusal and suppression. Patch-authored
example, API/guide/skill and source-aware tapered_rib_height eval ship. Actual
stdio makes 14 MCP and two Agent history requests, zero model calls: native
13800→13224 mm³, read-only review/typed angle/thickness/suppression previews,
mandatory verify:false/no-verify geometry, protected wrong-end/unit/range edits
and canonical Undo/Redo pass. No new host or release claim.

Only after P16-006 merges to main, solid 61→63 earns partial credit for actual
square-ended tapered addition, parametric height/thickness/signed angle, owned
source/end and protected MCP/history. Arbitrary curved/intersecting/multiple-rib
families and existing constant-width rib receive no new credit. Other fixed
scores remain sketch 60, assembly 70, drawing 50, I/O 60, GUI 45, ecosystem 14.
Weighted CAD 51.60→52.10 (about 52%), agent operation 93→94. Before that merge,
actual earned baseline remains 51.60 / 93; additional approximately 60% goal
continues and runtime/planning receives no feature credit.

MCAD-P16-006 is complete in PR #112 (merge 2c8d8a8), after every exact-head
workflow and applicable native/Windows/Desktop/packaged-smoke job passed.
The conditional tapered-rib credit is now earned: solid 63, weighted CAD
52.10 (about 52%), agent operation 94; other scores and all weights unchanged.
No additional host/release or arbitrary rib-family claim. The 60% goal continues.

MCAD-P16-014 registers ADR-067 before implementation and supplies only immutable
nominal metric basic flat-root profile arithmetic: seven coarse size pairs,
explicit right/left hand, square ends over 1–32 pitches, SI radius/diameters
and ideal section/volume. Independent tests use meridional dimensions/slopes,
handed pitch/phase, separately derived piece integrals and numerical angular
sections, plus invalid count/nonfinite/outside-end refusal. Runnable SDK example,
API/architecture/guide/skill state the arithmetic-only boundary. No body/mesh,
new source/schema/patch or eval is claimed. Further physical construction/
sampling admission and measured Mock/OCCT source evidence remain required for
P16-007. This prerequisite earns no score: actual 52.10 / 94.

MCAD-P16-014 is complete in PR #113 (merge 0109be9), after all exact-head
workflow/job checks pass. Profile arithmetic receives no maturity increment.

MCAD-P16-015 implements ADR-068's physical nominal metric thread prerequisite.
A body-owned full-cylinder witness, coarse dimensional agreement, explicit hand,
whole pitches and radial phase axis admit a proper immutable screw envelope.
Analytic Mock stores actual thread data with exact ideal volume/area/COM/support
bounds and integer screw-band clipping to closed square-ended physical meshes.
Canonical exact keys, quality/error/normal/f32/budget checks and unsupported
consumer refusal prevent synthetic fallback or proximity-welded seams.
Native builds per-revolution Frenet valley cuts in local mm with explicit
precision and deterministic non-destructive clipping, validates one oriented
solid, reflects proved right geometry for left hand and places back in SI.
Actual BREP volume/area/COM use adaptive epsilon1e-9; default quadrature's
32-pitch COM error was corrected without loosening acceptance or substituting
ideal values. Proper output transforms retain transient metadata/tight queries;
legacy direct rotate/mirror and improper placements refuse for these records.
Independent tests grade coarse sizes, both hands/whole-pitch extrema, analytic
piece/surface/triangle integrals, actual phase/radii, closed edges, tilted bounds,
source/output rotations, owned face IDs, protected stock and invalid quality/
precision/unit/source/consumer failures. Both-kernel shared SDK example,
API/architecture/authoring guide/skill document actual runtime scope.
No source tag/schema/patch/MCP operation or eval yet: P16-007 must supply these,
authored original-source/semantic/cache admission and protected canonical history.
No feature/host/release increment: solid63, weighted CAD52.10 / agent94 unchanged.

MCAD-P16-015 is complete in PR #114 (merge 82e88a7), after all four exact-head
workflows and applicable jobs succeeded. Local mandatory checks passed with
1120 tests successful, 0 failed and 35 ignored, plus the agent-plugin checks.
Runtime infrastructure receives no maturity increment.

MCAD-P16-016 accepts [ADR-069](../adr/ADR-069-guarded-nominal-thread-source.md)
before P16-007 source implementation. It defines the required nominal source
discriminants, original authored circle/cylinder and phase frame, mandatory owned
consumed side, explicit coordinated stock/thread edits, and all-new output
producers for noncircular caps and actual native split faces. Closed schemas,
no-inference migration/genuine old-reader refusal, warm-cache re-admission,
mandatory native candidate verification, canonical history/failure protection
and a source-aware independently graded real MCP eval remain implementation
requirements. Ambiguous thread-surface roles refuse; no native subface stability
or general downstream thread editing is promised. This documentation-only task
does not add the proposed wire tag, source feature, patch capability or eval.
P16-007 remains planned. No feature credit: weighted CAD52.10 / agent94 unchanged.

MCAD-P16-016 is complete in PR #115 (merge edfb059), after its documentation-path
applicable CI and Design Review workflows and every job succeeded. Desktop and
Release PR workflows were inapplicable by their declared path filters. All four
mandatory local checks passed (1120 passed, 0 failed, 35 ignored, plugin passed).
Design receives no feature credit; actual weighted CAD52.10 / agent94 unchanged.

MCAD-P16-007 implements ADR-069 through the distinct closed nominal_metric_thread
source. Required basic-flat-root representation/coarse size/explicit hand/whole
turns/square ends retain authored manufacturing intent; current circle/distance/
NewBody stock, checked sketch phase and owned full cylindrical side are mandatory.
Raw fallbacks and strict dimensional expressions validate before parameter
application, including empty/absent scopes. Competing stock/side aliases and stale
optional annotations refuse. No implicit stock resizing or ref weakening occurs.
All real output faces belong to the thread, including noncircular caps/native
split faces. Current unique end discovery is required; shared matching/resolution/
provenance refuses transient thread-surface subface indexing. No edge/ancestry
claim is fabricated. Actual cached outputs recheck source, mass/area/COM, closed
face mesh and requested boundary phase; an opposite-hand cache body fails despite
equal scalar properties. Both kernels retain the original stock on failures.
Strict schemas, no-inference migration, genuine older-reader refusal, deterministic
packed/expanded/source-authoring round trips and canonical transaction history
accompany the tag. CLI/MCP native candidate verification is mandatory even with
verify:false/no-verify; actual stdio tests exercise creation, reviewed hand edit,
independently graded eval, failed-batch/artifact protection and Agent Undo/Redo.
The runnable source example, creation/left edit patches, API, architecture,
authoring guide and skill distinguish nominal geometry from production threads.

P16-007 is complete in PR #116 (merge 508e7c8). All four mandatory local checks
passed (1129 passed, 0 failed, 36 ignored, plugin passed), and every applicable
exact-head CI, Design Review, Desktop and Release job succeeded; the PR release
publisher was skipped. Solid63→65 earns partial fixed-rubric credit
for usable source-authored nominal metric external threads in both kernels,
independent physical pitch/radius/hand/volume/area/COM checks, protected source/
ref/cache/history and native verified MCP edits. Weighted CAD52.10→52.60 (about
53%), agent94→95 for the new independently graded source-aware authoring task.
Other scores/weights remain unchanged. Actual weighted CAD52.60 / agent95.
No extra credit for ADR-067/068/069 planning/runtime, legacy generic helix or
nominal calculations alone; no production fit, rounded-root, partial/internal/
multi-start/runout, general downstream thread-face editing or host/release claim.

MCAD-P16-017 supplies ADR-070's closed circular variable-fillet reference and
acceptance gates before P16-008 implementation. The target is an original authored
cylinder's owned complete end rim, a positive single-period cosine radius law
and sketch-defined phase independent of native seams. It derives the ideal sphere
envelope/contact equations and independent constant-limit removed volume, with
nonconstant surface-integral, ownership, wrong-phase cache, reader/patch/history
and MCP completion requirements. Legacy Mock variable fillets change IDs only;
that path supplies no physical proof. A direct pinned-OCCT SDK probe produced
valid constant/variable circular solids, with approximately 1 micrometre sampled
contact error for the variable fixture; ADR-070 records its recipe and explicitly
unbounded exploratory integral comparison. This is not an authored feature or
general geometry/sampling proof. Native radius coordinates/periodicity,
global regularity/exposure, physical Mock and bounded construction/sampling still
require evidence and a further runtime ADR before source implementation.
P16-008 remains planned. P16-017 is design only: no new kernel body, source tag,
schema, example or runnable MCP operation, and no score. Actual52.60 / 95.

P16-017 is complete in PR #117 (merge a7a57f9) after all mandatory local checks
(1129 passed, 0 failed, 36 ignored, plugin passed) and every applicable exact-head
CI / Design Review job succeeded. Desktop/Release PR workflows were inapplicable
by their path filters. No design score: actual52.60 / 95.

MCAD-P16-018 implements ADR-071's read-only body-owned full-cylinder rim witness.
Explicit analytic Mock stores deterministic current rim IDs with real cylinder
and cap/side records, and exact circle queries refuse stale/foreign/legacy IDs.
Native OCCT proves the full stock and actual cap/side edge incidence using IsSame
(TShape plus Location). The regression fixture's translated cap circles share a
TShape, so a pointer-only test cannot distinguish them. Public native IDs retain
the pinned cadrum unique-edge iterator convention and pass actual edge/circle
queries. Independent centre/plane/radius/circumference, mass/area/COM, closed mesh,
proper placement, reversed hints, equal-volume different stocks and protected
refusal are checked in both kernels. No new feature/schema/MCP command or fillet
solid/sampling is admitted. P16-008 remains planned; no score, actual52.60 / 95.

P16-018 is complete in PR #118 (merge 21e35bd), after all four mandatory local
checks (1134 passed, 0 failed, 36 ignored, plugin passed) and all applicable
exact-head CI/Design Review/Desktop/Release jobs succeeded; the PR release
publisher was intentionally skipped. The optional OCCT-disabled build probe
failed at unchanged pre-existing ungated conversion imports; no such configuration
readiness is claimed. This read-only prerequisite earns no score: actual52.60 / 95.

MCAD-P16-019 implements ADR-072's pure CircularFilletReference: explicit SI lengths
and angles, ideal cosine sphere-envelope points/normals and analytic parameter
derivatives, deterministic seam, exact constant-limit volume and a conservative
global exposure factor for r_max<=R/4, H>=4*r_max. The trigonometric-polynomial
factorization proves exposure/injectivity and regularity over all angles in real
arithmetic; finite identity, tangent, contact and independent volume tests check
its implementation. This is not a floating-point interval certificate, numerical
quadrature bound or native approximation admission. Nonconstant volume requests
refuse, including one-ULP positive amplitudes. No kernel solid/mesh/source tag,
schema, sampling or MCP operation is supplied. P16-008 remains planned; reference
arithmetic earns no feature credit, actual52.60 / 95.


P16-019 is complete in PR #119 (merge ab91af1), after all four mandatory local
checks (1139 passed, 0 failed, 36 ignored, plugin passed) and all applicable
exact-head CI/Design Review/Desktop/Release jobs succeeded; the PR release
publisher was intentionally skipped. No score: actual52.60 / 95.

MCAD-P16-020 implements ADR-073's bounded ideal nonconstant reference volume.
The removed-corner divergence integral is analytically reduced over the normal's
characteristic circle, then enclosed on four rational angular charts using
outward binary64 arithmetic, a bounded atan series and explicit absolute SI width
and leaf budgets. It refuses unmet precision/budget; identical authored radii
use a bounded constant formula, positive amplitudes never snap constant. Independent
original surface integrals, constant meridional integration, scales, refinement,
determinism and protected refusals check the implementation. No Mock/OCCT solid,
native approximation, mesh/source/schema/MCP operation or feature credit is
introduced. P16-008 remains planned; actual52.60 / 95.


P16-020 is complete in PR #120 (merge d1a77ef), after all four mandatory local
checks (1147 passed, 0 failed, 36 ignored, plugin passed) and all applicable
exact-head CI/Design Review/Desktop/Release jobs succeeded; the PR release
publisher was intentionally skipped. No score: actual52.60 / 95.

MCAD-P16-021 / ADR-074 supplies a standalone pinned-Linux native diagnostic,
separate from product binaries. It measures actual single-solid BREP contacts,
interior inferred sphere support/radius and native volume estimates, then invokes
the actual Rust bounded ideal reference. Constant, dense-law, tighter-tolerance,
C2 and properly rotated wrong-phase controls distinguish native fidelity from
stored-law or scalar-volume agreement. Live measurements reproduce approximately
1µm top-contact discrepancy and a native volume estimate outside the ideal
interval for the tested variable recipes. Finite failure witnesses reject these
recipes; finite agreement would not establish full admission. Report-contract
negative tests and a live CI diagnostic preserve the evidence; no GeometryKernel
operation, supported source/schema/MCP feature, production sampling or score is
introduced. P16-008 remains planned; actual52.60 / 95.

P16-021 is complete in PR #121 (merge 04c5251), after all four mandatory local
checks (1147 passed, 0 failed, 36 ignored, plugin passed), six Python report tests,
local live diagnostic/workflow policy and all applicable exact-head CI jobs
succeeded. CI also ran the actual native diagnostic successfully and saved its
JSON artifact; optional artifact download from this cloud was access-restricted,
so its body was not independently inspected locally. The PR publisher was
intentionally skipped. No score: actual52.60 / 95.

MCAD-P16-022 / ADR-075 supplies bounded rational-chart reference patches before
investigating direct controlled approximation. Private second-order interval jets
enclose ideal positions/normals/chart derivatives and Hessian norms; an all-patch
Taylor/barycentric bound plus rounded-corner uncertainty encloses positional
linear interpolation error. Exact declared seams and shared corners produce
identical representative vertex bits. Shared interval primitives preserve the
existing volume algorithm. Independent constant-limit derivatives, angular and
numerical derivative oracles, interpolation, refinement/scales and boundary
refusals check the implementation. No mesh, actual Mock/native fit or body,
source/schema/MCP operation or editable NURBS is supplied. Position error alone
cannot admit geometry; P16-008 remains planned, actual52.60 / 95.


P16-022 is complete in PR #122 (merge b22abb0), after all mandatory local checks
(1154 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. The PR publisher was
intentionally skipped. No score: actual52.60 / 95.

MCAD-P16-023 / ADR-076 constructs transient bicubic reference poles from corner
positions/derivatives and bounds position error via the bilinear reference bound
plus a degree-elevated Bernstein control-net deviation. Rounded shared boundary
controls are identical, including quarter-chart wraparound. Independent geometry
tests and a live pinned OCCT diagnostic compare actual poles/degrees, positions
and derivatives across forty constant/nonconstant/contact/interior/seam cases.
Negative report controls reject displaced surfaces and corrupt measurements.
No native or Mock solid, support trimming, normal/G1/volume certificate, source,
schema or MCP operation is supplied. P16-008 remains planned; actual52.60 / 95.


P16-023 is complete in PR #123 (merge 0962891), after all mandatory local checks
(1158 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. Native Bezier diagnostic and
artifact upload succeeded in CI; the PR publisher was intentionally skipped.
No score: actual52.60 / 95.

MCAD-P16-024 / ADR-077 supplies whole-patch derivative-control-hull certificates
for the actual current reference poles: positive local Jacobian area in m²,
ideal-normal orientation and unit-normal component/error bounds. Recompute
without trusting stored position metadata; nonfinite, degenerate, reversed,
interior-corrupted or precision-uncertifiable cases refuse. Independent planar,
Bernstein/angular and scaling tests check the implementation. The live native
diagnostic measures normals/area at all 1000 samples and full-cycle seam normal
gaps; its numerical allowances remain separate from mathematical bounds. No
G1, support trimming, global embedding, physical Mock/native solid, source/schema
or MCP operation is admitted. P16-008 remains planned; actual52.60 / 95.


P16-024 is complete in PR #124 (merge ea822ab), after all mandatory local checks
(1162 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. Native normal/area diagnostic
and artifact upload succeeded; the PR publisher was intentionally skipped.
No score: actual52.60 / 95.

MCAD-P16-025 / ADR-078 builds a deterministic conforming full-period reference
band by uniform dyadic refinement, with explicit SI position and dimensionless
normal-error requests and bounded patch-count refusal. All cells require current
position/normal certificates; no partial band or sample-based fallback is returned.
Independent coverage/shared-edge/contact/refinement/scale/budget tests and a live
native sewing diagnostic check the implementation. Actual native open-shell face
connectivity, incidence, two boundary cycles, Euler characteristic and sewn face
surfaces are measured. No exact G1/global embedding, support trimming, physical
Mock/native solid, mass, mesh policy, source/schema/MCP operation or feature
admission is introduced. P16-008 remains planned; actual52.60 / 95.


P16-025 is complete in PR #125 (merge e6ca7b7), after all mandatory local checks
(1166 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. Native band sewing diagnostic
and artifact upload succeeded; the PR publisher was intentionally skipped.
No score: actual52.60 / 95.

MCAD-P16-026 / ADR-079 closes actual diagnostic bands with polynomial side skirts
and planar caps, producing valid connected closed native positive-volume solids
at two refinement levels for constant/nonconstant laws. Actual incidence/closure,
cap orientation, surface witnesses, native GProp stability and volume differences
against matching bounded ideal references are measured. Skirt/cylinder error is
explicit: these skirts are not exact trimmed stock supports. Six synthetic report
contract tests preserve negative gates; live CI performs actual native construction.
No production GeometryKernel/Mock body, G1/global embedding/independent solid mass
certificate, mesh, source/schema/MCP operation or feature admission is supplied.
P16-008 remains planned; actual52.60 / 95.


P16-026 is complete in PR #126 (merge 38ca404), after all mandatory local checks
(1166 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. Actual closed native diagnostic
and report upload succeeded; PR publisher was intentionally skipped.
No score: actual52.60 / 95.

MCAD-P16-027 / ADR-080 independently bounds signed volume/first-moment boundary
integrals of current mathematical polynomial band/skirt/cap geometry with outward
Bernstein arithmetic. Native GProp COM/moments are measured separately and
compared using explicit numerical allowances; these comparisons do not certify
all native sewing/conversion/integration error. Independent analytic prism,
translation/scaling/orientation/current-pole and high-order quadrature controls
plus invalid-boundary/report refusals exercise the contract. No embedding/G1,
unchanged cylindrical support, physical Mock/product body, source/schema/MCP
operation or feature admission is added. P16-008 remains planned; actual52.60 / 95.


P16-027 is complete in PR #127 (merge 6fbb201), after all mandatory local checks
(1171 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. Native independent mass/COM
comparison and artifact upload succeeded; PR publisher was intentionally skipped.
No score: actual52.60 / 95.

MCAD-P16-028 / ADR-081 bounds current mathematical band/skirt parametric area
with outward Bernstein cross-product integration, deterministic restriction
refinement and explicit SI width/work refusal. Planar caps and global embedding
are excluded. Shared private polynomial arithmetic and band validation preserve
ADR-080 mass controls. Four additional geometry regressions cover analytic
side-area/scaling/reflection, actual-pole/refinement/independent quadrature and
refusals; eight synthetic report tests include corrupted area inventories. Live
native diagnostics measure each curved face, separate caps and total area and
compare the curved sum independently. No physical Mock/product/source/MCP body,
G1/support/whole-solid fidelity or native-error admission is added. P16-008 remains
planned; actual52.60 / 95.


P16-028 is complete in PR #128 (merge 01874c4), after all mandatory local checks
(1175 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. Actual independent curved-area
comparison and artifact upload succeeded; PR publisher was intentionally skipped.
No score: actual52.60 / 95.

MCAD-P16-029 / ADR-082 constructs immutable positive-weight rational support
contacts with exact encoded side circle/end-plane identities and outward
whole-curve positional bounds including actual weights/poles/endpoint rounding.
Actual native edges form four valid closed contact wires; current native controls,
positions/D1, physical contact law and rigid-turn phase/length controls are measured.
Four independent geometry regressions and five synthetic report controls exercise
the contract; CI runs actual construction. No support trimming, compatible blend
surface/G1, global embedding, physical Mock/product/source/MCP body or feature
admission is added. Existing band certificates cannot be retained after swapping
new boundaries. P16-008 remains planned; actual52.60 / 95.

P16-029 is complete in PR #129 (merge 235d0f4), after all mandatory local checks
(1179 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. Native rational edge/wire
construction and report upload succeeded; PR publisher was intentionally skipped.
No score: actual52.60 / 95.

MCAD-P16-030 / ADR-083 constructs actual retained cylinder quadrants and top-plane
faces from rational contact edges on unchanged analytic supports. Native projected
pcurves and forced SameParameter use explicit numerical gates, with current
support frames, contact controls/ranges, edge/vertex tolerances, boundary witnesses,
valid closed face topology, retained-region classification and independent area
expressions. A real quarter-turn transform preserves constant-law regions and
fails variable-law authored phase. Seven synthetic report tests exercise negative
parsing gates; live CI constructs the faces and uploads its measurements. Wire
construction may copy edges, so no retained TShape/semantic ownership is claimed.
No global pcurve certificate, compatible blend/G1, sewn shell/solid, physical Mock,
mesh, source/schema/MCP operation or feature admission is supplied. Existing band
certificates cannot transfer to these contacts. P16-008 stays planned; actual52.60 / 95.

P16-030 is complete in PR #130 (merge 19d3550), after all mandatory local checks
(1179 passed, 0 failed, 36 ignored, plugin passed) and all applicable exact-head
CI/Design Review/Desktop/Release jobs succeeded. Native analytic support trimming
and report upload succeeded; PR publisher was intentionally skipped.
No score: actual52.60 / 95.

MCAD-P16-031 / ADR-084 directly represents the ideal characteristic circle with
fixed positive-weight degree-(8,2) rational charts, an orientation-preserving
stereographic reparameterization of ADR-072. Private immutable actual controls
have outward whole-patch position bounds. A separate fixed 256-span analytic
Hermite cylinder pcurve includes fourth-derivative and stored-control rounding
bounds; it resolves a native projection recipe that passed positional gates but
failed the unchanged normal gate. Native blend faces retain supplied low-degree
contact edges with explicit linear blend pcurves, and unchanged cylinder/planar
supports sew into two actual closed connected positive-volume diagnostic solids.
Current surfaces/contacts/pcurves, off-knot paired owner normals, two-face edge
incidence/Euler/orientation, independently bounded ideal/constant volume and
actual transformed-solid phase/classifier controls are measured. Five independent
Rust geometry tests and nine synthetic parser tests protect the contract; CI
performs actual native construction and uploads its report. No global rounded/
native normal/G1/embedding certificate, physical Mock/production runtime/mesh,
source/schema/MCP operation or editable NURBS is admitted. Previous polynomial
certificates do not transfer. P16-008 stays planned; actual52.60 / 95.


PR #131 merged P16-031 into main at
`79421a4956d6ba17d4c8858da8981275565079cc` after all four applicable exact-head
workflows and their complete job inventories succeeded (release publisher
intentionally skipped). Required local checks passed: 1184 Rust tests succeeded,
36 ignored, plus agent-plugin validation. Actual native construction/report upload
succeeded. No score: actual52.60 / 95.

MCAD-P16-032 / ADR-085 adds complete closed-chart rounded rational derivative,
normal, positive Jacobian and orientation certificates tied privately to the
construction reference. Outward homogeneous differences include actual pole/weight
rounding; derivatives retain original u/v scale through shared de Casteljau
restriction. An analytic ideal Jacobian bound and deterministic bounded complete
cover prove regularity and normal precision, or refuse without mutation/partial
certificates. Mathematical support/seam normal bounds follow; native D1/Jacobian,
analytic constant derivatives and paired owner normals remain numerical witnesses
with separate allowances. Independent analytic/scale/boundary tests, actual-control
negative tests and synthetic report refusals protect the contract. No global native
G1/embedding, physical Mock/runtime/mesh, source/schema/MCP operation/eval or editable
NURBS admission; P16-008 remains planned. No score: actual52.60 / 95.

P16-032 local mandatory checks passed before push: formatting, workspace all-target
Clippy with warnings denied, 1189 successful Rust tests (0 failed, 36 ignored),
and agent-plugin validation. Eight rational geometry tests, two private control
refusals, eleven synthetic report tests and the actual pinned native probe pass.
Eight complete chart certificates use 411 total leaves; maximum normal bound
9.973e-10 and minimum Jacobian lower bound 2.393e-6 m². Native evidence remains
numerical, not a global certificate. Completion requires this PR's merge.


PR #132 completed P16-032 at main
`bcbbdcd14e31ac10fe3df280357c58f2a5d330b2`, after all four exact-head applicable
workflows and full job inventories succeeded (PR publisher intentionally skipped).
Local required checks passed with 1189 Rust successes, 0 failures, 36 ignored
and agent-plugin validation. Actual native construction and report upload
succeeded. No score: actual52.60 / 95.

MCAD-P16-033 / ADR-086 certifies captured CURRENT post-sewing native rational
surface definitions instead of transferring supplied-control certificates from
sampled agreement. A fixed immutable descriptive snapshot stores outward SI
homogeneous intervals from original millimetre poles and positive weights;
common-weight gauge normalization carries its uncertainty. A shared complete
cover proves requested position AND normal precision, positive Jacobian/
orientation and derivative errors, or refuses deterministically without partial
output/mutation. Native capture includes current raw controls, domain, degree,
nonperiodicity, identity Location and orientation. A bounded-stdin Rust example
recomputes all eight proofs and echoes actual inputs for strict capture/reference/
request binding. Independent analytic/scale/gauge/unit/phase/refusal tests and
synthetic report tests protect the contract; CI runs the actual native capture
and proof. This certifies underlying surface definitions, not floating native
evaluation, trims, contact coincidence, exact G1, embedding or whole-BREP fidelity.
No physical Mock/runtime/mesh, source/schema/MCP operation/eval or editable NURBS
admission. P16-008 remains planned; no score, actual52.60 / 95.

The live Rust snapshot process also refuses over-budget input bytes, invalid
chart and unattainable last-request precision with no partial certificate stdout.
An independent exact binary64-fraction oracle tests native-to-SI coordinate
enclosures, including finite-input conversion-overflow refusal.

P16-033 local mandatory checks passed before push: formatting, workspace all-target
Clippy with warnings denied, 1193 successful Rust tests (0 failed, 36 ignored),
and agent-plugin validation. Eleven rational geometry tests, three private
control/conversion tests, thirteen synthetic report tests, seven support-report
tests and the final live pinned native capture/proof all pass. Eight current
native definition certificates use 649 leaves; maximum position bound
1.277e-15 m, maximum normal bound 9.998e-10 and minimum Jacobian lower bound
2.393e-6 m². All three real-process refusal cases emit no partial stdout.
Completion requires this PR's merge; no score, actual52.60 / 95.

PR #133 completed P16-033 at main
`52274847083e18bc1aeba7f4bd5357f7b57ecfe5`, after all four exact-head applicable
workflows and full job inventories succeeded (PR publisher intentionally skipped).
Actual native capture/proof and diagnostic upload succeeded. No score:
actual52.60 / 95.

MCAD-P16-034 / ADR-087 adds immutable CURRENT native 3D contact curve snapshots
and whole-curve side/end position certificates with outward raw mm-to-m conversion,
common weight gauge and positive Bernstein denominator hulls. The pinned native
probe captures current post-sewing definitions with explicit identity Location,
fixed degree/knots/range and nonperiodicity, then recomputes sixteen proofs in
a bounded read-only Rust process. Strict echo binding ties every proof to raw
capture, law, kind, chart and precision. Independent analytic/scale/gauge and
deformation/unit/phase/precision tests complement synthetic parser refusals and
real-process no-partial-output controls. This proves 3D curve definitions only:
attached pcurve lifts/full contact gaps, native floating evaluation, derivatives,
exact G1, embedding and whole-BREP fidelity remain separate. No feature, Mock/
runtime body, source/schema/MCP operation/eval or editable NURBS admission.
P16-008 remains planned; completion requires merge; no score, actual52.60 / 95.

P16-034 mandatory local checks passed before push: formatting, workspace all-target
Clippy with warnings denied, 1197 successful Rust tests (0 failed, 36 ignored),
and agent-plugin validation. Three new integration geometry tests and a private
exact-fraction conversion test pass, alongside fifteen synthetic reference-report
tests and seven support-report tests. Actual pinned native capture produces
sixteen complete current 3D curve-definition certificates with maximum position
bound 1.070e-15 m against 1e-12 m requested. All six real-process refusal controls
emit no partial stdout. Existing reference data and eight surface certificates
are unchanged. Completion requires merge; no score, actual52.60 / 95.

PR #134 completed P16-034 at main
`0f3957e31be000ad07781223670418c00caab8c6`, after all four applicable exact-head
workflows and their full job inventories succeeded (PR publisher intentionally
skipped). Actual native capture/proof and diagnostic upload succeeded. The merge
tree equals the tested head tree. No score: actual52.60 / 95.

MCAD-P16-035 / ADR-088 certifies complete current END/PLANE contact boundary
position gaps. An immutable planar pcurve snapshot lifts raw native mm UV controls
through the current raw plane frame with outward SI/homogeneous arithmetic.
Recomputed whole edge/lift/surface definition certificates share their ideal end
contact, yielding outward pairwise gap sums under explicit precision/work refusal.
The actual attached blend line must be canonical (t,1) over [0,1], established
from current representation metadata rather than samples. A bounded Rust process
qualifies lines and recomputes eight proofs with strict capture/reference/request
binding and no partial stdout on refusal. Independent affine/analytic scale/gauge/
deformation tests and actual native capture complement synthetic parser checks.
Side contacts, quarter seams, floating native evaluation, paired normal agreement,
exact G1, global embedding and whole-BREP fidelity remain separate. No production
Mock/runtime, mesh, source/schema/MCP operation/eval or editable NURBS admission;
P16-008 stays planned. Completion requires merge; no score, actual52.60 / 95.

P16-035 mandatory local checks passed before push: formatting, workspace all-target
Clippy with warnings denied, 1202 successful Rust tests (0 failed, 36 ignored),
and agent-plugin validation. Four new geometry integration tests, a private
exact-fraction affine conversion/overflow test, seventeen synthetic report tests
and seven support-report tests pass. Actual pinned native capture/proof yields
eight complete end-boundary certificates with maximum pairwise gap bound
2.377e-15 m against 1e-12 m requested. All eight real-process refusal controls
emit no partial stdout. Existing reference, eight native surface certificates
and sixteen 3D contact certificates are unchanged. No score: actual52.60 / 95.

PR #135 completed P16-035 at main
`b345f1b386f8cf7a9e0ee269a5df6728e19c6eaa`, after all four applicable exact-head
workflows/full job inventories succeeded (PR publisher intentionally skipped).
Actual native boundary proofs and report upload succeeded; merge tree equals
tested head tree. No score: actual52.60 / 95.

MCAD-P16-036 / ADR-089 certifies complete current cylindrical SIDE position
gaps among native 3D edge, attached cylindrical-pcurve lift and blend boundary.
An immutable descriptive snapshot encloses original angle_rad/height_mm controls
and the actual owning cylinder's raw origin/axes/radius. Complete fixed-basis
control-difference hulls, supplied Hermite remainders, global circle Lipschitz
and outward frame/radius error bounds certify the whole lift without sampling.
Recomputed edge/lift/blend certificates share the ideal side contact; all three
outward pairwise gap sums must meet explicit precision/work budgets. Current
unit weights/basis/range and canonical attached blend lines (t,0) are qualified
from actual data. A bounded read-only Rust process proves eight captures and
echoes every input/budget for strict binding, with no partial stdout on refusal.
Independent analytic/cubic/frame scale/deformation tests complement actual native
proof/refusal controls and synthetic parser tests. Existing end-boundary and
surface/edge certificates remain intact. Floating native evaluation, quarter
seams, paired normals/G1, embedding and whole-BREP fidelity remain separate.
No production Mock/runtime, mesh, source/schema/MCP operation/eval or editable
NURBS admission. P16-008 stays planned; no score, actual52.60 / 95.

P16-036 mandatory local checks passed before push: formatting, workspace all-target
Clippy with warnings denied, 1207 successful Rust tests (0 failed, 36 ignored),
and agent-plugin validation. Four new geometry integration tests, a private
exact-fraction conversion test, nineteen synthetic report tests and seven
support-report tests pass. Actual pinned native capture/proof yields eight
complete side-boundary certificates with maximum pairwise gap bound 3.741e-13 m
against 1e-12 m requested. All eleven real-process refusal controls emit no
partial stdout. Existing reference, surface, 3D contact and end-boundary
certificates are unchanged. Completion requires merge; no score, actual52.60 / 95.


PR #136 completed P16-036 at main
`eb3cbc5c426b61988e00c1eb6376a181d558dd37`, after all four applicable exact-head
workflows/full job inventories succeeded (PR publisher intentionally skipped).
Native side proofs/contract/upload succeeded; merge tree equals tested head tree.
No score: actual52.60 / 95.

MCAD-P16-037 / ADR-090 registers complete current quarter seam POSITION gaps.
Actual post-sewing quadratic Bezier edge definitions and canonical attached
(1,t)/(0,t) lines bind current adjacent surface boundary rows. Direct outward
homogeneous comparisons cover the full original unit span and include raw mm-to-m
conversion and positive common weight gauges. All three pairwise gaps must meet
explicit SI precision; bounded read-only transport emits no partial result.
No paired-normal/G1, floating native evaluation, embedding, unchanged support seam,
whole-BREP, production Mock/runtime/mesh/source/schema/MCP operation admission.
P16-008 remains planned; no score, actual52.60 / 95.


P16-037 mandatory local checks passed before push: formatting, workspace all-target
Clippy with warnings denied, 1212 successful Rust tests (0 failed, 36 ignored),
and agent-plugin validation. Four new independent geometry tests, one private
exact-fraction conversion test, twenty-two synthetic report tests, seven support
report tests and repository hygiene pass. Actual pinned native capture yields
eight complete seam certificates with maximum gap bound 8.268e-17 m against
1e-12 m requested. All ten real-process refusal controls emit empty stdout.
Existing reference, surface, 3D contact, end and side certificates are exactly
unchanged. Completion requires merge; no score, actual52.60 / 95.


PR #137 completed P16-037 at main
`31faa9a7998e92d444ab509abc8e74964bc08419`, after all four exact-head applicable
workflows and full job inventories succeeded (PR publisher intentionally skipped).
Test/native proof/contract/upload succeeded; merge tree equals tested head tree.
No score: actual52.60 / 95.

MCAD-P16-038 / ADR-091 adds complete corresponding-parameter paired-normal chord
bounds for current side/end/seam boundaries. Four actual surface certificates
and cylindrical lifts are recomputed once per case. Outward actual support-frame
cross-product/normalization errors and whole actual pcurve angle errors combine
with ideal radial/axial/shared-seam identities; all twelve gaps per case must meet
explicit precision. Bounded read-only two-case transport binds all raw inputs,
charts, canonical attached lines, unit weights and precision/work budgets with
no partial stdout. Finite normal gaps do not establish exact G1, native floating
evaluation, embedding, untouched support seams or whole-BREP/production fidelity.
No Mock/runtime/mesh/source/schema/MCP operation/eval or editable NURBS admission.
P16-008 remains planned; no score, actual52.60 / 95.


P16-038 mandatory local checks passed before push: formatting, workspace all-target
Clippy with warnings denied, 1216 successful Rust tests (0 failed, 36 ignored),
and agent-plugin validation. Three new independent geometry integration tests,
one private analytic support-frame test, twenty-four synthetic report tests,
seven support tests and repository hygiene pass. Actual pinned native capture
produces two complete cases / 24 paired normal gap bounds with maximum
4.994e-10 against 1e-9 requested. All eleven real-process refusal controls emit
empty stdout. Existing reference and surface/contact/end/side/seam position
certificate records are exactly unchanged. Completion requires merge; no score,
actual52.60 / 95.


PR #138 completed P16-038 at main
`411491a53dfc4ad2f65cbb08dafd29f7c724aebb`, after all four exact-head applicable
workflows/full job inventories succeeded (PR publisher intentionally skipped).
Native proof/Test/contract/upload succeeded; merge tree equals tested head tree.
No score: actual52.60 / 95.

MCAD-P16-039 / ADR-092 constructs bounded closed indexed reference triangle
shells for ideal fillet plus original cylinder/caps, sharing canonical vertex IDs
without tolerance welding. Whole Taylor/interpolation bounds include current
rounding; outward actual facet normals must align with complete ideal cell
normal enclosures. Deterministic complete dyadic refinement obeys explicit
position/normal/vertex/triangle budgets with no partial result. Immutable f64
arrays and signed boundary integrals do not authorize physical Mock/runtime.
Actual pinned native faceted solids independently match current vertices,
triangular connectivity/normals, volume/area/COM; all four native refusal controls
emit no partial stdout. Constant polygon-frustum volume/area/z moment and
variable ideal-volume refinement are independently tested. Global embedding,
exact G1, native floating fidelity and production/source ownership remain
separate. Existing rational-BREP proofs are unchanged. No schema/golden/new MCP
operation/eval or score admission; P16-008 stays planned, actual52.60 / 95.


P16-039 mandatory local checks passed before push: formatting, workspace all-target
Clippy with warnings denied, 1220 successful Rust tests (0 failed, 36 ignored),
and agent-plugin validation. Three independent geometry tests plus a private
malformed-shell test, four synthetic report tests, desktop workflow policy and
repository hygiene pass. Actual pinned native re-run also qualifies every current
face as planar; both fine solids have 2114 vertices / 4224 faces. Current vertex,
face connectivity/normal, volume, area, COM and first-moment comparisons pass.
All four real-process refusal controls emit empty stdout. Fine position bounds
are 3.907e-5 / 4.297e-5 m for constant/variable laws. Completion requires merge;
no score, actual52.60 / 95.

PR #139 completed P16-039 at main
`f14f9dc764c95ff5102d89a60a9b4ec19cea1b47`, after all four exact-head applicable
workflows and complete job inventories succeeded. The Windows UI startup timed
out on the first attempt and passed on one same-head failed-job rerun, without
source/assertion changes. Both merge parents match the tested base/head and the
merge tree equals the tested head tree. No score: actual52.60 / 95.

MCAD-P16-040 / ADR-093 preserves reference-mesh error bounds through the existing
TriangleSolid 1e-9 m admission grid. Actual ordered vertex/facet bijection,
outward vertex movement and facet-normal changes produce separate final-quality
certificates for the actual stored solid, with unchanged STL/intersection caps.
The native diagnostic additionally constructs and measures both fine grid cases
and runs the four empty-stdout refusal controls for both raw and grid modes.
Independent integer-grid volume/actual area, scales, determinism, correspondence,
collapse, final-quality and cap refusal tests cover the conversion. API,
architecture, example, MCP guide and skill updated; no schema/golden/new MCP
operation/eval or production kernel/source admission. Completion requires merge;
P16-008 remains planned and no score is assigned, actual52.60 / 95.

P16-040 mandatory local checks passed before push: formatting, workspace all-target
Clippy with denied warnings, workspace tests (1224 passed, 0 failed, 36 ignored),
and agent plugin contract. Three new public integration tests and one private
regression test passed; seven Python contract tests, release policy and repository
hygiene passed. The pinned native probe constructed two raw and two actual grid
solids and passed all eight empty-stdout refusal controls. Fine constant/variable
grid movement bounds are 8.075e-10 / 8.392e-10 m; final position bounds
3.907e-5 / 4.297e-5 m and normal chord bounds 0.121996 / 0.131550 meet the
requested 1e-4 m / 0.5 quality. No production feature/score admission.

PR #140 completed P16-040 at main
`fbf86df4630fb71eda1d4c8fc783584a6720c7c7`, after all four exact-head applicable
workflows and complete job inventories succeeded. Both merge parents match the
tested base/head and the merge tree equals the tested head tree. Windows native
UI passed without reruns. No score: actual52.60 / 95.

MCAD-P16-041 / ADR-094 adds typed certified-grid reference-body import through
GeometryKernel: actual analytic Mock facet records and native triangle BREP.
Both query actual mass/area/COM/bounds and facets and support proper placement;
native markers enforce proper rigid transforms through repeated/compound
placement. Legacy Mock and analytic-mode generic mesh import refuse. Analytic
Mock unsupported edits/support witnesses refuse without replacing physical
geometry with synthetic plumbing. Native face identity retains its body handle
and local index. Independent integer-grid mass, facet geometry/closure, owned
IDs, placement and refusal tests plus a runnable both-kernel example accompany
API/architecture/MCP-guide/skill updates. Input error bounds do not extend to
placed/native/f32 coordinates; no source/schema/golden/MCP operation/eval or
P16-008 feature/score admission. Completion requires merge, actual52.60 / 95.

P16-041 mandatory local checks passed before push: formatting, workspace all-target
Clippy with denied warnings, workspace tests (1227 passed, 0 failed, 36 ignored),
and agent plugin contract. Three new real backend integration tests passed both
standalone and in the full workspace; the both-kernel example created constant
and variable coarse bodies with 1088 triangles/faces each. Independent integer-
grid volume, actual area/centroid/bounds, closed/current facets and proper
placement agree. Release policy and repository hygiene checks passed. No score
is assigned; completion requires merge, actual52.60 / 95.

PR #141 completed P16-041 at main
`8b42e454c262d5d6fe77861323b8dfd09544d191`, after all four exact-head applicable
workflows and complete job inventories succeeded. Both parents match tested
base/head and the merge tree equals the tested head tree. No score: actual52.60 / 95.

MCAD-P16-042 / ADR-095 privately retains the original reference in raw/grid meshes
and adds immutable outward grid signed-volume flux versus ideal-volume interval
comparisons. Explicit SI width/work/absolute-discrepancy budgets fail closed;
backend mass stays actual and no geometry is mutated. Independent integer-grid
determinants, analytic constant-limit volume/scales, variable-law binding and
refinement/refusal tests cover the integral contract. Both physical kernels
retain actual grid mass; the example/CI report comparisons and actual masses
separately. API/architecture/MCP-guide/skill/example updated. No exact embedding,
native/placed/f32, ideal area/first-moment or source/schema/golden/MCP-operation/
eval admission; P16-008 remains planned. Completion requires merge; no score,
actual52.60 / 95.

P16-042 mandatory local checks passed before push: formatting, workspace all-target
Clippy with denied warnings, workspace tests (1232 passed, 0 failed, 36 ignored),
and agent plugin contract. Three new geometry integration tests, one exact
binary-endpoint tetrahedron unit test and a new actual both-kernel mass
comparison passed. Release policy/hygiene passed. The coarse example bounds
constant/variable absolute flux differences by 4.527e-8 / 4.547e-8 m³
(relative 0.007208 / 0.007254); the variable reference uses 242125 cells
within the unchanged 262144 cap at 1e-12 m³ width. These are input-grid
integral bounds, not native/embedding/source certificates. No score.


PR #142 completed P16-042 at main
`16c29bdb4ac3f38ae8765ffbc27c8a2610f33394`, after all four exact-head applicable
workflows and complete job inventories succeeded. Both merge parents match
tested base/head and the merge tree equals the tested head tree. No score: 52.60 / 95.

MCAD-P16-043 / accepted ADR-096 adds ideal complete/blend/support unsigned area
enclosures with analytic characteristic-arc reduction and deterministic outward
rational-chart integration. Explicit total SI width/work budgets refuse without
partial output or mutation. Independent constant/scaled solutions and original
two-dimensional surface/support integration, admitted boundary, refinement,
determinism and invalid/tiny-amplitude/refusal tests cover the API. Both-kernel
regression and example/CI keep ACTUAL faceted areas separate from ideal results.
API/architecture/guide/skill/examples updated. No source/schema/migration/golden/
MCP operation/eval, native conversion, area discrepancy or ideal first-moment
certificate; P16-008 stays planned. Completion requires merge; no score, 52.60 / 95.

P16-043 mandatory local checks passed before push: formatting, workspace all-target
Clippy with denied warnings, workspace tests (1237 passed, 0 failed, 36 ignored),
and agent plugin contract. Four new independent geometry tests and the new
actual both-kernel area regression passed (current kernel suite: 5 passed).
Release policy and repository hygiene checks passed. No score.

The fresh both-kernel example passed independent component/width/work checks and
a high-precision constant-area oracle. At 1e-7 m² total full width, variable ideal
area is [0.001857449956914607, 0.0018575499518287903] m² using 6672 leaves;
actual faceted area remains approximately 0.001850633968 m² in BOTH kernels.
Constant ideal area is approximately 0.001871360554477447 m² (zero cells).
These remain separate measurements, not a native area discrepancy certificate.


PR #143 completed P16-043 at main
`0133c2545598a71a23979e37780a8e2e22f3f6c0`, after all four exact-head applicable
workflows and complete job inventories succeeded. Merge parents match tested
base/head and the merge tree equals the tested head tree. No score: 52.60 / 95.

MCAD-P16-044 / accepted ADR-097 adds immutable stored-grid unsigned area versus
privately bound ideal-area comparisons. Outward actual facet areas, ideal total
width/work and absolute discrepancy budgets fail closed; relative bounds divide
by the positive ideal lower bound. Independent exact dyadic box/integer-grid
areas, analytic constant/scales, variable binding/refinement/determinism and
precision/work/discrepancy/refusal tests cover the contract. Both kernels retain
actual area with a separate numerical conversion allowance; the existing
example/CI reports full comparison intervals and scope flags. API/architecture/
guide/skill/example updated. No embedding/native/placed/f32, ideal first-moment,
source/schema/migration/golden/MCP operation/eval admission. P16-008 stays planned;
completion requires merge; no score, actual52.60 / 95.

P16-044 mandatory local checks passed before push: formatting, workspace all-target
Clippy with denied warnings, workspace tests (1241 passed, 0 failed, 36 ignored),
and agent plugin contract. Three new geometry tests, one exact dyadic-box unit
test and the extended actual both-kernel area comparison regression passed
(current backend suite: 5 tests). Release policy/hygiene passed. No score.

The fresh both-kernel example passed independent area/difference/component/width/
work checks and a high-precision constant-area oracle. At 1e-7 m² ideal full
width, coarse constant/variable absolute grid-area discrepancy upper bounds are
6.81745e-6 / 6.91599e-6 m² (relative 0.00364305 / 0.00372338). Actual kernel
areas stay separate. These certify input-grid mathematics, not native conversion.


PR #144 completed P16-044 at main
`01bfc76ed05f6194a0b91c333cc1f68d80eb8941`, after all four exact-head applicable
workflows and complete job inventories succeeded. Merge parents match tested
base/head and the merge tree equals the tested head tree. No score: 52.60 / 95.

MCAD-P16-045 / accepted ADR-098 adds canonical ideal first-moment/centroid
enclosures. Degree-four characteristic integration and deterministic outward
rational charts bound removed Mx/axial correction; independent volume and stock
intervals complete body moments and positive-volume division yields centroid.
Explicit SI full widths for volume/EACH moment/EACH centroid and separate per-pass
leaf caps fail closed. Independent original point/tangent divergence integrals
and constant meridional integrals, SI scales, radius boundary, signed x/exact y,
refinement/determinism/tiny-amplitude and protected precision/work refusals cover
the contract. Both physical kernels retain actual mass/centroid; example/CI shows
ideal intervals separately. API/architecture/guide/skill/example updated. No
actual-grid moment/native/placed/f32/source/schema/migration/golden/MCP-operation/
eval admission; P16-008 stays planned. Completion requires merge; no score, 52.60 / 95.

P16-045 mandatory local checks passed before push: formatting, workspace all-target
Clippy with denied warnings, workspace tests (1246 passed, 0 failed, 36 ignored),
and agent plugin contract. Four new independent geometry tests and one new actual
both-kernel mass/centroid retention regression passed (current backend suite:
6 tests). Release policy/hygiene passed. No score.

The fresh both-kernel example passed independent widths/work/symmetry/outward-
quotient checks and a high-precision constant-moment oracle. The variable case
uses 23535 volume leaves and 7944 moment leaves at 1e-11 m³ / 1e-12 m⁴ /
1e-6 m requested full widths. Ideal centroid x is
[-1.0088551874924314e-5, -9.92901510337542e-6] m and z is
[0.009977095338815828, 0.009977146449501754] m; exact y is zero. Actual
faceted z remains approximately 0.009976556160260556 m in both kernels.
These are separate ideal and actual measurements, not a native/grid-moment certificate.

PR #145 completed P16-045 at main
`f5401964f751315ec2712781ab4efa0170cec299`, after all four exact-head
workflows and complete job inventories passed (release publisher skipped as
expected). The merge tree equals tested head `6c503608745d265d392cdc4bbc33aece25bd5e41`.
Scores remain 52.60/95; P16-008 remains planned.

MCAD-P16-046 is registered before implementation under accepted ADR-099:
compare actual stored-grid signed first moments and positive-volume centroid
quotients with the privately bound ideal. No source feature admission or score.

P16-046 implements signed tetrahedral fluxes from actual stored-f64 triangles,
positive-volume outward centroid quotients and privately bound ideal differences.
Both finite positive SI budgets apply to each coordinate; invalid precision/work,
nonfinite/invalid division or any excess refuses without mutation. Independent
exact dyadic tetrahedron/translated-box endpoint comparisons and integer-grid
moments cover scales, analytic constants, variable binding/refinement/replay and
protected failures. A separate physical both-kernel regression and example/CI
report preserve actual measured mass/centroid. No schema/golden/new MCP operation
or eval, native/embedding/source admission or score; P16-008 stays planned.

P16-046 mandatory pre-push checks passed: formatting, workspace all-target
Clippy with denied warnings, workspace tests (1251 passed, 0 failed, 36 ignored)
and agent plugin contract. Release policy/hygiene passed. New tests comprise
one exact dyadic unit test, three independent geometry tests and one physical
both-kernel comparison regression (current backend suite: 7 passed).

The fresh example passed independent high-precision constant and outward
interval quotient/difference/budget/work checks. Maximum per-coordinate centroid
errors are 1.363093368143831e-7 m (constant) and 5.902892433531927e-7 m
(variable); maximum first-moment errors are 4.53248010261894e-10 m⁴ and
4.5724731845054467e-10 m⁴ respectively, below explicit 2e-6 m / 1e-9 m⁴
budgets. Ideal work remains 23535 volume / 7944 moment leaves for the variable
case. These are signed boundary-integral/quotient comparisons, not native or
exact global embedding certificates. Actual backend mass/centroid stay separate.

PR #146 completed P16-046 at main
`73eff3a795b86ecada2e760bae1b03c2e275afef`, with parents matching tested
base/head and an identical merge/tested-head tree. All four exact-head workflows
and full job inventories succeeded (release publisher skipped as expected).
Scores remain 52.60 / 95; P16-008 stays planned.

MCAD-P16-047 is registered before implementation under accepted ADR-100:
a guarded physical grid-faceted periodic fillet operation on an actual current
cylinder/end-rim pair with explicit phase, quality and discrepancy budgets.
Source/history/schema/MCP admission remains a separate P16-008 gate.

P16-047 implements the named numerical `grid_faceted_periodic_cosine` runtime.
Current cylinder/end-rim and explicit proper phase precede privately generated
canonical grids and all quality/integral error gates. Analytic Mock stores actual
facets; OCCT builds the actual BREP and applies a full SI matrix, then rechecks
placed solid validity/self-intersection/actual volume before final insertion.
Stock stays immutable and all output facets are new; default Mock refuses and
counting forwards. Independent integer-grid/constant analytic mass, physical
contacts, scaled/placed/small-phase/seam controls and protected failures cover
both kernels. Dedicated example plus existing CI report, API/architecture,
guide and skill document the numerical scope. Canonical bounds do not certify
native/placed/f32 conversion. No source/schema/golden/MCP-operation/eval or
maturity credit; P16-008 stays planned pending its separate source contract.

P16-047 mandatory pre-push checks passed: formatting, workspace all-target
Clippy with denied warnings, workspace tests (1257 passed, 0 failed, 36 ignored)
and agent plugin contract; release policy/hygiene also passed. Six new shared
physical regressions passed (3 analytic Mock, 3 OCCT), covering independent mass,
contacts/placement/phase, stock/face ownership and protected refusals.

Both documented example entry points executed successfully. Independent checks
validated unchanged canonical arithmetic, all explicit budgets and scope flags,
constant analytic moments, actual both-kernel mass/area/placed centroid, four
contact vertices, owned facets and stock preservation. The placed variable case
retains actual volume approximately 6.22297048807090e-6 m³ and 1088 new facets;
maximum measured contact-vertex discrepancy is 3.649659129610206e-9 m, below the
separate 1e-8 m numerical allowance. Its canonical volume discrepancy bound is
4.54712916126606e-8 m³ (1e-11 m³ ideal width). These measurements do not certify
native/placed/f32 conversion. No score: 52.60 / 95.

P16-047 merged as PR #147, merge commit
`78f62e25eb43a413ea2dbb15097dea3b3cdfa14b` with tested head
`9f94738d4a584ebb2243ee64a896ef5ffa75e737` and identical merge/head trees.
All four exact-head workflows and full eleven-job inventories passed (release
publisher skipped as expected). No maturity credit: runtime only, 52.60 / 95.

P16-008 begins under accepted ADR-101 before source implementation: distinct
numerical faceted circular-fillet intent, canonical-grid-only quality bounds,
actual source/rim/phase/facet cache checks and strict transaction/reader/MCP
acceptance. No score is assigned before the completed feature main merge.

P16-008 implements the distinct ADR-101 `faceted_circular_fillet` source with
mandatory numerical representation/verification tags and explicit SI quality.
Original authored cylinder/positive-X phase/current owned complete end-rim,
strict fallback/parameter admission, all-new physical facet ownership and actual
geometry cache checks accompany the existing physical Mock/OCCT runtime.
Both schemas, no-inference migration, genuine pre-extension enum refusal,
expanded/packed/authoring round trips, typed/coordinated DesignPatch history,
a source example, guide/skill and actual independently graded physical MCP eval
are included. Canonical quality bounds remain separate from native/placed/f32
numerical consistency. Exact smooth/G1/general contour support is excluded.
No score increase is assumed for this bounded numerical approximation.

Initial targeted physical source tests passed in both kernels, including cold/
warm ownership and equal-volume/area/COM wrong-phase cached shells that reach
and fail the actual corner/phase gate. Persistence/history tests and both-schema
29-case refusal matrices passed. Fresh native edited output measures
6229.982623368721 mm³ with 1088 physical facets; independent STL integration,
closure/dimensions and maximum/minimum phase contacts agree. Final workspace
and actual MCP/CI evidence are required before completion.

The actual native MCP acceptance regression passed in the final workspace run
(510.71 seconds in this debug cloud build). It creates through DesignPatch,
checks read-only preview/review, edits radii, independently grades a fresh native
STL, refuses an equal-volume wrong-phase export, preserves source/review/history
on structural and physical budget failures even with verification disabled by
the caller, and verifies canonical Undo/Redo. These are protocol/backend tests,
not new real model-host or smooth-surface certification evidence.

Additional regressions found two concrete native issues before publication.
Grid faces inherited the legacy +Z display-normal placeholder; marked grid
member faces now derive finite oriented flat display normals from actual
triangles, preserving physical coordinates/face IDs and unrelated members.
The normal-direction assertion stays enabled. Generic native edge discovery
lists only top edges, so it could not establish that a hint selected the current
start rim. Admission now uses the complete typed rim witness and direct physical
edge query, rejecting current start/seam/zero bindings. Refusal tests keep the
actual stock body warm; body-local indices are not treated as global identities.
Final checks must cover these fixes before push.

P16-008 final mandatory pre-push checks passed: formatting, workspace all-target
Clippy with denied warnings, all workspace tests without filters (1264 passed,
0 failed, 37 ignored; --no-fail-fast collects every suite), and the agent plugin
contract. Final source tests cover proper reversed/tilted frames and actual
outward display normals in both kernels. The actual native MCP regression
passed again under the final implementation in 498.12 seconds. Existing example
validation counts include the added source: state 51, structural native rebuild
34, nonempty feature-graph derivation 33. No golden changed. Main completion
requires the exact-head applicable CI and merge; scores stay 52.60 / 95.

Both native reference example entry points executed again after the display
normal change. Independent fresh-report checks passed for canonical volume/
area/moment/centroid arithmetic and budgets, actual Mock/OCCT mass and placed
contacts, all 1088 owned facets and stock preservation. Canonical-only
certificate scopes remain unchanged. Release policy, version and hygiene checks
also passed. No further source or test changes followed the successful suite.

P16-008 completed in PR #148, merge commit
`5f08c6a19217cf61c561ea7eae786a750a5094d0`, tested head
`24d88d66c0cc262fdec5930052800b794b0909bb`, based on
`78f62e25eb43a413ea2dbb15097dea3b3cdfa14b`. The merge and tested-head trees
are identical. All four exact-head workflows and full eleven-job inventories
passed: CI 38865471751, Design Review 38865471720, Desktop 38865471872 and
Release 38865471551 (release publisher skipped as expected). Fresh local
reference reports and independent checks, the actual MCP regression and all
1264 workspace tests passed. Native diagnostic artifact metadata was verified;
no artifact body was needed. No additional score for the bounded all-faceted
approximation: weighted CAD 52.60 and agent operation 95.

Phase 17 starts with P17-001 / accepted ADR-102, before new controls are
implemented. The roadmap registers purpose, dependencies, IDs, completion,
tests and risks. The contract requires atomic sketch/owner creation, explicit
global planes followed by bounded existing datum-plane chains, separate
resolved view frames and authored references, null physical evidence for
sketch-only states, shared DesignPatch/verification/history and actual new
Linux/Windows native interaction evidence. Its initial independent physical
fixture uses analytic ellipse extrusion and absolute placement controls rather
than synthetic Mock mass. Existing sketch forms remain global-only; no new
command, `.ocad` field, GUI capability or maturity increment is supplied here.
P17-002 implementation follows only after this design task's main merge.

P17-001's mandatory pre-push checks passed: formatting, workspace all-target
Clippy with denied warnings, all workspace tests without filters (1264 passed,
0 failed, 37 ignored across 182 suites), and the agent plugin contract. This
documentation-only change adds no executable test, schema, example or golden.
Changed-document links and all Phase 17 registration sections/IDs were checked.
Completion requires applicable exact-head CI and the design task's main merge;
no new GUI or host capability and no maturity increment are claimed.

P17-001 completed in PR #149, merge commit
`189f1b4ff8e44e106f1aea30495f7aa0f6e01c8c`, tested head
`68817fa909e9186fd8caaa1091d2abed5315f66e`, based on
`5f08c6a19217cf61c561ea7eae786a750a5094d0`. Merge and tested-head trees
are identical. The two applicable exact-head workflows and full three-job
inventories passed: CI 38873024523 and Design Review 38873024481. The
documentation paths did not trigger Desktop/Release. No feature or maturity
credit: 52.60 / 95. P17-002 starts under accepted ADR-102.

P17-002 implements atomic global-plane sketch creation in the shared backend,
typed Agent/MCP and native Tauri name/plane form. Backend document identity and
revision checks, deterministic free IDs, native candidate verification and
existing transaction/history publication protect one AddSketch/AddFeature
batch. Empty and point-only solved global drafts require one active owner and
no active consumer; unsupported workplanes/entities, ambiguous/suppressed
owners and contradictory drafts still fail in both cold and cached preflight.
Closed-profile consumers remain strict. No source schema or sampling changes.

Six targeted backend tests passed, including independent exact ellipse-prism
volume/absolute bounds/centroid in Mock and OCCT on XY/XZ/YZ, null sketch-only
evidence, required assertions, late collision rollback, packed/expanded round
trips and canonical Undo/Redo. The actual MCP/Agent regression passed for all
three planes and an existing-part seed; fresh STL/source grading rejects a
same-volume cyclic-axis decoy. Current guide, skill, example and eval include
the new commands. Existing sketch/parity tests and all-target Clippy passed.

Actual Linux Tauri/WebKit execution passed the extended native editor script:
11 checks, including five new creation groups (review/cancel/late reply,
identity/collision refusal preserving source/history/preview, all three planes
and canonical history, sketch-only creation, and recovery after a display
failure following a successful backend commit). This found and fixed the
omitted empty profiles array display assumption. A committed creation now
reports view-refresh failure separately from backend refusal. The same new
checks run in the required real Windows WebView2 CI gate; completion still
requires that exact-head CI, full mandatory checks and the feature main merge.
No Windows completion or maturity increment is claimed before those gates.

P17-002's four mandatory pre-push checks passed: workspace formatting,
all-target Clippy with denied warnings, the complete unfiltered workspace
suite (1,271 passed, zero failed, 37 unchanged ignored across 184 suites), and
the agent-plugin contract. This includes the new actual MCP four-case
regression and the existing long native fillet/thread physical regressions.
The locked native Tauri build, desktop workflow policy, MCP eval self-test,
documentation links and whitespace checks also passed. Implementation is
ready for review; completion and the conditional GUI 45→47 / weighted CAD
52.60→52.90 credit require the feature main merge and all applicable exact-head
CI, including the actual Windows native interaction gate. Agent operation
remains 95; no new external-host or production geometry claim is made.

P17-002 completed in PR #150, merge commit `71773f73d18d3ad5c9a1569510baebd0457dc11b`, tested head `9b91f7f9605901caa4a960c1bac62f5419e65a85`, base `189f1b4ff8e44e106f1aea30495f7aa0f6e01c8c`. Merge parents match that base/head and the tested and merged trees are identical. All four exact-head workflows and their full eleven-job inventories passed (ten successes and the expected skipped PR publisher): CI 38885414759, Design Review 38885414741, Desktop 38885414766 and Release 38885414733. Actual Linux and Windows native creation interaction gates passed. Partial fixed-rubric credit is GUI 45→47, weighted CAD 52.60→52.90; agent operation remains 95. The increment is solely the new usable sketch creation lifecycle and verified atomic source/owner/history, without duplicate geometry, form or external-host credit.

P17-003 starts under accepted ADR-102: preserve existing datum associations, regenerate source-bound display frames and refuse unsupported chains. No new score before its feature main merge and required native acceptance.

P17-003 implements existing datum-plane choices in the shared creation/editing gateways. Inspection regenerates admitted active acyclic global/custom-rooted chains and returns SI display frames separately from unchanged authored datum references. Creation preserves the legacy global JSON form, requires exactly one non-null global/datum choice and atomically derives the owner dependency. Datum edits require expected document identity and revision; upstream edits invalidate reviews. Unsupported chains, malformed/foreign/stale input and required assertions preserve source/history/output. Cold/cached profile-free drafts keep strict solve, ownership and consumer guards. No persisted schema, new geometry family or sampler is introduced.

Six targeted backend regressions passed, including both kernels on global XY/XZ/YZ roots and a tilted custom root with signed chained offsets, independent exact volume/absolute bounds/centroid, parameter and local-coordinate motion, cached/cold refusal, no-parameter contradiction guards, required assertions and packed/expanded canonical history. The actual typed MCP/Agent regression passed five cases including an existing-part seed; fresh SI STL grading checks bounds/centroid/volume/closure and rejects equal-volume wrong placement. Source catalogues are compared by stable ID and authored feature order is checked separately and exactly. Example, authoring guide, skill, API/architecture and independently graded eval are supplied.

The locked native Tauri build and actual Linux WebKit interaction script passed all 16 checks, including five new datum groups: labelled regenerated choice and cancel, atomic association/local point-ellipse edits, analytic physical prism, upstream parameter motion plus stale-command refusal without source/history/preview change, and canonical parameter Undo/Redo with subsequent local edits. The same new helper runs unconditionally in the required Windows WebView2 native gate, with its path enforced by workflow triggers and the policy contract. Linux/protocol evidence does not establish Windows completion. Whole-workspace mandatory checks, exact-head Windows/all applicable CI and main merge still gate completion/credit. Actual merged maturity remains 52.90 / 95.

P17-003's four mandatory pre-push checks passed: workspace formatting,
all-target Clippy with denied warnings, the complete unfiltered workspace
suite (1,278 passed, zero failed, 37 unchanged ignored across 186 suites), and
the agent-plugin contract. This includes the actual five-case datum MCP/Agent
regression, six new backend regressions and existing long native fillet/thread
physical checks. The locked Tauri build, actual Linux native interactions,
desktop workflow policy, MCP eval self-test, changed-document links and
whitespace checks passed. Completion and GUI 47→49 / weighted CAD
52.90→53.20 credit remain conditional on the feature main merge and all
applicable exact-head CI, including the actual Windows native interaction
step. Agent operation remains 95; no external-host credit is added.

P17-003 completed in PR #151, merge commit
`1956514704f9ab337fa1572149819e045e5a9301`, tested head
`4e763ca9231430c92d43324c4e33a3046fab04a4`, base
`71773f73d18d3ad5c9a1569510baebd0457dc11b`. Merge parents and the
identical tested/merged trees were verified. All four exact-head workflows and
their full eleven-job inventory passed (ten successes and the expected skipped
PR publisher): CI 38897216640, Design Review 38897216668, Desktop 38897216501,
Release 38897216201. Actual Linux and Windows native datum interaction gates
passed. Partial fixed-rubric credit is GUI 47→49, weighted CAD 52.90→53.20;
agent operation remains 95. The increment covers the usable source-bound datum
choice/edit/parameter lifecycle, without repeat geometry or external-host credit.

P17-004 starts under ADR-102: center/radius pointer intent uses the existing
typed atomic point/circle sketch form batch. Review stays read-only; cancel,
invalid radius, stale replies and failed apply preserve source/history/output.
No additional score before its feature main merge and new native acceptance.

P17-004 implements the native center/radius circle tool using a disposable SVG
preview and the existing two-operation typed Point/Circle batch. The backend
retains source IDs, workplane association, native verification and one-entry
history. Half gestures cannot submit an unrelated numeric form. Escape/Discard,
tool/sketch/view/reload/upstream changes invalidate drafts/reviews and late
responses. Degenerate radius, stale/foreign requests and late ID collisions
leave no orphan point. No transport, persisted schema or kernel primitive is
added. An existing empty-parameter shortcut now resolves unit-bearing circle
source on disposable models in cold/cached regeneration, including a missing
parameter argument; no dummy parameter or authored literalization is used.

Five targeted backend regressions passed: independent placed circle prisms in
analytic Mock and OCCT on XY/YZ/XZ and a tilted datum, cold/warm/no-parameter
source preservation, volume/area/bounds/centroid, atomic invalid/collision
rollback, construction-only null mass, required assertions and packed/expanded
one-entry history. The actual typed MCP/Agent regression passed five cases,
with independent source/fresh SI STL grading, wrong-placement equal-volume
control and canonical Undo/Redo. Example, authoring guide, skill, eval and
API/architecture updates are supplied.

The locked Tauri build and actual Linux WebKit script passed all 21 checks,
including five new pointer groups: preview/half/Escape/discard/late real IPC;
zero radius and actual control/reload changes; construction atomic Undo/Redo;
free IDs and native 150*pi mm³ prism; upstream invalidation and actual stale
command refusal. The test aligns only the SVG viewport origin to CSS pixels
for WebKit's integer mouse coordinates; input properties, product helpers,
IPC and model results are not replaced. The same helper runs unconditionally
in required Windows native CI. Whole-workspace mandatory checks, exact-head
Windows/all applicable CI and main merge still gate completion/credit. Actual
merged maturity remains 53.20 / 95.

P17-004's four mandatory pre-push checks passed: workspace formatting,
all-target Clippy with denied warnings, the complete unfiltered workspace
suite (1,284 passed, zero failed, 37 unchanged ignored across 188 suites), and
the agent-plugin contract. This includes the new five backend regressions and
actual five-case circle MCP/Agent test, plus existing long native fillet/thread
checks. The locked Tauri build, actual Linux 21-check native script, Desktop
policy, MCP eval self-test, 102 changed-document local links and whitespace
checks passed. Completion and GUI 49→50 / weighted CAD 53.20→53.35 credit
remain conditional on feature main merge and all applicable exact-head CI,
including the actual Windows native pointer step. Agent operation remains 95;
no new external-host or circle-geometry credit is claimed.

P17-004 completed in PR #152, merge commit
`419941f9077b25d4fd336171035c9403b1a31ccb`, tested head
`640ecbbaa1969909e8cdc8e129dccd5536c69c82`, base
`1956514704f9ab337fa1572149819e045e5a9301`. Exact merge parents and
identical tested/merged trees were verified. All four exact-head workflows and
their full eleven jobs passed (ten successes and the expected skipped PR
publisher): CI 38911857585, Design Review 38911857656, Desktop 38911857609,
Release 38911857542. New Linux and Windows native pointer gates passed.
Partial fixed-rubric credit is GUI 49→50, weighted CAD 53.20→53.35; agent
operation remains 95. Credit covers the usable pointer lifecycle and protected
atomic publication/history, without repeat circle geometry or host credit.

P17-005 starts under ADR-102: the backend expands one opposite-corner rectangle
form into four authored points and four edges in a single guarded DesignPatch.
The canvas owns only disposable local-mm input/preview. No new score before
feature main merge and the required independent/native acceptance.

P17-005 implementation expands a creation-only opposite-corner form into four
ordinary Point entities and four Line entities, with stable p0..p3/e0..e3
suffixes and the existing 64-source-operation cap. No stored Rectangle helper,
associative group or implied constraints are introduced. Empty/absent parameter
graphs resolve authored unit-bearing point/line source on disposable models.
The native canvas supplies local-mm input/preview only; review/apply/history
remain the shared DesignPatch gateway.

Seven backend acceptance tests pass, including both-kernel independent
20x10x6 mm prism volume/surface area/absolute placement, four corner directions,
construction-only null mass, late derived-ID collision and assertion refusal,
expanded batch limits and packed/expanded atomic history. The actual typed
MCP/Agent regression passes five placements (XY/YZ/XZ, tilted datum, seeded XY
datum) with read-only review, source/history preservation, fresh native STL
grading and rejection of equal-volume wrong placement and invalid source decoys.
Example, guide, skill, public API and feasible eval are included. The locked
Tauri build passes. The four mandatory pre-push checks pass: fmt, workspace/all-target Clippy,
workspace tests (1292 passed, 0 failed, 37 ignored across 190 suites) and the
agent-plugin contract. New native pointer interaction acceptance and applicable
exact-head CI remain pending; no maturity increment.

P17-005 completed in PR #153, merge commit
f9a1754b19affa210dc26c9f5dee2aaf81b1ba3a, tested head
45e83fca18613274df8676be85fceda1d1e5e476, base
419941f9077b25d4fd336171035c9403b1a31ccb. Exact parents and identical
tested/merged trees were verified; local main synchronized cleanly.
All four exact-head workflows/full eleven jobs passed (ten successes, expected
skipped PR publisher): CI 38927542204, Design Review 38927542436, Desktop
38927542706, Release 38927542437. New actual Linux/Windows rectangle gates
passed, including 26 Linux native checks. Required local checks passed with
1292 tests, zero failures, 37 ignored and 190 suites. GUI 50→51 gives weighted
CAD 53.35→53.50; agent operation stays 95. Credit is the new rectangle input/
review/cancellation/atomic-history lifecycle, without repeated geometry credit.

P17-006 investigation found that exact native OCCT line/arc profiles already
work, but the explicitly selected physical analytic Mock rejects arcs. The
separately registered P17-020 / ADR-103 prerequisite establishes a bounded
one-arc/two-radii sector prism before implementing new arc pointer controls.
No synthetic Mock geometry, sampled-polygon mass or new GUI score is accepted.

P17-020 implementation supplies the ADR-103 kernel-neutral closed-sector
validator, independent physical analytic Mock sector mass/area/centroid/world
bounds and five-face meshes through the existing ADR-063 builder. Native keeps
exact line/arc solids and tracks admitted sectors for outward triangle-derived
display normals and proper rigid placements. Strict source Arc radius/angle
evaluation now rejects wrong literal units before an optional radius seed can
discard the error; empty/absent parameter graphs work without dummy parameters.
Only disposable copies resolve source expressions.

Seven new geometry/native/desktop tests pass: minor/half/major/reversed arcs,
global/tilted/negative/symmetric/rigid placement, analytic interior arc extrema,
real closed outward meshes, endpoint-inclusive budgets, owned Mock face
ancestry, refused operations, source/history preservation and packed/expanded
round trips. The actual five-case MCP/Agent regression also passes verified
read-only patches, unit/reference/late-ID/stale refusal, atomic six-entity
Undo/Redo, exact-source/fresh-SI export grading and same-volume wrong-circle/
placement adversaries. Example, API/architecture, guide, skill and feasible eval
are included; no schema, migration, dependency or golden change.

The four required local checks pass: formatting, workspace/all-target Clippy,
workspace tests (1300 passed, zero failed, 37 existing ignored across 194
suites) and the agent-plugin contract. The locked Tauri build also passes.
Completion takes effect with this task
PR's main merge after applicable exact-head CI, including existing native
desktop compatibility. No new GUI interaction is introduced; P17-006 follows
this prerequisite. No maturity increment: weighted CAD remains 53.50, GUI 51,
agent operation 95 because exact native arc extrusion already existed.

P17-020 completed in PR #154, merge 180c02ae6c9f9c85a23cfa16ea66cfaf2db77094,
tested head 8de5feb6596d7c4fcf8d51847ba2d87929607ea6 and base
f9a1754b19affa210dc26c9f5dee2aaf81b1ba3a. Exact parents, identical tested/merged
trees and clean synchronized main were verified. All four workflows/full eleven
jobs passed (ten successes, expected skipped publisher): CI 38965944056,
Design Review 38965944039, Desktop 38965943816, Release 38965943934, including
actual Windows native compatibility. Required local tests: 1300 passed, zero
failed, 37 existing ignored across 194 suites. No maturity increment.

P17-006 begins under ADR-104: three-click centre/start/end-direction input with
explicit CW/CCW and optional shared-ID radial closure. The backend expands a
creation-only constant-unit form to ordinary Point/Arc/Line source in one
revision/identity-guarded patch. Native Linux/Windows new-control acceptance,
both-kernel physical source/history tests and independently graded actual MCP
are required before completion and any conditional GUI credit.

P17-006 backend and native controls now implement the ADR-104 three-click arc
intent, explicit direction and optional shared-ID radial closure. Nine targeted
backend tests pass, including both-kernel minor/major world bounds/centroid/area/
volume and real outward closed meshes on global/tilted planes, 0° wrap, source
cold/cache preservation, strict unit/geometry/identity/revision/late-ID refusal,
construction/open drafts, assertions, expanded budgets and packed/expanded
canonical history. Actual MCP/Agent parity passes ten cases (five placements ×
CW/CCW), with independent source/branch and fresh native SI output grading and
wrong-source/branch/equal-volume-placement adversaries. The locked Tauri build
passes. Actual Linux Tauri/WebKit passes all 32 interaction groups, including
six new arc groups using real coordinate events, product handlers and IPC.
All four required local checks pass: formatting, workspace/all-target Clippy,
workspace tests (1310 passed, zero failed, 37 existing ignored across 196
suites) and the agent-plugin contract. New actual Windows WebView2 arc
acceptance and the full applicable exact-head CI inventories are required
before completion with this task PR's main merge. Only that qualifying
merge changes GUI 51 to 52 and weighted CAD 53.50 to 53.65; agent operation
stays 95. The increment is solely the new usable three-click arc branch/
radial-closure lifecycle with atomic source/history; no repeated geometry,
workplane, old-form, physical-parity or external-host credit.

P17-006 completed in PR #155, merge b40404624ef8d7b02141dfe48558df592bd749ec,
tested head 93cc19e26dcf7375b9ee8c27caaaf318eeb7b5e0 and base
180c02ae6c9f9c85a23cfa16ea66cfaf2db77094. Exact parents, identical tested/merged
trees and clean synchronized main were verified. All four workflows/full eleven
jobs passed (ten successes, expected skipped publisher): CI 38977162174,
Design Review 38977162116, Desktop 38977162099, Release 38977162102. New actual
Linux Tauri and Windows WebView2 arc acceptance passed; local tests were 1310
passed, zero failed, 37 existing ignored across 196 suites. New three-click
arc/branch/radial-closure operation and atomic history earn GUI 51 to 52,
weighted CAD 53.50 to 53.65; agent operation stays 95.

P17-007 begins under ADR-105: three-click centre/major endpoint/point ON ellipse
intent. The shared backend derives radii and local orientation, then adds an
ordinary centre Point and existing Ellipse in one guarded DesignPatch. No new
geometry family, persisted source tag, helper or sampler. Existing physical
Mock ellipse mass/bounds and native exact extrusion remain the physical base;
default Mock ellipse tessellation still refuses. New native interaction,
independent source/placed physical evidence, all required checks and exact-head
CI are required before completion or any conditional GUI credit.

P17-007 implements the ADR-105 constant-unit three-click ellipse intent and
backend-only radius/orientation derivation. Eight new desktop tests pass for
placed Mock/native physical/source/history evidence, construction, strict
units/axes/identity/revision/late-ID refusal, expanded budgets and packed/expanded
round trips. Two new native tests cover outward display normals on existing
ellipse prisms and proper rigid copies/compounds, with improper transforms
refused. Actual MCP/Agent regression passes five placements, independently
grading ordinary intended source and fresh SI native export. An orientation
decoy preserves volume, COM and axis-aligned bounds but fails directional
support checks; wrong-axis and equal-volume translated controls also fail.
The locked Tauri build and actual Linux Tauri/WebKit driver pass all 38
interaction groups, including six new ellipse groups with real coordinate
events, product handlers and IPC. The same new helper runs unconditionally in
the required Windows WebView2 gate. Example, API/architecture/ADR, guide, skill
and feasible eval are included. No schema, migration, dependency or golden
change. Mandatory whole-workspace checks and full applicable exact-head CI,
including new actual Windows acceptance, remain pending; completion and the
conditional GUI 52 to 53 / weighted CAD 53.65 to 53.80 credit require main merge.
Agent operation remains 95; existing geometry and old forms earn no repeat credit.

All four required P17-007 local checks pass: formatting, workspace/all-target
Clippy, workspace tests (1321 passed, zero failed, 37 existing ignored across
199 suites) and the agent-plugin contract. MCP eval self-test, desktop workflow
policy, Python cache hygiene, JavaScript/Python syntax, 121 changed-document
local links and whitespace checks also pass. The locked Tauri build and actual
Linux native 38-group acceptance passed. Full applicable exact-head CI and new
actual Windows native ellipse acceptance still gate this task's qualifying
main merge and conditional GUI credit; no completion/score is claimed from
local or protocol evidence alone.

P17-007 completed in PR #156, merge ed41a31fcb85eec970358405860f4cc475426585,
tested head 4f64f882c675196c4a3a5372c500050b956c7f2c and base
b40404624ef8d7b02141dfe48558df592bd749ec. Exact parents, identical tested/merged
trees and clean synchronized main were verified. All four workflows/full eleven
jobs passed (ten successes, expected skipped publisher): CI 38992105266,
Design Review 38992105258, Desktop 38992105332, Release 38992105145. New actual
Linux Tauri (38 groups) and Windows WebView2 ellipse acceptance passed; local
tests were 1321 passed, zero failed, 37 existing ignored across 199 suites.
New three-click ellipse operation and atomic source/history earn GUI 52 to 53,
weighted CAD 53.65 to 53.80; agent operation stays 95.

P17-008 begins under ADR-106: source-selected existing point/line/circle
constraints, backend-owned target catalogue/DOF and actual solver refusal
explanations. Existing guarded DesignPatch/history and admission remain the
mutation path. New native controls, both-kernel placed physical/source evidence,
MCP parity/grading and all mandatory checks/CI still gate completion and any
conditional GUI credit. No schema, solver-policy or geometry expansion planned.

P17-008 implements source-selected horizontal/vertical, authored point-pair
distance/coincidence, circle radius and explicit authored constraint removal.
Disposable backend catalogues/suggested IDs drive the new controls; backend
remaining DOF and actual runtime solver refusal reasons are displayed. Solved
copy admission precedes dependent datum regeneration so contradictory/redundant
candidates retain meaningful diagnostics. Existing solver policy, source schema,
DesignPatch/transaction/assertion/profile/history/publication gates are unchanged.

Six new desktop Rust regressions pass, including genuine analytic Mock and OCCT
volume 600*pi mm3 / area 320*pi mm2, independent XY/YZ/XZ/tilted bounds/COM,
meshes, DOF 3→2→1→0, source retention, vertical/coincidence, required assertions,
late-ID collision, same-patch tombstones and packed/expanded file history.
The five-case actual MCP/Agent regression passes with independent intended
constraint source, solved dimensions, fresh SI physical output and adversaries
for direct-radius source substitution and equal-volume wrong placement.
The locked Tauri build and actual Linux native 44-group acceptance pass; both
native drivers append six new constraint groups unconditionally. New example,
ADR-106, API/architecture, MCP guide, skill and eval are included. Full local
mandatory checks, complete exact-head CI and actual Windows acceptance still
gate completion and conditional GUI credit; no score is awarded here.

All four required P17-008 local checks passed: formatting, workspace/all-target
Clippy with denied warnings, workspace tests (1328 passed, zero failed,
37 existing ignored across 201 suites), and the agent-plugin contract.
The final locked Tauri build and actual Linux native 44-group acceptance pass.
Eval self-test, Desktop policy, repository hygiene, changed-document local links,
JavaScript/Python syntax and whitespace checks pass. Actual new Windows controls
and the complete exact-head CI inventory remain required for its qualifying
main merge and conditional GUI credit. No score is awarded before that merge.

P17-008 completed in PR #157: merge 9956038284a9a64086db1c1944708bb2c48a15bc,
tested head d162f394cb525856f43da75f74d9f5c217bac785, tested base
ed41a31fcb85eec970358405860f4cc475426585. Exact parents, identical tested/merged
trees and clean synchronized main were verified. Four workflows/full eleven
jobs passed (ten successes, expected skipped publisher): CI 38998735639,
Design Review 38998735685, Desktop 38998735669, Release 38998735743.
Actual Linux and Windows native 44-group acceptance passed, including the six
new selected-constraint groups. Windows step job 117296958743 succeeded
2026-10-17 03:30:46–03:31:44 UTC. All four local checks passed; workspace tests
1328 passed, zero failed, 37 existing ignored across 201 suites. New native
source-selected constraint controls/diagnostics earn GUI 53 to 54, weighted
CAD 53.80 to 53.95; agent operation remains 95. No repeated solver/geometry credit.

P17-009 begins from that clean main under ADR-107: dedicated source-selected
flag-only construction controls, independent backend eligibility, and backend
before/after closed loops plus consumer/verified-solid consequences. Existing
DesignPatch, solve/profile/assertion/publication/history gates stay unchanged.
Both-kernel source/physical/refusal/history and actual new Linux/Windows controls,
MCP/Agent parity and independent grading, examples/docs and all checks/CI remain
required. No score is awarded before its qualifying main merge.

P17-009 implements backend source construction eligibility/current flags and
prepared closed-profile/consumer consequences. Dedicated canvas/dropdown controls
review only SetSketchConstruction, preserving all other source expressions and
IDs through the existing verified plan/apply/history boundary. Six new desktop
regressions pass: all admitted families, point semantics, genuine analytic
Mock/native OCCT placed circle V=600*pi mm3/A=320*pi mm2 and meshes, exact Mock
ellipse records, consumed ellipse/rectangle/circle and outer-alias refusal,
required assertions, no-op/stale/cubic/helper exclusion and packed/expanded
history. No synthetic Mock mass is used as physical evidence.

Five actual MCP/Agent placement cases pass with real target/profile inspection,
flag-only parity, source/history/refusal and independent exact source plus
fresh native SI grading. Wrong unconsumed flags, equivalent-unit numeric
rewrites and equal-volume translated output fail grading. Example, ADR-107,
API/architecture, MCP guide, skill and eval are included. Locked Tauri build,
JavaScript/Python syntax, eval self-test, Desktop policy and hygiene pass.
Six new control groups are appended unconditionally after the existing 44 in
both native drivers. Local actual Linux GUI execution was interrupted while
requesting its required environment GUI permission; it is not claimed as run.
All four final local mandatory checks, actual Linux/Windows controls and the
complete exact-head CI inventory still gate completion/merge/conditional credit.

All four final P17-009 local checks passed: formatting, workspace/all-target
Clippy with denied warnings, workspace tests (1335 passed, zero failed,
37 existing ignored across 203 suites), and the agent-plugin contract.
The locked Tauri build and static syntax/policy/hygiene/link/eval checks pass.
Actual new Linux native 50-group controls remain unrun because environment GUI
launch approval was interrupted. Actual new Windows acceptance and complete
exact-head CI also remain required. This task stays in progress/draft until
all native/CI gates pass; no P17-009 score is awarded before qualifying merge.

P17-009 completed in PR #158: merge 62a1b3654451c4d352d7dbbc07bbf8d7f085b7fc,
tested head a256cef6c2d7aae733a0a5ecc54baf827fb47933 on base
9956038284a9a64086db1c1944708bb2c48a15bc. The final local required checks
passed (1335 passed, zero failed, 37 existing ignored across 203 suites), as did
the locked Tauri build. The earlier interrupted Linux permission requests were
superseded by an approved actual 50-group Linux native run. The unconditional
Windows native step also passed all 50 groups. All four exact-head workflows
CI 39038776291, Design Review 39038776275, Desktop 39038776247 and Release
39038776412 passed; merge parents and identical tested/merged trees were verified.
New usable selected-flag controls and backend profile/solid consequence review
earn GUI 54 to 55, weighted CAD 53.95 to 54.10 (about 54%). Agent operation
remains 95; existing operations, geometry/solver and host coverage earn no
repeated credit.

P17-021 begins from that clean main under ADR-108, before P17-010 trim controls.
It corrects ordinary point/circle source-length resolution and fail-closed
literal/radius handling without new UI, schema, geometry admission or score.
Completion requires independent both-kernel placed physics, intersection/source
regressions, real MCP/Agent/history/round-trip evidence, documentation/example/eval
and all mandatory local/exact-head CI gates.

P17-021 implements strict ordinary point/circle source resolution, fail-closed
literal/radius seeds and parameter-only raw-patch length validation. Analytic
curve-edit/offset planners evaluate the current strict parameter scope. Existing
cubic/arc expression admission stays bounded; ordinary point/circle legacy
scientific literals are retained. Five desktop regressions cover genuine
analytic Mock/native OCCT V=600*pi mm3/A=320*pi mm2 with XY/YZ/XZ/tilted
placement and oriented meshes, source-driven shared-cache changes, line/circle
and line/arc intersections, raw/typed/parameter-only invalid units, assertions,
packed/expanded source round trips, read-only review and canonical history.
Actual MCP/Agent parity covers five placements, explicit branches/offsets,
source/history/refusal and independent exact-source/fresh SI STL grading with
wrong-placement and equivalent-source adversaries. An example, ADR-108,
API/architecture, MCP guide, skill and feasible eval are included. Targeted
regressions and eval self-test pass; final mandatory local and exact-head CI
gates remain required. No new UI control, schema/golden change or score.

P17-021 completed in PR #159: merge 86b5ebfaba7ec09bef1a7e034e5778505aedb97d,
tested head 376a9a70f19e0c71a22e4f1bbc6ce96e85df740e on base
62a1b3654451c4d352d7dbbc07bbf8d7f085b7fc. All four local required checks
passed (1344 passed, zero failed, 37 existing ignored across 205 suites),
as did the locked Tauri build and four exact-head workflows: CI 39048171994,
Design Review 39048171993, Desktop 39048171969, Release 39048171951. The actual
existing Windows 50-group native compatibility step passed 16:16:12Z–16:17:22Z
on 2026-10-17. Exact parents, identical tested/merged trees and clean synchronized
main were verified. This source-length prerequisite earns no increment: weighted
CAD 54.10, GUI 55 and agent operation 95 remain the baseline.

P17-010 starts from that clean main under ADR-109: source-selected trim/extend,
read-only backend intersections, explicit branch/end/constraint removals and
verified solved endpoint/profile/solid consequences through protected atomic
DesignPatch/history. No new geometry or solver admission. Independent both-kernel
physics, source/refusal/history/MCP evidence, actual new Linux/Windows controls,
example/guide/skill/eval and all local/CI gates remain required before completion
and any conditional GUI credit.

P17-010 implementation evidence (2026-10-18): five focused desktop regressions
pass on genuine analytic Mock and native OCCT, including independent cylinder
V=600*pi mm3, A=320*pi mm2 and placed XY/YZ/XZ/tilted bounds/COM/closed meshes,
strict source/end/branch/removal/identity checks, open no-solid drafts, consumed
loop refusal, required assertions, globally fresh IDs, packed/expanded files
and canonical history. Actual MCP/Agent parity passes in five placements with
independent intended-source/fresh SI grading and adversaries.

The locked Tauri build and actual Linux WebKit product controls pass all 56
groups, including six unconditional new curve groups shared with Windows:
canvas/source picks and explicit discovery, retained-length endpoint refusal
and explicit removals, keyboard branch choice/read-only review/analytic apply/
history, line start/extension plus arc cutter/target edits, loop/consumer refusal,
and control/view/upstream/reload/stale/late-IPC cancellation. No IPC response or
geometry is mocked. Source fixtures were generated through the verified backend.

All four mandatory local checks pass: 1,350 workspace tests passed, zero failed
and 37 existing ignored across 207 suites; fmt/clippy/plugin are clean. The
final locked Tauri rebuild passes and its binary/source hashes still match the
actual Linux 56-group evidence. All 35 native part examples rebuild through
structural patches; all 52 documents validate, all 34 persisted feature graphs
derive exactly, and canonical writer-form checks include the new source.
Existing golden files are unchanged. Exact-head Windows native 56 and all
applicable CI plus actual main merge still gate completion and credit.
Only that qualifying merge earns GUI 55 to 56 and weighted CAD 54.10 to 54.25
for the new usable selected trim/extend lifecycle; agent operation stays 95.
Existing planner/kernel/solver/workplanes and host coverage receive no repeat
credit. Before the qualifying P17-010 main merge, the baseline is CAD 54.10 /
GUI 55 / agent 95; after it, CAD 54.25 / GUI 56 / agent 95. The task PR records
tested head/base, all exact-head CI runs/native step and actual merge parents/tree.


P17-010 qualifying merge (2026-10-18): PR #160 merged as
8512b229dcbd57d1900e4bfe95496a61cbd40ec8, parents tested base
86b5ebfaba7ec09bef1a7e034e5778505aedb97d and tested head
491298280acf59a2013ffca200f1ba1aeb8c9c45, with identical tested/merged trees
and clean synchronized main. Exact-head CI 39095243925, Design Review
39095243919, Desktop 39095243935 and Release 39095244130 all succeeded:
11 jobs, ten success and only the expected PR release publisher skipped.
The actual Windows native 56-group step succeeded 04:00:33Z–04:01:58Z;
Linux actual 56 groups and all four local checks (1,350 passed, zero failed,
37 existing ignored, 207 suites) passed. This earns GUI 55 to 56, weighted CAD
54.10 to 54.25 (about 54%) for usable selected trim/extend controls and the
explicit branch/removal/review lifecycle. Agent operation stays 95. Existing
planner/kernel/solver/workplanes/host evidence receives no repeated credit.

P17-011 starts from that clean merged main under ADR-110. Selected independent
offset copies reuse ADR-052's bounded analytic planner and create one new sketch
and authored owner through reviewed DesignPatch/history. Source selection/order,
constant positive unit-bearing magnitude, side, backend-generated IDs and
independence are explicit. Source/physical/refusal/history/MCP evidence, actual
new Linux/Windows controls, example/guide/skill/eval and all local/CI gates plus
main merge are required before completion or conditional GUI credit. Current
baseline remains weighted CAD 54.25 / GUI 56 / agent operation 95.


P17-011 implementation evidence (2026-10-18): six focused desktop regressions
pass using genuine analytic Mock and native OCCT, including independent outward/
inward circle and ordered rectangle dimensions, placed XY/YZ/XZ/tilted cylinder
V=1014*pi mm3 and A=494*pi mm2, world bounds/COM and oriented closed meshes,
source parameter independence, existing exact tangent line/arc topology, globally
free IDs, catalogue exclusions, assertions/refusals, packed/expanded source and
canonical Undo/Redo. Actual stdio MCP/Agent parity passes in five placements,
with independent exact literal/original-source grading and fresh SI STL plus
wrong-side/centre/association/source-rewrite/nonfinite adversaries; zero model
turns. All 36 native part examples rebuild from structural patches, all 53
example documents validate, all 35 feature graphs derive exactly and canonical
writer-form checks pass. Existing goldens/schemas/solver/kernel admission stay
unchanged. API/architecture/ADR-110, MCP guide/skill, runnable original source,
recipe and independently graded offset_selected_sketch eval are included.

The locked Tauri build and actual Linux WebKit product controls pass all 62
groups, including six unconditional new offset groups shared with Windows:
source/canvas/keyboard picks and explicit sides/generated IDs, read-only review,
atomic copy/owner apply and canonical history, independent parameter edits and
real circle/rectangle extrusion dimensions, ordered list moves/removal/refusal,
unit/collapse/open refusal and complete form/tool/view/source/upstream/reload/
stale/late-real-IPC cancellation. No IPC or geometry is mocked.

All four final mandatory local checks, exact-head Windows native 62 groups,
complete CI inventory and actual main merge still gate completion/credit. Only
the qualifying merge earns GUI 56 to 57 and weighted CAD 54.25 to 54.40 (about
54%) for usable selected offset controls and their protected review lifecycle.
Agent operation stays 95; existing planner/kernel/solver/workplanes/host evidence
receives no repeat credit. Before that merge, baseline remains CAD 54.25 / GUI
56 / agent 95. The task PR records tested head/base and qualifying merge proof.


P17-011 final mandatory local evidence (2026-10-18): all four checks pass,
including 1,357 workspace tests passed, zero failed and 37 existing ignored
across 209 suites. Formatting, workspace/all-target clippy with warnings denied,
and the actual agent-plugin test pass. Required exact-head Windows native,
complete four-workflow/11-job CI inventory and qualifying main merge remain
pending; score stays CAD 54.25 / GUI 56 / agent operation 95 until that merge.

The final locked Tauri rebuild also passes. Actual Linux native 62 groups pass
again on that final binary (2026-10-18 05:50:06Z–05:52:36Z), with unchanged
source/binary SHA-256 hashes throughout the run. Exact-head Windows native and
the complete CI inventory plus qualifying main merge still gate completion.


P17-011 qualifying merge (2026-10-18): PR #161 merged as
7e74204770a96c82c47496e402c64969ae1ea8da, parents tested base
8512b229dcbd57d1900e4bfe95496a61cbd40ec8 and tested head
3a0bca7c80cfa1f3566f4dd73217e3152659966c, with identical tested/merged trees
and clean synchronized main. Exact-head CI 39102077011, Design Review
39102077029, Desktop 39102077017 and Release 39102076984 all succeeded:
11 jobs, ten success and only the expected PR release publisher skipped.
Windows actual native 62 groups succeeded 06:03:26Z–06:05:06Z; final Linux
actual native 62 groups succeeded 05:50:06Z–05:52:36Z with unchanged source/
binary hashes. All four local checks passed: 1,357 tests, zero failed,
37 existing ignored, 209 suites. This qualifying merge earns GUI 56 to 57,
weighted CAD 54.25 to 54.40 (about 54%) for selected independent offset controls
and their protected review lifecycle. Agent operation stays 95; existing
planner/kernel/solver/workplanes/host evidence receives no repeated credit.

P17-012 starts from that clean merged main under ADR-111. Backend-owned
original stock/semantic entry/existing circle-axis choices feed shared verified
standard-hole and optional explicit seat controls. Both-kernel independent
physical/source/refusal/history and actual MCP/Agent/eval evidence, example,
guide/skill, actual new Linux/Windows controls and all mandatory local/CI gates
plus a qualifying main merge are required before completion or credit.
Current baseline: weighted CAD 54.40 / GUI 57 / agent operation 95.


P17-012 focused implementation evidence (2026-10-19): five desktop regressions
pass in genuine analytic Mock and native OCCT. They independently measure plain,
counterbored and countersunk holes, blind/through distance, both end entries,
removal volume, first moments, unchanged stock bounds, oriented closed meshes
and actual bore/seat contact rings. Additional YZ/XZ/tilted placements preserve
axis/entry and source ownership; collision-free feature IDs, required assertions,
unit/shape/ref/stale refusals, expanded/packed read-only review, live seat
parameters and canonical Undo/Redo are covered. Existing standard-hole and both
seat feature suites remain green after factoring their shared input guard.

Actual stdio MCP/Agent parity passes all six seat/entry combinations with no
model turns. Independent eval verifies exact original stock/sketch/reference/
parameter intent and fresh native SI STL contact/volume/COM/closure; equal-volume
translation, wrong entry/axis/standard, unused-source rewrite, literal/expression
substitution and nonfinite adversaries fail. The executable source is authored
through actual MCP DesignPatch. All 37 native part examples rebuild structurally,
all 54 example documents validate, all 36 feature graphs derive exactly and
canonical writer-form checks pass. No existing golden or schema changes.

ADR-111, API/architecture, MCP authoring guide, skill, source/typed recipes and
create_native_standard_hole eval are included. Actual new Linux/Windows controls,
all four final mandatory local checks, complete exact-head CI and a qualifying
main merge still gate completion/credit. Only that qualifying merge conditionally
earns GUI 57 to 58 and weighted CAD 54.40 to 54.55 (about 55%) for newly usable
source-selected standard-hole and explicit seat creation with protected review/
history. Existing standards/kernel/workplanes/host capability receives no repeat
credit. Current actual baseline remains CAD 54.40 / GUI 57 / agent operation 95.


P17-012 final locked native Tauri build and actual Linux WebKit controls pass
all 68 groups, including six new unconditional hole groups shared with Windows:
backend stock/owned entry/circle/standard choices and read-only contacts; plain
bore creation; explicit counterbore and countersink with independent analytic
review dimensions/volume; byte/history/preview-preserving unit/containment/angle
refusal; every form/source/tool/view/upstream/reload/cancel and late-real-IPC
invalidation, live datum/stock entry motion and stale actual apply. Original
source and canonical Undo/Redo are checked. No IPC or geometry is mocked.
Final actual run 2026-10-19T03:11:58.022581+00:00 to 2026-10-19T03:14:54.753314+00:00
retains identical source/binary hashes. Whole-workspace mandatory checks,
exact-head Windows native and all applicable CI plus the qualifying main merge
still gate completion/credit. Baseline stays CAD 54.40 / GUI 57 / agent 95.


P17-012 qualification correction (2026-10-19, before merge): fresh provenance
inspection of an 8 mm plain blind bore exposed the legacy normal-only discovery
assigning its floor to the stock, tying the retained end reference. Reuse the
existing seat source-plane/outward-normal classifier for admitted owned plain
holes, assign internal floors/walls to their hole producer, validate the actual
closed mesh and require one resolved final selected-entry provenance in every
creation plan/apply. Wider ordinary feature geometry and source wire remain
unchanged. A dedicated both-kernel cold/warm/parameter regression checks the
actual selected face triangles lie on the original end plane. Actual MCP/Agent
coverage expands to eight cases, adding both blind entries and fresh provenance
checks; create_native_blind_hole grades exact source, final reference and fresh
SI contacts/solid. All final mandatory, native and exact-head CI evidence must
be repeated on this correction before merge or credit. The initial 1,363-pass
head is superseded for qualification; current score remains 54.40 / GUI 57 / 95.


P17-012 corrected final native evidence: six focused desktop regressions, actual
MCP/Agent eight cases and both existing seat/standard suites pass. The final
locked Tauri build and actual Linux 68 groups pass again from 2026-10-19T04:08:02.031248+00:00
to 2026-10-19T04:10:59.810148+00:00 with identical source/binary hashes,
including both hole producer implementations. The native review now confirms
entry ownership after the backend final-reference gate. A fresh complete four-
check local run and latest-head Windows/all CI plus merge remain required;
initial-head evidence cannot qualify this correction. No score update yet.


P17-012 completed in PR #162, merged 2026-10-19 as
1135d9884ff7cadab841c82ef63fec322d212dcd, exact parents base
7e74204770a96c82c47496e402c64969ae1ea8da and tested head
97a50146e42813668281e2566422abee0bac8b08. Tested/merged trees are identical;
main was clean/synced. Final four local checks: 1,364 passed, zero failed,
37 pre-existing ignored, 211 suites. Corrected actual Linux and exact-head
Windows native controls pass all 68 groups. Windows run 39200848885 / job
117893339505 native step succeeded 04:49:13Z–04:51:19Z. CI 39200848887,
Design Review 39200848912, Desktop 39200848885 and Release 39200849021 all
completed successfully: 11 jobs, ten successful and the expected PR release
publisher skipped. The initial head/evidence was superseded after correcting
blind-floor face ownership; only final head 97a50146 qualifies. This earns
GUI 57→58 / weighted CAD 54.40→54.55 (about 55%) for usable source-selected
standard-hole/seat creation with verified review and atomic history. Agent
operation stays 95; no repeated kernel/standard/workplane or host credit.

P17-022 starts from that clean merged main before P17-013. Physical analytic
Mock previously rejected all revolves, so ADR-112 registers a separate bounded
geometry prerequisite rather than silently using legacy synthetic Mock mass
in GUI evidence. It implements exact annular-sector mass/area/centroid/absolute
bounds and closed outward display meshes for a strict one-sided right rectangle
on global XY/YZ/XZ about an in-plane edge-parallel axis. Positive inner radius,
NewBody/no target and finite bounded positive angle are required. Axis-touching
rectangles exposed an existing native sweep failure and remain outside common
admission. Both directions/sides, full/minor/half/major turns, rotated in-plane
rectangles, axial-origin shifts and proper rigid placements are independently
measured. A negative-side native half-turn previously had the right volume but
wrong angular branch; aligning the spine's start to the profile radial side
corrects COM, absolute bounds and contacts without altering the source schema.

P17-022 evidence includes physical Mock/native regressions, cold/warm cache and
failure output preservation, strict source/units/dependency/history and packed/
expanded round trips, a patch-authored canonical example and six actual
MCP/Agent placements/branches with no model turns. The independent source and
fresh native SI STL grader rejects equal-volume wrong placement and loss of
datum/angle association. Full required local checks, existing actual native
Linux/Windows compatibility and exact-head CI/main merge still gate completion.
This prerequisite earns no maturity credit; current actual baseline stays
CAD 54.55 / GUI 58 / agent operation 95. P17-013 native creation controls follow
only after P17-022 merges.
