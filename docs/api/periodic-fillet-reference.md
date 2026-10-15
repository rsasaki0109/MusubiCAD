# Bounded ideal periodic fillet reference

P16-019 implements [ADR-072](../adr/ADR-072-bounded-periodic-fillet-reference.md)
as transient Rust arithmetic. This is an ideal surface reference, not a geometry
kernel body, native approximation, source feature or MCP command.

`CircularFilletReference::new(R, H, r_min, r_max)` takes four Length values.
All must be finite, above 1nm and at most 1000m; require r_min<=r_max<=R/4 and
H>=4*r_max. Getters return these explicit Length values. The local reference
cylinder has base z=0 and axis +Z; its radius is maximal at +X. Real source
placement and phase admission remain P16-008 work.

`radius_at(theta)` returns the cosine radius as Length. `sample(theta,alpha)`
takes Angles with theta in [0,2*pi], alpha in [0,pi/2], and returns:

- `point_m`: the ideal rolling-sphere envelope point in metres.
- `normal`: its dimensionless outward unit sphere/surface normal.
- `theta_tangent_m`, `alpha_tangent_m`: analytic parameter derivatives in metres
  per radian, calculated by private first-order chain rules.

Only the declared theta=2*pi endpoint is canonicalized to zero, retaining the
same values/derivatives at the seam. Other out-of-range angles refuse. The radius
expression is evaluated as r_min+(r_max-r_min)*(1+cos(theta))/2 to avoid cancelling
the small minimum against the mean. No positive amplitude is inferred constant.

`exposure_margin_m2()` returns `(2-7*q+q*q/2)*R^2`, q=r_max/R. ADR-072 proves
that it is at least 9*R^2/32 in real arithmetic and bounds the positive factor in
the moving-sphere residual. This establishes global exposure and regularity of
the ideal surface over the admitted range. Floating-point evaluation is not
an outward-rounded numerical certificate, a mesh error bound or native proof.

`constant_limit_volume_m3()` gives the ideal constant-radius rounded-cylinder
volume only for identically authored minimum/maximum values. An explicitly
unequal pair, even one floating-point step apart, refuses. This original constant-only method retains its refusal semantics. No area/COM
result or backend mass-property substitution is supplied.

See the [executable example](../../examples/periodic_fillet_reference.md).
Kernel solids, bounded meshes/sampling, native parameter/contact/periodicity
admission and guarded source/reader/cache/history/eval remain separate work.

## Bounded nonconstant volume (P16-020)

[ADR-073](../adr/ADR-073-periodic-reference-volume-bounds.md) adds
`volume_bounds_m3(max_width_m3, max_cells)`. The first argument is a finite positive
absolute **full enclosure width in cubic metres**, not a relative tolerance;
the second is an explicit leaf budget in 4..=262144. For example 1e-12 m³ is
0.001 mm³. There is no implicit sampling quality or precision default.

The result `CircularFilletVolumeBounds` contains `body_lower_m3`, `body_upper_m3`,
`removed_lower_m3`, `removed_upper_m3`, and `cells`. It bounds the ideal reference
volume; it does not measure an OCCT solid. The characteristic-circle arc is
integrated analytically, preserving tangential displacement, then one angular
integral is enclosed by directed binary64 interval arithmetic on rational charts.
IEEE-754 nearest-rounded operations and correctly rounded sqrt are assumed;
private adjacent-float helpers preserve Rust 1.78 compatibility. The integrator
uses neither libm trigonometry nor a finite-sample convergence estimate as a bound.

Largest-width cells are bisected deterministically. Invalid domains, insufficient
budgets, stalled subdivision or unattainable precision return an error; no coarse
midpoint is returned as a precise answer. The final width includes stock-volume
and roundoff bounds. Identical authored radii use the enclosed constant formula
with `cells=0`; an explicit positive amplitude retains the nonconstant path even
when tiny. The returned enclosure need not have the same width at different
scales, and the API promises no native fidelity or approximation admission.


P16-021 uses this actual Rust API in the
[native fidelity diagnostic](native-periodic-fillet-diagnostic.md). Native mass
estimates and contact failures remain separate from the ideal volume enclosure;
passing that diagnostic confirms counterexamples, not supported geometry.

## Bounded reference patches (P16-022)

