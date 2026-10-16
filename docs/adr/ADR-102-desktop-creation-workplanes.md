# ADR-102: Desktop sketch creation and source-bound workplanes

Status: Accepted for the Phase 17 creation contract; no editor implementation
is supplied by MCAD-P17-001. Feature-specific geometry admission stays with the
existing ADRs and any separately accepted extension.

Task: MCAD-P17-001. Roadmap: Phase 17, before MCAD-P17-002 implementation.

## Context and inspected boundaries

ADR-058 already requires shared backend commands, DesignPatch, candidate
regeneration and verified history. P14-002/003/004 implemented bounded feature
editing, semantic selection and interactive edits of existing sketches. They
do not provide a new-sketch product form. In particular,
`modules/desktop/src/sketch_editor.rs::authored` requires exactly one active
authored sketch feature and `Workplane::Global`. `SketchEditorRow` returns
source and solved sketches; it does not provide a resolved datum placement.
The frontend currently labels the workplane through `workplane.plane`.
Removing that admission predicate alone cannot implement datum-aware editing.

The source model already has `Workplane::DatumPlane { feature_id }`, and datum
regeneration resolves its frame without replacing the authored association.
`AddSketch` and `AddFeature` already permit explicit source creation. Shared
`verify_editor_document` supports a part with no active body, returning null
mass/bounds and checking required assertions. Native verification tessellates
when a body exists. Reuse these properties; do not create a placeholder solid.
The current document templates are samples, not a new empty-part UI.

The next increment needs a complete sketch lifecycle before additional pointer
tools: choose an explicit plane, create a sketch with its feature owner, review,
apply, edit and undo. Source identity and placement must agree across Tauri,
Agent/MCP and the physical result.

## Decision: one shared creation intent and atomic patch

Introduce a bounded, typed backend creation intent in P17-002. It carries the
expected document identity, current source revision, stable sketch and feature
IDs, nonempty display names and an explicit admitted workplane choice.
Backend-generated ID suggestions
must be deterministic for that revision; explicit IDs pass the same namespace,
uniqueness and collision checks as other DesignPatch clients. Unknown fields
and choices refuse. Do not resolve collisions by silently renaming an identity.
Planning and applying both check the expected document identity; a source-only
revision digest is not a substitute for that identity.

The backend creates a concrete revision-guarded DesignPatch containing
`AddSketch` and its owning sketch `AddFeature`, with the necessary graph inputs
and deterministic position. There must be exactly one active authored owner.
Neither operation can be published separately, and failed batches leave no
orphan sketch, feature or dependency. New sketches start empty; existing source
forms or later pointer tools add geometry through their own reviewed atomic
patches. Preserve existing IDs, expressions and feature order.

P17-002 adds sketches to existing part documents and tests genuinely sketch-only
parts. It does not by itself add a blank-document file creation command. If a
new empty-part product flow is needed, register its separate task and specify
path creation/overwrite and document identity admission first. Assemblies,
drawings, projected/generated sketches and multiply owned/suppressed sketches
do not become editable through this contract.

## Workplane admission and presentation

P17-002 admits explicit global XY, XZ and YZ. P17-003 extends the backend choices
to existing active datum-plane features whose dependency chain terminates in
an admitted global or valid custom base. Admit only unique, acyclic, correctly
ordered plane dependencies whose actual current regeneration succeeds. Refuse
missing, suppressed, wrong-kind and cyclic nodes, and any face-reference base
anywhere in that initial datum chain. Arbitrary custom/face workplane editing,
datum creation/reparenting, assemblies and renderer-face inference require
separate admission; existing source support does not grant those UI commands.

Backend inspection lists source-bound choices and supplies disposable resolved
SI origin, orthonormal local axes and normal for an admitted datum. Reuse the
current datum executor and sketch placement orientation, including the existing
XZ extrusion direction. Reject unsupported or malformed frames through existing
finite/orthogonality/geometry guards; the UI cannot weaken their tolerances.
Transport no kernel handles. Return the original authored workplane separately
from the resolved display frame; never write the latter over the association.

Regenerate and validate the complete dependency chain when planning/applying;
a client-supplied cached frame is never authority. Declare the actual datum
feature dependency on the sketch feature. An upstream datum parameter change
must move the sketch and its admitted solid through normal regeneration and
invalidate prior reviews. A datum with no current physical solid remains valid
construction geometry; it has no invented volume.

