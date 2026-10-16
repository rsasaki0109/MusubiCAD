# ADR-070: Closed circular variable-fillet acceptance

Status: Accepted for MCAD-P16-017's reference and acceptance contract only.
Physical construction, native radius-law admission and sampling are not accepted
by this decision. ADR-100 and ADR-101 subsequently admit a separately named,
quantified numerical faceted runtime/source approximation; exact smooth support
and general curved-contour acceptance remain outside that admission.
Date: 2026-10-14.

## Context and decision

ADR-038 supports a linear radius law on one straight edge. Its native bridge
uses 65 radius samples and explicitly rejects curves and tangent chains.
`MockGeometryKernel::variable_fillet_edge` changes a body ID; the existing
feature test deliberately disables physical-volume checks for that Mock path.
Neither establishes physical Mock support for a curved contour. Keep that
legacy behavior separate from the new admission.

Choose one new investigation target: the complete convex end rim of an original
authored circular cylinder, with a positive, smooth, single-period radius law.
Accept the following mathematical reference and failure/compatibility gates
before constructing a body. Do not enable the feature until both physical
kernels meet them. A constant circular fillet is a required limiting test,
not sufficient completion of the variable-contour feature.

## Authored contour and phase

Use an original exact-circle Sketch / Distance / NewBody extrusion, with no
intervening cut, draft, thread, fusion, pattern or import. In its checked proper
sketch frame the cylinder is radius R, height H, with z in [0,H]. Select exactly
its full end rim at z=H through a mandatory current semantic Edge ref owned by
that extrusion. Verify the actual complete circle's centre, radius and plane,
the full cylinder witness and unique current ownership; matching endpoints,
an edge ID, a role string, bounds or scalar mass alone is insufficient. Competing
consumers, stale optional annotations and ambiguous aliases refuse.

The positive authored sketch X direction is phase zero; sketch X cross Y is the
positive extrusion axis. Check finite unit orthogonal axes before normalization.
Do not infer phase or direction from a BREP seam, native curve parameter, edge
enumeration, world X or the lexicographic endpoint rule of ADR-038. Proper rigid
placement rotates the whole reference. Reversing the BREP edge orientation or
changing its seam must leave the authored result unchanged.

The reference law is

```text
m = (r_min + r_max)/2,  a = (r_max - r_min)/2
r(theta) = m + a*cos(theta),  0 <= theta <= 2*pi
r'(theta) = -a*sin(theta)
```

Require finite SI lengths with 0 < r_min <= r_max < min(R,H). These inequalities
are necessary, not a sufficient global nonintersection or precision admission.
Radius and derivative agree at the seam; radius is maximal at authored X and
minimal at its opposite direction. Nonconstant cases must exercise a > 0.
No open-chain endpoint law, arbitrary frequency/phase, arcs, tangent chains,
concave/hole rims, other end, multiple edges, face-wide law or editable NURBS
is admitted here. Further geometric or sampling decisions require an ADR.

## Ideal rolling-sphere reference

In the local cylindrical frame let e_r and e_theta be the radial/tangential unit
vectors. A sphere tangent to both the cylinder and its end plane has centre

```text
C(theta) = (R-r(theta))*e_r(theta) + (H-r(theta))*e_z
C'(theta) = -r'*e_r + (R-r)*e_theta - r'*e_z
```

An exposed envelope point X=C+r*n must satisfy both
`|n|=1` and `n dot C' = -r'`. The sign follows by differentiating
`|X-C(theta)|^2-r(theta)^2=0` at fixed X. For the outward corner branch, use
0 <= alpha <= pi/2 and the following explicit reference:

```text
k = r'/(R-r),  q = cos(alpha)+sin(alpha)
t = (k*k*q + sqrt(1+k*k*(q*q-1))) / (1+k*k*q*q)
n_r = t*cos(alpha),  n_z = t*sin(alpha),  n_theta = k*(t*q-1)
X(theta,alpha) = C(theta) + r(theta)*n(theta,alpha)
```

