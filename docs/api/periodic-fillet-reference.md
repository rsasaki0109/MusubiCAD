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

## Independent closed boundary integrals (P16-027)

[ADR-080](../adr/ADR-080-closed-reference-boundary-integrals.md) adds
`CircularFilletBezierBand::closed_diagnostic_integrals()`. It recomputes from
current control points and returns `CircularFilletBoundaryIntegrals` with
`volume_m3: [lower, upper]` and `first_moments_m4: [[lower, upper]; 3]`.
It ignores mutable position/normal certificate metadata. The scope is the
mathematical polynomial band, its ruled skirts to z=0 and planar caps; the
unchanged cylindrical support and ideal fillet are different geometries.

The method requires an ordered complete dyadic cover with at most 4096 cells,
finite poles, exact shared polynomial boundaries and a common positive planar
top height. Representation equality on shared poles/grid boundaries is checked
exactly; this is not a numerical proximity test. The top height comes from the
actual top poles, including their rounding, rather than an ideal stock height.
Malformed, nonplanar, nonfinite or over-budget covers refuse.

For J=x_u*y_v-x_v*y_u, integrate z*J, x*z*J, y*z*J and z²*J/2 on each band
patch. Skirts and bottom have zero z-directed flux. The top cap's area and
first moments follow from Green integrals of its actual cubic boundary curves.
Bernstein derivative/product identities and exact basis integrals, evaluated
with outward arithmetic, enclose these polynomial integrals without sampled
quadrature. Native GProp and ideal-volume integration are independent of this
calculation. Signed flux bounds alone cannot prove embedding or a physical solid;
reversed boundaries remain signed, without automatic orientation repair.

The existing `run_closed_reference_probe.py` command now measures actual native
COM in metres and first moments in m⁴, and compares both to these independent
intervals. A positive volume interval also permits interval division for COM.
The numerical comparison allowances are separately reported: 1e-15 m³ volume,
1e-17 m⁴ first moments and 1e-10 m COM. Mathematical interval widths must fit
within the corresponding volume/moment comparison allowances. SI/mm conversion,
sewing, native cap-plane rounding and native integration error are outside the
polynomial certificate; passing comparison does not certify all native error.
Synthetic report fixtures test rejection logic only.

Independent regressions cover analytic prism volume/moments, XY translations,
SI scaling, changed interior poles, reversed orientation, invalid boundaries,
and six-point Gauss-Legendre surface quadrature for real constant/variable bands.
The existing CI diagnostic executes actual native comparisons and retains its
report. This adds no area/embedding/G1 certificate, Mock/native product body,
mesh, source/schema or MCP operation; circular authoring remains unsupported.

In the local four-case run, mathematical volume interval widths were about
1.2e-18 m³ (coarse) and 4.5e-18 m³ (fine), and every measured native volume/COM
lay within its mathematical interval even before the comparison allowance.
More cells can widen rounding enclosures through accumulation; these widths do
not measure geometric approximation error or prove convergence. The variable
fine case's actual native COM was approximately (-1.00088037e-5,
-3.21070960e-13, 9.97712089e-3) m. These fixture observations are not universal
native accuracy guarantees.

## Curved parametric area bounds (P16-028)

[ADR-081](../adr/ADR-081-curved-reference-area-enclosures.md) adds
`CircularFilletBezierBand::curved_diagnostic_area_bounds(absolute_width_m2,
max_leaves)`. `CircularFilletAreaBounds` returns `curved_lower_m2`,
`curved_upper_m2` and the number of integration `leaves`. The scope is the
current mathematical band plus ruled skirts down to z=0, explicitly excluding
the two planar caps. It counts parametric area with multiplicity; it does not
establish global embedding, regularity or a physical solid.

