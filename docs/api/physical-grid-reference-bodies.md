# Physical grid reference bodies

P16-041 / [ADR-094](../adr/ADR-094-physical-grid-reference-bodies.md) adds
`GeometryKernel::import_circular_fillet_grid(&CircularFilletGridSolid)`.
The only input is the immutable, privately constructed ADR-093 grid certificate;
there are no caller-supplied vertices or substitute mass/error metadata.

The default trait implementation and legacy `MockGeometryKernel::new()` refuse.
`MockGeometryKernel::with_analytic_primitives()` explicitly stores the actual
TriangleSolid in a new physical record. Generic `import_triangle_solid` remains
unsupported in this mode. Mass, area, centroid and bounding box are computed
from the stored grid coordinates. Each triangle gets a current physical facet
ID; raw diagnostic chart groups are not semantic topology IDs. Fresh imports
have fresh owned IDs. OCCT constructs a genuine faceted BREP through its existing
triangle-solid bridge, including validity, self-intersection and positive-volume
checks. The existing bridge sews in metres at 1e-8 m tolerance and checks volume
agreement within max(1e-18 m³, 1e-8 relative), then round-trips its BREP. These
numerical checks do not extend the input coordinate certificate. It preserves
existing body-local face-index conventions: native ownership
is the pair (body handle, local face index); identical numbers on different bodies
do not identify the same face. Out-of-body indices refuse.

Both kernels support ordinary proper rigid placement of the actual facets and
leave the input body queryable. Native reference-body markers survive placement
and are checked through compound members, so reflections/scales/nonfinite
placement refuse. Original grid error bounds apply only to the input unplaced
coordinates. There is no transformed/native/f32 error certificate. Rendering
quality describes tessellation of the already faceted body, not refinement of
the ideal reference. Analytic Mock validates mesh quality and emits actual f32
facets with owned IDs, never a synthetic box.

Analytic Mock shape edits and stock/cylinder/rim/support queries outside this
record's capabilities refuse. The native backend retains ordinary facet
operations; these do not propagate any reference certificate. Full-cylinder,
complete-circle and cuboid queries must not invent analytic supports from these
facets. Existing raw/rational diagnostics, STL/intersection caps and unrelated
kernel behavior are unchanged.

This API constructs a physical **reference** body. It does not apply a variable
fillet to an authored cylinder or consume a stock rim. Production smooth
supports, source/semantic ownership/history/cache/schema/old-reader admission,
exact G1/global embedding and native floating fidelity remain separate. There
is no new source feature, MCP operation, eval or maturity score; P16-008 stays
planned. See [the runnable example](../../examples/physical_grid_reference_bodies.md).

Integration comparison tolerances are 1e-14 m³ volume, 1e-10 m² area, 1e-9 m
centroid and 2e-7 m bounding coordinates; rendered coordinates are checked within
5e-9 m unplaced / 3e-8 m placed. These are numerical test evidence, not new
whole-geometry error certificates.

ADR-095 adds [input-grid signed-volume comparisons](grid-reference-volume-difference.md).
Backend mass remains actual measured grid/BREP mass. The input comparison does
not certify native or placed coordinates; the example reports both separately.
