# ADR-075: Bounded periodic reference patches

Status: Accepted for MCAD-P16-022's reference patch bounds only.
Date: 2026-10-14.

## Decision and scope

ADR-074 rejects the tested built-in native fillet recipes. Preserve ADR-070's
ideal rolling-sphere semantics and investigate direct controlled approximation
from that reference instead of silently widening native tolerance or redefining
radius. First supply position, normal and derivative enclosures on explicit
parameter rectangles and a proven linear-interpolation error bound. This is
reference arithmetic, not an accepted faceted fillet, closed mesh, native spline
construction, editable NURBS, source feature or P16-008 completion. No score.
Future native fitting and any tessellation policy still require admission.

## Coordinates and enclosures

Cover theta with four rational quarter-circle charts, chart j in 0..3, u in
[0,1]. Rotate ((1-u²)/(1+u²), 2u/(1+u²)) by j*pi/2. For alpha use the same
rational coordinates with v in [0,1]. Thus theta=j*pi/2+2*atan(u) and
alpha=2*atan(v); u,v are dimensionless chart coordinates, not radians.
Admit finite nonempty rectangles within [0,1]². Reuse ADR-072's bounded lengths.

Evaluate the exact reference formula with private second-order interval chain
rules. Return outward position/normal component enclosures, first derivatives
with respect to u/v and second-derivative Euclidean norm upper bounds in metres.
Extract ADR-073's existing private binary64 interval primitives into a shared
private module; do not create a new public interval package or alter the volume
algorithm. Retain its nearest-rounded IEEE-754 arithmetic/correctly-rounded sqrt
assumptions, adjacent-value rounding and Rust 1.78 compatibility. No trigonometric
libm assumption, finite differences, dependency or hidden I/O enters production
reference evaluation. Refuse invalid/nonfinite intermediate bounds.

A second-order jet stores value f, gradient f_i and Hessian f_ij. Product and
unary composition use exact chain rules, with every scalar operation enclosed:

```
(f*g)_ij = f_ij*g + f_i*g_j + f_j*g_i + f*g_ij
h(f)_ij = h''(f)*f_i*f_j + h'(f)*f_ij
h(x)=1/x: h'=-1/x², h''=2/x³
h(x)=sqrt(x): h'=1/(2*sqrt(x)), h''=-1/(4*x*sqrt(x)).
```

Require strictly positive sqrt/divisor intervals for these admitted expressions.
A dedicated square operation retains its exact nonnegative value range while
using product derivatives. Wide rectangles may refuse if interval dependency
cannot establish a valid domain; do not infer acceptance from point samples.

## Corners, seam and rounding

For corners ordered (u_min,v_min), (u_max,v_min), (u_max,v_max), (u_min,v_max),
compute degenerate-coordinate position enclosures and select a finite binary64
representative inside each component interval. Bound its Euclidean displacement
from the exact corner by outward interval subtraction/norm. Canonicalize only
the exact u=1 representation to the next chart's u=0 for corner evaluation,
including chart3 to chart0. This is an identity of the rational quarter-circle
coordinates; it does not clamp nearby inputs. Shared dyadic vertices in a chart,
and the declared chart seams, therefore receive deterministic identical corner
coordinates. Full patch derivatives remain in the caller's original chart:
its endpoint derivative scale differs from that of the next chart.

## Linear interpolation proof

Let F map the rectangle to the ideal surface. Let Muu,Muv,Mvv be upper bounds
on ||F_uu||, ||F_uv||, ||F_vv|| throughout the entire rectangle, and hu,hv its
parameter widths. For any triangle with vertices among its four corners,
Taylor expansion at a parameter point p, with barycentric weights lambda_i,
gives cancellation of the first-order terms because sum(lambda_i*(p_i-p))=0.
Each coordinate variance is bounded by h²/4. Cauchy-Schwarz bounds
sum(lambda_i*|delta_u*delta_v|) by hu*hv/4. Consequently

```
||F(p)-sum(lambda_i*F(p_i))||
 <= (Muu*hu² + 2*Muv*hu*hv + Mvv*hv²)/8.
```

The maximum rounded-corner displacement adds to this bound, since barycentric
weights are nonnegative and sum to one. Compute both contributions and the final
sum with outward arithmetic. Triangles (0,1,2) and (0,2,3) cover the rectangle;
the bound describes their parameter-corresponding positional deviation in metres.
It also bounds Hausdorff distance between that reference patch and those two
triangles under this correspondence. It is not a bound on face normals, mass,
mesh self-intersection or any different fitted surface.

No fixed grid, adaptive subdivision, user quality default, mesh budget or
manufacturing tolerance is chosen by this API. A caller can subdivide and inspect
these bounds; rounded-corner uncertainty can limit attainable precision. A
small positional bound alone cannot admit a closed solid or a tangent native
fillet. Closure/orientation/embedding, supports/phase, normal/volume errors and
native fitting all remain separate proof obligations.

## Validation and deliverables

Check exact constant-radius contact derivatives/Hessians independently, compare
chart derivatives to ADR-072's angular derivatives and numerical Hessians, and
exercise interior interpolation against returned error bounds. Check nested
rectangles/refinement, normal and point component bounds, shared dyadic corners,
all chart seams including closure, multiple scales and malformed inputs. Numerical
oracles use explicit tolerances; the enclosure proof comes from the interval
chain rules and Taylor inequality, not finite sampling.

Preserve existing interval/volume regressions. Update reference API/architecture,
example, authoring guide, skill and plans. No serialized source/schema/migration,
golden, GeometryKernel operation or executable MCP eval applies. Four mandatory
local checks and all applicable exact-head CI jobs gate merge. Real Mock/native
construction and complete source/reader/cache/history/eval remain P16-008 work.
