# ADR-076: Bounded Bezier reference patches

Status: Accepted for MCAD-P16-023's transient surface approximation only.
Date: 2026-10-14.

## Decision

Construct explicit bicubic Bezier control nets from ADR-075's ideal reference.
Measure these actual surfaces in the pinned OCCT SDK, instead of relying on a
native fillet's stored radius law. This admits a separately quantified positional
approximation of a reference patch, not an authored fillet or native solid.
P16-008 remains planned and earns no credit from this prerequisite.

## Construction and precision

Use the four rational charts and nonempty finite rectangles from ADR-075. Return
4x4 control points in metres, indexed first by u and then by v. Corner points
are exactly ADR-075's rounded shared representatives. At each corner, obtain
outward enclosures of F_u, F_v and F_uv from its second-order jet and construct
the adjacent Hermite control points with signed hu/3, hv/3 and hu*hv/9 terms.
Select a finite binary64 representative inside each resulting component interval.
These are the actual control coordinates, not exact Hermite interpolation claims.

For a declared u=1 corner evaluate the next chart at u=0, including wraparound.
Transform u and mixed derivatives by 1/2 for the original chart; v derivatives
are unchanged. Signed zero canonicalizes as in ADR-075. Shared edge controls
use only the edge's endpoints and along-edge derivatives, giving identical bits
when the along-edge parameter interval agrees. No subdivision policy, global
patch layout, default tolerance or work budget is selected. This is a fixed-size
operation; invalid domains or nonfinite arithmetic refuse.

## Positional bound

Let L be the bilinear interpolant of the four returned corners, and S the
bicubic surface of the returned control net. Bilinear corner weights are
nonnegative, sum to one and have mean parameter p. ADR-075's Taylor/variance
proof therefore also bounds ||F(p)-L(p)||, including rounded-corner error.
Degree elevation represents L by a bicubic net Q_ij=L(i/3,j/3). Compute these Q
with outward arithmetic on the exact returned binary64 corners. The nonnegative
Bernstein basis gives, over the entire rectangle,

```
||F-S|| <= ADR075_corner_interpolation_bound + max_ij ||P_ij-Q_ij||.
```

Enclose each subtraction, Euclidean norm and final sum outward. This bound is
valid for the actual returned control coordinates regardless of any rounding
in their construction; it does not assume derivative midpoint accuracy. Publish
the reference-to-bilinear and control-net deviation contributions separately.
This also bounds positional Hausdorff distance under the same parameter map.
Correctly rounded basic binary64 arithmetic/sqrt assumptions remain ADR-073's.

## Native evidence and boundaries

An isolated OCCT 8.0.0 diagnostic constructs Geom_BezierSurface from these exact
control coordinates using metre-valued coordinates (no millimetre conversion).
Check actual poles, degrees, evaluations and derivatives against an independent
Bernstein evaluator, then measure sample positions against the angular reference.
Use constant and nonconstant fixtures, all four charts, endpoint contacts and
interior points; include a displaced control-net negative control. Native sample
agreement tests implementation fidelity; the all-patch bound comes from the
interval proof and Bernstein convex hull, not finite sampling. State explicitly
the numerical comparison allowance for native evaluation.

No BREP, mesh, closed solid, mass estimate, support trimming, G1 continuity,
normal-error bound, editable NURBS, schema or MCP operation is introduced. Shared
edge positions alone do not establish a regular tangent-continuous embedded
surface. The positional certificate applies to the mathematical Bezier surface
of returned poles; native floating evaluation has its separately tested numerical
allowance. A later construction/admission ADR must handle those remaining gates
before actual Mock/OCCT bodies or source support.

## Deliverables and checks

Test independent Bernstein values and analytic derivative limits, interpolation
error/refinement/scales, shared edge control bits, malformed inputs and corrupted
native results. Add a runnable Rust fixture and a live native CI diagnostic,
update API/architecture/example/authoring guide/skill/plans, run all four mandatory
local checks and exact-head CI. No schema, migration or golden changes apply.
