# Physical arc sectors (MCAD-P17-020)

[ADR-103](../adr/ADR-103-physical-arc-sector-parity.md) establishes a bounded
physical Mock subset of the existing exact native line/arc profile capability.
It prepares P17-006; it adds no pointer control or new document feature tag.

## Kernel-neutral admission

`opencad_geometry::validate_arc_sector_profile(&SolvedSketch) -> Result<()>`
checks an ordered closed profile with exactly one three-point `Arc` and two
radial `Line` segments. The lines meet at the arc's circle centre. Start,
through and end points must be distinct, finite and on that same circle;
the through-point distinguishes minor/major and clockwise/counterclockwise
branches. A shortest-arc guess or a compatibility polygon is not accepted as
physical evidence. The function does not mutate source or placement.

Coordinates are metres. Radius must exceed 1 nm and be at most 1000 m;
positions stay within 1e6 m. Closure allows arithmetic roundoff only
(`64*EPSILON*max(1,coordinate magnitude)`), without snapping thin gaps.
Radius agreement uses the minimum of that roundoff rule and 1 nm. Placement
axes are finite orthonormal vectors (1e-9 dimensionless tolerance); ambiguous
zero/full turns and through-points within 1e-9 rad of an endpoint refuse.

`MockGeometryKernel::with_analytic_primitives()` uses this validation and
stores an independent analytic sector record. Ordinary default Mock remains
synthetic and is unsuitable for physical assertions. Native OCCT keeps its
existing broader exact arc admission. Validated native sector extrusions and
their rigid placements use outward triangle-derived display normals instead
of the generic +Z placeholder. No OCCT type crosses the kernel boundary.

Distance/symmetric normal `NewBody` extrusion and proper rigid placement are
admitted in both kernels. A symmetric length is half the resulting total
height, matching the existing extent contract. Physical Mock refuses oblique
or through-all extrusion, sector Booleans, joins/cuts, chord segments, general
mixed arc outlines and cuboid/cylinder stock witnesses. Native's wider
operation support is unchanged.

## Independent measurements and display

For radius `r`, selected positive sweep `t` in radians and total height `h`:

- Volume: `r*r*t*h/2` in m³.
- Surface area: `r*r*t + r*(t+2)*h` in m².
- Planar centroid distance: `4*r*sin(t/2)/(3*t)` along the selected bisector;
  height centroid is `h/2` from the extrusion start.

World bounds include the centre, endpoints and analytic circle coordinate
extrema over the selected branch at both extrusion ends. Mesh facets never
determine the reported mass, area, centroid or bounds. Density is kg/m³.

Physical Mock reuses ADR-063's angular/linear deflection grid and budgets
(8192 angular samples, 100000 vertices, 200000 triangles), restricted to the
sector with exact endpoints. The existing builder checks f32 error,
nondegenerate triangles, outward normals, paired oriented edges and positive
volume. Five local face keys represent two caps, one curved wall and two
radial walls. Physical Mock rigid copies own fresh face IDs with explicit
one-to-one ancestry; Native retains its body-scoped enumeration convention.
Unrepresentable quality requests refuse; they do not relax settings.

## Source and protocol example

[physical_arc_sector.json](../../examples/agent/physical_arc_sector.json)
adds three explicit-unit points, one source Arc and two radial Lines in one
revision-guarded DesignPatch after typed sketch creation. Its six operations
share stable endpoint IDs and preserve the source-bound datum dependency.
Use `patch_dry_run`, then `patch_apply` with `verify: true` and the latest opaque
`history`; the existing MCP schema advertises that history argument. Raw patch
responses return history at `response.history`, while typed sketch/feature
responses wrap it at `response.history.history`. Keep each opaque value
unchanged when passing it to the next operation. One Undo/Redo handles all
six source additions. No schema migration or new MCP tool is required.
Arc radius/angle expressions use strict length/angle evaluation before solving,
including empty/absent parameter graphs; wrong literal units refuse without
letting an optional solver radius seed silently replace the authored value.

For centre (2,-3) mm, radius 10 mm, CCW 0..90° and a 6 mm extrusion on the
seeded +5 mm XY datum, volume is `150*pi` mm³, area `80*pi+120` mm²,
bounds (2,-3,5)..(12,7,11) mm, centroid
`(2+40/(3*pi), -3+40/(3*pi), 8)` mm. A full radius-5 mm circle has the same
volume and is the wrong shape.

Eval `create_physical_arc_sector` checks exact source/ownership/dependency and
preserved prior graph/parameters, then exports a fresh native SI STL and grades
absolute bounds, centroid, volume and oriented closure. Physical mesh tolerances
remain 0.1 mm placement and 1% volume; SDK analytic native checks use 1e-12 m³
volume, 1e-10 m² area and 3e-7 m coordinate tolerances. These are separate gates.
Actual protocol regression covers XY/YZ/XZ, tilted and seeded datum placements,
read-only preview, unit/reference/collision/stale refusal, atomic history and
same-volume wrong-shape/placement adversaries:

```sh
cargo test -p opencad-geometry --test physical_arc_sectors
cargo test -p opencad-kernel-occt --test physical_arc_sectors
cargo test -p opencad-desktop --test physical_arc_sectors
cargo test -p opencad-cli --test physical_arc_sector_mcp
```

This prerequisite earns no maturity increment: exact native arc extrusion
already exists. New pointer lifecycle acceptance belongs to P17-006.
