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
