# ADR-080: Independent closed reference boundary integrals

Status: Accepted for MCAD-P16-027's mathematical diagnostic only.
Date: 2026-10-15.

## Decision

Independently enclose signed volume and first-moment boundary integrals of
ADR-079's mathematical polynomial approximation using outward binary64
Bernstein arithmetic. Do not substitute ideal fillet mass for this approximation,
or use OCCT's own integration as its independent oracle. This is a transient
reference API and diagnostic, not a GeometryKernel/Mock body or source feature.
No feature or maturity credit is earned; P16-008 remains planned.

## Integral and numerical contract

For an oriented bicubic band S=(x,y,z), let J=x_u*y_v-x_v*y_u.
Divergence gives band contributions integral(z*J), integral(x*z*J),
integral(y*z*J), integral(z*z*J/2) to signed volume and first moments.
The ruled skirts have x/y independent of v, so J=0. Bottom caps at z=0
contribute zero. A top cap at its actual common pole height h contributes
h*A, h*Ax, h*Ay, h*h*A/2, respectively. Green's theorem on the ordered
cubic top curves gives A=integral(x*dy), Ax=integral(x*x*dy/2),
Ay=-integral(y*y*dx/2). These formulas concern the closed mathematical
polynomial boundary, including its approximate side, not the ideal cylinder.

Use Bernstein derivative/product identities and integral(B_i^n)=1/(n+1)
with outward primitive operations and interval binomial ratios. No sampled
quadrature, fitted mass, midpoint output or tolerance inflation replaces these
bounds. Return intervals in m³ and m⁴. Check finite poles, a bounded complete
ordered dyadic cover, identical shared boundary representations and one common
planar top height; malformed/nonfinite/over-budget inputs refuse. Equality here
checks exact shared binary representations, not geometric closeness. Do not
trust editable error metadata as a mass certificate. Derived COM intervals require
positive bounded volume; signed boundary integrals alone do not prove embedding,
orientation, validity or a physical solid.

The actual native diagnostic independently checks closed valid positive solids
and emits actual GProp centre of mass and first moments. Compare those to the
mathematical intervals with separately named SI numerical allowances. SI/mm
conversion, sewing tolerances, cap-plane rounding and OCCT integration are not
covered by the mathematical polynomial intervals. Passing native comparisons
therefore does not certify all native mass error or manufacturing accuracy.

## Validation and remaining gates

Check a degree-elevated planar prism with independently known volume/first
moments, translations/scaling, malformed boundaries/nonfinite/budget refusals,
and independently implemented high-order numerical surface quadrature of real
constant/nonconstant bands. Run actual native comparisons in CI and negative
report controls for corrupted COM/moments/enclosures. Update example/API,
architecture, authoring guide/skill and plans. Schema/migration and MCP eval do
not apply: no document operation is added. All mandatory local and CI gates apply.

Unchanged cylindrical support/G1 continuity, global embedding, independent
whole-solid area/fidelity bounds, physical Mock/runtime/meshes and guarded
source/history/reader acceptance remain unresolved. No editable NURBS or
circular variable-fillet creation is admitted.
