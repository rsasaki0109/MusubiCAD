# Native periodic-fillet diagnostic

P16-021 / [ADR-074](../adr/ADR-074-native-periodic-fillet-counterexamples.md)
reproduces a pinned OCCT counterexample. A successful diagnostic **does not admit
the curved fillet feature**. It validates a constant control, then confirms
measured failures of the tested nonconstant recipes against the ideal contract.

On Linux, after preparing the default OCCT-enabled workspace:

```sh
cargo test --workspace
python tools/test_periodic_fillet_probe.py
python tools/run_periodic_fillet_probe.py \
  --occt-root target/cadrum-occt-v800-x86_64-unknown-linux-gnu \
  --output target/periodic-fillet-diagnostic.json
```

The runner needs a C++17 compiler, the already prepared OCCT 8.0.0 headers/static
libraries, Cargo and Python. It compiles
`modules/kernel-occt/diagnostics/periodic_fillet.cpp` in a temporary directory.
That executable is separate from product binaries and exports no kernel API.
It runs the actual `opencad-geometry` reference-volume example, which calculates
ADR-073 bounds; no copied golden volume or mocked native output supplies the live
verdict. No SDK download, unsupported-host fallback or test skip is performed.

The output JSON includes `diagnostic_passed`, `candidate_admitted` (always false),
raw `native` and `reference` reports and `findings`. Validity is checked before
native output, and each profile records `brep_valid`, actual solid count, physical
measurement counts, native volume estimate in m³, estimate refinement delta in
m³, and `max_sampled_*_error_m` in metres. These error maxima concern measured
points, not a supremum bound. The reference report carries actual bounded ideal
body-volume endpoints in m³ and its explicit absolute width/budget evidence.

Six native profiles use a 10mm-radius, 20mm-tall cylinder: constant 1mm control;
0.5–1.5mm baseline with 65 radius samples; 257 samples; tighter builder settings;
C2 internal continuity; and a proper pi/2 rotation of the actual baseline body.
The last preserves volume while changing physical phase. Stored law samples
still concern the original builder and cannot prove the rotated output's phase.

Measured locally with the pinned Linux SDK on 2026-10-14:

| Recipe | Native volume estimate, mm³ | Maximum sampled top-contact error, µm |
|---|---:|---:|
| Constant control | 6270.002661669 | below 1e-8 |
| Baseline 65 samples | 6268.441657854 | 0.999359 |
| Dense 257 samples | 6268.441657529 | 0.999313 |
| Tighter builder parameters | 6268.441657484 | 0.999348 |
| C2 internal continuity | 6268.441657474 | 0.999348 |
| Baseline rotated pi/2 | 6268.441657854 | 707.117394 |

The independently calculated ideal nonconstant interval was
[6268.436280479, 6268.437280476] mm³, full width below 0.001mm³. Unrotated
nonconstant profiles also showed approximately 0.734µm sampled inferred-sphere
support error. Changing only the tested interpolation/settings did not remove
the discrepancy. This does not establish that no other OCCT construction can work.

The grader uses a 10nm geometric counterexample threshold and a separate
1e-15m³ volume comparison allowance. These are diagnostic thresholds, not accepted
production approximation tolerances. Native volume refinement stability is an
estimate, not a certified enclosure; the mathematical interval belongs only to
the ideal reference. One observed point outside a required geometric threshold
is sufficient to reject that recipe, whereas finite sampled agreement cannot
prove complete geometry, periodicity or source admission.

Invalid/nonfinite/incomplete reports, insufficient witnesses, failed controls,
changed counterexamples, missing tools/SDK or native failure produce a nonzero
exit. A disappeared discrepancy requires investigation rather than automatic
feature admission. Python tests use synthetic reports solely to exercise these
refusals; they are not native shape evidence. CI runs the live diagnostic after
the full workspace tests and uploads its JSON. P16-008 remains planned; actual
Mock/native construction, full approximation/mesh bounds and guarded source/
phase/cache/history/MCP acceptance remain separate work.


The rational-reference runner also performs P16-037 / ADR-090 current quarter
seam proofs. See [the seam API](periodic-fillet-reference.md#current-native-quarter-seam-boundary-certificates-p16-037)
and [example](../../examples/periodic_fillet_reference.md). Eight complete direct
position-gap certificates and ten real-process refusals supplement the existing
surface/end/side proofs. Native floating evaluation, paired normals/G1 and
whole-BREP/production admission remain unproved.
