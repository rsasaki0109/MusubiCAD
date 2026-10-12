# MusubiCAD development roadmap

Status: active

This is the canonical roadmap for work after the initial modeling, assembly, and
drawing milestones. The companion [implementation status](implementation-status.md)
records what is actually present in the repository. Historical `Task-###` numbers
remain in source comments and older planning notes for traceability; they are not
the active schedule.

## Planning contract

Every planned change has one canonical ID in the form `MCAD-P{phase}-{number}`.
The number is zero-padded within a phase and IDs are never reused. A pull request
continues to follow the repository rule `Task-XXX: Short imperative title`; the
canonical `MCAD-P…` ID belongs in the PR description and commit body.

Status values have a precise meaning:

- **Complete** — implementation, required tests, and documentation are present.
- **In progress** — work has started but the definition of done is not met.
- **Planned** — accepted scope with no implementation committed yet.
- **Deferred** — intentionally held until a dependency or product decision changes.

The Design Graph remains the source of truth in every phase. B-Rep and mesh data
are disposable regeneration outputs, model edits use transactions, AI edits use
`DesignPatch`, serialized maps and traversal are deterministic, and all values in
the public model carry explicit units.

## Phase overview

| Phase | Focus | Depends on | Exit outcome |
|---|---|---|---|
| 0 | Planning and repository baseline | — | One source of truth for scope, status, tests, and links |
| 1 | Desktop distribution | 0 | Reproducible Tauri builds and installable artifacts |
| 2 | Sketch solver completion | 0, existing solver | Complete supported constraint set with diagnostics |
| 3 | Transaction and DesignPatch unification | 0, existing file/AI APIs | Atomic backend edits with reliable undo/redo |
| 4 | Plugin API | 0, 3 | Versioned, deterministic extension boundary with an example |
| 5 | CAD reference and output quality | 2, 3, existing assembly/drawing | Stable references and end-to-end regression coverage |
| 6 | Intent Integrity | 3, 5 | Fail-closed, explainable, incremental, Git-native regeneration |
| 7 | Design authoring | 3, 6 | New designs authored end-to-end through validated, reviewable `DesignPatch` operations |
| 8 | Agent distribution | 7 | A coding-agent plugin that installs in two lines and verifies every change |
| 9 | Robot showcase | 8 | Visible, reproducible CAD → robot motion without a GPU |
| 10 | Assembly intent | 6, 9 | Assembly and motion intent checked by the same fail-closed assertions as parts |

Phase 1 and Phase 2 may proceed in parallel after Phase 0. Phase 3 is the
integration gate for mutating workflows; Phase 4 depends on that gate so plugins
cannot bypass validation. Phase 5 consumes the solver and transaction contracts.
Phase 6 converts those foundations into the product's primary differentiation:
reviewable proof that regenerated geometry still satisfies authored intent.

## Phase 0 — Planning and repository baseline

**Objective:** make the implementation state, active task IDs, acceptance tests,
and documentation links agree with the repository.

**Dependencies:** none.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P0-001 | Canonical roadmap and ID scheme | This document; phase dependencies and gates | Complete |
| MCAD-P0-002 | Implementation inventory | [Implementation status](implementation-status.md) with code evidence and follow-up IDs | Complete |
| MCAD-P0-003 | Documentation consistency | README, developer guide, historical plans, and broken local links corrected | Complete |
| MCAD-P0-004 | Verification contract | Required Rust, integration, fixture, and desktop smoke-test matrix documented | Complete |

**Definition of done:** a contributor can choose an active ID, find its module,
dependencies, required tests, and known risks without relying on stale plan text.

**Tests and checks:** documentation-only review; local Markdown link audit;
`cargo fmt --all -- --check` as a non-mutating repository sanity check. No source
or schema behavior changes are introduced by this phase.

**Known risks:** implementation status can drift as code changes. Every feature
PR must update the status table when its exit criteria change.

## Phase 1 — Desktop distribution

**Objective:** turn the existing Tauri shell into a reproducible, downloadable
desktop product while retaining CLI and Agent API parity.

**Dependencies:** Phase 0; existing Tauri shell, `opencad-desktop`, and release
workflow.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P1-001 | Native build matrix | GitHub Actions builds for Windows x64, Linux x64, macOS arm64, and macOS x64 | Complete |
| MCAD-P1-002 | Artifact contract | Versioned archives/installers, SHA-256 checksums, and quick-start instructions | Complete |
| MCAD-P1-003 | Desktop smoke tests | Open sample, preview, parameter edit, regenerate, pick, and export checks | Complete |
| MCAD-P1-004 | Trust and release policy | Explicit code-signing/notarization scope and credential-gated release steps | Complete |

**Definition of done:** the shared `run_desktop_smoke` contract and integration
test open `examples/bracket.ocad.d`, edit a parameter through the backend,
regenerate, pick, export, and prove the source fixture is byte-for-byte
unchanged; the packaged Linux AppImage invokes the same contract headlessly.
The CLI release contract remains green. Tagged native artifact confirmation is
tracked separately by `MCAD-P1-001` and `MCAD-P1-002`.

**Tests:** `python tools/test_desktop_release_policy.py`; workflow validation;
platform build matrix; install/open smoke tests; CLI/desktop command-parity
tests; checksum verification. OCCT tests are marked integration tests where
the kernel is required.

**Known risks:** OCCT and MSVC ABI compatibility, Tauri system dependencies,
wgpu adapter differences, platform signing credentials, and unsigned-artifact
security warnings.

MCAD-P1-004 keeps `desktop.yml` as an unsigned, `contents: read` CI contract
and adds the separate `desktop-signed-release.yml` workflow. The signed path
uses the `desktop-release` environment, validates a `v<version>` tag on `main`,
fails closed when Windows or Apple credentials are missing, verifies
Authenticode/codesign/notarization and checksums before publication, and marks
Linux as checksum-only. The policy and credential gates are complete. By
product-owner direction on 2026-08-23, provisioning production certificates,
environment reviewers, and a credentialed publication run is deferred; the
current supported distribution remains the verified unsigned artifact path.

Local Windows and Linux evidence is recorded in
[`desktop-release-evidence.md`](desktop-release-evidence.md): Tauri 2.11.4
produced x86_64 MSI and NSIS installers, the versioned packaging/checksum
contract passed, the release executable passed headless smoke, and an
administratively extracted MSI payload passed the same smoke contract. Both
installers are intentionally unsigned. A native Ubuntu 22.04 build also
produced the x86_64 DEB and AppImage; both packaged payloads passed the full
Mesa Vulkan smoke contract. GitHub Actions run `32612751044` completed the
required four-platform native matrix, artifact contract, and packaged Linux
smoke, completing MCAD-P1-001. Tagged run `32616853187` then produced all four
downloadable v0.1.1 Desktop artifacts with independently verified checksums,
while run `32616853192` published and independently smoke-tested the matching
CLI release, completing MCAD-P1-002. Credentialed signing evidence remains the
only external Phase 1 gate.

## Phase 2 — Sketch solver completion

**Objective:** implement the constraint variants already represented in the
serializable sketch model and make solve diagnostics trustworthy.

**Dependencies:** Phase 0; current `opencad-sketch` and `opencad-solver` APIs.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P2-001 | Equal constraint | Line-length and radius residuals, validation, and unit-aware tests | Complete |
| MCAD-P2-002 | Parallel and perpendicular | Direction residuals with degeneracy handling and tolerance tests | Complete |
| MCAD-P2-003 | Solver diagnostics | DOF, redundancy, over-constraint, and non-convergence messages tied to explicit tolerances | Complete |
| MCAD-P2-004 | Sketch regression coverage | Deterministic fixtures and examples for supported constraint combinations | Complete |

