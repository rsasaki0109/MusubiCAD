# Grid reference signed-volume differences

P16-042 / [ADR-095](../adr/ADR-095-grid-reference-volume-differences.md) adds
`CircularFilletGridSolid::volume_comparison(CircularFilletGridVolumePrecision)`.
A private copy of the original reference binds every comparison to the grid
that was generated from it; callers cannot supply another reference or mass.

Precision fields are finite positive `maximum_reference_interval_width_m3` and
`maximum_absolute_difference_m3` (both m³), plus `max_cells` in 4..=262144.
ADR-073 computes the ideal reference volume at the requested full interval width.
The actual stored binary64 grid triangles are independently integrated outward:
`F = sum(det(a,b,c)/6)`. Require positive finite lower bounds for F and the
ideal volume V. Outward subtraction bounds `D=F-V`; the largest absolute endpoint
bounds the absolute difference, and outward division by V's positive lower bound
bounds relative difference. The caller's absolute discrepancy must be met.

The immutable comparison exposes `grid_signed_volume_flux_m3()`,
`ideal_volume_m3()`, `signed_difference_m3()`,
`absolute_difference_upper_m3()`, `relative_difference_upper()` and
`reference_cells()`. Constant references use their analytic formula and zero
integration cells. Invalid precision/work, integration exhaustion, nonfinite
arithmetic/nonpositive volume or excessive discrepancy returns no comparison.
No mesh mutation, coarsening, alternate reference, midpoint or native mass
substitution is permitted. Tightening the ideal interval cannot remove a real
grid discrepancy. Some requested widths exhaust the existing cell budget and
must refuse even if looser precision succeeds.

This certifies an exact signed boundary-integral difference for the actual
stored unplaced f64 coordinates, under the existing outward arithmetic
assumptions. Interpreting signed flux as physical volume retains the usual
oriented embedded-shell qualifications. Existing numeric TriangleSolid admission
is not upgraded to exact global embedding. There is no native, placed, f32,
ideal area/first-moment, exact G1 or production source certificate. Backend mass
continues to measure its actual grid/BREP geometry; it is never replaced by V.

The [physical reference example](../../examples/physical_grid_reference_bodies.md)
now emits the comparison and both actual backend masses separately. CI runs it
and retains `grid-reference-volume-comparison.json` in its existing native
reference diagnostics artifact. Independent tests use integer-grid determinants,
constant-limit analytic volume, scale-cubed units, variable reference binding,
mesh/reference-integral refinement and refusal without geometry mutation.
The native comparison tolerance (1e-14 m³) remains numerical test evidence,
separate from the exact integral enclosure. No source/schema/golden/new MCP
operation/eval or score change; P16-008 remains planned.
