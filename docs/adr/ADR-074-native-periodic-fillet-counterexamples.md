# ADR-074: Reproducible native periodic-fillet counterexamples

Status: Accepted for MCAD-P16-021's diagnostic only; no geometry admission.
Date: 2026-10-14.

## Decision

Make the pinned OCCT circular variable-fillet investigation reproducible in CI.
A standalone native diagnostic lives inside `modules/kernel-occt/diagnostics`;
a Python tool builds/runs it against the already prepared OCCT 8.0 SDK and grades
its SI measurements against ADR-073's actual Rust reference-volume enclosures.
This executable is not linked into the product and exposes no GeometryKernel,
BREP, source or MCP operation. P16-008 remains planned, with no maturity credit.

## Question and measurements

Use an actual R=10mm, H=20mm cylinder and uniquely owned complete top circular
rim. Test a constant 1mm control and the cosine 0.5–1.5mm fixture. Construct real
native fillets using the existing exploratory array recipe, validate the BREP,
count actual solids, and inspect the resulting top-plane/cylinder contact edges.
At 257 native-curve parameters, compare physical positions with the intended
cosine radius at their actual azimuth. Count both contacts; empty measurement
sets are an error, never zero error. On blend faces classify UV samples as IN,
calculate outward normals using face orientation, and infer supporting spheres
from r=(H-z)/(1-n_z) for 0.15<n_z<0.9. Measure actual cylinder-support and radius-law
residuals at inferred centre azimuth. These are finite witnesses, not global bounds.

Measure BRepGProp volume at two adaptive tolerances and report both the estimate
and refinement difference. The refinement difference is not a certified native
volume enclosure. Independently run the actual Rust `volume_bounds_m3` reference
example with an absolute full width of 1e-12m³. A native estimate outside that
interval is a diagnostic discrepancy, not a proof about exact native integration
error. Do not substitute the ideal interval for native mass.

Repeat the variable case with 257 law samples instead of 65, tighter builder
parameters, and C2 internal continuity. Keep the default rational representation.
A separate wrong-phase control properly rotates the actual baseline native body
by pi/2 around the cylinder axis. It preserves volume while changing contact
positions, demonstrating that scalar volume and the stored law alone cannot
establish authored phase. Stored-law measurements refer to the builder's original
law; the rotated output must still be rejected against authored phase zero.

## Diagnostic verdict and refusal

The fixed fixture uses a 10nm contact/support/radius threshold and a separate
1e-15m³ volume-estimate comparison allowance. These are counterexample-detection
thresholds, not accepted production approximation tolerances. Require a valid
single solid, finite SI measurements, complete uniquely named profiles and
nonempty top/side/interior sets. The constant control must agree with its bounded
ideal volume and all measured geometric thresholds. Every nonconstant recipe
must exhibit an actual geometric counterexample and an out-of-interval volume
estimate beyond the stated allowance. Dense/tight/C2 profiles do not establish
that no other builder setting could work; record only the tested recipes.
The rotated control must show a strong phase discrepancy despite a volume
matching the baseline estimate within the separate comparison allowance.

Success means the investigation reproduced its controls and counterexamples;
it never means the candidate feature is supported. If a future pinned kernel
improves and the counterexample disappears, fail the diagnostic for review instead
of treating a finite sampled match as full admission. Unsupported inputs, missing
SDK/compiler, failed build/native execution, malformed/nonfinite/incomplete
reports or failed controls return a nonzero tool status. No fabricated fallback
measurements, production code mutation or silent skip.

## Implementation boundaries

Run this diagnostic on Linux with the prepared pinned headers/static libraries
and a C++17 compiler. Other hosts are not claimed verified by this tool. No new
download or dependency is introduced. Keep all concrete OCCT types in the native
kernel module; only explicit SI JSON values cross the diagnostic boundary.
The Rust reference example has no new core API or geometry algorithm. The Python
runner performs explicit external I/O; core/reference modules remain pure.

Preliminary isolated SDK experiments also found that calling Add with a custom
Law_Function did not yield a usable path in this pinned binary (process crash).
Replacing the law after Simulate did not establish that the custom law survived
Build; its resulting stored-law/geometry measurements matched the array recipe.
Neither path is used in the reproducible executable or admitted as exact cosine
construction. Upstream master source was inspected only as an API hint, not as
proof of the pinned binary's implementation.

Add the live diagnostic and report-contract tests to CI. Independent Python
negative tests cover malformed reports, absent measurements, misleading mass-only
or wrong-phase controls and vanished counterexamples; synthetic parser fixtures
are explicitly not native geometry evidence. Update plans, authoring guide, skill,
architecture/API diagnostic notes and examples. No schema, migration, golden or
runnable MCP eval applies. Four mandatory local checks and all applicable exact-
head workflows/jobs still gate merge.

## Remaining decisions

The observed contact discrepancy is approximately 1 micrometre, while changing
the sampled radius interpolation or tested builder tolerances barely changes it.
The pinned builder's current recipe therefore does not satisfy the ideal contract.
Do not enable it by raising tolerances to the measured residual or by replacing
its mass. A future construction/approximation ADR must establish the actual
surface's semantics, phase, periodicity, complete error bounds, real Mock/native
geometry and bounded closed meshes before guarded P16-008 source implementation.
