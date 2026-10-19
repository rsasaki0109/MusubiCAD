# Desktop editing ownership

P17-012 / [ADR-111](../adr/ADR-111-reviewed-native-hole-controls.md) adds usable
standard-hole controls. `hole_creation` lists current source/physical stock,
owned entry and existing exact-circle choices; typed plan/apply creates one
existing standard/seated feature through verified DesignPatch/history. The
product's `hole-controls.js` holds only disposable form/review state. Backend
rebuilds choices and verifies removal/first moments/mesh; source IDs, current
document identity/revision, file publication and history remain authoritative.
Tauri/Agent/MCP share this gateway. Existing explicit analytic Mock/native OCCT
admission is reused; no workplane/schema/sampling expansion or frontend geometry.

[ADR-058](../adr/ADR-058-backend-only-desktop-editing.md) is the accepted Phase 14
contract. P14-001 registers the architecture and acceptance plan; feature-tree,
selection patch and interactive sketch implementation follow in P14-002/003/004.
This document does not claim those editors are implemented.

| Boundary | Ownership |
|---|---|
| Frontend | Viewport/camera, selected display row, form and sketch drafts, disposable overlays |
| Tauri adapter | Typed request/result transport into shared backend commands and queries |
| Shared backend | Source graph queries, admitted capabilities/forms, explicit-unit and semantic/revision admission, DesignPatch planning and verification |
| Transaction/regeneration | Disposable candidate source/history, kernel execution, assertions, rollback and atomic publication |
| File/render layers | `.ocad` source persistence; disposable BREP, tessellation and scene presentation |

Every model mutation uses a shared backend command with CLI or Agent parity.
The planner returns a concrete patch and effect preview. Apply admits the source
revision, validates and regenerates a candidate, then publishes source/history
and scene. Failed candidates retain previous source/history/output. Undo/redo
uses backend history and verifies the restored candidate before publication.
Transported history stays opaque to frontend code.

Existing `modules/desktop/src/patch.rs` already demonstrates candidate-only
patch-and-regenerate orchestration. Parameter/history file helpers currently
validate/persist source without that geometry verification step; P14-002 must
close the publication gap for the editor gateway. A successful source write
alone does not prove successful CAD regeneration.

Feature tree operations use stable graph feature IDs. Selection-generated
mutations require backend semantic bindings and matching source/scene revisions;
name heuristics, renderer indices and cached handles do not own source identity.
Sketch cursor/overlay changes are drafts; entity/constraint/workplane commits
use backend DesignPatch and preserve IDs/expressions. Generated projection stays
derived and unsupported geometry families refuse editing.

Each implementation needs source/history parity, meaningful rollback/refusal,
analytic Mock/OCCT fixtures, examples/guide/skill/eval and actual interaction
evidence. Static handler tests and headless smoke remain useful but do not alone
establish an interactive editor. Record IPC harness, actual Tauri, Linux cloud
and real external-host evidence separately. New sampling/spline/NURBS scope
continues to require its own admission ADR.

## P14-002 implementation boundary

`modules/desktop/src/editor.rs` owns authoritative feature rows, bounded typed FeatureEdit intents, revision-guarded DesignPatch planning and disposable candidate/history verification. Tauri and Agent/MCP adapt these same functions. Product forms review exact expression/profile/position/suppression and resulting volume before apply. Native publication checks required assertions and tessellation; parameter/undo/redo now use the same verification boundary. Suppressed graph inputs cannot feed active consumers. The editor stores no alternate graph or persisted UI schema. [API scope and limitations](../api/feature-editor.md) include part-only commands, positive distance NewBody creation, six length fields, unknown physical solid counts and the existing file writer/revision-check concurrency boundary.

## P14-003 selection implementation

