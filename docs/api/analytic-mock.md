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
inertia, topology and tessellation refuse. `rotate_body`/`mirror_body` refuse;
use the admitted proper `transform_body` for rotation. Non-fallible face history
reports no entries: this mode produces no semantic topology. It cannot supply
viewport, drawing, export or semantic face discovery by substituting a cube.
Unknown handles never yield synthetic measurements or identity transforms.

This is infrastructure, not a document-level seated-hole feature. Schema,
DesignPatch, semantic face and usable tessellation admission remain required
before MCAD-P16-003/004. Existing CLI/MCP Mock selection is not switched to this
mode. Native authoring remains through verified DesignPatch/transactions.

The directly related native primitive fix centers ordinary rectangle/circle
`Symmetric { length }` extrusion on the profile plane, with total depth twice
the supplied length, as already done for exact ellipse/cubic primitives. This
corrects its previous one-sided placement in both new-body and cut operations.

Run the [SDK example](../../examples/analytic-mock/README.md),
`cargo test -p opencad-geometry --test analytic_mock`, and
`cargo test -p opencad-kernel-occt --test analytic_primitives`.
The native integration suite independently checks both implementations; no
synthetic ID result is counted as physical evidence.
