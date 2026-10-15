# ADR-086: Certify current native rational surface definitions

Status: Accepted for read-only diagnostic surface snapshots only.
Date: 2026-10-15.

## Decision and boundaries

P16-032 certifies supplied rounded controls. Extend its complete-cover proof to
current post-sewing native Bezier surface definitions, instead of transferring a
certificate merely because sampled points or approximate control comparisons
pass. Register P16-033 before implementation. Keep P16-008 planned and scores
unchanged. No Design Graph, GeometryKernel operation, editable NURBS, source/
schema/patch, physical Mock/runtime, mesh policy or new MCP operation is added.

Use a fixed, immutable descriptive snapshot of nine-by-three Cartesian poles
and positive weights. Its constructor accepts explicitly labelled millimetre
controls from the native boundary and stores only outward SI homogeneous
intervals. It supplies no surface/body builder or control-editing API. A separate
reference method reads the snapshot and certifies it against the caller's
validated canonical reference and quarter chart. Arbitrary edited data must pass
the same proof; constructor acceptance is not geometric admission.

## Native capture and numerical transport

The pinned OCCT diagnostic exports current post-sewing Geom_BezierSurface poles
in their ORIGINAL native millimetres, current weights, degree, nonperiodicity,
unit parameter bounds, forward face orientation and identity Location. Retain
SI witnesses separately. Do not multiply to rounded SI values before capture:
that would certify a rounded export rather than the native definition.

Use 17 significant decimal digits and the existing serde_json float_roundtrip
feature for binary64 transport. The Rust probe echoes the exact request data,
reference parameters, chart and budgets; the grader binds those to the current
native capture by binary64 data identity. This is transport identity, not a
geometric comparison. Geometry witnesses still use explicit tolerances.

For raw native pole q_mm, its exact physical value is q_mm/1000 metres. Construct
I::exact(q_mm)/I::exact(1000), then multiply by the exact positive weight with
outward arithmetic. Never treat the rounded product q_mm*0.001 as exact SI.
All returned position/derivative/Jacobian bounds use SI; weights and normal
errors are dimensionless, original u/v are dimensionless.

## Reuse the complete interval proof

Share P16-032's immutable homogeneous interval proof. Add its positional error
norm and requested metre tolerance to each cell's acceptance and global extrema.
The same complete deterministic cover must certify BOTH position and normal
precision, positive Jacobian/orientation and derivative errors. No sample-only
or partial certificate can succeed. Existing normal_bounds semantics remain.

Rational weights have a common positive gauge. Normalize the snapshot's entire
homogeneous net by ideal_W00/actual_W00 using outward interval arithmetic. The
ideal anchor interval encloses the exact ideal coefficient; each possible true
normalization is constant across u/v and leaves the native rational geometry
unchanged. Carry all normalization uncertainty into subsequent differences and
denominator/derivative bounds. A uniform positive weight rescaling must not be
mistaken for a different shape; nonuniform weight changes receive a real proof,
not ratio-only acceptance. Refuse nonpositive/invalid normalization.

Reuse delta homogeneous differences, original-coordinate differentiation,
positive denominator hulls, quantitative ideal Jacobian lower bound and cross
error identities from ADR-085. Expose the new position bound plus its existing
normal/Jacobian/alignment/derivative bounds and complete leaf count. Position
requests must be finite positive Length; normal requests strictly (0,1); leaf
budget 1..=16384 and depth 28. Exhaustion/nonfinite/unresolved bounds refuse with
no geometry mutation or certificate output.

## Live validation and refusals

Extend the existing native solid diagnostic rather than build another surrogate.
After native construction and current capture, run a bounded-stdin Rust example
on exactly those eight native surface snapshots and attach its recomputed
certificates. The fixture requests position 1e-12 m, normal 1e-9 and 4096 leaves
per chart. Existing control, current pcurve, topology, independent ideal/constant
volume, actual classifiers and equal-volume phase decoys remain required.

Independent Rust geometry tests use exact constant-chart derivatives/Jacobian
ranges and the separate square-root envelope across scales/charts/boundaries.
Reject collapsed/reversed/deformed, nonuniform-weight, wrong-unit, wrong-phase,
nonfinite or missing data, invalid budgets and unattainable position/normal
requests. Verify deterministic refusal without mutation and common weight-gauge
invariance. Synthetic report tests exercise transport/representation/certificate
refusals only. CI executes the actual native capture and Rust proof and retains
its report. Update API, architecture, example, MCP guide, skill and plans, then
all mandatory local checks and applicable exact-head CI before merge.

## What this certificate does and does not prove

It certifies the mathematical rational surface DEFINITION encoded by captured
current native poles/weights over the complete closed unit square, including
native-coordinate conversion error. Matching raw representation, identity
Location, bounds and face orientation are separate required capture gates.
The complete-cover proof now applies to these native surface definitions, not
just to the originally supplied controls. Approximate support/seam normal
implications follow at corresponding chart parameters as in ADR-085.

It does not bound every floating native evaluation algorithm, native trims or
pcurve reparameterization, 3D contact-edge coincidence, native integration,
exact G1, global embedding or whole-BREP fidelity. Snapshot evidence has no
persistent ownership/cache/source authority. Physical Mock/runtime, bounded
mesh and guarded source/ownership/cache/history/reader admission remain before
P16-008. No schema/migration/golden or runnable MCP eval is appropriate because
no authored operation is admitted.

Raw native pole JSON uses showpoint so signed zero remains a floating value
through Python transport. Echo comparison rejects booleans as numeric values
and checks binary64 identity including zero sign; geometry comparisons retain
their separate explicit tolerances.