**Definition of done:** every serialized constraint that the public API advertises
contributes equations or returns a clear validation error; solved coordinates,
DOF, and diagnostics are deterministic across repeated runs; all comparisons use
documented tolerances.

**Tests:** pure sketch round trips; solver residual/Jacobian tests; under-, fully-,
over-, and contradictory cases; unit conversion tests; and the canonical
`examples/sketch_constraints_regression.ocad.d` fixture exercised by
`modules/file/tests/sketch_regression.rs`. No OCCT dependency is required for
the solver unit suite.

**Known risks:** singular Jacobians, zero-length lines, conflicting constraints,
expression units, and changing solver convergence behavior for existing fixtures.

## Phase 3 — Transaction and DesignPatch unification

**Objective:** make every model mutation atomic and make dry-run, apply, UI, CLI,
and Agent API use the same validated transaction boundary.

**Dependencies:** Phase 0; existing `opencad-core` transaction primitive,
`opencad-ai` DesignPatch, and `.ocad.d` persistence.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P3-001 | Atomic model transaction | Multi-operation apply, rollback on regeneration failure, and unchanged document on error | Complete |
| MCAD-P3-002 | DesignPatch parity | Shared validation path for dry-run and apply, including assembly and drawing operations | Complete |
| MCAD-P3-003 | Backend history | Serializable backend undo/redo snapshots or reversible commands, independent of viewport state | Complete |
| MCAD-P3-004 | Preconditions | Stale-document detection, deterministic conflict errors, and patch rebase coverage | Complete |
| MCAD-P3-005 | Surface parity | UI commands exposed through CLI and Agent API with one command/patch contract | Complete |

**Definition of done:** a failed patch or regeneration leaves the serialized
Design Graph byte-for-byte unchanged; successful operations are undoable and
redoable through the backend; dry-run and apply return the same validation result;
stale preconditions are rejected before mutation. MCAD-P3-003 and MCAD-P3-004
are complete: the
file layer records deterministic full-document snapshots outside the `.ocad`
schema, desktop/Tauri/Agent clients transport them opaquely, and viewport
camera/selection state is excluded. P3-004 adds a versioned complete-state
revision precondition and deterministic parameter/feature/assembly/drawing
rebase conflict handling.
P3-005 is complete: every UI model mutation has a CLI or Agent route,
parameter edits use the shared `DesignPatch`/history boundary, and command
parity includes behavioral and source-contract regression tests.

**Tests:** core transaction tests; AI patch round trips; failure-injection
rollback tests; file checksum/determinism tests; UI/CLI/Agent parity tests;
assembly and drawing patch regressions.

**Known risks:** snapshot size, partial OCCT side effects, concurrent edits,
legacy desktop-local history, and accidental mutation of cached B-Rep data.

## Phase 4 — Plugin API

**Objective:** replace the current placeholder extension crate with a small,
versioned, deterministic API that cannot bypass model validation or module
boundaries.

**Dependencies:** Phase 0 and Phase 3; stable feature registry and transaction
boundary.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P4-001 | Versioned contracts | Feature, importer, exporter traits; serializable manifest and API version | Complete |
| MCAD-P4-002 | Registry and capabilities | Deterministic registration order, capability declarations, and security boundary | Complete |
| MCAD-P4-003 | Product integration | CLI and Agent API discovery/invocation through validated transactions | Complete |
| MCAD-P4-004 | Compatibility evidence | Example plugin, contract tests, failure handling, and developer documentation | Complete |

**Definition of done:** a versioned example plugin can be discovered and invoked
from CLI and Agent API, produces deterministic output, and cannot access document
ownership, raw OCCT types, or unvalidated mutations. MCAD-P4-001 establishes the
linked Rust v1 contract, manifest compatibility rule, and serializable request /
result boundary. MCAD-P4-002 adds BTree-ordered discovery, explicit data-only
capabilities, and host policy rejection. P4-003 exposes deterministic CLI and
Agent discovery/invocation. Feature and
importer results cross the shared dry-run, DesignPatch, transaction, and history
boundary; exporter persistence remains host-owned.
P4-004 supplies a buildable example crate, directional version tests, exact
feature/importer/exporter golden output, returned-error document isolation, and
an authoring guide. Linked plugins remain trusted in-process code; panic and
process isolation are not claimed.

**Tests:** trait/manifest serialization; registry ordering; capability rejection;
API compatibility; importer/exporter golden files; plugin failure isolation and
example smoke tests.

**Known risks:** ABI and versioning policy, untrusted plugin execution, registry
side effects, dependency bloat, and leaking kernel-specific types.

## Phase 5 — CAD reference and output quality

**Objective:** improve the stability and evidence of already shipped topology,
assembly, drawing, mass-property, and rendering workflows.

**Dependencies:** Phase 2 and Phase 3; existing Assembly and Drawing models.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P5-001 | Semantic TopoRef specification | Reference identity, fingerprint fallback, tolerance policy, and migration guidance | Complete |
| MCAD-P5-002 | Feature reference stability | Boolean, fillet, chamfer, and pattern regeneration regressions with stable references | Complete |
| MCAD-P5-003 | Drawing HLR quality | Split partially occluded edges and preserve deterministic visible/hidden segments | Complete |
| MCAD-P5-004 | Assembly robustness | Cycle/path validation, nested-document errors, interference tolerance, and recovery behavior | Complete |
| MCAD-P5-005 | End-to-end golden suite | Mass, bounding box, topology, assembly, drawing, and review artifacts across representative fixtures | Complete |
| MCAD-P5-006 | Future geometry scope | Requirements and ADR for NURBS editing or new kernel features before implementation | Complete (implementation deferred) |
| MCAD-P5-007 | Flagship actuator housing | 22-node parametric example, OCCT/DesignPatch regressions, Feature-build animation, and README review/orbit evidence | Complete |

**Definition of done:** semantic references survive the supported feature edits;
drawing output handles partial occlusion deterministically; assembly failures are
localized and non-destructive; golden fixtures cover the engineering evidence
shown by CLI, desktop, and Agent API.

MCAD-P5-001 is complete: `TopoRef::identity()` separates persisted semantic
identity from kernel hints, explicit unit-labelled fallback policies replace
anonymous matching thresholds, equal-score candidates use a kernel-ID
tie-break, and legacy TopoRef JSON remains schema-compatible with documented
history/sync migration guidance. Feature-specific regeneration regressions are
delivered by MCAD-P5-002.

MCAD-P5-002 adds an OCCT regression harness for boolean-hole, fillet, chamfer,
and linear-pattern parameter edits. It asserts that semantic identity survives,
the regenerated reference points at a current face, and stale stored face/edge
IDs fall through to current discoveries when derivation history cannot bridge
separate regeneration runs.

MCAD-P5-003 replaces whole-edge midpoint classification with deterministic
projected-boundary and depth-crossing subdivision. Explicit tolerances,
visible-hidden-visible and order-independence tests, and an exact partial-
occlusion SVG golden cover the drawing output contract.

MCAD-P5-004 confines canonical child paths to their assembly root, verifies
loaded document kind and identity, rejects canonical aliases, and detects
indirect nested cycles by document ID and path. Per-instance failures remain
localized and retryable. Interference checks use validated meter/cubic-meter
tolerances and return pairs in deterministic instance-ID order.

