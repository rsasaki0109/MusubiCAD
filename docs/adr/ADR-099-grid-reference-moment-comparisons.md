# ADR-099: Compare stored-grid first moments and centroid

Status: Accepted for MCAD-P16-046 read-only reference arithmetic.
Date: 2026-10-16.

## Decision

Bind every comparison to the immutable grid's original ideal reference. Accept
ADR-098's explicit volume/moment/centroid full widths and separate per-pass leaf
cap, plus finite positive maximum absolute first-moment difference (m⁴) and
centroid difference (Length), applied to EACH coordinate. Do not divide errors
by ideal moments, which can be exactly zero.

For each actual stored-f64 oriented triangle (a,b,c), use outward arithmetic:
Vt=dot(a,cross(b,c))/6 and Mt=Vt*(a+b+c)/4. Sum in stored triangle order.
Require the resulting volume interval to have strictly positive lower bound;
divide each signed moment by that interval outward. Subtract ADR-098's ideal
moment and centroid intervals outward and take the maximum absolute endpoint
per coordinate. Refuse nonfinite arithmetic, unattainable ideal widths/work,
or any coordinate exceeding either supplied error budget. Return immutable
intervals, upper error bounds and independent ideal volume/moment leaf counts.
No caller-provided ideal results, midpoint substitutions or partial successes.

## Meaning and boundaries

These are signed boundary integrals and algebraic quotients of the actual
stored coordinates. Interpreting them as solid mass requires a properly oriented
embedded closed shell; this query adds no exact global embedding certificate to
TriangleSolid's existing numerical admission. It does not certify native
conversion, placements or f32 conversion. Native/Mock mass properties remain
actual backend measurements, never replaced by ideal intervals. Existing grid,
triangle/intersection and integration caps remain unchanged. P16-008 stays
planned. No schema, source feature, editable NURBS, new MCP operation/eval or score.

## Verification

Use exact dyadic tetrahedron/translated-box rational moments and centroid
quotients checked by integer endpoint comparisons; independently accumulate
integer-nanometre tetrahedral moments for real grids. Test constants/scales,
variable reference binding/refinement, deterministic replay, signed/zero axes,
invalid precision/work and each independent discrepancy budget with unchanged
grid after refusal. Compare actual both-kernel mass/centroid separately with
numerical tolerances. Extend existing example/CI JSON, API/architecture, guide,
skill and canonical plans. Mandatory local and exact-head CI gate merge.