At alpha=0 the contact is `R*e_r+(H-r)*e_z`; at alpha=pi/2 it is
`(R-r)*e_r+H*e_z`. Between them, nonzero r' generally gives nonzero n_theta.
Simply revolving a theta-dependent meridional quarter circle omits this
tangential displacement and fails the envelope equation. Contact tests must
measure physical points and tangency, not only radius samples or mesh bounds.

This local reference does not prove a globally exposed, regular, nonintersecting
envelope for every positive law. Before admitting a numerical range, prove or
bound regularity, orientation, global overlap and contact exposure over that
range. The native builder must also be measured to establish what its radius
parameter denotes: original-rim azimuth, spine abscissa or another coordinate.
A positive builder result or `BRepCheck` alone cannot settle those questions.
If its geometry differs from this reference, refuse this representation or
accept a separately named, quantified approximation in a further ADR; never
silently redefine authored theta or insert ideal mass values into native output.

## Independent physical measurements

For the constant limit r_min=r_max=r, integration of the removed meridional
corner gives area `A=(1-pi/4)*r^2` and first moment about its inner radial line
`r^3/6`. Thus the exact removed volume is

```text
V_removed = 2*pi*((R-r)*(1-pi/4)*r^2 + r^3/6)
V_body = pi*R^2*H - V_removed
```

Both physical kernels must match that independent formula, actual side/end
contact dimensions, area, first moments and bounds, including placed cylinders.
It is not the straight-prism removal formula from ADR-038.

For a regular nonconstant admitted envelope, the exact reference volume can be
written as a surface integral. Use `X_theta cross X_alpha` with outward winding
on the fillet and the untouched planar/cylindrical portions:

```text
V_reference = ( integral_0^2pi integral_0^(pi/2)
                  X dot (X_theta cross X_alpha) d_alpha d_theta
                + integral_0^2pi R^2*(H-r(theta)) d_theta
                + integral_0^2pi H*(R-r(theta))^2/2 d_theta ) / 3
```

The bottom plane is z=0 and contributes zero. Use a separately derived reference
integrator with bounded truncation/roundoff and refinement evidence, not the
production mesh tessellator or kernel mass implementation. This is an analytic
definite-integral specification; the exploratory figures below supply no certified
integration bound or accepted tolerance for a nonconstant native solid. Establish actual geometry
and independent integral bounds before choosing tolerance or claiming parity.
Likewise area and first moments must come from independent surface integrals.

Require one valid closed positive-volume solid, not a shell or compound surrogate.
Require real Mock analytic records and a bounded closed outward face-bearing
mesh, actual native BREP properties and actual native contact/surface witnesses.
Default synthetic Mock must refuse this new feature. A physical Mock may not
reuse the legacy ID-only path. A construction/mesh ADR must specify coordinate
precision, chord/radius-law error, seam identity, vertex/triangle budgets and
quality refusal before implementation; ADR-038's 64 intervals are not a proof
of periodic-law accuracy or mesh closure.

## Source, cache and transaction gates

Use a distinct closed feature tag rather than optional fields on legacy `fillet`.
Required intent includes original target, owned full-rim ref, explicit positive
minimum/maximum Length values and the closed single-period cosine representation.
The eventual source contract must fix the exact wire fields after runtime proof;
this document introduces no usable tag, schema, patch or command.

Keep the stock immutable. Every actual output face belongs to the new feature;
do not fabricate pointer history or retain trimmed stock-face bindings. Native
face splits need not match Mock classes. Discovery/selection must inspect current
physical groups and refuse ambiguous subfaces; no general downstream editing
is promised. Cache admission must check source/frame/ownership plus actual
surface/contact phase. Rotating the law by pi/2 preserves scalar volume but
changes contact positions, so a wrong-phase cached body must fail.

Creation and coordinated stock/radius edits use DesignPatch and transactions.
Strict units/static fallbacks must validate even without parameter scopes or
when expressions are present. Failed regeneration preserves source, history,
previous output and artifacts; preview/review are read-only and successful
edits have canonical Undo/Redo. CLI/MCP candidates must force actual native
verification rather than allow verify:false/no-verify to bypass admission.

