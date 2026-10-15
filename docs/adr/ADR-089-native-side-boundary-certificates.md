# ADR-089: Certify current native cylindrical side boundary gaps

Status: Accepted for read-only side-contact diagnostics only.
Date: 2026-10-15.

## Decision and scope

Register P16-036 before implementation. Extend P16-035's complete end-contact
position-gap evidence to the eight actual CYLINDRICAL SIDE boundaries of the
existing two-law native solid diagnostic. Capture the current attached pcurve,
actual owning cylinder frame/radius and attached blend line after sewing. Do
not transfer a supplied-pcurve certificate from sampled proximity alone.

Accept only the existing fixed degree-3 nonperiodic 256-span/769-pole unit-weight
basis: knots j/256, internal multiplicities three and endpoint multiplicities
four, range [0,1]. Qualify the attached blend Geom2d_Line as (t,0) over [0,1].
These exact fixed parameter/representation data gates are distinct from geometry
proximity comparisons, which retain explicit tolerances. Unsupported definitions
refuse. No production Mock/runtime, mesh, editable NURBS, source/schema/DesignPatch
or new MCP operation is admitted; P16-008 remains planned and scores unchanged.

## Complete cylindrical lift bound

An immutable descriptive snapshot reads original native angle_rad/height_mm
controls, original origin_mm/radius_mm and dimensionless X/Y/Z directions from
the actual owning cylindrical support. Convert lengths with outward I(q)/1000,
never rounded SI exports. Require finite data, positive resolved radius and
exactly 769 controls. The snapshot constructs no cylinder or editable curve.

Recompute the supplied reference's ADR-084 fixed-span Hermite angle/height bounds
from the validated reference/chart. Actual and supplied pcurves have the SAME
qualified nonrational basis. Its nonnegative Bernstein partition of unity on
every closed span bounds their complete component differences by the maximum
outward control difference. Add these to the supplied angle/height bounds to
obtain ea (radians) and eh (metres) against ideal theta(t), z(t). This covers all
256 spans and their endpoints without geometric sampling or new subdivision.

For actual cylinder O,Rn,X,Y,Z, let dx=norm(X-ex), dy=norm(Y-ey), dz=norm(Z-ez)
and zmax be the maximum absolute actual height-control enclosure. For all angles,
the actual-frame versus canonical-frame position difference is at most

    F = norm(O) + abs(Rn)*(dx+dy) + zmax*dz.

For canonical cylinders, the horizontal position difference from ideal radius R
is at most abs(Rn-R)+R*ea, since the unit circle is globally 1-Lipschitz in angle.
The complete lifted pcurve position bound is therefore

    F + sqrt((abs(Rn-R)+R*ea)^2 + eh^2).

All differences, extrema, norms, sums and products use outward SI intervals.
Treat current binary64 frame/radius data as definitions, not assumed exact SI
canonical geometry; include their discrepancies even for the default fixture.
Finite-input overflow, unresolved arithmetic or precision failure refuse.

## Complete side boundary gap

Recompute the current side 3D edge certificate using ADR-087, the cylindrical
lift certificate above and the current blend surface certificate using ADR-086.
The ideal corresponding boundary B(t,0) equals the same ideal side contact.
Return all three component position bounds and outward pairwise sums for
edge/cylinder-lift, edge/blend and cylinder-lift/blend over the full [0,1] span.
ALL pairwise sums must meet a finite positive SI gap budget. Retain the existing
surface proof's strict (0,1) blend-normal budget and 1..=16384 leaf budget, returning
its normal bounds as surface-only evidence. Do not accept external certificates.
Wrong edge kind/chart, invalid data/budgets or exhausted work yield no partial
result or geometry mutation. This certifies position gaps, not paired normals.

## Capture, transport and validation

Capture original [angle_rad,height_mm] controls, actual weights, range, degree,
knots, multiplicities and periodicity from the current attached native pcurve.
Read frame/radius directly from the actual cylindrical face owning that edge,
with the existing identity Location, support type and forward-orientation gates.
Capture actual attached blend line type/origin/direction/range. Require unit
weights in addition to IsRational=false; never assume that flag alone describes
the stored weight data. Export raw data with 17-digit showpoint binary64 transport
and use serde_json float_roundtrip. Preserve the existing rounded SI witnesses.

A bounded-stdin Rust example consumes exactly eight requests within 1 MiB,
qualifies the fixed line and unit weights, echoes every raw input/reference/
precision/work budget, and emits only after all eight recomputed proofs succeed.
The grader binds echoes to current captures by binary64 data identity including
signed zero and numeric boolean refusal. Use fixture gap 1e-12 m, blend-normal
1e-9 and 4096 leaves. Keep existing topology, volume, phase, end-boundary, pcurve
and numerical normal gates unchanged. Actual CI capture/proof and real-process
refusals are required; synthetic reports validate parsing only.

Tests compare independent analytic side contacts and direct cubic/cylinder
evaluation across charts/scales/laws, actual frame/radius/angle/height/edge/blend
deformations, combined-budget refusal, wrong units/phase/kind/chart and invalid
or unattainable precision/work. An exact-fraction oracle validates raw SI
enclosures. Refusal subprocesses include noncanonical line, nonunit weight,
changed frame/radius/control/unit, missing controls, input bytes and final-request
precision without partial stdout. Update API, architecture, example, MCP guide,
skill and plans, then all four mandatory local checks and all applicable exact-
head workflows/full job inventories before merge. No schema/migration/golden or
runnable MCP eval applies to a read-only diagnostic with no authored operation.

## Remaining admission

Together P16-035/036 cover end/side corresponding-parameter POSITION gaps between
current mathematical definitions. Native floating evaluation, quarter seams,
paired normal/G1 agreement, global embedding and whole-BREP fidelity remain
separate obligations. No production fillet admission or maturity credit follows.