The exact document used for scene generation creates a disposable SelectionFrame. Its versioned digest includes document identity, patchable source revision, actual mesh geometry and overlay bindings, excluding camera and disposable kernel IDs. Backend source ownership resolves declared refs through current regeneration provenance, or authored overlay IDs through unique active profile consumption. The selection mutation API never accepts an authoritative client frame/provenance. It rebuilds the native frame, rejects stale/ambiguous/missing/consumed/unsupported targets and publishes only through the P14-002 verified gateway. Tauri, Agent and MCP share these planners/applicators. Existing name/role hints remain presentation only; picking no longer focuses editable parameters via those guesses. This introduces no sampler/kernel feature/source schema; ADR-058 remains the accepted contract.

Interactive sketch forms (P14-004): `inspect_sketches`, `plan_sketch_edit`,
`apply_sketch_edit` share Tauri/Agent/MCP backend solve, DesignPatch verification
and atomic history publication. Use explicit units, stable authored IDs and the
current revision; review never writes. Authored global sketches and the
ADR-102 admitted datum-plane sketches are supported; datum edits require
current document identity and a regenerated display frame;
unsupported/ambiguous/stale/failed-solve candidates refuse. See
`docs/api/sketch-editing.md` and `examples/agent/interactive_sketch_edit.json`.

## Phase 17 creation boundary

[ADR-102](../adr/ADR-102-desktop-creation-workplanes.md) extends ADR-058's
accepted architecture for new-sketch controls and a bounded existing-datum
workplane lifecycle. P17-001 registers the contract; it adds no command, form,
persisted schema; its design-only merge did not change the then global-only
sketch editor.

P17-002 must atomically create a source sketch and its owning sketch feature
through one revision-guarded DesignPatch, first for explicit global planes.
Sketch-only states retain null physical evidence instead of a placeholder body.
P17-003 must inspect/regenerate admitted datum chains, expose a disposable
resolved view frame separately from the unchanged authored datum reference,
and preserve the actual feature dependency. Frontend frames never own source
placement. Unsupported chains and stale reviews refuse through the shared
verified gateway. Later pointer/feature forms use the same lifecycle.

Each implementation needs actual newly introduced Linux Tauri and Windows
WebView2 interaction evidence, typed Agent/MCP parity, independently measured
physical Mock/OCCT fixtures including equal-volume wrong-placement controls,
read-only review, refusal and canonical history/persistence tests. Existing API
documentation describes current behavior until those implementation PRs land.

P17-002 implementation: `sketch_creation` owns document/revision admission,
deterministic free-ID suggestions and an atomic source-sketch/owner DesignPatch.
Tauri and typed Agent/MCP routes use the same native verified planner/applicator.
The product form owns only a name/plane draft and discards canceled/late reviews;
source IDs and opaque history come from the backend. Profile-free regeneration
is limited to solved, uniquely owned, unconsumed authored global drafts; actual
consumers retain strict profile admission. Sketch-only null physical evidence
is distinct from existing-body evidence. No source schema or sampler changes.
See [API and exact scope](../api/sketch-creation.md).

P17-003 implementation: `sketch_workplanes` inspects admissible active source
chains and obtains disposable frames from complete current regeneration. The
feature module supplies bounded iterative global/custom-rooted datum-chain
admission; cold/cached preflight only extends profile-free drafts to that same
source scope. Each datum owner still resolves actual construction output during
execution. Graph derivation supplies and validates the datum dependency/order.
Source keeps its `DatumPlane` association and local expressions; `SketchEditorRow`
returns the resolved frame separately, never a replacement authored custom plane.
Creation accepts exactly one global/datum identity; datum local edits require
document identity/revision. Thin Tauri/Agent/MCP adapters keep shared verification,
required assertions, transactions and history. The native product only renders
backend choices/labels/frames; source or parameter changes invalidate drafts and
reviews. No new `.ocad` schema or geometry/sampling admission is introduced.

P17-004 implementation: the canvas converts prescribed local-mm pointer positions
into the existing typed Point/Circle batch, with a disposable preview and fresh
source IDs. It owns no solver, physical mass or world-frame authority. The same
plan/apply sketch gateway serves Tauri and Agent/MCP; no additional transport or
persisted entity is introduced. The empty-parameter optimization now resolves
unit-bearing circle radius/center source on disposable regeneration models in
both cold/cached paths, including a missing parameter argument. Authored strings
and workplane references remain unchanged. Half gestures, Escape, invalid radius,
stale replies, source changes and failed verification cannot publish a point
without its circle. Actual native interaction evidence is required separately
for Linux and Windows.