[ADR-075](../adr/ADR-075-bounded-periodic-reference-patches.md) adds
`patch_bounds(chart, u_min, u_max, v_min, v_max)`. Chart is 0..3; the finite,
nonempty coordinate intervals must lie in [0,1]. These coordinates are explicitly
dimensionless, with theta=chart*pi/2+2*atan(u) and alpha=2*atan(v). Rational circle
coordinates evaluate the reference without trigonometric libm calls.

`CircularFilletPatchBounds` returns ideal position bounds in metres, dimensionless
ideal normal component bounds, `du_*_m`/`dv_*_m` chart-derivative component bounds,
and upper Euclidean norms of F_uu,F_uv,F_vv in metres. These derivatives are with
respect to chart coordinates, not radians. Wide boxes can be conservative or
refuse if arithmetic cannot establish the required domains. The same IEEE-754
rounding/sqrt assumptions as ADR-073 apply. Shared interval primitives remain
private; a second-order jet encloses chain rules, not numerical differences.

Four `corner_points_m` are ordered (u_min,v_min), (u_max,v_min), (u_max,v_max),
(u_min,v_max). `corner_rounding_error_upper_m` bounds their displacement from the
exact ideal corners. Declared u=1 seams canonicalize to the next chart's u=0,
including chart3 to chart0, for bit-identical shared corner positions. Nearby
inputs do not snap. The patch derivatives remain in the original chart, whose
endpoint parameter scale differs from that of its neighbour. Ideal position
boxes describe the reference; use the separate rounding bound when reasoning
about representative vertices.

For triangle indices (0,1,2),(0,2,3), the returned
`linear_interpolation_error_upper_m` encloses parameter-corresponding positional
deviation from the ideal patch, including corner rounding. Its exact-corner term
is `(Muu*hu²+2*Muv*hu*hv+Mvv*hv²)/8`, using whole-patch derivative norm bounds.
This establishes a positional Hausdorff bound for those two triangles; it does
not establish normal error, closure/embedding, mass, native spline fitting or
feature admission. No mesh is returned, and no grid/quality/budget default is
chosen. Invalid rectangles or nonfinite enclosure arithmetic refuse. Subdivision
can sharpen the bound, with a precision floor from rounded corners.

See the [reference example](../../examples/periodic_fillet_reference.md) and
[independent tests](../../modules/geometry/tests/circular_fillet_patch.rs).


## Bounded native Bezier reference patches (P16-023)

[ADR-076](../adr/ADR-076-bounded-bezier-reference-patches.md) adds
`bezier_patch(chart, u_min, u_max, v_min, v_max)` with the same domain as
`patch_bounds`. `CircularFilletBezierPatch.control_points_m` is a 4x4 bicubic
control net indexed [u][v][xyz], in metres. The resulting mathematical surface
uses normalized local coordinates in [0,1]², rather than the caller's original
chart rectangle. This API constructs fixed-size transient reference data.

`position_error_upper_m` bounds position and parameter-corresponding Hausdorff
error throughout the patch. It adds `reference_to_bilinear_error_upper_m` to
`control_net_deviation_upper_m`, using outward rounding. The first contribution
is ADR-075's Taylor/corner bound, also valid for bilinear corner weights; the
second bounds the difference between actual returned poles and the degree-elevated
bilinear net using Bernstein's convex hull. Derivative enclosures help construct
the poles; no exact Hermite interpolation or tangent-continuity claim follows
from choosing rounded representatives. Editing poles invalidates their bound.

Shared edges with identical along-edge intervals have bit-identical control
coordinates, including all four quarter-chart seams. This establishes shared
positional curves, not normal error, regularity, G1 continuity or closure of a
solid. No mesh, BREP, mass, manufacturing tolerance/default layout, source feature
or editable NURBS is supplied. Invalid domains/nonfinite arithmetic refuse.

Run the actual pinned Linux SDK surface diagnostic:

```sh
python tools/run_reference_bezier_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/reference-bezier-diagnostic.json
```

It invokes the actual Rust example, passes its metre-valued poles into
`Geom_BezierSurface`, then checks native degrees/poles, points and first
local derivatives against an independent Bernstein evaluator. Forty constant and
nonconstant cases cover all charts, side/top contacts, interiors and full-cycle
shared seams. One thousand sampled native positions are also compared to the
angular reference. The explicit 1e-14 m evaluation allowance is a numerical test
allowance, not an all-parameter native floating-point error certificate. The
all-patch certificate applies to the mathematical surface of the returned poles.
A uniformly displaced surface is a negative report control; successful diagnostics
always retain `candidate_admitted: false`. CI saves both native diagnostic reports.