MCAD-P5-005 adds the central
[`mcad_p5_005_end_to_end.json`](../../fixtures/golden/mcad_p5_005_end_to_end.json)
manifest and a CLI-hosted end-to-end test. The test regenerates the bracket
fixture through OCCT, resolves its semantic face/edge references against the
current topology, regenerates the two-bracket assembly, compares mass and
bounding boxes with unit-labelled tolerances, and pins the partial-occlusion
SVG. It also runs the same CLI review twice and compares `review.json`,
`review.html`, and `github-summary.md` byte-for-byte with the checked-in review
directory. The manifest links the Agent `DesignPatch` input and the resulting
Desktop preview geometry evidence to the same fixture, and the test executes
both `AgentApi::patch_dry_run` and `opencad_desktop::preview_document`.

MCAD-P5-006 accepts ADR-012 and the future-geometry admission requirements.
They require a separate task and feature ADR covering Design Graph ownership,
units/tolerances, TopoRefs, transaction/DesignPatch parity, schema migration,
kernel boundaries, failure atomicity, and tests before NURBS editing or a new
kernel operation begins. No geometry feature is implemented by this planning
task; implementation remains explicitly deferred.

MCAD-P5-007 adds a robot-joint actuator housing that composes only the admitted
kernel-neutral features: stepped joined hubs, shaft and bearing cuts, an
eight-hole circular cut pattern, a six-rib circular union, and mirrored mounting
ears and holes. Nineteen explicit-unit parameters drive its 22-node Feature
Graph. A real OCCT regression, checked-in DesignPatch review, deterministic
Feature-build animation, and 360° orbit make the same model executable evidence
for the CLI, Desktop template, Agent review workflow, and README.

**Tests:** geometry tolerance tests; OCCT integration tests; TopoRef migration and
round trips; assembly/drawing examples; deterministic SVG/mesh/review golden
regressions; mass and bounding-box comparisons with explicit units.

**Known risks:** kernel topology naming limits, floating-point tolerance choices,
mesh-dependent HLR approximations, external component paths, and fixture churn
when OCCT versions change.

## Phase 6 — Intent Integrity

**Objective:** exceed heuristic-only CAD recovery with a fail-closed contract for
change impact, reference provenance, design assertions, incremental regeneration,
semantic merge, and human/agent explanation.

**Dependencies:** Phase 3 and Phase 5; existing dirty propagation, semantic
TopoRefs, DesignPatch dry-run/apply parity, semantic merge, `.ocad.d`, and the
flagship actuator fixture.

The research basis, acceptance metrics, sequencing rationale, and explicit
non-goals are defined in the
[FreeCAD differentiation plan](freecad-differentiation-plan.md).

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P6-001 | Regeneration trace and impact preview | Shared serializable trace, exact dirty-node prediction, kernel/solver call counts, CLI/Desktop/Agent query parity | Complete |
| MCAD-P6-002 | Incremental content-addressed regeneration | Dirty-subgraph execution, disposable versioned cache, cold-regeneration equivalence, 22/100/250-node benchmarks | Complete |
| MCAD-P6-003 | Semantic reference provenance | Exact/derived/fingerprint/ambiguous/missing status, candidate evidence, fail-closed repair patches | Complete |
| MCAD-P6-004 | Executable design assertions | Typed unit-explicit engineering assertions evaluated by dry-run and regeneration | Complete |
| MCAD-P6-005 | Git-native semantic merge | CLI merge driver, stable semantic conflicts, DesignPatch resolution, branch/merge golden workflow | Complete |
| MCAD-P6-006 | Unified intent inspector | One backend dependency/impact/reference/assertion/trace query surface across Desktop, CLI, and Agent API | Complete: `inspect_parameter` / `inspect_reference` queries (Agent API, MCP `query_document`, `musubicad intent`); failed-regeneration inspection (`PartModel::inspect_regeneration`, Agent `opencad.inspect_regeneration_document`, MCP `inspect_regeneration`, `musubicad intent <path> regen`); scripted flagship inspection-and-repair scenario (`docs/examples/inspect-and-repair.md`, end-to-end tested); Desktop Intent Inspector (Regeneration and Intent panels through `inspect_regeneration_cmd`, `inspect_parameter_intent_cmd`, `inspect_reference_intent_cmd`; parity-tested) |

MCAD-P6-003 is complete: `ReferenceProvenance` classifies every face/edge
resolution as `exact`, `derived`, `fingerprint`, `ambiguous`, or `missing`,
records the source feature, role, scored candidate set, tolerance policy, and a
human-readable reason, and refuses to pick among equal-score candidates. The
`required` flag makes an ambiguous or missing reference fail closed instead of
choosing by incidental kernel order. `RegenReport.reference_provenance`,
`musubicad regen`, and the design-review artifact surface the same status, and
adversarial fixtures cover exact, derived, fingerprint, ambiguous, and missing
outcomes deterministically.

MCAD-P6-004 is complete: serializable, unit-explicit `Assertion`s live in the
`.ocad` document (`graph/assertions.json`), carry a stable id, name, severity
(`required`/`advisory`), and a typed rule (parameter range, mass range,
bounding-box limit, body count, required semantic reference, assembly DOF, and
interference limit). `musubicad regen` evaluates them against regenerated
evidence, the design review embeds the same results and rejects a change when a
`required` assertion fails, and the actuator acceptance is covered by OCCT
regression tests. `RequiredReference` assertions consume the P6-003 provenance.

MCAD-P6-002 is complete: `PartModel::regenerate_with_cache` derives a versioned
content key per feature (definition, solved source sketch, upstream output
identity, kernel backend tag) and serves unchanged nodes from an in-memory,
disposable `RegenerationCache` with zero kernel calls. `RegenReport.cached_nodes`
and `RegenerationTrace.output_hashes` expose the reuse, failed regeneration
restores the previous document outputs, and OCCT regressions prove that editing
`upper_hub_height` re-executes only the hub and downstream while changing
`bolt_circle_radius` leaves the base and hubs cached. Checked-in 22/100/250-node
chain benchmarks gate deterministic call counts and cold/incremental equivalence.

MCAD-P6-005 is complete: `musubicad merge-driver` resolves `git merge` of
expanded `.ocad.d` documents semantically
([ADR-015](../adr/ADR-015-git-merge-driver.md)). For each changed file it
reconstructs the complete base, ours, and theirs documents from Git, runs the
whole-state merge from MCAD-P7-001, and writes that file of the merged result,
so the directory stays checksum-consistent. Conflicting intent stops the merge
with typed conflicts, which `musubicad conflicts` lists again. Uncertain cases
fail closed to Git's ordinary conflicts. `modules/cli/tests/git_merge_driver.rs`
merges real branches in a temporary repository on every CI run.

**Definition of done:** the flagship model can undergo adversarial edits and
concurrent branch changes while MusubiCAD deterministically explains the exact
impact, refuses ambiguous or assertion-breaking results, executes no unnecessary
solver/kernel work, merges independent intent, and leaves source/history
byte-for-byte unchanged on failure.

**Tests:** pure trace and cache-key tests; Mock and OCCT execution-count tests;
reference ambiguity fixtures; assertion dry-run/apply parity; cold/incremental
equivalence; semantic merge order independence; Desktop/CLI/Agent surface parity;
GitHub review goldens.

**Known risks:** incomplete cache keys, overly strict reference rejection,
assertion-language scope creep, benchmark noise, and review DTO/schema growth.

## Phase 7 — Design authoring

