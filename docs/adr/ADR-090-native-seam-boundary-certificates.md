# ADR-090: Certify current native quarter seam position gaps

Status: Accepted for read-only quarter-seam diagnostics only.
Date: 2026-10-15.

## Decision and scope

Register P16-037 before implementation. Complete the eight quarter-seam POSITION
gap checks in the existing two-law native rational diagnostic. Read-only native
inspection found current Geom_BezierCurve edges on [0,1] and attached Geom2d_Line
pcurves (1,t) on chart c and (0,t) on chart (c+1)%4. Capture actual degree, poles,
weights, periodicity, Location and line definitions after sewing; refuse any
unsupported representation, range or adjacency. Exact representation identity
gates are distinct from geometric proximity, which uses explicit SI tolerances.

Compare CURRENT definitions directly: there is no need to transfer an ideal
reference error or assume neighboring rounded reference charts coincide exactly.
An immutable descriptive degree-2 edge snapshot encloses raw millimetre poles
with I(q)/1000 before weighting. The existing current surface snapshot supplies
its actual homogeneous u=1 / u=0 boundary rows. Positive weights and the fixed
Bernstein basis make these the complete corresponding-parameter boundaries.
No native geometry, editable curve, source feature or sampling method is created.

## Complete direct rational comparison

For two homogeneous quadratic curves A/W and B/V, normalize a positive common
weight gauge using their first coefficients. Take coefficient differences BEFORE
hulls, then enclose the whole closed unit interval using the nonnegative
Bernstein partition of unity:

    B/V - A/W = (deltaA - (A/W)*deltaW) / V.

Use outward coefficient hulls, positive denominator lower bounds, SI conversion,
products, differences, divisions and Euclidean norm. Compute edge/left,
edge/right and left/right bounds independently from the raw definitions. ALL
three finite bounds must fit a finite positive Length budget. No samples,
subdivision/work heuristic or external certificates enter this proof; at most
three controls per curve give a fixed amount of work. Nonfinite data, overflow,
nonpositive/unresolved weights and unattainable precision refuse without partial
results. Common positive weight rescaling must preserve geometric acceptance.

This proves mathematical captured boundary POSITION gaps only. Existing complete
surface, side/end contact, topology, volume, phase and numerical normal checks
remain mandatory. It does not prove native floating evaluation, normals/exact G1,
global embedding, untouched support seams or whole-BREP fidelity. P16-008 stays
planned, no production Mock/mesh/source/schema/editable NURBS/MCP admission or
maturity credit follows.

## Capture, transport and tests

Capture raw poles_mm and weights from the current Geom_BezierCurve, identity
Location and fixed degree/range/nonperiodicity. Capture both attached line
origins/directions/ranges together with actual chart roles; canonicalize ordering
by chart c at u=1, including the 3-to-0 wrap. Preserve signed zero and full
binary64 round-trip transport. A bounded 256 KiB read-only Rust example accepts
exactly eight requests, qualifies chart adjacency and line representations,
echoes every raw input and budget, and emits only after all eight proofs succeed.
The Python grader binds each proof to current captures by binary64 identity,
checks all three gap budgets and checks native numerical witnesses against the
bounds plus the existing explicit floating-evaluation allowance. Gap budget is
1e-12 m; existing numerical tolerances are unchanged.

Independent tests use analytic rational quadratic evaluations, translations,
positive common gauges, internal-control/weight deformations invisible at
endpoints, wrong units, invalid data and precision/combined-bound refusal.
Actual native capture/proof and real subprocess refusal controls are required;
synthetic report tests exercise transport/grading only. Update API, architecture,
example, MCP guide, skill and plans. All four mandatory pre-push checks and all
applicable exact-head workflows/full job inventories must pass before merge.
No schema/migration/golden or runnable MCP eval applies to this read-only probe.
