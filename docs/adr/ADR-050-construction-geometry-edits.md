# ADR-050: Explicit construction geometry edits

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P13-001

`EntityBase.construction` already persists a boolean and profile detection
already excludes construction lines/arcs/circles. Add a deterministic
`set_sketch_construction` DesignPatch operation with `sketch_id`, `entity_id`
and explicit `construction`; do not add a context-sensitive toggle. CLI, Agent,
MCP and later Tauri use the existing patch/history boundary.

Edit the stored Point/Line/Circle/Arc/Rectangle entity's base field in place,
preserving stable IDs, coordinates and constraints. A rectangle helper uses its
own stored ID; expansion inherits its flag. Derived corner/edge IDs do not
become independently editable source entities. Point flags do not remove
endpoints from line/arc constraints or geometry. Construction remains useful to
the solver while excluded from manufacturing profiles. No new kernel operation
or sampled curve is needed.

Stage the final candidate and refresh profiles/solve state. Emit a semantic
entity modification and mark the sketch's feature/consumers dirty. Compare
existing unchanged consumer profile bindings before/after construction edits:
if a `profile:outer` alias would select a different entity set, refuse rather
than silently switching the solid to another loop. Removing a consumed boundary
requires explicit consumer removal/rebinding in the same patch. No-op edits
have no semantic diff. Failures preserve graph/history.

The document field is unchanged: absent construction stays false, with no new
document version branch. The additive patch tag requires schema/API updates;
source/patch round trips cover compatibility. Boolean flags are dimensionless,
sketch coordinates use SI metres and primitive constraints retain their units.

Named evidence: `construction_edit_preserves_ids_and_constraint_geometry`,
`construction_profile_alias_rebinding_refuses_atomically`, and
`construction_edit_round_trips_with_history_and_native_volume`. Include a
construction-guide plate example and feasible MCP eval. Verify prepared SI
dimensions and unchanged native plate volume; the existing Mock path verifies
the same profile exclusion and preserves its documented synthetic body.

Reference: [SolidCraft comparison](../plans/solidcraft-reference.md). Its
construction and shared-command workflows inform authoring parity; no upstream
code is copied, and editable splines remain gated separately by ADR-012.
