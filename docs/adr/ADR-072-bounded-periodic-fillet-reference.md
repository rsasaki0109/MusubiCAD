# ADR-072: A regular, exposed periodic fillet reference

Status: Accepted for MCAD-P16-019's transient analytic reference only.
Date: 2026-10-14.

## Decision and boundaries

Implement ADR-070's ideal single-period rolling-sphere envelope as a pure
`CircularFilletReference` in `modules/geometry`. Supply finite point, unit normal,
analytic parameter derivatives, contacts and a global exposure lower bound.
This is reference arithmetic, not a Mock or OCCT body, native approximation
admission, mesh/sampling method, source tag or MCP command. P16-008 remains planned.
ADR-071's current physical rim query does not prove the new solid by itself.

Inputs are explicit Length values for cylinder radius R, height H and minimum/
maximum fillet radii. Admit finite dimensions above 1nm and at most 1000m with
`0 < r_min <= r_max <= R/4` and `H >= 4*r_max`. These are reference-domain bounds,
not a promise of representable kernel geometry throughout that numerical range.
The reference is local: cylinder axis +Z, base z=0, centre on that axis and
radius maximum at +X. Authored source placement/phase is a separate future gate.
Parameters are explicit Angles: theta in [0,2*pi], alpha in [0,pi/2]. Reject
nonfinite/out-of-range values; only the declared theta=2*pi endpoint maps to zero
for a deterministic seam. Do not clamp or infer other angles or radii.

Use ADR-070's r=m+a*cos(theta), centre C and outward characteristic-arc formula.
Differentiate those expressions analytically with a private two-parameter
first-order dual scalar. Numerical differences are test oracles, not production
derivatives. No new dependency, finite-difference step or configurable sampling
is introduced. Normals are sphere normals; parameter tangents must be independent,
orthogonal to the normal and have outward cross-product orientation.

## Global exposure and absence of self-intersection

This proof concerns the exact ideal reference, not an OCCT-generated surface.
For X=X(theta,alpha), define the sphere residual at another station s:

```text
F(s) = |X-C(s)|^2 - r(s)^2.
```

F(theta)=F'(theta)=0 by the envelope equation. Since r(s)=m+a*cos(s), F is a
degree-two trigonometric polynomial. In delta=s-theta it factors as

```text
F(theta+delta) = (1-cos(delta))*(A+B*cos(delta)+D*sin(delta)).
```

To check the factor independently, its global second-harmonic coefficients are
`c2=a*a/2+a*X.x`, `s2=a*X.y`, and its first-harmonic coefficients are
`c1=-2*a*(R+H-X.z-m)-2*(R-m)*X.x`, `s1=-2*(R-m)*X.y`.
Rotate harmonic j by j*theta to obtain c_j',s_j'. Then
`B=-2*c2'`, `D=-2*s2'`, `A=-2*c2'-c1'`.

Write rho=|X.xy| and X_r=X dot e_r(theta). A point on an admitted sphere has
rho<=R, X_r>=R-r_max, and 0<=H-X.z<=r_max. Consequently

```text
sqrt(B^2+D^2) <= 2*a*(R+a/2)
A-sqrt(B^2+D^2)
  >= 2*(R-m)*(R-r_max) - 2*a*(R+r_max-m) - 4*a*(R+a/2)
  >= 2*R^2 - 7*R*r_max + r_max^2/2
  >= 9*R^2/32 > 0,                 since r_max/R <= 1/4.
```

The second inequality uses m<=r_max and a<=r_max/2. The quadratic decreases on
0<=r_max/R<=1/4. Thus F(s)>0 for every other station modulo the seam: no reference
surface point is covered by another moving sphere. This is an all-angle bound,
not a conclusion from a finite sampling grid. Two distinct stations cannot
produce the same surface point, since it would lie on both spheres. At a fixed
station the positive radial/axial normal components have ratio cot(alpha),
so the characteristic arc is injective too. Top and side contacts inherit this
uniqueness. Their interiors lie on the end plane/cylinder respectively; interior
blend points lie strictly below/inside them. H>=4*r_max separates the bottom.

## Regularity and orientation

The positive factor also gives F_ss(theta)=A+B>0. Differentiating F_s=0 along
theta and alpha gives

```text
grad_X(F_s) dot X_theta = -F_ss != 0
grad_X(F_s) dot X_alpha = 0.
```

The characteristic arc derivative X_alpha is nonzero: its radial/axial normal
pair is t*(cos(alpha),sin(alpha)), with t>0, whose angular derivative has nonzero
length. Thus X_theta and X_alpha are independent and the surface is regular.
The unit normal is orthogonal to both by differentiating the sphere equation.
Decrease a continuously to zero while retaining m; the admitted inequalities
remain true, so the cross-product orientation cannot flip or vanish. In the
constant limit X_theta cross X_alpha points outward, establishing the same
orientation for the whole admitted ideal reference.

The implementation returns the conservative dimensionless lower bound
`2-7*q+q*q/2` (q=r_max/R) times R^2 in square metres. This is a real-arithmetic
geometric certificate, not an outward-rounded floating-point interval. Floating
point evaluation and independent identity tests have explicit scale-aware
tolerances; do not claim certified numerical quadrature or mesh error from it.

## Independent measurements and refusal

Expose the constant-limit body volume only when the two authored radii are the
same parameter value. Do not snap a small positive amplitude to constant. Use
ADR-070's exact removed-volume formula; a nonconstant request refuses because
its independent bounded surface integral is not implemented by this task.
This method calculates an ideal reference, never a backend mass property.

Tests independently check sphere distance/envelope equations, both contacts,
seam value/derivatives, outward normals and finite-difference tangents, factor
identity against direct sphere distances, and the conservative all-angle margin
over representative/boundary dimensions. Finite samples test implementation of
the proof; they do not replace that proof. Independently integrate meridional
corner area/first moment and the constant reference surface to verify volume.
Refuse malformed lengths/angles, unadmitted radius/height ratios and nonconstant
volume requests, including a small explicit positive amplitude.

Update API/architecture, example, authoring guide, skill and plans. No serialized
data or source feature changes, so no schema/migration/golden changes or runtime
MCP eval are applicable. Four mandatory local checks and exact-head applicable
CI/jobs still gate merge. This prerequisite earns no score.

## Remaining implementation

Native experiments from ADR-070 and later investigation show deviations in
contacts and inferred sphere support even when the stored radius law closely
matches the cosine. This proof does not bound those deviations or establish
native parameter correspondence, periodic smoothness, actual fillet solids,
closed bounded meshes, nonconstant mass quadrature or source/cache admission.
Accept the actual construction and any quantified approximation/sampling in a
further ADR before implementing kernel bodies. Never insert ideal reference mass
into native output, silently change authored phase or claim constant-only curved
geometry completes P16-008.


P16-020 / [ADR-073](ADR-073-periodic-reference-volume-bounds.md) subsequently
accepts a separate bounded nonconstant volume API. The constant-only method's
refusal semantics and this ADR's no-solid/native-admission boundary are unchanged.
