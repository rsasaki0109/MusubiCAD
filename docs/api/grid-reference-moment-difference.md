# Stored-grid first-moment and centroid comparisons

P16-046 / ADR-099 adds the read-only kernel-neutral
`grid.moment_comparison(CircularFilletGridMomentPrecision)` API.

`ideal: CircularFilletMomentPrecision` specifies full ideal volume width (m³),
full width of EACH first moment (m⁴), full width of EACH centroid coordinate
(`Length`), and the unchanged 4..=262144 leaf cap applied separately to the
volume/moment passes. See [ideal moments](ideal-periodic-reference-moments.md).
`maximum_absolute_first_moment_difference_m4` and
`maximum_absolute_centroid_difference: Length` are finite positive allowed
absolute errors in EACH coordinate. No relative error divides by symmetry zeros.

The immutable comparison exposes stored-grid signed volume flux, three signed
first-moment intervals, three positive-volume centroid quotient intervals,
`ideal()` with its volume/moment leaf counts, signed differences and per-axis
absolute difference upper bounds. Actual stored-f64 coordinates and triangle
order determine tetrahedral sums; the privately bound original reference
supplies ideal values. Callers cannot replace reference results. Every arithmetic
operation and interval subtraction/division rounds outward. Invalid precision,
work exhaustion, nonpositive volume lower bound, nonfinite arithmetic or either
error budget exceeded on any axis refuses with no mutation or partial result.
Equality to a computed upper error bound is accepted.

The comparison is of signed boundary integrals and algebraic quotients. It
adds no exact global embedding, native/placed/f32 conversion or source-feature
certificate. Physical interpretation requires the usual oriented embedded-shell
conditions. Both-kernel mass properties remain actual backend measurements,
compared separately with numerical allowances. P16-008 remains planned.

Run `cargo run -p opencad-kernel-occt --example circular_grid_reference` for
constant and variable `moment_comparison` reports, alongside independent
`ideal_moments` and actual `bodies` measurements. The existing CI comparison
step/artifact includes these fields; no new MCP operation or eval is claimed.
