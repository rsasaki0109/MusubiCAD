# ADR-087: Certify current native 3D contact curve definitions

Status: Accepted for read-only diagnostic contact snapshots only.
Date: 2026-10-15.

## Decision and scope

Register P16-034 before implementation. Extend ADR-082's immutable contact
reference to the CURRENT post-sewing 3D side/end rational curve definitions.
P16-033 certifies the blend surfaces, but does not certify these edge curves.
Keep P16-008 planned and maturity unchanged. This task creates no feature,
Mock/runtime body, editable NURBS, source/schema/patch, mesh or MCP operation.

Accept only single-span clamped nonperiodic degree-2 side and degree-4 end
snapshots with finite Cartesian millimetre poles and finite positive weights.
The native capture must establish identity Location, canonical [0,1] range,
degree, knots and multiplicities on the current edge. Unsupported representations
refuse. These fixed representation/transport identity gates are distinct from
geometric proximity checks, which retain explicit tolerances.

## Complete corresponding-parameter bound

Convert each raw binary64 pole with outward I(q_mm)/I(1000), THEN multiply by
its positive weight. The immutable descriptive snapshot stores homogeneous SI
intervals, has no editing/construction authority and performs no I/O.

For supplied immutable reference controls construct homogeneous Bernstein
numerators A and denominator W; for the native snapshot construct B,V. Normalize
the native homogeneous curve by supplied_W0/native_V0, carrying outward interval
uncertainty throughout. This is a constant positive gauge and preserves geometry.
All denominator coefficient hulls must be finite and strictly positive.

Compute coefficient differences deltaA=B-A, deltaW=V-W before taking hulls.
For each axis, use the complete Bernstein convex hull to enclose
X=A/W and E=(deltaA-X*deltaW)/V. The outward norm of E bounds the entire native
curve against the stored supplied curve on the closed [0,1] domain. Add the
existing ADR-082 supplied-to-ideal position bound with outward arithmetic.
Require a finite positive requested SI Length and refuse when the total exceeds
it. Fixed at most five controls, no sampling, subdivision or work budget needed.
Overflow, unresolved denominator, malformed layout and insufficient precision
produce no certificate and mutate no geometry. A positive common weight gauge
must succeed; a changed nonuniform weight receives the same actual proof.

## Capture, transport and validation

The pinned native diagnostic reads current BRep_Tool::Curve with its explicit
Location, refuses nonidentity placement, and exports original poles_mm with
17-digit showpoint binary64 transport, current weights, periodicity, degree,
knots and multiplicities. Retain the separate rounded SI witnesses. A bounded
stdin Rust example processes exactly sixteen requests (two laws, eight contacts
each), echoes all raw data/reference/kind/chart/budget, and emits only after all
proofs succeed. Bind each certificate back to the current capture by binary64
transport identity, including signed zero, rejecting booleans as numbers.

Rust tests compare independently evaluated rational curves and analytic contact
dimensions across charts/scales and both laws, including changed poles/weights,
gauge invariance, wrong units/phase/kind/chart and invalid or unattainable budgets.
Check the raw SI interval against exact binary64 fractions. Synthetic report
tests validate parsing/binding only; CI must execute actual native capture and
proof, including last-request precision refusal without partial stdout. Preserve
all existing topology, pcurve, surface/normal, volume and classifier gates.

Update the API, architecture, example, authoring guide, skill and plans. All
mandatory local checks and all applicable exact-head CI precede merge. No
schema/migration/golden or runnable MCP eval applies to this read-only diagnostic.

## Limitations and remaining gates

This proves the mathematical current 3D rational curve definition against its
ideal corresponding-parameter side/end contact over the entire domain. It does
not certify the native floating evaluator, derivative/regularity, attached 2D
pcurve lifts/reparameterization, full edge-to-face gaps, quarter seams, exact G1,
global embedding or whole-BREP fidelity. Those are separate remaining obligations;
no surface/curve certificates alone admit production geometry or increase scores.