**Objective:** let humans and agents create new designs, not only edit the
parameters of built-in templates, through the same `DesignPatch`, transaction,
history, review, and merge contracts.

**Dependencies:** Phase 3 (atomic patch boundary) and Phase 6 (provenance,
assertions, trace, and merge consume structural changes).

Today every `PatchOperation` mutates an existing value, and new documents come
only from Rust-coded `DocumentTemplate` variants. No supported surface can add,
remove, or reorder parameters, sketches, constraints, or features.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P7-001 | Structural DesignPatch | [ADR-013](../adr/ADR-013-structural-design-patch.md); add/remove/move operations with author-chosen IDs, final-state validation, derived feature graph, fail-closed removal, `DesignState` v2 revisions, structural diff/rebase | Complete (ADR accepted; slices 1–6 delivered) |
| MCAD-P7-002 | MCP server | [ADR-014](../adr/ADR-014-mcp-server.md); `musubicad mcp` over stdio delegating to the Agent API, authoring guide and patch schema resources, stdio end-to-end test authoring a part from an empty document | Complete |
| MCAD-P7-003 | STEP export | Kernel-neutral `export_step`/`import_step` with millimetre units and deterministic headers; `musubicad export *.step` for parts and assemblies; Agent and MCP parity | Complete |
| MCAD-P7-004 | STEP import | [ADR-016](../adr/ADR-016-imported-step-solids.md); checksummed `imports/` attachments, `add/remove_attachment`, fail-closed `imported_solid` feature (new body, join, cut), `musubicad import-step`, MCP `import_step` | Complete |
| MCAD-P7-005 | Sketch constraints | Angle, midpoint, symmetric, and tangent constraints with dimensionless or metre residuals, angle-expression resolution, overlays, and schema | Complete |
| MCAD-P7-006 | Feature value validation | Dry-run rejection of degenerate lengths, counts, directions, and revolve angles for every patch and in whole-state validation | Complete |
| MCAD-P7-007 | Shell feature | [ADR-017](../adr/ADR-017-shell-feature.md); `shell` with inward uniform wall, fail-closed open-face references resolved on the target body, `thickness_expr`, OCCT volume-loss guard, Mock stand-in, enclosure example | Complete |
| MCAD-P7-008 | Parameter-stable authoring | Authoring examples constrain rectangle edges horizontal/vertical so parameter edits keep the shape; the MCP authoring guide requires shape constraints; OCCT regression test edits width/depth of authored parts | Complete |
| MCAD-P7-009 | Sketch freedom warnings | Dry-run warns `sketch_under_constrained` for added, edited, or parameter-driven sketches that keep degrees of freedom | Complete |
| MCAD-P7-010 | Deterministic topology IDs | [ADR-018](../adr/ADR-018-deterministic-kernel-topology-ids.md); OCCT face/edge IDs are enumeration indices, stored IDs verified by role instead of history remap, reproducible `--sync-topo-refs` | Complete |
| MCAD-P7-011 | Consumed references | [ADR-019](../adr/ADR-019-consumed-reference-provenance.md); provenance reports shell-opened face references as `consumed` (naming the feature) instead of `ambiguous` | Complete |
| MCAD-P7-012 | MCP host evaluation | `tools/mcp_eval.py`: headless Claude Code tasks against `musubicad mcp`, independently graded by regenerated volume and parameters; first baseline 3/3 | Complete |
| MCAD-P7-013 | Exact circle profiles | [ADR-020](../adr/ADR-020-exact-circle-profiles.md); circle profiles reach OCCT as true circles (analytic hole/boss volumes), curved kernel faces classified as cylindrical, goldens re-blessed | Complete |
| MCAD-P7-014 | Fully constrained examples | Bracket template and 12 example documents constrain base rectangle edges horizontal/vertical (via `examples/agent/constrain_bracket_base_patch.json`); parameter edits keep rectangles; skew-derived goldens corrected | Complete |
| MCAD-P7-015 | Line and arc profiles | [ADR-021](../adr/ADR-021-line-arc-profiles.md); arcs with endpoint points join loops, solver holds endpoints on arcs, exact line/arc edges in OCCT, slot example and eval task | Complete |
| MCAD-P7-016 | Canonical example documents | Examples rewritten in the current writer form; `example_canonical_form` test pins it (`MUSUBICAD_BLESS_EXAMPLES=1` rewrites after intentional writer changes) | Complete |
| MCAD-P7-017 | MCP eval tasks | Harness `setup` steps; `slot_plate`, `import_step`, and `fillet_bracket_top` tasks (six tasks; new ones 3/3 on first run) | Complete |
| MCAD-P7-018 | Loft feature | [ADR-022](../adr/ADR-022-loft-feature.md); `loft` through two or more workplane sections (new body, join, cut), exact square-frustum test, custom workplane y-axis fix (face-ref sketches were placed vertically) | Complete |
| MCAD-P7-019 | Designed extents | Face-ref workplanes and mirror planes resolve on the creating feature's body (pin mirror used the pin's own top); `example_extents` test pins every part example's bounding box; review goldens compare line-ending agnostic | Complete |
| MCAD-P7-020 | Loft eval task | `loft_cone` task (Ø40 → Ø20 mm over 30 mm, analytic 21 991.1 mm³); passed on its first run | Complete |
| MCAD-P7-021 | Target-body face refs | Fillet/chamfer/shell face refs resolve on the modified body (unique role on that body identifies the face); unresolved refs fail closed instead of rounding the top perimeter | Complete |
| MCAD-P7-022 | Sweep feature | [ADR-023](../adr/ADR-023-sweep-feature.md); closed section swept along a line/arc path sketch (new body, join, cut), Pappus-exact elbow test, open-chain sketches allowed, `sweep_elbow` eval task | Complete |
| MCAD-P7-023 | Parametric arc angles | [ADR-024](../adr/ADR-024-parametric-arc-angles.md); arc start/end angles may name parameters, resolved before solving without being persisted; sector test follows a 90° → 120° edit | Complete |
| MCAD-P7-024 | Assembly eval task | `assembly_third_bracket` task: add an instance and a distance mate to `assembly_two_brackets`; graded by volume (3 × bracket), instance/mate counts, and `mate_max_error` ≤ 1 µm | Complete |
| MCAD-P7-025 | Tangent shapes at a design angle | Slanted slot example: arc angles `slot_angle ± 90 deg` with line–arc tangent constraints, fully constrained; volume and extents verified at 30° and 60° | Complete |
| MCAD-P7-026 | Helical sweep | [ADR-025](../adr/ADR-025-helical-sweep.md); closed section swept along a right-handed helix through its centre (new body, join, cut), parametric pitch and height; coil spring matches the screw-motion volume within 5e-4 relative before and after edits | Complete |
| MCAD-P7-027 | Drawing eval task | `bracket_drawing` task: a new drawing of a set-up bracket with front/top views and an 80 mm dimension; graded by projections, dimension lengths, and SVG export | Complete |
| MCAD-P7-028 | Shaded preview cues | Offscreen renders draw feature edges (boundaries, >25° creases) and bake per-vertex ambient occlusion (bounded to 1e8 ray tests, cached across orbit frames); remainder of closed PR #1 | Complete |
| MCAD-P7-029 | Pin-ring example fix | The pin-ring seed boss moves 18 mm off the circular-pattern axis, so the example is a ring of four bosses rather than four copies stacked on the axis; OCCT volume equals plate + 4 bosses exactly | Complete |