Use the current poles and shared-band validation, ignoring mutable quality
metadata. Form outward degree-(5,5) Bernstein coefficients of S_u cross S_v.
The norm of their integrated vector gives a lower area bound; the mean of their
norm upper bounds gives an upper area bound by convexity. If the total interval
is too wide, deterministically refine the largest-width leaf (insertion order
breaks ties) with outward half-parameter de Casteljau restrictions and quarter
area weights. These are integration partitions, not geometry/mesh edits or
extra native faces. Replace endpoint sums with outward arithmetic so removing
a parent does not retain its uncertainty. Reflection preserves unsigned area.
Degenerate mathematical patches may contribute zero; that is no native admission.

Require a finite positive absolute width in m² and a leaf budget from 8 through
262144. Initial leaves must cover every band/skirt patch; insufficient work or
uncertifiable precision refuses rather than returning a partial certificate.
The existing live diagnostic requests 1e-7 m² with at most 32768 leaves per case.
This is mathematical integration precision, not a geometric approximation or
manufacturing tolerance.

The native report measures every actual sewn curved face's area in m², their
curved sum, both planar cap areas and total native area. Compare the curved sum
to the independent enclosure with a separately reported 1e-12 m² numerical
allowance, and check face/curved/cap/total inventories. Caps are native measured
values, outside this independent area certificate. Native SI/mm conversion,
sewing and integration error are not certified by the mathematical enclosure.
The runner allows up to 300 seconds for the larger Rust fixture calculation;
construction and comparison remain mandatory and bounded.

Analytic prism side-area, XY translation, SI scaling, reflection, current-pole,
width/work/precision/refinement and independent quadrature regressions accompany
the actual OCCT comparison. Existing signed mass/moment controls still apply.
This adds no MCP/source/Mock body, schema, editable NURBS or feature admission.
Unchanged support, G1, global embedding and whole-solid fidelity remain gates.

The local four-case probe measured approximately 1273.67208 / 1273.67252 mm²
constant coarse/fine curved areas and 1288.47848 / 1288.47895 mm² variable
coarse/fine curved areas. Every native value lay inside its mathematical area
interval before the numerical allowance. Interval widths were below 0.1 mm²;
leaf counts were 1949, 4410, 2780 and 4437, respectively. These fixture
observations neither certify all native error nor prove geometric fidelity.

## Rational support contact curves (P16-029)

[ADR-082](../adr/ADR-082-rational-support-contact-curves.md) adds
`CircularFilletReference::rational_contact_curve(kind, chart)`, where
`CircularFilletContactKind::{Side, End}` selects the support and chart is 0..3.
`CircularFilletContactCurve` has private immutable data with read-only
`kind()`, `chart()`, `degree()`, `control_points_m()`, `weights()` and
`position_error_upper_m()` accessors. It is transient mathematical data,
not an editable NURBS operation, source tag, GeometryKernel body or Mock record.

For t in [0,1], D=1+t² and the unrotated circle has C=1-t², S=2t. Rotate C/S
by the chart's quarter turn, preserving the authored +X radius maximum.
With m=(r_min+r_max)/2 and a=(r_max-r_min)/2, side homogeneous coordinates are
(R*C,R*S,(H-m)*D-a*C,D). Degree-two weights [1,1,2] and exact stored side
x/y poles [(R,0),(R,R),(0,R)], rotated by chart, give the encoded mathematical
identity x²+y²=R² regardless of rounded z poles. The identity concerns the
encoded curve; native coordinate conversion/evaluation error remains separate.

End homogeneous coordinates are
((R-m)*C*D-a*C²,(R-m)*S*D-a*C*S,H*D²,D²). Degree-four ideal weights
[1,1,4/3,2,4] are rounded positively, and every Cartesian z pole is exactly
the same stored H. The encoded curve therefore lies in its encoded plane;
its x/y contact law remains a bounded approximation. Shared axis/radius endpoint
tables preserve identical endpoint representations, including the full-period
wrap. These equality checks concern stored representation, not proximity.

