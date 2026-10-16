# Ideal periodic first moments and centroid (P16-045)

[ADR-098](../adr/ADR-098-ideal-periodic-reference-moments.md) adds
`CircularFilletReference::moment_bounds(CircularFilletMomentPrecision)`.
It describes the complete IDEAL reference body, with base z=0, axis +Z and
maximum radius at +X. This is a read-only mathematical reference, not a backend
mass query, placed/source body or production variable-fillet operation.

All precision values are explicit, finite and positive FULL absolute widths:

- `maximum_volume_interval_width_m3`: independent ideal volume width in m³.
- `maximum_first_moment_interval_width_m4`: maximum width of EACH coordinate's
  first-moment interval in m⁴.
- `maximum_centroid_interval_width`: Length, maximum width of EACH centroid
  coordinate in metres.
- `max_cells`: 4..=262144 leaves PER integration pass. Volume and moment passes
  have separate caps, not a shared total; both counts are returned.

`CircularFilletMomentBounds` contains `volume_m3: [lower,upper]`,
`first_moments_m4: [[lower,upper];3]`, `centroid_m: [[lower,upper];3]`,
`volume_cells` and `moment_cells`. First moments are volume integrals of x/y/z,
not inertia tensors or density-weighted moments. Divide by the strictly positive
volume interval outward to obtain centroid intervals. Canonical y reflection
gives exact My/centroid_y zeros; identical authored radii also give exact x zeros
and analytic constant moments with both cell counts zero. An explicitly unequal
radius pair, even one binary64 step apart, stays nonconstant.

The original rolling-sphere envelope and its tangential displacement are retained.
The characteristic arc is integrated analytically through trig degree four,
leaving two angular densities (removed Mx and a nonnegative axial correction).
Deterministic outward rational-chart subdivision restores the R⁴ SI scale.
Existing volume and area algorithms are unchanged. ADR-073's IEEE-754 nearest
rounding, correctly rounded sqrt and enclosed pi/atan assumptions apply. Numerical
test quadratures are independent oracles, not enclosure proofs.

Admission checks EVERY requested final width, including stock/volume/moment
summation and centroid division roundoff. A loose volume enclosure can prevent
a tight moment/centroid request. There is no implicit volume refinement, midpoint
substitution or coarse successful result. Invalid arithmetic, stalled refinement,
unattainable precision and exhausted work refuse without partial output or mutation.

The example requests 1e-11 m³ volume width, 1e-12 m⁴ per moment, 1e-6 m per
centroid coordinate and 262144 leaves per pass. Its `ideal_moments` JSON object
echoes these requests, all intervals/counts and `native_conversion_certified:false`.
Each `bodies[*].volume_m3` / `center_of_mass_m` still comes from ACTUAL Mock/OCCT
mass queries and remains separate. No actual-grid moment discrepancy or native/
placed/f32 conversion certificate follows. See
[example](../../examples/physical_grid_reference_bodies.md) and
[independent tests](../../modules/geometry/tests/circular_fillet_moments.rs).

No source/schema/migration/golden, editable NURBS, MCP operation/eval or maturity
credit is introduced. P16-008's runtime/source/ownership/phase/history/reader and
independently graded feature-eval gates remain separate.
