# Independent articulated copies (ADR-059, P13-011)

This API is available from current main; the published v0.2.0 binary predates it.
A parent assembly can store different child-joint displacements on each immediate
child assembly instance. The child document, its authored rest placements and its
mates remain unchanged. Parent-context constraints are reserved for P13-012.

## Inspect, patch, verify

Use MCP `inspect_articulation_source` with `{"path":"child.ocad.d"}` (CLI:
`musubicad inspect-articulation-source child.ocad.d`). It returns `source_doc`,
`source_revision`, `independent_joints`, `can_articulate_child`, an unsupported
reason when applicable, and a parent-revision patch precondition. Inspect the
parent separately to obtain its precondition; use the child's revision inside
its instance configuration. Never substitute a joint from another child owner.

```json
{
  "operations": [{
    "type": "set_instance_articulation",
    "instance_id": "instance:left",
    "articulation": {
      "source_doc": "doc:art_mechanism",
      "source_revision": "<copy the inspected child revision>",
      "positions": [{"joint_id":"joint:hinge", "position":"90 deg"}]
    }
  }]
}
```

Send this DesignPatch through `patch_dry_run`, optionally `review_patch`, then
`patch_apply`. Applying a configuration always verifies, including when a caller
requests `verify:false` or CLI `--no-verify`. Source snapshots are captured before
verification and compared again immediately before publishing the parent; a
changed parent or child dependency refuses without replacing the parent or
returning new history. This does not provide multi-process file locking.

An explicit `"articulation":null` restores that instance's rigid behavior.
Omitted fields, duplicate coordinates, unknown joint IDs, coupled followers,
wrong owners, stale revisions, unitless values, mismatched angular/linear units
and out-of-range coordinates refuse. Each unspecified independent driver uses
zero displacement; authored zero must itself satisfy all joint limits. Coordinate
expressions currently accept finite literals with `deg`, `rad`, `mm` or `m`.

Full hierarchical DOF is not solved at this stage: an assembly-DOF assertion
fails closed instead of certifying a root-only estimate. Body-count assertions
use physical leaves.

The component's mandatory `source_kind:"articulated_assembly"` is maintained by
the patch and persists while any of its instances carries a configuration. Old
readers refuse this new closed-enum variant instead of ignoring the pose. New
readers refuse missing or orphan guards. Legacy rigid documents retain their
schema version and meaning; the additive migration introduces no coordinates.
See the assembly and patch schemas for strict payloads and budgets.

## Geometry and consumers

Regeneration returns `assembly_leaves`, sorted by structured `instance_path`,
with owning `source_doc`/`source_revision`, composed placement, bounds in metres
and volume in cubic metres. These identities distinguish two copies of the same
source. Static meshes, screenshots, reviews, STEP/STL/3MF/GLB exports, mass and
BOM use the posed geometry; the BOM counts unchanged source parts.

Interference checks examine every physical leaf pair, including child-internal,
copy/copy and parent/child pairs. Native OCCT computes exact common volume.
Mock supports actual closed imported mesh solids and their bounded compounds
(ADR-060), but refuses exact intersecting mesh volume or inertia. Synthetic Mock
body IDs cannot certify articulated placement and refuse. Compound mass is the
sum of members, not a Boolean union of overlapping bodies.

Articulated topo-ref sync refuses; use DesignPatch for source changes. Opaque
Undo/Redo re-verifies child geometry and dependencies before publishing.
Hierarchical motion sampling, joint-pose inspection, reach, URDF and interactive
HTML mechanics refuse before publishing output. Nested articulated scopes,
root coupling graphs combined with articulation, patterns inside articulated
children and patterns targeting an articulated
component also refuse. Ordinary static nested rigid children remain supported.
Budgets: 256 root/child links per articulated scope, 64 supplied coordinates,
64 recursion levels and 100000 physical leaves; no new sampling method is used.

## Runnable analytic example

`examples/articulated_copies.ocad.d` contains two copies of a two-box hinge.
Each box is 10 × 20 × 30 mm, hence 6000 mm³; the four leaves total 24000 mm³.
The left hinge moves 90°; the right stays at zero beneath a 90° parent rotation
and a (100, 200, 0) mm translation. Neither child's source is modified.