Outward homogeneous Bernstein arithmetic bounds every ideal numerator N and
denominator W coefficient. Actual stored P_i*w_i,w_i and positive denominator
lower bounds wa,wi give component error no larger than
max|P_i*w_i-N_i|/wa + max|N_i|*max|w_i-W_i|/(wa*wi), then an outward Euclidean
norm. This includes forced support coordinates, shared endpoint rounding and
actual rounded weights; no sampled fitting, native output or ideal substituted
mass supplies the certificate. It is a whole-curve position bound at corresponding
chart parameters, not a derivative/G1 certificate or native error guarantee.
Invalid charts/numerical enclosures refuse; work is fixed degree and bounded.

```sh
cargo run -p opencad-geometry --example periodic_fillet_contacts
python tools/run_rational_contact_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/rational-contact-diagnostic.json
```

The live pinned SDK builds 16 actual weighted B-spline edges with clamped
knots and four valid closed four-edge/four-vertex contact wires. Measure current
edge controls, weights, degrees, positions and D1 against independent rational
evaluation, with separate 1e-14 m point and 1e-13 m-per-local-parameter derivative
allowances. Check the actual physical azimuth contact law, side cylinder/end
plane relations, endpoint/wire closure and native rigid-turn length invariance.
A native quarter-turn decoy preserves the constant law's contact relation while
failing the variable law's phase. That failure is witnessed at measured points;
it is not a whole-curve upper bound on the decoy's error. Native length is a
numerical measurement, not an independently certified length bound.

The initial local fixture's largest mathematical position bound was approximately
8.33e-16 m. Actual point/D1 discrepancies were approximately 7.16e-18 m and
1.10e-17 m per local parameter. The rotated variable-law witness erred by about
0.705 mm; the constant-law residual was below 5e-18 m. These observations do
not certify native conversion/integration error or manufacturing accuracy.
Synthetic report tests check rejection logic only; CI executes actual construction
and retains the report. Report flags remain `candidate_admitted: false` and
`product_body_created: false`, with edge/wire counts distinguished from bodies.

These curves are separate from the existing polynomial diagnostic band. Do not
replace its boundary metadata/edges with these curves without rebuilding the
surfaces and re-establishing their certificates. Native support trimming/pcurves,
compatible surface/G1 construction, global embedding, physical Mock/runtime/mesh
and guarded source/cache/history/reader/MCP acceptance remain further gates.
No .ocad schema, migration, golden or MCP operation is added.

## Native analytic support trimming (P16-030)

[ADR-083](../adr/ADR-083-native-contact-support-trimming.md) uses the rational
contacts to bound actual retained native cylinder quadrants and top-plane
faces. These are separate diagnostic faces, not a shell, blend or product body.
The original cylinder radius/frame and plane height remain analytic; no
off-cylinder ruled skirt substitutes for the stock surface.

```sh
python tools/run_trimmed_support_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/trimmed-support-diagnostic.json
python tools/test_trimmed_support_probe.py
```

The fixed fixture creates eight cylindrical quadrant faces and two planar
faces, each with one valid closed four-edge/four-vertex wire. No shell or solid
is constructed. Each cylindrical boundary consists of its rational side contact,
a bottom circular arc and two axial generators. The end contact encloses the
retained top-plane region containing the axis.

OCCT `GeomProjLib::Curve2d` projects the existing 3D edges onto the original
support, using explicit quadrant UV bounds and 1e-7 mm requested tolerance.
`BRepLib::SameParameter` is forced after attaching pcurves. Actual projection,
edge and vertex tolerances must remain at most 1e-6 mm (1e-9 m); current flags,
ranges, contacts and pcurve/3D correspondence are inspected. Wire builders can
copy edges while merging vertices: matching actual curve handles plus current
representation checks establishes contact geometry, not preserved edge identity
or semantic ownership. The non-rational azimuth mapping `2*atan(t)` means the
cylindrical pcurve is approximate, even though the mathematical encoded 3D side
curve lies on its cylinder.