See the [runnable fixture](../../modules/geometry/examples/periodic_fillet_bezier_patches.rs),
[independent geometry tests](../../modules/geometry/tests/circular_fillet_bezier.rs),
and [example](../../examples/periodic_fillet_reference.md).


## Normal and orientation certificates (P16-024)

[ADR-077](../adr/ADR-077-bezier-normal-and-orientation-bounds.md) adds
`patch.normal_bounds(reference, chart, u_min, u_max, v_min, v_max)`.
`CircularFilletNormalBounds` describes the mathematical Bezier surface of the
current poles using normalized local dimensionless coordinates. It recomputes
from these poles, ignoring all three stored positional-error fields. A normal
certificate cannot validate stale position metadata or prove positional fidelity.

The derivative control nets are `3*(P[i+1][j]-P[i][j])` and
`3*(P[i][j+1]-P[i][j])`. Their componentwise Bernstein hulls enclose derivatives
throughout the square. Outward cross-product, norm and division operations yield:

- `jacobian_area_lower_m2`, `jacobian_area_upper_m2`: bounds on the magnitude of
  the local derivative cross product, in m². A positive lower bound proves local
  immersion of this patch, including its boundary.
- `normal_lower`, `normal_upper`: dimensionless oriented unit-normal component
  bounds. Cross-product order is positive local u then positive local v.
- `ideal_normal_alignment_lower`: a strictly positive lower bound on the dot
  product with the ideal reference normal, proving matching orientation.
- `normal_error_upper`: an upper Euclidean unit-vector chord distance from the
  ideal normal. It is dimensionless, not an angle in radians.

Nonfinite poles/arithmetic, collapsed or reversed nets, hulls unable to prove
positive area/alignment and precision underflow refuse. Wide hulls may refuse
valid surfaces; no sampling fallback, repair or normal tolerance is inferred.
Certificates must be recomputed after changing poles, reference or rectangle.
No global embedding, exact G1 continuity, support trimming or solid is admitted.

On matching ideal shared boundaries, the sum of the two reference-normal error
bounds (rounded outward) bounds their normal gap. It does not prove zero gap.
The existing live native diagnostic now measures actual D1 unit normals and area
at all 1000 witnesses, checks independent Bernstein/ideal comparisons and all
quarter-chart seam pairs. Native normal evaluation has an explicit dimensionless
1e-11 test allowance, distinct from the metre-valued point/derivative allowance;
area comparisons have a 1e-24 m² absolute allowance and a 1e-11 relative comparison
allowance against Bernstein evaluation. Neither is an all-parameter native error
certificate. The all-patch guarantees follow from the mathematical derivative
hulls and interval arithmetic. Diagnostics continue to report no feature admission.

See [independent normal tests](../../modules/geometry/tests/circular_fillet_normals.rs).


## Complete periodic reference bands (P16-025)

[ADR-078](../adr/ADR-078-bounded-periodic-bezier-bands.md) adds
`reference.bezier_band(position_error: Length, normal_error: f64, max_patches: usize)`.
Require finite positive position error in metres, dimensionless unit-normal chord
error in (0,2], and a patch budget in [4,16384]. These are explicit caller requests,
not manufacturing defaults. The fixed hard cap bounds allocation/interval work.

`CircularFilletBezierBand` contains a uniform dyadic `divisions_per_chart_axis`,
ordered `cells` and maximum `position_error_upper_m` / `normal_error_upper`.
There are exactly 4*n*n cells, ordered chart/u/v. Each `CircularFilletBandCell`
contains `chart`, dimensionless `[u_min,u_max,v_min,v_max]` in `chart_domain`,
current `patch` poles/position bounds and independently recomputed `normals`.
Normal area coefficients use each cell's own normalized local parameter square.
Editing returned data invalidates their cover/certificates.

Try n=1,2,4,... and refine the whole grid when a coarse interval certificate
refuses or either requested error is exceeded. Check the next cell count before
allocation; return no partial result when the conforming level exceeds budget.
All four charts and their full transverse interval are covered by construction.
Shared complete edge nets match exactly, including the final periodic seam;
there are no T-junctions. Early level refusal and a geometric sum of bounded
levels keep interval work finite. No sampling fallback admits failed cells.

The isolated native diagnostic builds faces from these actual poles, converts
metres to millimetres explicitly and sews the whole band:

```sh
python tools/run_reference_band_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/reference-band-diagnostic.json
```