MCAD-P7-001 is delivered in the six slices listed in ADR-013: `DesignState` v2
with parameter/assertion operations; sketch operations; feature-graph
derivation proven against every fixture; feature operations with diff, impact,
and review rendering; structural rebase conflicts; and assembly/drawing
structural operations.

Slice 1 is delivered: `add_parameter`, `remove_parameter`, `add_assertion`,
and `remove_assertion` with ID grammar, final-state validation, sorted
dependent listing on removal, and derived parameter dependency edges.
`DesignState` carries sketches and assertions; revision v2 hashes them and v1
remains accepted only for value-edit patches. Document, CLI, and Agent paths
share `opencad_file::document_design_state`; assertion changes are diffed,
rendered by CLI diff/review, and rebased by stable ID. Coverage is in
`modules/ai/tests/structural_patch.rs` and
`modules/file/tests/structural_patch.rs`.

Slice 2 is delivered: add/remove sketch, sketch entity, and sketch constraint
operations plus `sketch_exists`, `sketch_entity_exists`, and
`assertion_exists` preconditions. Final-state validation covers point and
entity references, dimension/constraint pairing, parameter use in coordinate
and constraint expressions, `face_ref` workplanes, sketch-feature users, and
closed-profile resolution for every extrude, hole, and revolve on a touched
sketch. Touched sketches get re-detected profiles and an `unknown` solve state.
Sketch changes are diffed, dirty the consuming sketch feature suffix, render in
CLI diff/review, and rebase per `<sketch>/<member>` ID. Coverage is in
`modules/file/tests/sketch_patch.rs`, including regeneration of an edited
fixture.

Slice 3 is delivered: `opencad_feature::derive_feature_graph` derives Feature
Graph entries and edges from definitions and the authored display order. It
reproduces the order, entries, edge set, and regeneration order of all 13 part
examples; eight match byte-for-byte and five pattern templates differ only in
cosmetic edge order (`modules/file/tests/feature_graph_derivation.rs`). A
registration test fails if a definition gains an unregistered input field.

Slice 4 is delivered: add, remove, move, suppress, and replace feature
operations plus add/remove semantic reference operations. `DesignState` carries
`feature_order` in revision v2; the file layer re-derives `feature_graph` only
when Feature Graph inputs change. Removal lists every consumer; added features
must resolve sketches, references, parameters, and closed profiles; moves may
not place a feature before its inputs. `feature_moved` is diffed and dirties
nothing. `opencad_ai::authoring_patch` expresses a part as one structural
patch, and the definition of done below is met by
`modules/file/tests/authoring_rebuild.rs`; an OCCT review of
`examples/agent/add_top_fillet_feature_patch.json` regenerates the added
fillet. Coverage is in `modules/file/tests/feature_patch.rs`.

Slice 5 is delivered: structural conflicts carry a reason (`add_add`,
`remove_modify`, `anchor_missing`, `order`, `invalid_result`). Rebase drops
additions the new base already contains and requires the rebased patch to
apply to the new base. `semantic_three_way_merge` merges the complete v2 state
by stable ID, merges feature display order with ID-ordered same-anchor
inserts, and is independent of which side is "ours". The result must pass
`validate_design_state`. `musubicad merge` now writes back sketches, assertions,
and a re-derived Feature Graph; before this change the other side's sketch and
assertion edits were silently dropped. Coverage is in
`modules/file/tests/structural_merge.rs`,
`modules/file/tests/state_validation.rs`, and a CLI merge test.

Slice 6 is delivered: add/remove component, instance, mate, connector
(remove), assembly pattern, sheet, drawing view, and drawing dimension
operations with fail-closed removal and document-level validation. Component,
pattern, and dimension changes are diffed. `authoring_patch` covers
assemblies and drawings, and all 18 checked-in examples are rebuilt from empty
documents: the 4 assembly and drawing examples byte-for-byte
(`modules/file/tests/assembly_drawing_patch.rs`,
`modules/file/tests/authoring_rebuild.rs`).

**Definition of done:** starting from an empty part document, a checked-in
patch sequence rebuilds `examples/bracket.ocad.d` with canonical-equal
`graph/*.json`, and the 22-node actuator is reproduced by patches with mass and
bounds matching the P5-005 golden within its tolerances; every failed structural
patch leaves the document, history, and revision unchanged.

**Tests:** per-operation local validation; final-state referential validation;
failure-injection atomicity; dry-run/apply parity; derivation-equals-fixture
checks; removal dependent listing; v1/v2 revision rules; rebase conflict and
merge-order determinism fixtures; patch schema validation; OCCT integration for
the flagship rebuild.

**Known risks:** feature input fields missing from graph derivation, verbose
removal patches, larger revision payloads, and ID-naming burden on agents.

## Phase 8 — Agent distribution

**Objective:** make MusubiCAD a plugin that coding agents (Claude Code, Codex,
Cursor, Gemini, and others) install in one step, and make every agent edit
verified before it is written.