Every boundary has 17 current native witnesses, including endpoints, with a
separate 1e-9 m pcurve/3D allowance. Report sample `uv` has units `[rad,m]` on
cylinders and `[m,m]` on planes; all 3D coordinates are metres and normals are
dimensionless. Current contact poles use a 1e-14 m conversion allowance and
weight ratios a 1e-14 allowance. These tests do not certify whole-pcurve error.

For `r=m+a*cos(theta)`, independent retained area expressions are
`R*((H-m)*pi/2-a*(sin(u1)-sin(u0)))` for a quadrant and
`pi*((R-m)^2+a^2/2)` for the end plane. Actual native surface integration must
agree within a separate numerical 1e-10 m² allowance. Fixed physical points
inside/outside the retained region are classified on current faces; an actual
quarter-turn transform preserves the constant control while the variable law
fails the authored-phase region test.

The initial local fixture measured a maximum pcurve/3D witness gap of about
1.04e-10 m and area residual of about 1.59e-14 m². They are numerical observations,
not a global projection bound or manufacturing accuracy claim. Seven synthetic
report tests prove parser refusals only; live CI performs the native construction
and retains its report. Flags stay `candidate_admitted: false` and
`product_body_created: false`. Existing polynomial band certificates do not
transfer to new contact boundaries. Compatible blend construction, support/seam
G1, embedding, physical Mock/runtime/mesh and guarded source/cache/history/reader
acceptance remain required. No new MCP operation or editable NURBS is exposed.

## Rational characteristic-circle surfaces (P16-031)

[ADR-084](../adr/ADR-084-rational-rolling-sphere-surfaces.md) supplies a direct
rational representation of the ideal rolling-sphere envelope. This supersedes
polynomial interpolation only in a new transient reference/diagnostic path;
it adds no authorable circular fillet or production GeometryKernel/Mock body.

`CircularFilletReference::rational_patch(chart)` returns immutable
`CircularFilletRationalPatch` with degree `(8,2)`, a 9-by-3 SI control net,
positive weights and an outward whole-patch positional rounding bound. For
`k=r'/(R-r)`, its transverse characteristic normal is

```text
Q = 1+(1+2*k²)*v²
nr = (1-v²)/Q
nt = 2*k*v*(1-v)/Q
nz = (2*v+2*k²*v²)/Q.
```

These satisfy the sphere/envelope equations and cover the exposed arc. Quarter
parameter `u` gives `theta=chart*pi/2+2*atan(u)`. Transverse `v` is stereographic;
recover the old reference angle as `alpha=atan2(nz,nr)`, **not** `v*pi/2`.
The ideal charts share smooth support/quarter boundaries. Rounded controls have
a newly computed positional certificate from actual poles/weights and outward
homogeneous coefficients; previous polynomial-band certificates are not reused.
P16-032 below separately certifies whole-chart rounded derivatives and normals;
this construction alone does not imply those bounds or native fidelity.

`cylindrical_contact_pcurve(chart)` returns immutable
`CircularFilletCylindricalPcurve`: 256 cubic Hermite spans, 769 shared controls
in `[angle radians,height metres]`, and angular/height/lifted positional bounds.
The bounds include the analytic fourth-derivative interpolation remainder and
outward rounding of every actual control. atan uses bounded half-angle/series
arithmetic. This is an explicitly admitted diagnostic pcurve approximation,
not a new mesh sampling or editable NURBS policy. Invalid charts/enclosures
refuse; both constructions have fixed bounded work.

```sh
cargo run -p opencad-geometry --example periodic_fillet_rational
python tools/run_rational_reference_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/rational-reference-diagnostic.json
python tools/test_rational_reference_probe.py
```

The native diagnostic builds two actual valid connected closed positive-volume
solids, one per law. Each has four rational blend charts, four unchanged analytic
cylinder quadrants and two unchanged planar caps: 10 faces, 20 two-owner edges,
12 vertices, Euler characteristic 2. Blend faces explicitly use the supplied
degree-2/4 contact edges with linear blend pcurves and native isoparametric seams.
Current sewn contact/surface controls and bounded cylinder pcurve controls/knots
are checked. Native SameParameter requests 1e-9 mm; sewing and edge/vertex
tolerance ceilings remain 1e-6 mm (1e-9 m).