For the documented constant/variable fixtures, an explicit 1e-4 m position,
0.8 normal-chord request and 4096-patch budget produce n=16 / 1024 faces per band.
These coarse demonstration requests do not authorize a manufacturing feature.
The native diagnostic checks actual BREP validity, one connected face graph in
one open shell, zero solids, one/two-face edge incidence, Euler characteristic
zero and exactly two degree-two free-edge cycles. Both cycles have 4*n vertices;
actual boundary points must match distinct side/top reference contacts and phase.
Actual sewn face surfaces/poles and nine values per face are compared to the
original control nets and an independent Bernstein evaluator with a 1e-12 m
numerical allowance. Sewing tolerance is 1e-6 mm = 1e-9 m, confined to this diagnostic.
CI saves the live report; synthetic report tests are parser/contract controls only.

This supplies a complete bounded reference band and tested native open-shell
topology. It does not establish exact G1, global embedding, support trimming,
closed physical Mock/native solids, mass, mesh policy, editable NURBS or source
feature support. Successful diagnostics retain `candidate_admitted: false` and
`solid_created: false`. P16-008 remains planned.

See [band tests](../../modules/geometry/tests/circular_fillet_band.rs) and
[runnable fixtures](../../modules/geometry/examples/periodic_fillet_bezier_bands.rs).


## Closed native diagnostic approximations (P16-026)

[ADR-079](../adr/ADR-079-closed-native-reference-diagnostic.md) extends the isolated
investigation to actual closed native solids. It adds a runnable diagnostic,
not a product GeometryKernel operation or circular-fillet feature:

```sh
python tools/run_closed_reference_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/closed-reference-diagnostic.json
```

The actual Rust fixture supplies constant/nonconstant bands at explicit 1e-4 m
and 2e-5 m position requests, 0.8 normal chord error and 4096-patch budget. For
these fixtures they produce n=16 and n=32. Each transverse alpha=0 boundary A(u)
receives a ruled polynomial skirt down to z=0, with top poles preserved exactly.
Planar caps close the top and bottom wires; the bottom orientation is explicitly
outward. Coordinates convert metres to millimetres, with 1e-6 mm sewing tolerance.

The skirt follows the polynomial contact curve's x/y coordinates. It is an
approximation of the cylinder side, and does not satisfy the original unchanged
cylindrical-support gate. Measured skirt/cylinder radial deviations are retained
in the report (about 18.9 nm coarse and 1.19 nm fine for these samples). Do not
substitute this representation for an authored cylinder or claim support trimming.

Check actual native validity/closure, one solid and shell, connected faces,
zero free edges, two-face incidence, Euler 2, two outward planar caps and positive
native mass. Counts are 4*n*n+4*n+2 faces, 4*n*(n+2) vertices and 4*n*(2*n+3)
edges. Actual band/skirt surface witnesses use independent polynomial evaluation
with a 1e-12 m numerical allowance. No negative-volume orientation repair occurs.

For each law, ADR-073's actual ideal-volume API uses matching radii and a 1e-12 m³
absolute-width request. Native GProp volume estimates use epsilons 1e-9 and
1e-11; their delta is recorded as a stability observation, not a certified error.
The report subtracts the bounded ideal interval from the native estimate using
outward subtraction. Example signed differences, in mm³:

| Law | Coarse estimate minus ideal | Fine estimate minus ideal |
|---|---|---|
| Constant 0.5 mm | approximately -0.00437218 | approximately -0.000273497 |
| Variable 0.5–1.5 mm | approximately [-0.00489114, -0.00389114] | approximately [-0.000774674, 0.000225322] |

Interval overlap does not prove equality. These comparison intervals hold the
native estimate fixed; they do not certify all native integration error or
whole-solid fidelity. Two-level improvement is evidence for these fixtures, not
a general convergence theorem or manufacturing tolerance. Constant analytic
volume is an independent control. Missing closure/orientation, nonpositive or
unstable mass, altered surfaces, inconsistent references or missing/resolvably
worsening refinement refuse. CI runs actual construction and saves its report.
Synthetic contract tests do not establish native geometry.

The report distinguishes `native_diagnostic_solids_created: 4` from
`product_body_created: false` and `candidate_admitted: false`. Physical Mock
runtime/mesh, exact or separately admitted support/G1 fidelity, global embedding,
independent solid area/volume/moment bounds and guarded source/cache/history
remain unresolved. No schema, migration, editable NURBS or MCP operation is added.
See [runnable fixture](../../modules/geometry/examples/closed_periodic_reference.rs).
