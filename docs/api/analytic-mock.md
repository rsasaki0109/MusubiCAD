# Bounded analytic Mock primitives

MCAD-P16-002 / ADR-062 adds the Rust SDK constructor
`MockGeometryKernel::with_analytic_primitives()`. It selects physical runtime
records independently of the native OCCT implementation. `new()` and `default()`
retain the existing Mock plumbing; ordinary synthetic IDs are not physical
volume/dimension evidence. This constructor is not a new CLI/MCP command or a
persisted document setting.

Supported `GeometryKernel` operations:

| Operation | Admission |
|---|---|
| `make_wire_from_sketch` | Closed right rectangle (four corners; optional matching line segments), exact single circle, or ADR-103 one-arc/two-radii sector; finite orthonormal sketch placement |
| `extrude` | Distance or symmetric normal extrusion; `NewBody` without a target, or circle `Cut` with admitted stock target |
| `boolean` / `body_boolean` | Subtract a strictly radially contained, axially bounded cylinder from rectangular stock; one concentric chain only |
| `mass_properties` | Independent SI volume, surface area, positive-density mass and centre of mass |
| `bounding_box` | Actual stock corners, analytic cylindrical support bounds or sector branch extrema |
| `transform_body` / `translate_body` | Finite proper rigid rotation and signed translation, with immutable input records |
| `tessellate` | Actual outward closed stock, cylinder, sector and concentric-cut meshes; strict quality and output budgets (ADR-063) |
| `rectangular_stock_geometry` | Unique outward in-face pick on uncut tracked cuboid; OCCT verifies actual BREP types/topology privately |
| `assign_face_ref` / `face_derivation_history` | Body-owned actual face IDs, retained outer faces on cuts, split-wall ancestry and new IDs/history on transforms |
| `has_exact_rigid_placement` | True only for an actual tracked analytic body in this mode |

Lengths/radii must be above 1e-9 m and at most 1000 m; world positions/bounds
must remain within 1e6 m. Direction norms use the existing dimensionless vector
epsilon; normal/orthogonal/rotation admission uses 1e-9 dimensionless tolerance.
Radial material clearance must exceed 1e-9 m. Only arithmetic roundoff within
`64 * f64::EPSILON * max(1, stock_height_m)` is coalesced for coincident cut
endpoints/axes. Near-but-distinct axial/radial steps within 1e-9 m refuse instead
of erasing thin material. At most 100000 wires, 100000 bodies and 64 cut records
per stock are retained. Density is finite and positive in kg/m³; non-finite
measurement results refuse. Profile/body state is owned by the kernel instance,
ordered deterministically, and never serialized as model source.

Union/intersection, mixed/nonconcentric/oblique/laterally intersecting/out-of-stock
cuts, through-all, other shape operations, compounds, imports, STEP export,
inertia and edge/circular-edge queries refuse. `rotate_body`/`mirror_body` refuse;
use the admitted proper `transform_body` for rotation. Unknown handles never
yield synthetic measurements, meshes, face membership or identity transforms.

## Physical mesh and face witness (P16-010 / ADR-063)

P17-020 / [ADR-103](../adr/ADR-103-physical-arc-sector-parity.md) adds exact
one-arc/two-radii sector records to this explicit physical mode, with analytic
mass, selected-branch world bounds, five-face closed meshes and rigid placement.
See [arc-sector admission and tests](arc-sectors.md) for the bounded contract.
This does not admit sectors as cuboid/cylinder stock or generic Boolean inputs.

`TessellationSettings` requires positive finite linear deflection in metres and
angular deflection in degrees (at most 180). A uniform half-gap grid has at least
eight samples; perforated caps retain all four true rectangle-corner rays.
Actual angular gaps, sagitta plus measured f32/arithmetic coordinate error,
triangle nondegeneracy, outward winding, opposite paired geometric edges and
positive signed tetrahedral volume are checked before returning a mesh. Quality
finer than representable coordinates refuses; no automatic relaxation. The
analytic exact mass/area/centre/bounds path remains independent of facets.
Budgets: 8192 angular samples including anchors, 100000 vertices and 200000
triangles. Vertices are duplicated per triangle for physical flat normals.
Budget estimates and f32 collapse refuse without publishing partial output.

