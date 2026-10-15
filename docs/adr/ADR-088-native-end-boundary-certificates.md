# ADR-088: Certify current native end contact boundary gaps

Status: Accepted for read-only end-contact diagnostics only.
Date: 2026-10-15.

## Decision and scope

Register P16-035 before implementation. P16-033 proves current surface definitions
and P16-034 proves current 3D contact curves. Neither proves the attached pcurve
lifts coincide with those curves. Close the END/PLANE contact part first, using
the actual eight end boundaries of the existing two-law native solid diagnostic.
Current read-only inspection confirms clamped nonperiodic degree-4 five-pole
plane pcurves and blend Geom2d_Line(t,1), all on [0,1]. Unsupported definitions
refuse; do not infer these representations from sampled proximity.

No production Mock/runtime, mesh, editable NURBS, Design Graph, source/schema/
patch or new MCP operation is admitted. P16-008 remains planned; no score.
Cylindrical side pcurve lifts, quarter seams, native floating evaluation, exact
G1, global embedding and whole-BREP fidelity remain separate obligations.

## Complete lift and pairwise position bounds

Introduce an immutable descriptive planar-contact snapshot. Read ORIGINAL native
plane origin O_mm, dimensionless X/Y directions, five Cartesian UV poles in mm
and positive weights. Construct each homogeneous SI pole with outward arithmetic:

    (I(O_mm)/1000 + (I(u_mm)/1000)*I(X) + (I(v_mm)/1000)*I(Y))*I(weight).

An affine lift commutes with rational Bernstein evaluation because the same
positive denominator multiplies O. Do not round the lifted point or convert the
frame/UV to rounded SI before enclosing it. All finite frame/control data and
intermediate arithmetic must validate. No hidden I/O or native types enter
geometry; the snapshot has no construction or editing authority.

Reuse ADR-087's full-curve common-gauge proof on the lifted homogeneous degree-4
curve against the ideal end contact. Recompute the current 3D edge certificate
with the same reference/chart and reuse ADR-086's complete native surface proof.
That surface certificate covers the canonical end boundary B(t,1), whose ideal
reference equals the same end contact. Do not accept caller-supplied certificates.

For each pair among edge E, plane lift P and blend boundary B, return the outward
sum of its two complete position error bounds. Require ALL three pairwise bounds
to meet the caller's finite positive SI gap budget. Keep the surface proof's
strict (0,1) normal budget and 1..=16384 leaf budget; return its normal bounds as
blend-surface evidence only, not a new plane/blend G1 certificate. Invalid kinds,
chart, nonfinite/overflow data, precision or work exhaustion refuse with no
partial result or geometry mutation. No new sampling method is introduced.

## Current capture, qualification and transport

Export the current post-sewing plane origin in raw mm and current X/Y directions
from the actual native support, plus current attached pcurve poles_mm, weights,
degree, knots/multiplicities, range and nonperiodicity. Existing capture already
establishes identity Location, support type and forward orientation. Export the
actual attached blend pcurve type, origin, direction and range. Qualify ONLY the
canonical Geom2d_Line with origin (0,1), direction (1,0), and range [0,1]. These
exact fixed representation/data gates, like the existing canonical domain gates,
are distinct from geometric proximity comparisons, which use tolerances. Do not
silently accept shifted/reversed/scaled lines under the canonical proof.

Use 17-digit showpoint raw data and serde_json float_roundtrip. A bounded stdin
Rust example accepts exactly eight requests and echoes the complete reference,
raw edge/frame/UV/blend data, line definition and precision/work budgets. It
qualifies the supported line representation and emits only after all proofs
succeed. The Python grader binds every echo to the actual current capture by
binary64 identity including signed zero, rejecting numeric booleans. Fixture gap
request is 1e-12 m, blend-normal request 1e-9 and 4096 leaves per chart.

## Validation and remaining admission

Test affine lifting against independently computed plane transformations, analytic
end dimensions across charts/scales/laws, positive common weight gauge, and actual
edge/plane/blend deformations. Refuse wrong units/phase/kind/chart, changed frame/
pcurve/line/domain, invalid precision/work and unattainable final-request precision
without partial stdout. Test raw affine coordinate enclosures with exact integer
fractions. Synthetic reports validate parser/binding refusals only. CI executes
actual native construction/capture and all eight recomputed boundary proofs,
retaining the existing topology, volume, phase, side-pcurve and normal gates.

Update API, architecture, example, MCP guide, skill and plans. All four mandatory
local checks and all applicable exact-head workflows/full job inventories precede
merge. No schema/migration/golden or runnable MCP eval applies to this diagnostic.
Bounds concern the mathematical captured definitions at corresponding parameters;
floating native evaluation remains separate numerical witness evidence. Successful
end-boundary proofs do not certify side contacts, whole-BREP fidelity or admit
the production variable-fillet feature.