Before feature completion update both schemas, no-inference migration,
deterministic packed/expanded/authoring round trips and genuine older-reader
refusal. Supply examples, MCP authoring guide, skill and actual MCP eval with
independently graded source/phase and equal-volume wrong-phase/constant/straight
decoys. Source-field grading and physical surface measurement remain distinct.

## Exploratory pinned-native evidence

A direct OCCT 8.0 SDK experiment on 2026-10-14 used the same pinned prebuilt
headers/static libraries as cadrum; it did not change MusubiCAD's curved-edge
refusal or run through an authored feature. In millimetres, construct
`BRepPrimAPI_MakeCylinder(10,20)` and uniquely select its circular end edge at
z=20. Call `BRepFilletAPI_MakeFillet::Add(edge)`,
`SetParams(1e-6,1e-6,1e-8,1e-6,1e-8,1e-6)`, then `SetRadius(samples,1,1)` with
65 points `(i/64, 1+a*cos(2*pi*i/64))`, i=0..64, and `Build()`.
Both a=0 and a=0.5 produced one-contour/one-edge valid solids according to
`BRepCheck_Analyzer`. Actual `BRepGProp::VolumeProperties` with epsilon 1e-9 gave:

| Case | Actual volume, mm^3 | Maximum sampled end-contact radius error, mm | Maximum sampled side-contact radius error, mm |
|---|---:|---:|---:|
| Constant r=1 | 6270.00266166868 | 1.78e-15 | 0 |
| r_min=0.5, r_max=1.5 | 6268.44165785435 | 0.00099676435 | 0.00099674482 |

For each actual end-plane boundary and cylinder/fillet contact edge, sample
65 uniformly spaced native curve parameters. Exclude the unchanged cylinder
seam line and bottom edge. At each point p compute theta=atan2(p.y,p.x),
rho=hypot(p.x,p.y) and compare `10-rho` on the end plane, or `20-p.z` on the
cylinder, against `1+a*cos(theta)`. The constant volume matches the formula
above. Nonconstant discrepancies are approximately 1 micrometre at these
samples; they are not a bound over the entire contact curve.

After building the nonconstant case, `GetLaw(1,edge)->Bounds` returned
[0,62.8318530717959] mm (the original rim circumference). Its endpoint radii
were both 1.5 mm and its endpoint first derivatives were approximately -3.39e-16.
At 257 uniform parameters over those bounds, the stored law differed from
`1+0.5*cos(2*pi*u)` by at most 1.21105e-7 mm. That sampled interpolation error
is much smaller than the measured contact discrepancy. Normalizing the law's
bounds therefore cannot by itself prove the mapping to actual contact azimuth;
increasing the sample count alone is not an established remedy. Matching two
endpoint derivatives does not prove periodic smoothness of the whole output.

Separately, exploratory composite-Simpson evaluation of the reference surface
integral with central-difference derivatives (step 1e-5 rad) gave
6268.43680815, 6268.43678219 and 6268.43678056 mm^3 on 64x32, 128x64 and 256x128
theta/alpha intervals. Refinement suggests a native/reference difference near
0.00488 mm^3 for this fixture, but supplies no certified error or proof of
global exposure. Neither measurement establishes the mapping from law parameter
to physical contact azimuth,
periodic derivative continuity, entire blended surface or a general accepted
range. Do not adopt this experimental array's 64 intervals as a production
sampling decision. These figures inform the next investigation, not feature
completion or score.

## Completion and remaining decisions

P16-017 accepts only this target/reference and its acceptance gates. Next obtain
native radius-coordinate/periodicity/contact evidence and an admitted regular
range. Then register a separate runtime/sampling prerequisite in the roadmap
before using its ID, accept its construction ADR and implement real Mock/OCCT
geometry. P16-008 supplies the guarded source, compatibility, history, examples
and eval only after that runtime proof. If the candidate cannot meet the gates,
record the failure and revise the admission rather than shipping a surrogate.

This design adds no physical body or runnable source feature, changes no schema
or golden, and earns no maturity credit. Broader curved contours remain future
work; constant-only circular geometry cannot complete P16-008.