`RectangularStockGeometry` is transient: entry face ID, `origin_m`, unit
`u_axis` / `v_axis`, `outward_normal`, `face_spans_m` and
`inward_thickness_m`. Axes need not form a right-handed frame. Match a finite
unit `FacePick` to exactly one face by plane distance <=1e-9 m, cross-normal
norm <=1e-9 with positive dot, and position inside its spans (1e-9 m tolerance).
Mock only admits uncut tracked stock. Native privately verifies one valid solid,
six planar faces without inner wires, twelve straight edges, eight unique
cuboid corners and orthogonal dimensions, using OCCT exact type/topology queries.
Neither kernel uses nearest faces or a bounding box as proof. Source extrusion
ownership and semantic-reference validity remain feature-layer obligations.

Mock face IDs are deterministic within a kernel instance and distinct for
unrelated bodies. Cuts retain outer IDs and unchanged internal IDs; split
same-radius walls record actual overlapping ancestry. Transforms allocate new
IDs with derivation pairs `(result_id, input_id)`. Membership validates the
specified body. Native keeps its existing body-scoped enumeration convention;
its stock witness ID corresponds to actual triangle metadata. Numeric IDs are
runtime observations, never globally unique source identities.

This is infrastructure, not a document-level seated-hole feature. Schema,
DesignPatch/source/semantic ownership is implemented separately for counterbores
in [MCAD-P16-003](seated-hole-admission.md); countersinks use MCAD-P16-004 on P16-011. Existing CLI/MCP Mock selection is not switched to this
mode. No document MCP eval is claimed for this SDK query or mesh infrastructure.
Native authoring remains through verified DesignPatch/transactions.

The directly related native primitive fix centers ordinary rectangle/circle
`Symmetric { length }` extrusion on the profile plane, with total depth twice
the supplied length, as already done for exact ellipse/cubic primitives. This
corrects its previous one-sided placement in both new-body and cut operations.

Run the [SDK example](../../examples/analytic-mock/README.md),
`cargo test -p opencad-geometry --test analytic_mock`, and
`cargo test -p opencad-kernel-occt --test analytic_primitives`.
The native integration suite independently checks both implementations; no
synthetic ID result is counted as physical evidence.

Independent mesh/witness evidence: `cargo test -p opencad-geometry --test physical_mesh`
and `cargo test -p opencad-kernel-occt --test physical_mesh`. Native keeps its
existing tessellation contract; no Mock sampling budget is imposed on arbitrary OCCT shapes.

## Bounded countersink cut (P16-011, ADR-064)

`GeometryKernel::cut_countersunk_stock(body, &CountersinkCut)` cuts the actual
cylinder/frustum union on original rectangular stock. The transient input has
`entry: FacePick` (outward normal), `bore_radius_m`, `seat_radius_m`,
`bore_depth_m`, `seat_depth_m`; all dimensions are metres. It does not carry
Design Graph ownership, an angle expression or persisted source. An actual
unique cuboid witness is mandatory. Entry plane offset allows arithmetic
roundoff only; radii/depths and lateral material use strict 1 nm margins.

Explicit analytic Mock admits either tracked extrusion end; Native also admits
other physical cuboid faces. The future source feature uses only the common
extrusion-end admission. Default Mock and unknown stock refuse. Already-cut,
compound and curved targets refuse. New generic cylindrical cuts on tapered
Mock stock refuse; arbitrary variable-radius intersections are not admitted.

Mass integrates the piecewise linear radial envelope exactly: a frustum and
its bore overlap once. Area includes conical lateral area and actual openings;
COM uses exact first moments. Meshes retain ADR-063 quality/budgets/closed
winding and body-owned face IDs, with distinct conical walls and no fictitious
shoulder at the continuous bore join. Rigid transforms preserve physical data.
Run `analytic_countersink` and the independent `countersink` integration tests
in both geometry and kernel-occt crates. The runtime prerequisite itself exposes no .ocad or MCP command; P16-004
supplies the angle-bearing document contract through existing patch commands.