## Backend-owned pointer arcs

P17-006 / [ADR-104](../adr/ADR-104-pointer-arc-intent.md) routes native three-click
centre/start/end-direction input through the same typed sketch gateway as
Agent/MCP. The backend computes radius, projects the endpoint, selects the
explicit CW/CCW branch and expands four ordinary source entities (six with
shared-ID radial closure). Existing CCW Arc source encodes CW through endpoint
reference/angle swapping; no persisted helper or new source tag is introduced.
The creation form admits constant unit-bearing inputs only and preserves
centre/start expressions. Existing raw parametric Arc support is unchanged.

The frontend owns disposable SVG arcs and input intent, with cancellation/late
reply invalidation. Solved inspection carries numeric display radians separately
from authored source. The profile-free draft predicate gains only successfully
solved bounded explicit-endpoint construction arcs, checked through ADR-103's
kernel-neutral validator. Consumers, source ownership, required assertions,
native candidate verification and atomic history retain their shared gates.
Both native drivers execute new arc checks unconditionally; workflow path policy
includes the helper. No new kernel representation, sampler or NURBS admission.

## Backend-owned pointer ellipses

P17-007 / [ADR-105](../adr/ADR-105-pointer-ellipse-intent.md) adds a creation-only
centre/major endpoint/point-on-curve intent to the shared sketch gateway.
The backend inverts the existing analytic ellipse equation and derives radii/
local angle; the frontend owns only disposable SVG and original click intent.
One identity/revision-guarded DesignPatch adds ordinary Point + existing Ellipse,
with centre expressions, datum association and owner preserved. No persistent
axis helper or new `.ocad` source family. Legacy numeric/parametric editing stays.

Keep strict inversion/geometry/unit/ID admission, candidate solve/assertions,
native verification, atomic publication/history and late-reply invalidation.
The existing native whole-prism display path had +Z placeholder normals; mark
ellipse prisms and their proper rigid copies for the existing oriented-triangle
display-normal path. BREP, physical triangles, topology ownership and sampling
are unchanged. Default Mock ellipse tessellation remains an explicit refusal;
its genuine analytic mass/bounds are checked independently. No new geometry
admission, sampler, NURBS editing or external-host claim.

Both native drivers unconditionally execute the new ellipse helper; Desktop
path policy includes it. Independent directional supports distinguish reflected
orientations that preserve volume, COM and world bounds, alongside intended
source checks and actual MCP parity/fresh SI export grading.

## Source-selected sketch constraints

P17-008 / [ADR-106](../adr/ADR-106-selected-sketch-constraints.md) adds only
backend-derived target catalogues and suggested constraint IDs to disposable
sketch inspection rows. The product offers declared-source line, circle and
point-pair selection plus explicit authored-constraint removal. It displays
actual backend DOF and solver refusal reasons. A shared review function, existing
sequence/path invalidation and unchanged apply/history handler serve both
selected controls and compatible raw forms. No frontend solve or graph write.

The shared solved-copy boundary retains the actual runtime SolveStatus solely
for refusal explanation. Solve admitted copies before dependent datum-frame
regeneration so a failed solid cannot obscure contradiction/redundancy. Do not
change solver classification/rank/tolerance, first-point anchoring, successful
state admission, `.ocad` SolveState or profile/assertion/publication gates.
Same-patch removed-ID tombstones remain enforced. No new source/geometry/sampler.

Both native drivers unconditionally append the constraint helper after the
existing 38 groups. New actual Linux/Windows controls, both-kernel independent
physics and actual MCP/Agent parity/fresh SI grading are separate acceptance
requirements. Source grading also rejects a direct radius overwrite producing
the same solid. No duplicate solver/geometry/host credit.


## Source-selected construction flags (ADR-107)

