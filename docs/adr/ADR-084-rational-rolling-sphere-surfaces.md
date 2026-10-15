# ADR-084: Construct rational rolling-sphere reference surfaces

Status: Accepted for transient reference and diagnostic native construction.
Date: 2026-10-15.

## Decision

Represent ADR-072's characteristic circle by a rational parameter through its
known side/end normals. This constructs the full ideal rolling-sphere envelope
without polynomial interpolation or a square-root fit. Use four immutable
degree-(8,2) positive-weight rational Bezier charts and outward whole-patch
position bounds for their actual stored controls. Construct native diagnostic
surfaces and sew them with unchanged analytic cylinder/top/bottom supports.
This is not a production GeometryKernel/Mock body, authorable source or editable
NURBS feature. P16-008 remains planned and no maturity credit is awarded.

## Characteristic-circle parameter and regularity

At theta, write r=m+a*cos(theta), k=r'/(R-r). A characteristic normal satisfies

    nr²+nt²+nz²=1, nt=k*(nr+nz-1).

Its side/end points are (1,0,0), (0,0,1); their tangents meet at (1,k,1).
Quadratic rational weights [1,1,2*(1+k²)] therefore parameterize that circle:

    Q=1+(1+2*k²)*v²
    nr=(1-v²)/Q
    nt=2*k*v*(1-v)/Q
    nz=(2*v+2*k²*v²)/Q,             0<=v<=1.

Substitution proves both equations. nr,nz are nonnegative and never both zero.
The polar angle alpha=atan2(nz,nr) increases from 0 to pi/2: its derivative's
numerator before the common positive factors is 2+2*v²+4*k²*v>0. Thus this is
the same exposed characteristic arc as ADR-072, with a regular orientation-
preserving transverse reparameterization. v is NOT alpha/(pi/2). Side normal
is radial, end normal axial; ideal adjacent quarter charts share smooth geometry
with their theta-parameter scale change. This algebra concerns the ideal surface,
not a certificate of rounded/native G1 or global embedding.

On each quarter t, let D=1+t² and rotated C,S give cos(theta)=C/D,
sin(theta)=S/D. Put N=m*D+a*C, L=R*D-N, K=-a*S, and

    Wc=L²*(1+v²)+2*K²*v²
    A=(1-v²)*L², B=2*K*L*v*(1-v), Z=2*v*L²+2*K²*v².

The homogeneous surface denominator and numerators are

    W = D²*Wc
    X = L*C*Wc + N*(A*C-B*S)
    Y = L*S*Wc + N*(A*S+B*C)
    Zp = (H*D-N)*D*Wc + N*D*Z.

Their bidegree is (8,2). Wc's v-Bernstein coefficients are L²,L²,2*(L²+K²).
L has positive t-Bernstein coefficients in the admitted range; K has one sign
within a quarter, so K²'s Bernstein coefficients are nonnegative. Multiplying
by D² preserves positive denominator coefficients. The surface uses no sampled
fit and introduces no source admission.

## Bounded cylindrical pcurve construction

The initial native projection recipe can meet the positional sewing tolerance
while failing the 1e-9 normal witness gate: measured cylinder/blend normal gaps
were about 8e-9 with the existing request. Tightening the request alone still
left about 6e-9 for the variable law because projection/SameParameter use native
approximation floors. Keep the gates. Supply a separate bounded analytic
cylindrical side pcurve, with 256 uniform dyadic cubic Hermite spans per quarter,
instead of relying on that projection for the rational side contact.

