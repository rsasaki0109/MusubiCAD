# ADR-077: Bezier normal and orientation bounds

Status: Accepted for MCAD-P16-024's transient reference-surface certificates only.
Date: 2026-10-15.

## Decision and scope

Extend ADR-076's positional approximation with an independently recomputed
whole-patch regularity/orientation and unit-normal error certificate. Operate
on the actual supplied binary64 poles; do not trust stored positional bounds.
This fixes a missing prerequisite for investigating native construction, not
G1 continuity, support trimming, global embedding, closed bodies or P16-008.
No feature, schema, MCP operation, sampling policy or maturity credit is added.

## Proof and numerical rules

For a bicubic net P, normalized local parameter derivatives have control nets

```
U_ij = 3*(P_(i+1,j)-P_(i,j)),  i=0..2, j=0..3
V_ij = 3*(P_(i,j+1)-P_(i,j)),  i=0..3, j=0..2.
```

Bernstein weights are nonnegative and sum to one. Each derivative lies in its
componentwise control hull. Enclose the differences, multiplication by three,
cross product W=U cross V, sum of squares and sqrt using ADR-073's outward
arithmetic. W has units m² and |W| is area per dimensionless local parameter
area. Require a finite strictly positive lower bound on |W|. This proves local
immersion of the mathematical surface over its entire closed parameter square.
It proves neither injectivity nor global embedding.

Divide W's component intervals by the positive magnitude interval to enclose
the oriented unit normal N. Compare to ADR-075's ideal normal component bounds
on the explicitly supplied chart rectangle. Require a finite strictly positive
lower bound on N dot n_ideal. Return both normal component bounds, the area
lower/upper bounds, alignment lower bound and upper Euclidean bound on
|N-n_ideal| (dimensionless unit-vector chord distance, not radians).
Round every scalar contribution outward. The sum of squares is nonnegative
even where lower-end outward addition reaches below zero at the subnormal floor.

This is a conservative fixed-size certificate: wide derivative hulls, any
nonfinite pole/arithmetic, degenerate/reversed orientation or underflow that
prevents a positive lower bound refuse. Refusal means not certified by this
method; do not silently regularize poles, sample around failures, infer a
certificate from corners or reuse a stale position certificate. No default
normal tolerance or subdivision policy is selected. A successful certificate
applies only to the current poles, supplied ideal reference and rectangle;
editing any of them requires recomputation. It does not prove position fidelity.

## Seams and native evidence

When two admitted rectangles share the same ideal boundary with matching
parameter correspondence, triangle inequality bounds their unit-normal gap by
the sum of their returned reference-normal error bounds, with outward addition.
This bounds possible normal mismatch; it is not proof of zero gap or G1.
Keep ADR-076's identical shared edge poles as a separate positional property.

Extend the live pinned OCCT diagnostic: actual D1 cross products are normalized,
compared to independent Bernstein derivatives and the angular ideal normals,
checked for positive orientation/area and against the returned bounds. Measure
actual full-cycle seam normal gaps at the same parameter witnesses. Use an
explicit dimensionless numerical allowance for native normal evaluation,
separate from the existing metre-valued point/derivative allowance. Finite
samples validate implementations; derivative hulls establish whole-patch bounds
for the mathematical surfaces of the returned poles. No all-parameter native
floating-point error certificate is claimed.

## Validation and remaining gates

Check independent planar cross products and area, angular normals for constant
and nonconstant cases across all charts/contact/interior/seam rectangles, SI
quadratic area scaling, malformed/nonfinite nets, collapsed/reversed nets and
interior corruption with unchanged corners. Check that stale positional fields
do not affect recomputation. Negative native report controls reject false
normals, areas, orientation and certificate metadata. Update API, architecture,
example, guide, skill and plans; all mandatory checks and exact-head CI gate merge.

Actual support trimming, physical Mock/native solids, closed meshes, normal
continuity at supports/internal seams, global embedding and native mass bounds
still need further construction/admission decisions. No editable NURBS or
serialized feature is introduced. P16-008 remains planned.
