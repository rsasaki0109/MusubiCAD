# Ideal periodic reference area (P16-043)

[ADR-096](../adr/ADR-096-ideal-periodic-reference-area.md) adds the read-only
`CircularFilletReference::surface_area_bounds_m2(max_width_m2, max_cells)`.
The reference domain and authored phase remain ADR-072's. Both constant and
variable rolling-sphere envelopes are supported as ideal arithmetic.

`max_width_m2` is a finite positive absolute FULL TOTAL area enclosure width in
square metres, including all support contributions and roundoff. It is neither
a relative tolerance nor a mesh setting. `max_cells` admits 4..=262144 angular
leaves. Exhaustion, stalled refinement, invalid arithmetic and unattainable
precision return an error without a partial result or mutation.

`CircularFilletReferenceAreaBounds` returns:

- `body_lower_m2`, `body_upper_m2`: complete unsigned ideal boundary area.
- `blend_lower_m2`, `blend_upper_m2`: rolling-sphere envelope area.
- `retained_side_m2`, `retained_end_m2`, `bottom_m2`: ordered lower/upper support
  area pairs, with no artificial interface surface.
- `cells`: angular leaf count; zero only for identically authored constant radii.

The positive area Jacobian is integrated analytically over each characteristic
arc, then enclosed over theta with deterministic rational-chart subdivision and
private outward binary64 arithmetic. ADR-073's IEEE-754 nearest-rounding and
correctly rounded sqrt assumptions apply. No finite sampling convergence estimate
or libm trigonometry enters the proof. Explicit unequal radii retain the variable
path even one binary64 step apart. Widths scale in square metres.

For the 10 mm radius / 20 mm height cylinder, radii 0.5..1.5 mm, an explicit
`surface_area_bounds_m2(1e-7, 262144)` requests a total full width of 0.1 mm².
See the [example](../../examples/periodic_fillet_reference.md) and independent
[surface/analytic tests](../../modules/geometry/tests/circular_fillet_reference_area.rs).

The existing `circular_grid_reference` example and CI JSON now include an
`ideal_area` object with all bounds, width request, cell count and
`native_conversion_certified:false`. Each body's `area_m2` still comes from its
actual kernel mass query. These faceted areas differ from the ideal result;
neither a native conversion certificate nor a native-area discrepancy bound is implied.
This API has no document, schema, source, DesignPatch or MCP operation. Ideal
first moments, production variable fillets and their source/phase/history/eval
gates remain separate. No maturity credit is assigned.


P16-044 separately adds [stored-grid area differences](grid-reference-area-difference.md).
That comparison bounds the actual input triangles, counted with multiplicity;
it still does not certify native conversion or replace kernel area.
