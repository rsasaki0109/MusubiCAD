# ADR-085: Certify rounded rational reference normals

Status: Accepted for mathematical rounded reference geometry only.
Date: 2026-10-15.

## Decision and scope

Certify the entire closed unit parameter square of each immutable ADR-084
rational chart, including its boundaries. Bound the actual stored poles/weights'
derivative and normal error against the ideal rational envelope, and require
positive Jacobian and orientation. Bind the chart to its construction reference
privately; do not accept unrelated public certificate metadata as geometry.
No editable NURBS, new mesh policy, physical Mock/runtime/source operation or
maturity credit is introduced. P16-008 remains planned.

Use outward homogeneous Bernstein arithmetic and deterministic dyadic
restriction, not finite normal sampling. Share the existing private outward
de Casteljau split without changing its area-integration semantics. Extract the
unchanged ideal homogeneous chart equations for construction and verification
to use one definition. Differentiate polynomials in the ORIGINAL u,v coordinates
before restriction; restricting a derivative does not rescale its parameter.

## Quantitative ideal Jacobian lower bound

ADR-072 gives F_ss>=M=2R²-7R*r_max+r_max²/2>0 and
C' dot X_theta=F_ss/2, C' dot X_v=0. With a=(r_max-r_min)/2 and
K=a/(R-r_max), ADR-084's rational characteristic arc satisfies

    |n_v|=2*sqrt(1+k²)/(1+(1+2k²)*v²) >= 1/sqrt(1+K²).

Also |C'|<=sqrt(R²+2a²) and theta_u=2/(1+u²)>=1. Thus, in square metres per
original dimensionless u,v area,

    |X_u cross X_v| >= r_min*M /
        (2*sqrt(1+K²)*sqrt(R²+2a²)) = J0 > 0.

Evaluate this lower bound outward, not with the old floating reference metadata.
A cell's direct ideal derivative interval cross-product can improve its lower
bound; both are valid bounds for the same ideal Jacobian. ADR-072 and ADR-084
fix the outward ideal orientation. This proof concerns the ideal reference,
not an assertion that a finite set of native normals proves regularity.

## Actual-control derivative and normal errors

Let ideal homogeneous coordinates be A,W and actual encoded ones B,V. Build
deltaA=B-A and deltaW=V-W as interval-coefficient polynomials BEFORE restriction,
including actual Cartesian pole times weight. Differentiate those differences
before subdividing, to avoid losing cancellation by subtracting separately
restricted large coordinates. Positive denominator coefficient hulls bound W,V
away from zero. For X=A/W, Y=B/V, E=Y-X, exactly

    E=(deltaA-X*deltaW)/V
    E_u=(deltaA_u-X_u*deltaW-X*deltaW_u-E*V_u)/V,

and likewise for v. Use outward cell hulls for these identities to bound each
component and then the Euclidean norms e_u,e_v. Independent ideal derivative
bounds U>=|X_u|, T>=|X_v| give

    d = e_u*T+U*e_v+e_u*e_v >= |Y_u cross Y_v-X_u cross X_v|.

For a valid ideal Jacobian lower J, require d<J. This proves the rounded chart
is regular, its cross product points in the ideal hemisphere, its Jacobian is
at least J-d, and its unit-normal chord error is at most 2*d/J. A conservative
alignment lower bound is (J-d)/(J+d)>0. Bound its Jacobian above using the ideal
upper bound plus d. Take extrema over a COMPLETE parameter cover; never return
partial or sample-only certificates.

## Bounded deterministic refinement and boundary implications

Request a finite dimensionless maximum normal chord error strictly between
zero and one and an explicit leaf budget in 1..=16384. Traverse a depth-first
binary cover, alternating u/v half-splits, left child first. Splitting consumes
one additional cover leaf; a hard depth limit of 28 and the leaf budget bound
work/memory even for unattainable precision. A cell is accepted only when its
regularity/orientation and requested normal precision are proven. Exhaustion,
invalid inputs/nonfinite intervals/nonpositive denominators or unresolved bounds
refuse without mutating the chart. Geometry is never resampled or changed.

At v=0 the ideal normal is radial, and at v=1 it is axial. The whole-chart error
therefore also bounds rounded surface/support normal error at corresponding
parameters. Paired adjacent quarter normals differ by at most the sum of their
certificates, because their ideal seam normal is the same. For the supplied
cylinder pcurve, add its certified angular error by the unit-circle Lipschitz
bound. These statements concern mathematical supplied geometry and parameter
correspondence; they do not certify every native sewing/reparameterization or
evaluation error, exact G1, contact coincidence or global embedding.

## Validation and remaining gates

Test independent analytic constant-limit Jacobians, analytic rational derivatives
and square-root reference normals over constant/nonconstant laws and scales;
cover tight requests, deterministic budgets and unmodified controls. Private
negative tests cover collapsed/reversed/deformed/nonfinite controls and weights;
no public editing API is added. Extend the actual native diagnostic to compare
current D1/normals/Jacobians to these bounds with separate numerical allowances,
including support and seam witnesses and the existing wrong-phase body decoy.
Synthetic report tests prove parser refusals only. Live CI constructs native
solids and retains the report. Update API/architecture/example/guide/skill/plans,
all mandatory local checks and exact-head applicable CI before merge.

No .ocad schema/migration/golden or MCP operation/eval is added. Global native
fidelity/G1/embedding, physical Mock/runtime/mesh and guarded source/ownership/
cache/history/reader admission remain required before the authorable feature.
