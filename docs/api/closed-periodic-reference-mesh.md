# Closed periodic reference mesh diagnostic

P16-039 / [ADR-092](../adr/ADR-092-closed-periodic-reference-mesh.md) constructs
actual indexed f64 triangle shells approximating the ideal circular rolling-
sphere fillet, its original cylinder region and both planar caps. This is a
mesh prerequisite, not physical Mock/runtime or authorable fillet admission.

`reference.closed_reference_mesh(CircularFilletMeshPrecision)` requests a finite
positive `maximum_position_error: Length`, dimensionless unit-normal chord
`maximum_normal_error` strictly in (0,1), `max_vertices` in 14..=33026 and
`max_triangles` in 24..=66048. All cells must meet both bounds. Uniform dyadic
n=1..64 refinement is deterministic and complete; the next level must fit both
budgets before allocation. Exhaustion returns no partial shell.

`CircularFilletReferenceMesh` exposes immutable `vertices_m()`, `triangles()`,
`triangle_face_groups()`, `divisions_per_chart_axis()`,
`position_error_upper_m()`, `normal_error_upper()` and `boundary_integrals()`.
Its 4n angular stations each contain 2n+1 profile vertices, plus two cap centres.
There are 4n(2n+1)+2 vertices and 16n²+8n triangles. Quarter seams, the full period,
side/fillet contacts and caps share explicit IDs. No tolerance weld or STL-grid
rounding is performed. Groups 1..4 are fillet reference charts, 5..8 original
cylinder reference charts, 9 top and 10 bottom; these are NOT semantic/native IDs.

Fillet cells reuse ADR-075's full triangle interpolation bound and actual rounded
corners. Analytic side and cap bounds include actual stored vertex rounding.
Each actual facet's exact mathematical normal is enclosed outward from its
stored f64 coordinates, then compared with the complete ideal normal enclosure
of its cell; require positive lower alignment and requested chord precision.
No finite witness authorizes a triangle. All errors include their documented
parameter correspondence; cap fans use barycentric radial correspondence.
No later f32/STL/native conversion error is certified.

The generated shell passes connected adjacency, exactly two opposite uses per
edge, Euler two, finite nondegenerate locally outward facets and all ten groups.
`CircularFilletMeshIntegrals` encloses actual signed tetrahedron volume flux
(m³), unsigned facet area (m²) and first moments (m⁴). It is not a general solid
admission or physical Mock mass property. Global intersections/embedding and
production ownership/source/history remain separate requirements.

The pinned native probe constructs real triangular planar faces, sews them into
closed positive-volume solids, matches current vertices/face connectivity and
normals to the actual generated mesh, and independently measures volume, area
and COM. These native solids are faceted: they do NOT retain the analytic
cylinder supports of the separate rational-BREP diagnostic. Comparison tolerances
are numerical evidence: 1e-12 m vertices, 1e-9 normal chord, 1e-15 m³ volume,
1e-12 m² area, 1e-17 m⁴ moments and 1e-10 m COM. Native floating evaluation,
exact G1, global embedding and whole-BREP fidelity are not certified.

The fixture uses R=0.01 m,H=0.02 m and constant 0.0005 m / variable
0.0005..0.0015 m laws. Coarse/fine requests are 4e-4/1e-4 m position and
0.8/0.5 normal chord. Current levels n=8/16 yield 1088/4224 triangles. Fine
position bounds are 3.907e-5 m constant and 4.297e-5 m variable. Native mass
is compared to the ACTUAL triangle shell's integrals, never replaced with ideal
mass. Refinement is independently checked against ADR-073's ideal volume;
this does not provide an ideal/mesh volume-error certificate.

Run [the example](../../examples/closed_periodic_reference_mesh.md). CI performs
both synthetic parser tests and actual native construction, retaining
`closed-reference-mesh-diagnostic.json` in the existing diagnostics artifact.
The four actual refusal controls (truncated input, wrong units, missing triangle,
reversed winding) must emit no partial stdout. Existing rational/native boundary
proofs are unchanged. No source/schema/golden/MCP operation/eval or score changes;
P16-008 remains planned.