Its exact ideal coordinates are u=chart*pi/2+2*atan(t), z=H-m-a*cos(theta).
On [0,1], |u''''|<=48 and |z''''|<=768*a; the latter follows by differentiating
(1-t²)/(1+t²) and 2*t/(1+t²) four times and bounding their numerator terms.
For h=1/256, cubic Hermite interpolation error is at most M4*h^4/384 per
coordinate. Compute endpoint values/derivatives with outward arithmetic, using
three atan half-angle reductions and the existing bounded atan series; choose
rounded actual Bezier controls and add their coefficient-rounding error by the
convex-hull bound. A lifted positional bound is sqrt((R*angular_error)^2+z_error²)
by the circle's Lipschitz inequality. Store SI height/metre and angular/radian
controls privately, with read-only getters and fixed bounded work. This is a
new diagnostic pcurve approximation decision, not a mesh sampling admission.
Do not call an ideal-input interpolation estimate a bound for rounded controls.

Construct the native clamped degree-3 B-spline from these actual controls with
internal knot multiplicity 3, preserve the shared endpoint controls, then force
SameParameter and inspect current sewn pcurves and normals again. The mathematical
bound concerns the supplied pcurve; all native conversion/reparameterization
effects remain separate numerical evidence. Other cylinder/plane boundaries
can use the existing bounded-tolerance native projection. Use a 1e-9 mm native
SameParameter request; native edge/vertex floors are still checked against the
existing 1e-6 mm ceiling. Never relabel returned native tolerance as a certificate.

## Rounded representation and bound

Use private outward Bernstein arithmetic to enclose ideal homogeneous
coefficients N_i,W_i. Choose finite positive rounded weights w_i and Cartesian
poles P_i. Recompute component bounds from the ACTUAL stored data:

    error <= max|P_i*w_i-N_i|/wa
           + max|N_i|*max|w_i-W_i|/(wa*wi),

where wa,wi are positive denominator coefficient lower bounds for the stored
and ideal surfaces. Take an outward Euclidean norm. Invalid or nonpositive
enclosures refuse. This is a corresponding-(t,v) whole-patch position bound,
not an independent derivative/normal or all-native error certificate. Existing
polynomial-band certificates cannot be reused. Rounded chart boundaries need
not have bit-identical controls or exactly coincide with separately encoded
ADR-082 contact curves; all native stitching tolerances remain explicit.

## Independent and native evidence

Pure geometry tests independently evaluate the tensor rational Bernstein data
and derivatives. Compare to ADR-072's square-root reference at the derived alpha,
independent sphere/envelope constraints, contacts, phase, constant limit, scales
and quarter seam orientation. Test invalid charts and deterministic bounded work.

Pinned OCCT constructs actual weighted Bezier surfaces, inspects current poles,
weights, D1 and normals, and sews diagnostic solids from four blend charts plus
four trimmed cylinder quadrants, top and bottom planes per law. Preserve the
analytic supports and actual contact-curve representations; require validity,
connected two-face edge incidence, closure, Euler/orientation and positive volume.
Construct each blend face with the supplied side/end contact edges and explicit
linear (t,0)/(t,1) pcurves, plus its two native isoparametric seam edges. Do not
rely on the default surface rectangle's newly generated high-degree contact
edges to preserve the independently supplied contact representation during sewing.
Check the actual sewn 3D contact controls as well as the current surface controls.
Compare native volume with ADR-073's independently bounded ideal volume and
constant analytic limit. Actual surface/contact/support/seam witnesses and a
native quarter-turn phase decoy test fidelity. Numerical native gates: 1e-14 m
control/evaluation, 1e-12 m per local parameter D1, 1e-9 dimensionless normal,
1e-9 m sewing/edge/vertex ceiling, and 1e-12 m³ volume comparison allowance.
These measured controls do not prove global native G1, embedding or all native
conversion/sewing/integration errors. Do not repair an inverted/open result or
substitute polynomial skirts. Unsupported or missing evidence refuses.

Run live native construction and synthetic report refusal tests in CI, retaining
the report. Update API/architecture/example/MCP guide/skill/plans and all local
and exact-head CI gates. No .ocad schema/migration/golden, source operation or
MCP eval is added. Physical Mock/runtime/mesh, globally bounded normal/support
fidelity and guarded ownership/cache/history/reader/source admission remain.
