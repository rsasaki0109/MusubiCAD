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
| `make_wire_from_sketch` | Closed right rectangle (four corners; optional matching line segments), or exact single circle; finite orthonormal sketch placement |
| `extrude` | Distance or symmetric normal extrusion; `NewBody` without a target, or circle `Cut` with admitted stock target |
| `boolean` / `body_boolean` | Subtract a strictly radially contained, axially bounded cylinder from rectangular stock; one concentric chain only |
| `mass_properties` | Independent SI volume, surface area, positive-density mass and centre of mass |
| `bounding_box` | Actual stock corners or analytic cylindrical support bounds |
| `transform_body` / `translate_body` | Finite proper rigid rotation and signed translation, with immutable input records |
| `tessellate` | Actual outward closed stock, cylinder and concentric-cut meshes; strict quality and output budgets (ADR-063) |
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
in [MCAD-P16-003](seated-hole-admission.md); countersinks remain MCAD-P16-004. Existing CLI/MCP Mock selection is not switched to this
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
