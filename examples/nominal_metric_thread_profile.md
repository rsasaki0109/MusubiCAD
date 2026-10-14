# Nominal metric thread profile calculation

Run `cargo run -p opencad-geometry --example nominal_metric_thread`.
This SDK example calculates an M6×1, right-handed, eight-pitch nominal ISO metric
basic flat-root profile: 6 mm major diameter, 1 mm pitch, 8 mm length,
4.917468245269 mm basic minor diameter and 5.350480947162 mm pitch diameter.
The meridional crest flat is 0.125 mm, root flat 0.25 mm and each flank spans
0.3125 mm axially. Both ends are square clipping planes of the noncircular
section. `radius_at` evaluates the declared right/left screw phase in SI units.

This is arithmetic infrastructure for P16-007. It creates no Mock/OCCT solid,
feature, `.ocad`, export or MCP operation. The ideal volume is an independent
target for later physical measurements, not an exported-body result. Rounded
external roots, manufactured tolerance/fit classes, lead-ins and runout are
unsupported; the flat-root diameter must not be labelled rounded external d3.
See [ADR-067](../docs/adr/ADR-067-nominal-metric-thread-profile.md).