Measure current surface D1/normals and both owner faces of every shared edge,
including non-knot pcurve witnesses. The separate native numerical gates are
1e-14 m control/point, 1e-12 m per local parameter D1, 1e-9 dimensionless normal,
1e-9 m join and 1e-12 m³ volume allowance. Relative weight ratios use 1e-13.
Edge UV units are dimensionless on blend charts, rad/m on cylinders and m/m on
planes. Bottom edges retain the stock's right angle; blend/support/quarter
tangent-plane witnesses must agree. Independently bounded ADR-073 ideal volume,
the constant analytic formula and actual inside/outside solid classifiers check
the result. A real quarter-turned solid preserves mass and constant control,
while failing the variable law's authored phase.

The initial native projection recipe passed the position gate but failed the
normal gate. Tightening its request alone did not resolve the variable case;
the bounded analytic cylinder pcurve preserves the original gates. Initial local
whole-patch position bounds were at most 1.96e-13 m; supplied cylinder-pcurve
lift bounds at most 3.73e-13 m. Observed native join gap was about 5.26e-14 m,
paired normal gap about 5.14e-12, and constant/variable rotated-classifier mismatch
counts 0/4. These are numerical observations, not manufacturing accuracy or a
global native G1/embedding/conversion/sewing/integration certificate.

Eleven independent Rust geometry tests, three private control/refusal tests and
thirteen synthetic report-contract tests
protect the construction and refusals. Synthetic flags/samples are parser tests
only; CI executes actual native construction and retains its report. Flags remain
`candidate_admitted: false` and `product_body_created: false`; the report separately
counts two `native_diagnostic_solids_created`. Global native normal/support
fidelity, physical Mock/runtime/mesh and guarded ownership/cache/history/
reader/source acceptance remain before P16-008. No .ocad schema, migration,
golden, MCP operation/eval or editable NURBS feature is introduced.


## Rounded rational normal certificates (P16-032)

[ADR-085](../adr/ADR-085-rounded-rational-normal-certificates.md) adds
`CircularFilletRationalPatch::normal_bounds(maximum_normal_error, max_leaves)`.
The chart is privately bound to its construction reference. This read-only
method certifies the actual stored poles/weights on the entire closed unit
square, including all boundaries; it does not change their geometry.

The requested dimensionless unit-normal chord error must be finite and strictly
between zero and one; work is 1..=16384 cover leaves, with hard depth 28.
`CircularFilletRationalNormalBounds` contains positive lower/upper Jacobian
area bounds in m², positive ideal-normal alignment lower bound, dimensionless
normal error upper bound, u/v derivative error upper bounds in metres per
original dimensionless parameter, and the complete cover's leaf count.

Outward homogeneous Bernstein differences include stored Cartesian pole times
weight. Differentiate before restriction, always in original u/v coordinates.
Positive denominator hulls and quotient identities enclose derivative errors;
an independently proven ideal Jacobian lower bound makes rounded regularity,
outward orientation and unit-normal error quantitative. Deterministic left-first
alternating u/v half-splits refine a complete cover. Invalid precision/work,
nonfinite/unresolved intervals or exhausted work refuse without partial output.

The certificate also bounds radial/axial support-normal differences at v=0/1.
Adjacent quarter seam normal gaps are bounded by the sum of both charts' errors;
add the cylindrical pcurve's angular bound when comparing that supplied support
parameterization. These are mathematical approximate normal agreements, not
exact G1 or proof of contact coincidence/global embedding.