[ADR-107](../adr/ADR-107-selected-construction-geometry.md) uses the existing
SketchEdit/SetSketchConstruction transaction and history boundary. Backend
`construction_targets` uses the same authored-target predicate as planning,
independent of the constraint catalogue. No frontend numeric reconstruction,
profile inference, source mutation, schema or geometry/solver expansion.

Plans expose prepared before/after closed loops and existing profile consumers.
UI displays exact loop consequences and verified candidate solid evidence.
Point flags do not remove referencing curves. Existing consumed-loop binding
and solve/assertion/native publication guards remain mandatory. Current source
catalogues drive selection even during candidate previews; target/control/file/
upstream changes and Escape invalidate the shared review sequence and late IPC.

Both actual native drivers append six unconditional construction-control groups
after the existing 44 groups. Both-kernel placed physical/source/refusal/history
checks and real MCP/Agent parity plus independent source/fresh SI grading are
required separately. Wrong unconsumed flags and equal-volume wrong placement
fail grading; no repeated checkbox/geometry/solver/host maturity credit.


ADR-108 / P17-021 corrects the existing expression-to-solver boundary before
P17-010 controls. Ordinary point x/y and circle radii resolve on disposable
inspection/regeneration copies using the existing strict length evaluator, as
cubic controls and arc/ellipse sources already did. Parameter-only raw patches
validate expression-backed source lengths without introducing a whole-sketch
constraint-admission rule. Analytic curve-edit/offset planners evaluate a strict
current parameter scope. The low-level literal parser/radius seed rejects
unresolved/invalid units rather than silently treating them as metres or zero.
Authored graph source is preserved; regenerate a fresh PartModel from current
source after parameter changes, including when reusing RegenerationCache. No
new persisted fields, geometry admission, solver constraint or native control.


ADR-109 / P17-010 exposes the existing analytic curve planner through the shared
SketchEdit verification boundary. Source catalogues and read-only intersection
discovery are shared by Tauri, Agent and MCP; UI performs no intersection math.
The user explicitly supplies end, branch and constraint removals. A sole typed
form expands into a revision-guarded DesignPatch, capped at 64 operations.
Candidate solving must leave the new endpoint within 1 nm of the selected
original source intersection; generic editor verification alone does not
evaluate arbitrary patch expected effects. Keep old shared points/source and
verify loop/consumer/solid/assertion/display consequences before publication.

Apply re-derives current source, preserves transaction/file re-read and opaque
history guards, and cannot silently repair a consumed loop. Common review
sequence invalidation handles target/cutter/mode/end/removal/branch, source/file/
view/tool/upstream changes, Escape/discard and late actual IPC. Candidate IDs
never replace source catalogues. Six new unconditional real native groups
follow the existing 50 in both Linux and Windows drivers. Separate both-kernel
absolute physical/source/history evidence and real MCP/Agent/eval gates remain
required. No new solver, sampler, kernel admission or persisted schema.


### Reviewed independent offset creation (ADR-110)

MCAD-P17-011 adds `sketch_offset` beside sketch editing/creation because offset
creates both a new authored sketch and owner. Typed identity/revision-bound
intent selects ordered backend-catalogued ordinary curves, a constant explicit
positive length, outward/inward side and a name. Existing ADR-052 planning and
ADR-108 current-source resolution produce literal independent entities; the
backend allocates globally free stable IDs and appends the owner in one patch.
The symbolic global/datum workplane is retained. Original sources, constraints,
consumers, expressions and parameters remain exact; constraints are not copied.
There is no new geometry, sampler, solver, schema or associative dependency.

Read-only review and apply share complete solve/profile/assertion/solid/native
display verification, transaction/history and file re-read/publication guards.
Tauri and Agent/MCP call the same gateway. UI closure state holds disposable
ordered intent and reviewed rows only, with the common sketch review sequence
invalidating source/file/form/tool/view/upstream changes and late IPC. Candidate
rows never replace source catalogues. Real Linux WebKit and Windows WebView2
checks append six unconditional offset groups after the existing 56.