## Cylindrical stock and solid draft (P16-012, ADR-065)

`cylindrical_stock_geometry(&body, axis_hint)` obtains an actual full cylinder:
origin in metres, oriented unit axis, radius/height in metres and distinct
body-owned start/end/lateral face IDs. Native checks exact complete cylinder,
circle/cap/seam topology and actual outward analytic normals privately, including
indirect surface frames; Mock queries only its tracked Cylinder record. Axis
hint must be finite, unit and parallel/antiparallel within 1e-9. It selects
orientation, not shape. Box, cut/partial/tapered/compound/untracked stock refuses.

`draft_cylindrical_stock(body, &CylindricalDraftInput)` takes axis hint, neutral
origin in metres, unit neutral normal and signed radians. Plane is parallel to
caps, within stock height (only arithmetic endpoint roundoff). Angle magnitude
is 1e-8..45 degrees; endpoints must have radii in (1 nm,1000 m], differing by
more than 1 nm. World bound is 1e6 m. Positive narrows toward neutral normal;
negative widens. Neutral circle radius is unchanged. Unsupported/tiny/collapsed
or out-of-bound inputs refuse; no clamping or silent cylinder substitute.

Native constructs a real cone through an exception-catching BREP bridge. Mock
tracks an actual solid Frustum with exact polynomial volume/area/COM and endpoint
circle support bounds; closed outward conical meshes use ADR-063 quality/error/
budget gates. Reconstruction publishes new body-owned faces with no invented
history. Actual rigid transforms preserve physical measurements. Generic Mock
Booleans/shape edits on frustums refuse. This introduces no .ocad or MCP source
command; P16-005 owns the closed source/semantic/patch/old-reader contract.

## Bounded tapered-rib union (P16-013, ADR-066)

`TaperedRibInput` holds an outward `FacePick` at the footprint centre, finite unit
`length_axis_hint`, `length_m`, `base_thickness_m`, `height_m` and signed
`angle_rad`. Query actual `rectangular_stock_geometry`, then `admit` returns
transient `TaperedRibGeometry`: actual base centre, length/width/outward axes,
length, base/top thickness and height. Hints choose witnessed in-plane axes;
no source provenance is inferred from shape. `add_tapered_rib_stock` repeats the
actual stock witness/admission and returns a real connected union.

T(z)=T0-2*z*tan(angle), neutral base, square end planes. Dimensions are finite
in (1nm,1000m], angle magnitude 1e-8..pi/4, top thickness positive and width
delta >1nm. Added volume exceeds 64*EPSILON times stock volume. Require a full
strictly contained base rectangle, >1nm margins, arithmetic-only plane offset,
actual stock-axis alignment and world vertices within 1e6m. Unknown/modified/
curved stock, oblique hint, non-unit data, collapse and unresolvable addition
refuse. Runtime admits actual physical box faces; only later graph source can
establish original authored extrusion ends/import/boolean ownership.

Analytic Mock stores separate RibbedStock data. Its exact volume/area/first
moments include the added trapezoid and exclude the internal contact surface.
Bounds include actual stock and rib vertices; proper rigid placements retain
physical geometry, with accumulated-frame drift refusal. General shape edits/
booleans/cuts on this record refuse. Reconstruction has no fake ancestry.

A deterministic 4×4 coordinate grid partitions both stock caps and walls around
the rectangular footprint; omit the contact cell and add five true rib quads.
It produces 11 face IDs and 68 triangles (204 per-face vertices), with matching
shared boundary subdivisions, finite/f32-deflection checks, nondegeneracy,
paired opposite edges and positive oriented volume. Exact planar tessellation
needs no angular sampling; invalid quality, budgets or precision refuse without
looser retries. Native exact prism/union is verified separately, not replaced by
this grid. Default Mock remains unsupported. SDK example:
`analytic_tapered_rib`; independent tests: `tapered_rib` in geometry/kernel-occt.
No .ocad feature, MCP tool or new maturity/host/release credit yet; P16-006 owns
source/semantic/patch/schema/history/example/eval afterward.