The existing example requests 1e-9 normal error with 4096 leaves per chart.
The live native diagnostic compares actual current D1 cross-product Jacobians,
normal alignment, constant-chart analytic derivatives and paired boundary normals
to these bounds. Native numerical allowances remain separate: 1e-16 m² Jacobian,
1e-12 m per parameter D1 and 1e-9 normal. Sample checks are regression evidence;
the global mathematical proof is the complete outward interval cover. Native
conversion/sewing and evaluation errors still lack global certificates. Report
`rounded_reference_certificates.global_native_certificate` remains false.
No production Mock/kernel feature, source/schema, MCP operation/eval, mesh policy
or editable NURBS admission is added; P16-008 remains planned.


## Current native surface definition certificates (P16-033)

[ADR-086](../adr/ADR-086-native-rational-surface-certificates.md) extends the
proof to captured post-sewing native surface definitions. It does not transfer
the supplied-control certificate from a sampled control/point comparison.

`CircularFilletRationalSnapshot::from_millimeter_controls(&poles_mm, &weights)`
reads fixed 9-by-3 finite Cartesian native controls and positive weights into
immutable private SI homogeneous intervals. Raw native q_mm is converted using
outward `q_mm/1000`, not a rounded SI export treated as exact. Overflow refuses.
This is a descriptive read-only snapshot; it creates no surface or body and
supplies no editing, persistence or source authority.

`CircularFilletReference::certify_rational_snapshot(chart, &snapshot,
maximum_position_error: Length, maximum_normal_error, max_leaves)` certifies the
complete closed original unit square against that validated reference and
quarter. Position precision must be finite positive metres. Normal precision
and work/depth gates match P16-032. Common positive weight gauge is normalized
by an outward ideal/actual anchor ratio before homogeneous differences; the
normalization uncertainty remains in every bound. `CircularFilletRationalSnapshotBounds`
returns `position_error_upper_m` and `normal: CircularFilletRationalNormalBounds`.
One complete deterministic cover must satisfy both position and normal requests;
invalid/unresolved/exhausted requests refuse without partial output or mutation.

The actual native probe now exports ORIGINAL millimetre poles and current
weights after sewing, along with identity Location, degree, unit bounds,
nonperiodicity and forward orientation. Existing SI samples remain numerical
witnesses. Seventeen decimal significant digits and serde_json float_roundtrip
preserve binary64 inputs. The bounded-stdin Rust example
`periodic_fillet_native_snapshot` consumes eight requests, echoes the exact
request data, and emits only after all eight complete proofs succeed. Input is
limited to 256 KiB. It accepts no OCCT handles, source or body mutation.

Run the existing native probe command above; it constructs real native solids,
feeds their current captures to the Rust proof and grades both together. Each
request uses 1e-12 m position, 1e-9 normal and 4096 leaves. The report binds each
certificate echo to that current capture/reference/request and retains the
full proof result under `native.surface_definition_certificates`. Top-level
`native_surface_definition_certificates` reports extrema and total cover size.
Synthetic reports prove parser refusals only; CI runs the live capture/proof.

The certificate covers the mathematical native rational surface DEFINITION,
including native-coordinate conversion, across its entire canonical domain.
It bounds approximate normal agreement with ideal support/quarter boundaries
at corresponding parameters. Native floating evaluation, actual trim/pcurve
parameterization, contact-edge coincidence, native integration, exact G1,
global embedding and whole-BREP fidelity remain unproven. Both
`native_floating_evaluation_certified` and `whole_brep_certified` remain false.
P16-032's separate supplied-control summary retains its narrower scope. No
production Mock/kernel feature, mesh, schema/migration/golden, MCP operation/eval
or editable NURBS admission is added; P16-008 remains planned.

The live snapshot example also rejects inputs over 256 KiB, an invalid quarter
and an unattainable precision on the last of eight requests. All three must
return failure with no partial certificate stdout, after the same real native
capture; synthetic parser tests do not substitute for these executions.

Raw native pole JSON uses showpoint so signed zero remains a floating value
through Python transport. Echo comparison rejects booleans as numeric values
and checks binary64 identity including zero sign; geometry comparisons retain
their separate explicit tolerances.
