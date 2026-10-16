# Guarded grid-faceted periodic cylinder fillet

P16-047 / ADR-100 demonstrates a numerical kernel operation on an actual complete
cylinder and its current end rim. The explicit phase frame determines the cosine
law. It rebuilds the complete body as planar facets, preserves the stock, and
returns new physical output facets in analytic Mock and OCCT.

```bash
cargo run --locked --quiet -p opencad-kernel-occt --example faceted_circular_fillet
```

The JSON contains a constant case and a rotated/translated genuinely variable
case, each run through both kernels. It reports actual volume/area/centroid,
bounds and owned facets, plus independently checked contact vertex positions
and unchanged stock. Canonical grid quality/discrepancy metadata remain separate
from native/placed/f32 measurements; their certificate flags stay false.

Explicit canonical requests are 4e-4 m position / 0.8 normal chord, 1e-11 m³
volume width, 1e-7 m² area width, 1e-12 m⁴ moment width and 1e-6 m centroid width.
Maximum absolute errors are 1e-7 m³ / 1e-5 m² / 1e-9 m⁴ / 2e-6 m, with the
latter two applied per coordinate. Integration caps remain 262144 per pass and
the stored mesh retains the existing 10000-triangle admission. Actual contact
vertices use an explicitly separate 1e-8 m numerical allowance.

The existing `circular_grid_reference` CI executable includes the same report as
`guarded_operations`. No `.ocad` or DesignPatch authoring operation is added by
this runtime example. P16-008 must separately admit the source, cache ownership,
compatibility/history and actual MCP evaluation. See
[the runtime API](../docs/api/faceted-circular-fillet-runtime.md).

The separately admitted persisted feature is now documented in
[the source example](faceted_circular_fillet_source.md) and ADR-101. The runtime
example above remains a transient geometry diagnostic, not a source document.