The local 2D canvas remains view-only. Its unit-labelled coordinates refer to
the chosen plane; backend source expressions use explicit length/angle units
or admitted same-kind parameters. Resolved frames position disposable overlays
and preview geometry, not source relationships. Tauri can display human-readable
plane names, local coordinates and solve diagnostics without requiring users to
edit JSON, graph dependencies or opaque reference handles.

## Review, publication and history

The planner is read-only. It returns the concrete patch, admitted source intent,
source revision, candidate sketch/owner identity, resolved workplane where
applicable, solve/profile diagnostics and actual verification evidence. An
empty/underconstrained but otherwise admitted sketch may be an editable state;
contradictory or unsupported solves refuse. A sketch-only state reports no
solid and null mass/bounds. A solid preview must come from actual candidate
regeneration, not a frontend formula or a synthetic Mock ID.

Apply re-admits the document identity, revision, owner, workplane and complete
dependency chain, then stages source and history on disposable copies. Use the
shared verified gateway, required assertions and existing persistence boundary; publish only
after successful candidate verification. Recheck source before writing as
existing file commands do. This contract does not claim a new cross-process
locking or crash-atomic multi-file persistence guarantee. Required assertions
are never disabled to make a sketch-only or intermediate state pass.

Undo/Redo use the same backend candidate verification and opaque history,
restoring canonical source IDs, expressions, datum references and graph order.
Failure preserves source bytes, history and published output. Review invalidates
when source, path, selected workplane, draft or document identity changes.
Use bounded draft batches and existing command limits; discard late asynchronous
results after a newer review, cancel, file switch or upstream edit. A canceled
pointer gesture creates no source/history entry. Do not add a GUI-owned solver,
model copy, sampling method or persisted editor state.

## Transport and compatibility

Expose the same planner/applicator through shared backend, Tauri and Agent/MCP;
the exact transport names/DTOs are defined and documented in their implementation
task before exposure. Every UI model command needs CLI or Agent parity. The
existing generic `patch_apply_document` route alone does not prove that a new
typed form planner follows the same admission. Exercise the actual typed route.

Reuse existing `.ocad` workplane, sketch, feature and DesignPatch semantics.
This design adds no schema field or migration. Existing API descriptions remain
the implemented scope until their feature PR updates them. If implementation
requires new persisted semantics, first register/accept the extension and ship
both schemas, no-inference migration/admission, expanded/packed round trips and
genuine older-reader refusal together. Do not extend a permissively ignored
optional field to grant new geometry. No NURBS editing is admitted by this ADR.

## Named acceptance matrix

The implementation tasks must add and execute the following evidence. These
are future acceptance requirements, not tests added by this design PR.

| Task / evidence | Required result |
|---|---|
| P17-002 `global_sketch_creation_is_atomic_and_revision_guarded` | XY/XZ/YZ creation yields one source sketch and active owner with deterministic order; missing/duplicate IDs, malformed choices, wrong document kind/identity (including identical source in another document) and stale requests preserve source/history/output |
| P17-002 `sketch_only_review_preserves_source_and_reports_no_solid` | Empty and valid open/underconstrained sketches remain editable without a body; read-only review and null mass/bounds; required assertion failure still refuses |
| P17-002 `created_sketch_forms_match_agent_and_analytic_solid` | Same typed intent through backend/Agent/Tauri preserves source; subsequent admitted geometry/extrusion has independently checked volume, absolute bounds and placement in both kernels |
| P17-003 `datum_sketch_edits_preserve_authored_association` | Global- and custom-rooted datum chains, signed offsets and parameter edits regenerate in the current local frame without flattening source; upstream edits invalidate reviews |
| P17-003 `datum_sketch_refuses_unresolved_or_unsupported_chains` | Missing/suppressed/wrong-kind/cyclic/face-rooted chains, invalid frame, dependency order and stale resolved data refuse without source/history/output change |
| Each feature `creation_failure_and_history_preserve_canonical_source` | Failed staged batches and required-assertion/solve/native failures preserve source bytes and history; Undo/Redo restores logical canonical source, IDs, expressions and dependencies after successful apply |
| Each feature `native_creation_forms_and_pointer_tools_match_backend` | Actual Linux Tauri and Windows Tauri/WebView2 form/gesture review/apply/cancel/refusal/history behavior; inspect the newly introduced operations, not only old editor smoke |
| Each feature `actual_mcp_creation_eval_checks_source_and_physical_output` | Actual typed MCP route plus independent source-intent and native export/measurement grading; a correct volume with wrong plane/offset/ownership must fail |

