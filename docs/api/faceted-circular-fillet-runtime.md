# Guarded grid-faceted periodic cylinder fillet runtime

P16-047 / ADR-100 adds the kernel-neutral numerical operation
`GeometryKernel::grid_faceted_periodic_cosine(body, &FacetedCircularFilletInput)`.
It replaces an actual complete cylinder by a new faceted body. ALL supports and
caps are rebuilt as planar facets; no unchanged analytic support or exact G1
contact is promised. Default legacy Mock refuses. Analytic Mock and OCCT create
real solids, preserve the stock, and allocate fresh output-owned facets.

The explicit input contains:

- `end_edge_id`: a current end-rim index scoped to the supplied body;
- `origin_m`, unit `phase_x` and positive unit `axis`: the caller's authored
  frame, with finite orthogonal directions checked to 1e-12 BEFORE normalization;
- `cylinder_radius`, `cylinder_height`, `minimum_radius`, `maximum_radius`:
  explicit `Length` values; never derive phase from a BREP seam;
- `mesh_precision` and `grid_precision`: existing canonical position, normal
  and resource requests;
- `volume_precision`, `area_precision`, `moment_precision`: existing ideal
  widths/work and absolute canonical discrepancy requests (m³, m², m⁴, Length).

The kernel independently queries the actual full cylinder and both complete
cap/side rim circles, then checks the requested positive-frame end ID, expected
origin/radius/height within 1e-9 m and axis within 1e-12. A correct scalar volume
or matching edge number alone cannot pass. An original authored source and
semantic aliases must still be enforced by the future feature layer.

`input.prepare(&rims)` is a pure introspection method returning an immutable
`FacetedCircularFilletPlan`: canonical `grid()`, proper `placement()`, and
volume/area/moment comparisons. A caller-prepared plan supplies no authority
over a physical kernel body. The kernel rechecks its own current witness and
constructs the plan privately before executing. Invalid frames/lengths/rim,
coordinate range, precision/work or any discrepancy budget refuse before output
insertion. Limits remain 1 nm stored grid, 10000 triangles, one million
intersection candidates and 262144 leaves separately per ideal integration pass.

Analytic Mock transforms the actual stored facets. OCCT builds/sews the actual
canonical triangles, applies the complete proper 3x4 SI matrix with
`gp_Trsf::SetValues`, then repeats one-solid validity, BOP self-intersection and
actual grid-volume agreement checks. The full matrix preserves small explicitly
authored phase rotations. Only the final complete body is inserted, with the
existing proper-placement marker. Ordinary native face indices remain local to
(owner body, index); diagnostic chart groups are not native semantic identities.

Precision scope is explicit: rigorous bounds apply to the canonical stored input
grid and ideal reference. They do not become native, placed or f32 certificates.
Native body properties, contacts and bounds are numerical physical measurements
with separate test allowances. OCCT bounding boxes include the existing 1e-7 m
kernel padding; tests compare that padding separately from the actual vertex hull.
Mass properties always come from the actual body. Output cylinder/analytic
support queries refuse. No new source feature, schema, migration, MCP operation,
eval or score is introduced; P16-008 remains planned.

Run `cargo run -p opencad-kernel-occt --example faceted_circular_fillet` for
constant and placed-variable examples in both kernels. The existing
`circular_grid_reference` CI JSON includes the same `guarded_operations` report:
canonical input bounds/comparisons, actual stock/output properties and measured
contact vertices are separate. See [ADR-100](../adr/ADR-100-guarded-grid-faceted-circular-fillet-runtime.md).

ADR-101 now supplies a separate authored source wrapper with original
cylinder/rim/phase, persistence, transaction and actual output/cache checks.
See [the source contract](faceted-circular-fillet-source.md); this runtime's
canonical-only certificate scope is unchanged.
