# ADR-082: Rational support contact curves

Status: Accepted for MCAD-P16-029's transient contact construction only.
Date: 2026-10-15.

## Decision

Construct the ideal side/end contacts of ADR-070 with fixed positive-weight
rational Bezier curves, rather than use ADR-079's off-cylinder polynomial
contact approximation. Create actual native edges and closed contact wires.
This does not trim native supports, construct a fillet surface/body or admit
physical Mock/source geometry. P16-008 remains planned; no maturity credit.
No editable NURBS is exposed: curve data are private, read-only and transient.

## Rational representation and exact encoded support identities

On each quarter chart t in [0,1], put D=1+t². The unrotated circle has
C=1-t², S=2t; rotate (C,S) by the chart's multiple of pi/2. With
m=(r_min+r_max)/2 and a=(r_max-r_min)/2, side homogeneous coordinates are

    (R*C, R*S, (H-m)*D-a*C, D).

Use degree two, weights [1,1,2], and exact stored x/y pole coordinates obtained
by rotating [(R,0),(R,R),(0,R)]. These encoded binary64 values satisfy
x²+y²=R² for the mathematical rational curve by C²+S²=D², regardless of rounded
z poles. This is a support identity for the encoded curve, not a guarantee of
all native evaluation/coordinate-conversion error or a trimmed cylinder face.

The end contact is degree four with homogeneous coordinates

    ((R-m)*C*D-a*C², (R-m)*S*D-a*C*S, H*D², D²).

Ideal denominator Bernstein weights are [1,1,4/3,2,4]. Positive rounded weights
and rounded x/y poles approximate that curve; every Cartesian z pole is the
same stored H, so the encoded rational curve lies exactly in its encoded plane.
Construct quarter endpoints from one shared axis/radius table, including the
closed-period wrap, to preserve identical endpoint representations. The radius
law phase remains maximal at authored +X, never inferred from a native seam.

## Whole-curve position bound

Convert exact-input homogeneous power polynomials to Bernstein coefficients
using outward arithmetic. Enclose ideal numerator/denominator coefficients
N_i,W_i and the actual encoded P_i*w_i,w_i. With positive lower denominator
bounds wa,wi, coefficient convex hulls give, componentwise,

    error <= max|P_i*w_i-N_i|/wa
           + max|N_i|*max|w_i-W_i|/(wa*wi).

Take an outward Euclidean upper bound. Include canonical endpoint rounding,
actual rounded weights and all actual stored poles. Do not treat ideal weights
or metadata as native output. Bounds compare corresponding chart parameters;
no global derivative/G1/native error bound follows. Invalid chart, nonfinite
or nonpositive denominator/numerical enclosures refuse. Work is fixed degree
and bounded, with no curve fitting, sampling or new mesh policy.

## Independent native evidence and tests

Use pinned OCCT to build actual weighted B-spline edges with clamped knots and
four closed contact wires (two laws, two supports). Measure current edge poles,
weights, degrees, positions and D1 against independent rational evaluation;
check actual wire validity/closure/edge and vertex inventories. Native metre/mm
conversion/evaluation allowance is separate from the mathematical certificate.
Measure a native quarter-turn transformed decoy: the constant law retains its
physical azimuth contact relation, while the nonconstant law must fail it.
Native curves/wires alone never admit support trimming or a physical fillet.

Test encoded support identities, positive weights, exact shared endpoints,
constant/nonconstant reference/phase, scale and chart refusals; synthetic report
controls prove parser behavior only. Live CI performs actual construction and
saves its report. Update API/architecture/example/authoring guide/skill/plans and
all mandatory local/exact-head CI gates. No .ocad schema/migration/golden or MCP
operation/eval is added. Remaining surface/G1, support trimming, embedding,
physical Mock/runtime/mesh and source/cache/history/reader gates still apply.
