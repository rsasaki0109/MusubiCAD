# Grid-admitted periodic reference solid

P16-040 / [ADR-093](../adr/ADR-093-grid-admitted-reference-solid-bounds.md)
converts an immutable ADR-092 reference mesh into the actual stored
`TriangleSolid`, retaining whole-facet position and normal bounds after its
existing 1e-9 m grid rounding.

Call `mesh.grid_solid(CircularFilletGridPrecision {
maximum_position_error: Length, maximum_normal_error: f64 })` with finite
positive SI position precision and normal chord precision strictly in (0,1).
The immutable `CircularFilletGridSolid` exposes `solid()`,
`position_error_upper_m()`, `normal_error_upper()`,
`grid_displacement_upper_m()` and `grid_normal_change_upper()`.

The existing constructor checks connected closure, vertex fans, numeric
intersections and positive volume with unchanged limits (10000 triangles and
1000000 intersection candidates). Conversion additionally requires equal vertex
and face counts and a bijective original-to-stored vertex map derived from
ordered face/corner identity. Grid merging, degeneration, altered correspondence,
nonfinite arithmetic, admission failure or insufficient final precision returns
no certificate. There is no coarsening, retry or partial result. In particular,
a valid n=32 raw reference mesh has 16640 triangles and must refuse conversion.

Outward interval arithmetic encloses the maximum actual vertex movement D and
exact mathematical facet-normal change E. The final bounds are outward sums
`raw_position + D` and `raw_normal + E`. Barycentric interpolation bounds vertex
movement over each entire facet. The original ideal correspondence is retained;
finite sample witnesses never authorize a result. `solid().mass_properties()`
measures the actual stored grid polyhedron, rather than substituting ideal mass.

Only these untransformed stored binary64 coordinates are certified. Existing
numeric intersection checks are not an exact global embedding proof. Subsequent
rigid placement, f32 rendering, STL serialization, native conversion/evaluation,
ideal-versus-grid volume error and exact G1 remain separate. There is no product
kernel body, editable feature, source/history/cache admission or maturity score.

The existing example/probe now retain raw and grid diagnostics separately. OCCT
constructs both fixed fine grid cases, verifies current triangular planar faces,
vertices/topology/normals and independently measures actual grid volume, area and
COM. Explicit `--grid` input mode permits a 1e-9 m stock-bound expansion only;
raw transport keeps 1e-12 m, and matching/sewing/comparison tolerances stay as in
ADR-092. Both modes execute all four real refusal controls with empty stdout.
The CI diagnostic report carries `grid_native` and independently graded
`grid_cases`; a grid failure must fail the entire run.

See [the runnable example](../../examples/closed_periodic_reference_mesh.md).
