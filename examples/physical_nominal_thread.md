# Physical nominal metric thread SDK

Run either implementation on the same input:

```sh
cargo run -p opencad-geometry --example physical_nominal_thread
cargo run -p opencad-kernel-occt --example physical_nominal_thread
```

The [shared example](nominal_thread_sdk.rs) creates a real 6mm-diameter,
8mm-length circle extrusion, then cuts an M6×1 right-handed basic flat-root
envelope with square ends. Original stock remains unchanged. Ideal thread volume
183.657632385mm³, versus original stock226.194671058mm³; roots/crest and meridional
widths follow [ADR-067](../docs/adr/ADR-067-nominal-metric-thread-profile.md).
Density2700kg/m³ is explicit. Mock computes the analytic envelope and exports an
actual closed approximated mesh; OCCT builds the per-turn valley tool and measures
the real cut solid. Their mesh subdivision/face counts need not match.

Left hand and explicit phase-axis/rigid placement are admitted in
[ADR-068](../docs/adr/ADR-068-physical-nominal-metric-threads.md). This is a kernel
SDK example, not a `.ocad` feature or MCP operation. P16-007 must still supply
source ownership, closed schemas/reader admission, DesignPatch/history and actual
eval. No fit class, rounded production root, lead-in/runout or manufacturing
compliance is claimed. Unsupported precision/geometry/quality refuses.