For the initial physical fixture, reuse admitted analytic ellipse source:
semiaxes 20 mm and 10 mm, 6 mm extrusion. Its independent volume is
1200*pi mm³. With the profile centered at local zero and positive extrusion,
the independent global expectations are:

| Plane | Positive extrusion direction | Absolute world bounds, mm (x; y; z) |
|---|---|---|
| XY | +Z | [-20, 20]; [-10, 10]; [0, 6] |
| YZ | +X | [0, 6]; [-20, 20]; [-10, 10] |
| XZ | -Y | [-20, 20]; [-6, 0]; [-10, 10] |

Then test a datum offset/tilt and upstream parameter edit against independently
transformed absolute positions. Volume alone cannot detect the wrong plane or
datum. Expected placements must not be obtained by calling the production
placement resolver under test.
Use genuine physical Mock ellipse records (or an explicitly selected, admitted
analytic-primitives path for other fixtures), never legacy synthetic mass.
Tests must declare analytic and mesh/measurement tolerances separately, retain
existing admission tolerances, and include wrong-placement equal-volume controls.

Each implementation includes an example, API/architecture updates, MCP authoring
guide, skill and feasible actual independently graded eval, source expanded/
packed round trips and meaningful regressions. Run all mandatory pre-push checks
and require applicable exact-head CI with its full job inventory. Actual Windows
UI evidence is distinct from Linux, IPC harness, protocol and external model-host
evidence. Additional controls receive no credit merely for reusing old smoke.

## Consequences and alternatives

P17-002 implementation admission: the old regeneration preflight requires a
profile even for an empty owned sketch. Keep that strict profile validator for
consumers, but admit a successfully solved profile-free global draft with one
active authored owner, only the existing editor's point/line/circle/ellipse
entities, and no active feature input consuming that owner. Check actual solve
admission even on the no-parameter shortcut. Unsupported workplanes/entities,
ambiguous owners and active consumers keep strict refusal. Both cold and cached
regeneration use this predicate. Existing source fields/tags are unchanged;
older regeneration can refuse a profile-free draft rather than invent a solid.

P17-003 implementation keeps the global JSON creation form and adds exactly-one
`plane` / `datum_plane` selection. Inspection returns actual regenerated datum
choices and separate SI display frames; source keeps `DatumPlane { feature_id }`.
Datum editing additionally requires document identity. The draft predicate
extends only to admitted active acyclic global/custom-rooted plane chains;
actual cold/cached regeneration must still resolve each datum dependency.
No cached frame is accepted as mutation intent and strict profile consumers
remain unchanged. No persisted source field or tag is added.

P17-004 maps two local-mm pointer clicks to the existing typed sketch-edit
batch: one new center point and one circle with fresh source IDs. The second
click determines a unit-bearing radius; this is disposable input intent, not
a frontend solve or physical result. The shared backend validates the complete
batch, current source/identity/workplane and required assertions. Review and
apply reuse the existing commands in Tauri and Agent/MCP. A half-complete gesture
cannot submit the unrelated numeric form. Cancel, Escape, tool/sketch/view/file
changes and upstream edits invalidate the gesture/review; late replies cannot
restore a canceled review. This adds no command, source tag, sampler or kernel
geometry family. Explicitly admitted analytic Mock primitives and OCCT provide
independent circle-prism physical evidence.
The existing empty-parameter shortcut must not bypass solving a circle with
unit-bearing radius/center expressions. Resolve these on the disposable model
in cold/cached regeneration, including the no-parameter API path; do not add a
dummy source parameter or literalize the document to make extrusion succeed.

Keep the existing shared gateway and plain JS frontend; no new dependency is
needed by this design. A bounded datum-chain admission makes source association
testable before semantic face-plane editing is introduced. Reusing a copied
custom frame would lose parametric motion; publishing an unowned sketch would
split the transaction; relaxing the global-only guard without a resolved frame
would expose unsupported edits. Those alternatives do not satisfy this contract.

P17-001 is design only and earns no maturity or host credit. Later merged
features need their own explicit fixed-rubric evidence ledger; the working GUI
65 target and approximately 60% total remain estimates. Current scores stay
52.60 / 95 at its design merge; later feature credits are recorded in the
maturity plan's evidence ledger. Phase 14's existing forms, selection and pointer line editing are
not credited again, and ADR-012 continues to gate spline/NURBS editing.
