# Desktop editing ownership

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