```sh
musubicad regen examples/articulated_copies.ocad.d --json
musubicad inspect-articulation-source examples/articulated_copies.ocad.d/parts/mechanism.ocad.d
musubicad patch examples/articulated_copies.ocad.d examples/agent/articulated_copies_zero_patch.json
```

The supplied patch resets only the left driver to zero. Use a disposable copy
for edits. Tests independently check volumes, centres, swapped bounds, a 25 mm
prismatic displacement, all three collision pair categories, strict readers,
round trips and opaque Undo/Redo. The MCP eval has independent leaf-frame and
child-file checks; it does not compute expected poses with production helpers.
This stage earns no full flexible-subassembly maturity credit.

## Parent-context constraints (P13-012 / ADR-061)

Current main additionally admits parent-owned analytic connector constraints.
The `set_instance_articulation` object can contain `context_constraints`; a
constrained component's mandatory wire kind is `flexible_assembly`, which also
refuses P13-011 readers. Removing the last constraint restores the earlier guard.
Use exact named connector IDs returned by `inspect_articulation_source` (its
`connectors` field), retaining the assembly document that owns each definition.

```json
{
  "type": "set_instance_articulation",
  "instance_id": "instance:left",
  "articulation": {
    "source_doc": "doc:art_mechanism",
    "source_revision": "<inspected child revision>",
    "positions": [],
    "context_constraints": [{
      "id": "mate:parent_context",
      "joint_id": "joint:hinge",
      "moving": {"instance_path":["instance:left","instance:link"],"source_doc":"doc:art_mechanism","connector_id":"connector:link_tip"},
      "target": {"instance_path":["instance:target"],"source_doc":"doc:art_pair","connector_id":"connector:target"},
      "relation": {"type":"signed_distance","distance":"25 mm"}
    }]
  }
}
```

The outer instances are fixed. The moving connector belongs to the exact
immediate child part link moved by a declared independent bounded prismatic or
revolute joint beneath the grounded child root. The target connector belongs to
a fixed direct parent part. This solves internal motion from the parent context;
it does not alter saved child coordinates or child source. Each independent
movable driver in that flexible child must have exactly one saved coordinate or
one constraint; followers derive through existing couplings without extra DOF.

`signed_distance` measures the moving origin minus the target origin along the
target connector's +Z normal, in metres. It drives prismatic motion; zero makes
planes coincident, without claiming full three-dimensional point coincidence.
A zero/near-zero projected motion axis refuses as singular.

For revolute motion use `{"type":"signed_angle","angle":"180 deg","turns":0}`.
The angle is oriented from target +Z to moving +Z about the world joint axis.
Both normals must be perpendicular to that axis. Specify an angle in [-180,180]
degrees and an integer `turns` from -32 to 32 to select the displacement branch.
The result is angle minus rest angle plus full turns; joint and follower limits
still apply. No clamping, wrapping or branch guessing occurs.

Unsupported/free outer placements, undeclared/unbound drivers, conflicting saved
and solved drivers, follower inputs, repeated IDs, wrong owners, missing named
connectors, nested flexible scopes, root/child patterns and root couplings refuse.
The total saved-plus-solved coordinate budget is 64 per copy. Source snapshots,
mandatory verification and opaque Undo/Redo use the same publication gate as
P13-011. The neutral physical leaf scene is shared by static geometry, review,
BOM and complete internal/external collision checks. An admitted fully bound
system reports `context_dof:0` in `regen_document`; DOF assertions use that
system, rather than the generic root mate estimate. Hierarchical motion, reach,
URDF and HTML mechanics remain refused. No new sampling is introduced.

`examples/parent_context_slider.ocad.d` uses a rotated parent to drive 25 mm of
child displacement; `examples/parent_context_hinge.ocad.d` uses a parent 180°
connector-angle target to drive 90° of child rotation. Five 10 × 20 × 30 mm boxes
sum to 30000 mm³. The right copy remains at rest. The supplied patches change
only the parent constraint to 0 mm / 90°, respectively. Apply on a disposable
copy and inspect the five scoped leaves. The independent eval grades source
ownership, child-file hashes, DOF, frames, bounds and volumes.

P13-012 supplies this admitted flexible-subassembly workflow; general nonlinear
six-DOF constraints, closed loops and arbitrary deeper scopes remain deferred.
