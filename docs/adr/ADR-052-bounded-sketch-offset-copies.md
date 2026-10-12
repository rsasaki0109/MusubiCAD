# ADR-052: Bounded analytic sketch offset copies

- Status: Accepted
- Task: MCAD-P13-003
- Date: 2026-10-12

## Context

Offset topology and source association must be explicit. Copying solved
coordinates does not create a persistent dependency. SolidCraft's shared
command/source identity practices inform the API; no upstream code is reused.

## Decision

A read-only planner takes an explicitly ordered, counterclockwise closed list
of line/arc IDs with shared ordered endpoint IDs, or one circle, in one solved sketch. Signed `distance_m` is
positive outward and negative inward. `join: mitre_tangent` admits intersections
of consecutive straight supports and tangent line/arc or arc/arc joins only.
Admission is bounded to 256 curves, finite coordinates within 1e6 m, lengths
and radii above 1 nm, convex one-turn closed contours. Zero distances,
non-tangent arc joins, concavity, repeated IDs, ambiguous closure, collapsed
insets and reversed output edges refuse; there is no sampling or repair.

`association: independent_copy` is mandatory. Output is a new authored sketch
on the same workplane with caller-supplied fresh curve and point IDs; source
entities and constraints remain unchanged. No source constraints are copied.
Point IDs correspond to curve start vertices, then arc centres in curve order;
a circle uses one centre point. The planner returns AddSketch/AddSketchEntity
DesignPatch operations with a revision precondition. Existing verified patch
and history APIs own writes; consumers/features are explicit caller operations.
Source parameter edits do not update the independent copy. Persistent offsets
require a future graph-owned dependency definition, not cached coordinates.

Both kernels consume the ordinary resulting analytic sketch wire. JSON parsing enables serde_json `float_roundtrip` so trig-derived f64 source
coordinates remain exact through files and history transport; snapshot identity
checks are not relaxed. No dependency version is changed. Arm-review text goldens correct rotation
entries from 0.9659258262890684 to the existing source patch value
0.9659258262890683: the old parser rounded one ULP upward. Geometry bounds,
triangle counts and interference expectations remain unchanged. No new OCCT
public types, document entity tags, or patch schema tags are introduced.

## Validation

Named evidence must cover an 80 x 60 mm rectangle at +3 mm (86 x 66 mm,
6 mm extrusion volume 3.4056e-5 m3), circle 20 -> 25 mm radius, and a tangent
line/semicircle stadium with area 2 L R + pi R^2. Pure analytic tests cover
inward collapse, concavity, non-tangent joins, units and deterministic output.
Integration covers Mock/OCCT, source preservation, parameter independence,
transaction refusal, schema/expanded/packed round trips and undo/redo. Add an
example, MCP guide/skill and an offset eval with an independent volume grader.

## Consequences

This is a useful bounded offset tool, not a general topology-changing offset
or an associative sketch constraint. API and authoring documentation expose this limitation explicitly.
Mock's existing synthetic extrude mass remains disclosed; neutral wire sizes
and native OCCT volume provide physical evidence, rather than inventing Mock
accuracy. General concave and persistent offsets are follow-up work.