**Dependencies:** Phase 7 (MCP server, STEP export, structural patches).

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P8-001 | Verified patch apply | [ADR-026](../adr/ADR-026-verified-patch-apply.md); render-free `verify_patch`; dry-run `verification`; apply refuses patches that do not regenerate or miss declared expected effects; regeneration errors name the failing feature | Complete |
| MCAD-P8-002 | Review without a GPU | `musubicad review` and MCP `review_patch` write `review.json`, `review.html`, and `github-summary.md` with every check when no renderer is available; images are omitted and `images_skipped` says why; outputs with a GPU stay byte-identical | Complete |
| MCAD-P8-003 | `musubicad` command name | The CLI binary, help, usage messages, docs, release workflow, and eval harness use `musubicad`; release archives keep `opencad` as a symlink (`opencad.cmd` on Windows); crate names and `opencad.*` JSON-RPC method names are unchanged wire/API identifiers | Complete |
| MCAD-P8-004 | Verified CLI patch | `musubicad patch` refuses (document unchanged) a patch that does not regenerate or misses declared expected effects, prints the verified mass change, and offers `--no-verify` | Complete |
| MCAD-P8-005 | Agent plugin and installer | [ADR-027](../adr/ADR-027-agent-plugin-distribution.md); `install.sh`/`install.ps1` with checksum verification; Claude Code marketplace, Codex, Cursor, and Gemini manifests launching `musubicad mcp`; `skills/musubicad` skill and `skills.sh.json`; manifest, skill, and installer contract test in CI | Complete (Windows host launch unverified) |
| MCAD-P8-006 | URDF export | [ADR-028](../adr/ADR-028-urdf-export.md); kernel-free `kinematic_tree` from ground and mates (concentric → continuous, other mates → fixed, loops rejected); `GeometryKernel::inertia_about_com` (OCCT, mock); `export *.urdf` with per-part STL, mass, centre of mass, and inertia; CLI, Agent, and MCP parity; verified in MuJoCo | Complete |
| MCAD-P8-007 | Agent-first README | Headline, two-line install, abridged real Claude Code session as the opening GIF (`docs/assets/agent-demo/hero.gif`: verified hub edit with the agent's own preview, refused 100 mm bore, exported URDF in MuJoCo; `generate-hero-demo.py` checks every quoted fact against the transcript and renders only real regenerations), URDF-in-MuJoCo demo (`docs/assets/render-urdf-mujoco.py`); review evidence moved below. Lands with the release that ships the installer | Complete |
| MCAD-P8-008 | 3MF and GLB export | [ADR-029](../adr/ADR-029-3mf-and-glb-export.md); welded, millimetre 3MF (one object per assembly instance) and +Y-up GLB from regenerated meshes; closed-mesh and volume tests on examples; lib3mf strict, trimesh, and Khronos glTF Validator clean | Complete |
| MCAD-P8-009 | Assembly joints with limits | [ADR-030](../adr/ADR-030-assembly-joints.md); `AssemblyJoint` over mates (`revolute`, `continuous`, `prismatic`, `fixed`; SI limits, effort, velocity); `add_joint`/`set_joint`/`remove_joint` with fail-closed mate removal, diff, merge, authoring, and schemas; kinematic tree and URDF `<limit>`; robot arm example limits verified in MuJoCo | Complete |
| MCAD-P8-010 | GPU-free preview for agents | [ADR-031](../adr/ADR-031-gpu-free-preview.md); CPU rasterizer (`iso`/`front`/`top`/`right`, outlines, 2× supersampling); `musubicad preview`, Agent `opencad.preview_document`, MCP `preview_document` returning image content; skill asks agents to look after shape changes | Complete |

## Phase 9 — Robot showcase

**Objective:** make the CAD → robot path something people can see: the arm
moving, its dimensions changing, and the result in a simulator, all from the
Design Graph and all reproducible without a GPU.

**Dependencies:** Phase 8 (URDF export, joints with limits, CPU preview).

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P9-001 | Joint motion GIF | `KinematicTree::pose` forward kinematics (limits enforced, not clamped); `PreviewFraming` and `render_preview_framed` for a still camera; `musubicad animate-joints` with unit-bearing `--pose` keyframes, eased motion, and a per-joint range summary; README GIF `docs/assets/robot-arm-joints.gif` | Complete |
| MCAD-P9-002 | Parameter sweep GIF | `musubicad animate-sweep` regenerating a part or assembly across a unit-bearing parameter range through validated in-memory `set_parameter` patches, eased ping-pong loop, fixed framing; a value that does not validate or regenerate stops the sweep and is named; 5 × 7 bitmap captions (`draw_caption`) with a reserved bottom band, also on joint animations; README GIF `docs/assets/robot-joint-sweep.gif` | Complete |
| MCAD-P9-003 | Presentation style for CPU renders | `PreviewStyle` (`plain`, `studio`, `dark`): background gradient and a blurred ground shadow cast toward the viewer; `--style` on `preview` (MCP and Agent `style`), `animate-joints`, and `animate-sweep` (animations default to `studio`); `--aspect W:H` presets; `plain` output byte-identical to before | Complete |
| MCAD-P9-004 | Reach and workspace | Kernel-free `workspace` and `solve_reach` (grid within limits, damped least squares with backtracking); `musubicad reach` with joint values, an `animate-joints` pose, and `--gif` with a target marker; mates solved first so edits are reflected (also in `animate-joints`); arm link sketches constrained so length edits keep rectangles; checked-in reach loop patches with a mass expectation, end-to-end test, and README GIFs `reach-before.gif`/`reach-after.gif` | Complete |
| MCAD-P9-005 | Web joint viewer | `export *.html`: self-contained page (inline WebGL, no network) with regenerated meshes, orbit/zoom/pinch, a slider per joint within its limits, Animate and Reset; same forward kinematics as `KinematicTree::pose` (cross-checked to 1e-10 m); deterministic bytes; CLI, Agent, and MCP via `export`; README screenshot and `docs/assets/robot-arm-viewer.html` | Complete |
| MCAD-P9-007 | Six-axis arm example | `six_axis_arm_model` and `examples/six_axis_arm.ocad.d`: turntable yaw, shoulder/elbow pitch, wrist roll/pitch, flange roll; new turntable, wrist, and hand parts; symmetric roll connector frames; placements composed through coincident connectors (zero mate residual); OCCT regeneration with zero interference tested; two-column joint captions and shorter shadows for tall models; README GIF and web viewer | Complete |
| MCAD-P9-006 | Agent access | [ADR-032](../adr/ADR-032-robot-showcase.md); Agent API `opencad.reach_document`, `opencad.animate_joints_document`, `opencad.animate_sweep_document` building the CLI's flags (unknown params rejected); MCP tools `reach`, `animate_joints`, `animate_sweep`; skill section on reach and motion | Complete |

## Phase 10 — Assembly intent

**Objective:** extend Intent Integrity from single parts to assemblies and
their motion. An agent that lengthens a robot link must get the same
fail-closed answer for "do the parts still clear each other, at every joint
position" that it already gets for a part's mass or bounds.

**Dependencies:** Phase 6 (executable assertions, verified dry-run), Phase 8
(joints with limits), and Phase 9 (forward kinematics and joint poses).

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P10-001 | Assembly assertions | Assembly documents evaluate their assertions (`assembly_dof_at_most`, `interference_at_most`, mass, bounds, body count, parameter range) in `musubicad regen`, `verify_patch` / `patch_dry_run`, `patch_apply`, and `musubicad review`; a violated required assertion refuses the change, names the assertion and its evidence, and leaves the file unchanged | Complete |
| MCAD-P10-002 | Interference across joint motion | A typed assertion that samples every joint within its limits and counts interfering instance pairs at each pose with the existing explicit tolerances; deterministic pose order; the failing pose is named with unit-bearing joint values; [ADR-033](../adr/ADR-033-motion-interference.md) per-joint sampling with straddling-pair checks, brute-force cross-check, six-axis arm clearance patch | Complete |
| MCAD-P10-003 | Semantic drawing dimensions | [ADR-034](../adr/ADR-034-semantic-drawing-dimensions.md); part-edge chord dimensions with required provenance; all sheets checked by regen, verified dry-run/apply/review and SVG export; exclusive wire migration, Mock/OCCT tests, example and eval task | Complete |
| MCAD-P10-004 | Robot eval task | MCP eval task `robot_add_wrist_joint`: starts from a robot arm with `joint:wrist` removed, requires `add_joint`, URDF export, and a reach query; grader independently checks assembly joint IDs, URDF joint names, mate residual, and reach residual | Complete |
| MCAD-P10-005 | Six-axis motion clearance example | Apply the reviewed clearance patch to the six-axis example; regenerate URDF, joint GIF and web viewer from the updated Design Graph; preserve ADR-033 sampling limits and verify sampled clearance; generator retains limits/assertion and historical unsafe fixtures remain regression-tested | Complete |

**Definition of done:** a patch that makes two instances of a checked-in
robot assembly collide, at rest or at any sampled joint pose, is refused by
dry-run and apply with the instance pair and pose named, and the same patch
without the collision is accepted with identical evidence across CLI, Agent
API, and MCP.

Drawing semantic measurements resolve uniquely against regenerated final
topology and follow model parameter changes. Missing/ambiguous references on any
sheet refuse verified patches and export without changing documents or outputs.

**Tests:** pure assertion evaluation tests; assembly assertion round trips in
`.ocad.d`; OCCT integration tests on `examples/robot_arm_assembly.ocad.d` and
`examples/six_axis_arm.ocad.d` for accepted and refused patches; deterministic
pose sampling tests without a kernel.

Drawing checks include Mock provenance/endpoint tests, legacy and semantic file
round trips, structural authoring, OCCT analytic width edits, real MCP parity,
later-sheet refusal, wrong document identity, deterministic SVG and eval checker
self-tests. Semantic assembly dimensions remain a future instance-binding task.
Robot eval checks include the `robot_add_wrist_joint` MCP task, which removes a
joint in setup, asks the agent to restore it, and independently re-runs URDF
export plus `reach` against the finished document.

**Known risks:** pose sampling cost on large assemblies (pairs × poses exact
Booleans), sampling resolution missing a narrow collision window, and
intentional contact between connected links being counted as interference.

## Phase 11 — Modeling expansion

**Objective:** improve general mechanical CAD modeling through parametric
construction geometry and manufacturing features (fixed rubric target: about
six weighted maturity points across this phase, awarded only after merges).

**Dependencies:** Phase 10 complete; the existing Feature Graph, parameter
expressions, semantic provenance, verified DesignPatch and Mock/OCCT kernels.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P11-001 | Datum planes and axes | Parametric construction features; datum-plane sketch placement and datum-axis revolve input; fail-closed dependencies; Mock/OCCT analytic tests, round trips, example, authoring guide/skill and eval | Complete |
| MCAD-P11-002 | Draft | SI signed angle and datum/semantic neutral plane; fail-closed semantic side selection on convex planar solids; Mock contract and OCCT analytic volume/bounds, patch round trips, example, guide/skill and eval (ADR-036) | Complete |
| MCAD-P11-003 | Ribs | Parametric centerline rib thickness from a sketch with controlled extent and body joining; Mock/OCCT analytic volume tests, example, guide/skill and eval | Complete |
| MCAD-P11-004 | Threads and standard holes | ISO 273 clearance and ISO metric coarse tap-drill representation; diameter agreement, explicit thread metadata, Mock/OCCT analytic cylinders, patch/file round trips, example, guide/skill and eval (ADR-037) | Complete |
| MCAD-P11-005 | Variable and face fillets | Semantic face-perimeter selection and bounded linear radius law on one straight edge; native OCCT/Mock, analytic prism evidence, endpoint orientation/cache/rollback, example, schemas/migration, guide/skill and eval (ADR-038) | Complete |
| MCAD-P11-006 | Inter-body Booleans | Explicit source/target body references for union, subtraction and intersection; fail-closed missing/non-body/self/cyclic inputs and empty OCCT outputs; Mock/OCCT analytic box tests, cache/rollback, patch/file round trips, example, guide/skill/eval (ADR-039) | Complete |

**Definition of done:** each task ships one reviewed PR with both kernel paths,
analytic volume/dimension tests, a committed example, MCP authoring guide and
skill updates, and an eval task where feasible. Schema changes include schemas,
migration policy and deterministic round-trip tests. Scores change only after
main merges and cite the merged capability.

**Tests:** dependency/cycle/suppression refusal; parameter edits and cache
invalidation; Mock geometry contracts and OCCT analytic dimensions/volumes;
legacy/current file round trips; transactional DesignPatch dry-run/apply and
rollback; all mandatory workspace/plugin checks before push.

**Risks:** OCCT operation tolerances and unsupported face configurations;
ambiguous topology must fail closed. Datums must remain non-solid construction
outputs and never inflate body counts, mass or export meshes. Later variable
fillets and threads need bounded complexity and documented representation.

## Phase 12 — Manufacturing outputs

**Objective:** expand model-driven drawing annotations and manufacturing deliverables
(fixed rubric: about four to five weighted maturity points, awarded after merges).

**Dependencies:** Phase 11 complete; P10-003 semantic dimensions, verified drawing
DesignPatch, OCCT/Mock geometry and deterministic SVG/export infrastructure.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P12-001 | Angular dimensions | Explicit semantic straight-edge rays; derived SI angle and orthographic SVG arc; exact native line classification, Mock/OCCT analytic angle tests, refusal/rollback, schemas/migration, example, guide/skill/eval (ADR-040) | Complete |
| MCAD-P12-002 | Radius and diameter dimensions | Semantic complete-circle radius/diameter, exact native queries and Mock witnesses, SI-derived SVG leaders; analytic dimensions/volume, failure/rollback, schemas/migration, example, guide/skill/eval (ADR-041) | Complete |
| MCAD-P12-003 | Section views | Explicit cutting plane and exact polygonal slice geometry, even-odd material hatching; Mock/OCCT analytic area/volume and refusal/rollback tests, schemas/migration, example, guide/skill/eval (ADR-042) | Complete |
| MCAD-P12-004 | Detail views | Same-sheet parent link, rectangular projected-model crop and explicit magnification; deterministic segment clipping, Mock/OCCT analytic tests, schemas/migration, example, guide/skill/eval (ADR-043) | Complete |
| MCAD-P12-005 | Title blocks | Explicit per-sheet metadata and fixed SI frame layout; DesignPatch set/clear, XML-safe SVG, Mock/OCCT examples/analytic layout tests, schemas/migration, guide/skill/eval (ADR-044) | Complete |
| MCAD-P12-006 | PDF output | Canonical multi-page vector PDF via precise SVG layout, bundled font and strict glyph refusal; Mock/OCCT analytic paper/geometry tests, deterministic examples, guide/skill/eval (ADR-045) | Complete |
| MCAD-P12-007 | DXF output | Single-sheet millimetre paper-space primitives, exact polygon hatch clipping and UTF-8 labels; Mock/OCCT analytic tests, independent parser audit, examples, guide/skill/eval (ADR-046) | Complete |
| MCAD-P12-008 | Assembly BOM | Verified source catalog, recursive pattern/leaf quantities and structured instance provenance; strict nested regeneration, deterministic JSON/CSV, Mock/OCCT analytic tests, example, guide/skill/eval (ADR-047) | Complete |
| MCAD-P12-009 | DXF input | Explicit-unit planar primitive parsing into authored sketches through DesignPatch; selected-layer fail-closed admission, Mock/OCCT analytic extrusion, examples, guide/skill/eval (ADR-048) | Complete |
| MCAD-P12-010 | STL input | Explicit-unit connected closed mesh asset import with bounded topology/geometry validation; Mock/OCCT faceted solid dimensions/volume, schemas/migration/round trips, example, guide/skill/eval (ADR-049) | Complete |

**Definition of done:** one reviewed PR per task, backend commands/DesignPatch,
both kernel paths where geometry is queried, analytic measurements, example,
MCP authoring guide and skill updates, feasible eval task, additive schemas,
migration policy and deterministic round trips. No unsupported geometry is
silently approximated or substituted. Scores change only after main merges.

P12-009/010 close the fixed I/O rubric: drawing DXF output alone does not
implement DXF input or STL loading. Keep one task/PR and complete these exchange
boundaries before Phase 13; their limits must be explicit and unsupported inputs
must fail rather than losing geometry.

**Tests:** analytic angles/radii/diameters, model edits and unit conversions,
ambiguous/missing references, invalid projections/cutting planes, atomic patch
failures, legacy/current files, SVG/export structure and printable scale; all
mandatory formatting, clippy, workspace and plugin checks before push.

**Risks:** topology changes, foreshortened measurements, curved-edge confusion,
section robustness, export text/font/unit differences, and incorrect BOM counts
for repeated or nested components. New geometry/sampling decisions require ADRs;
physical host evidence remains distinct from automated Linux checks.

## Phase 13 — Sketch and assembly workflows

**Objective:** improve editable sketch construction and analytic tools, then
assembly presentation and coupled motion (fixed rubric: about four weighted
points after merges). Use SolidCraft's shared command and source-identity
practices as a reference, preserving MusubiCAD's DesignPatch contract.

**Dependencies:** Phase 12 complete, existing sketch solver/profiles, semantic
references, assembly joints, verified patches/history, Mock and OCCT.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P13-001 | Construction geometry editing | Explicit boolean construction edit preserving entity/constraint IDs; exclude helpers from solid profiles, refuse implicit consumer rebinding; Mock/OCCT analytic evidence, patch/history/schema tests, example, guide/skill/eval (ADR-050) | Complete |
| MCAD-P13-002 | Trim and extend | Bounded analytic line/arc edits with explicit intersection selection and retained stable IDs; transactional constraint/profile checks and refusal evidence (ADR-051) | Complete |
| MCAD-P13-003 | Sketch offsets | Explicit-unit convex line/tangent-arc and circle independent copies, caller-owned IDs, mitre/tangent joins and fail-closed inset admission (ADR-052); persistent association/concave repair deferred | Complete |
| MCAD-P13-004 | Ellipses | Parametric analytic closed ellipses with centre constraints and exact normal extrusion (ADR-053), schemas/migration and both-kernel analytic evidence; unsupported operations fail closed | Complete |
| MCAD-P13-005 | External projection | Associated cross-sketch line/circle/ellipse projection to explicit planes (ADR-054), stable semantic IDs, dependency/cache invalidation and fail-closed reference/degeneracy checks; body-edge projection deferred | Complete |
| MCAD-P13-006 | Exploded assembly views | Explicit persisted world-space SI offsets for declared top-level instances (ADR-055), transactional authoring/history and shared desktop/CLI/Agent/MCP presentation; deterministic PNG export and both-kernel analytic bounds/volume; nested children move rigidly, internal paths deferred | Complete |
| MCAD-P13-007 | Coupled joints | Semantic gear/rack displacement DAGs (ADR-056), explicit units/range admission, transactional source/history with old-reader wire refusal, shared pose/animation/reach/workspace and full posed pair checks; analytic Mock/OCCT placement and true native collisions, gear URDF mimic; rack URDF/HTML deferred | Complete |
| MCAD-P13-008 | Sketch splines (gate first) | Accepted ADR-057 under every ADR-012 gate; closed non-rational cubic spans with stable control-point edits/constraints, exact normal NewBody extrusion, Mock/OCCT analytic evidence, strict source/schema/refusal and warm-cache/history round trips, example/guide/skill/eval and real MCP stdio; general NURBS and knot/weight editing deferred | Complete |
| MCAD-P13-009 | Generated Python cache hygiene | Remove tracked bytecode, ignore reproducible interpreter caches and verify source-tree hygiene; corrective maintenance only, with no maturity credit | Complete |

**Definition of done:** one reviewed main-target PR per task with complete
backend/DesignPatch semantics, both kernels where geometry is executed, analytic
dimensions/volume, example, guide/skill and feasible eval. Schema changes require
migration policy and old/new round trips. Scores change only after main merges.

**Tests:** closed-profile identity and construction exclusion, solver constraints
and DOF, geometry edits/intersections/units, source projection invalidation,
assembly presentation versus mechanical placement, coupled motion/cycles,
transactional dry-run/history/rollback and deterministic file round trips. All
four mandatory pre-push checks apply.

**Risks:** profile aliases can silently select a different loop; edits must fail
closed until consumers are explicitly rebound. Trimming can invalidate attached
constraints; offset topology can change; projected sources can be ambiguous.
Coupled motion must never mutate derived joint state outside a transaction.
New sampling, spline or kernel decisions require accepted scoped ADRs.

## Phase 14 — Backend-owned desktop editing

**Objective:** provide a usable Tauri feature editor, semantic selection-to-patch
workflow and interactive sketcher, advancing the fixed GUI rubric from 10 toward
45 (about five weighted points after implementation merges). Start with an
accepted backend-only command architecture ADR; source graph and CAD execution
remain owned by the backend.

**Dependencies:** Phase 13 complete, verified DesignPatch/history, semantic refs,
sketch solver/profiles, shared desktop/CLI/Agent command helpers and the current
Tauri shell. Use the SolidCraft shared-command/rollback reference without
changing MusubiCAD's invariants or claiming its benchmark parity.

| ID | Scope | Deliverables | Status |
|---|---|---|---|
| MCAD-P14-001 | Backend-only GUI architecture | Accepted ADR-058 covering ownership, typed shared commands/queries, explicit units, semantic/revision admission, verified candidate regeneration/history/persistence, parity and named acceptance evidence; no feature implementation or maturity credit | Complete |
| MCAD-P14-002 | Feature-tree editing | Backend-derived stable feature rows and product forms for admitted create/edit operations, reorder and suppression, verified DesignPatch gateway, explicit dependency/refusal/rollback, CLI or Agent parity, Mock/OCCT analytic evidence, Tauri interaction evidence, example/guide/skill/eval | Planned |
| MCAD-P14-003 | Selection to reviewed patch | Backend semantic bindings and source/scene revision checks; concrete patch/effect preview and verified apply; ambiguous/missing/stale selections refuse; no mutation binding from feature-name heuristics; parity, both-kernel evidence and interaction example/eval | Planned |
| MCAD-P14-004 | Interactive sketcher | View-only cursor/draft overlays, stable source point/entity/constraint commands through backend DesignPatch, explicit workplanes/units, solve/profile diagnostics and atomic commit/undo/redo; supported primitive editing with Mock/OCCT analytic and actual UI interaction evidence, example/guide/skill/eval; new spline/NURBS/sampling scope stays separately gated | Planned |

**Definition of done:** architecture accepted before implementation; one task per
main-target PR, with the previous PR merged before the next branch. Every GUI
model mutation uses shared backend DesignPatch/transaction verification before
publishing source/history/scene. Supported create/edit/reorder/suppress, semantic
selection preview/apply and interactive sketch operations have working Tauri
controls and CLI or Agent parity, independently checked dimensions/volume,
meaningful rollback/refusal and UI interaction evidence, examples, API/guide/skill
updates and feasible eval. A raw JSON editor, static handler audit or headless
backend smoke alone does not complete interactive CAD editing. Maturity credit
requires a main merge and records the implemented behavior; this ADR alone adds
no points.

**Tests:** backend/CLI-or-Agent/Tauri source/history parity, analytic Mock/OCCT
fixtures, stale source/scene revisions, ambiguous/missing semantic selections,
invalid units, dependency-invalid reorder/suppression, failure after staged
success, previous output/history preservation, undo/redo and real editor
interactions. Run the four mandatory pre-push checks and all GitHub CI. Keep
Linux cloud/headless/browser evidence separate from actual Windows/Codex/Cursor/
Gemini host sessions tracked by the ecosystem phase.

**Risks:** source/scene drift, frontend heuristic rebinding, writes before CAD
regeneration succeeds, dependency changes during reorder/suppression, unit-kind
errors, stale history, unsupported geometry presented as editable, and static
tests overstating interactive coverage. Avoid frontend graph/solver/persistence
logic; gate new numeric or spline/NURBS decisions with their own accepted ADR.

## Cross-phase verification matrix

Every implementation PR must select the applicable rows and record the command
result. Documentation-only changes may use the documentation and formatting rows
alone.

| Evidence | Required check |
|---|---|
| Formatting | `cargo fmt --all -- --check` |
| Static analysis | `cargo clippy --workspace --all-targets -- -D warnings` |
| Workspace behavior | `cargo test --workspace` |
| Data/file contracts | Pure model round trips, canonical JSON, schema/migration tests |
| Sketch/geometry | Solver residual tests and OCCT integration tests where required |
| AI mutations | DesignPatch dry-run/apply, precondition, rollback, and semantic diff tests |
| Regression | Committed examples, golden geometry/render/review artifacts |
| Desktop/release | Platform build, install/open, command parity, checksum, and smoke tests |
