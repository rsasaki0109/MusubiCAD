# Drawing API

`opencad-drawing` stores drawing sheets as part of the Design Graph and derives
SVG output from referenced model meshes.

## Linear dimensions

`LinearDimension` defines an aligned measurement using explicit meter units:

- `id: DimensionId` (`dim:` prefix)
- `view_id: ViewId`
- either `start_model_m` and `end_model_m` (legacy fixed coordinates in meters),
  or `edge_ref` (a semantic edge in the view's referenced part)
- `offset_m`: perpendicular annotation offset in sheet meters

`LinearDimension::measured_length_m` returns the 3D measurement for resolved
points and an error for an unresolved semantic measurement. Serialized
dimension labels are intentionally unsupported, so displayed values cannot drift
from the model. `layout_linear_dimension` projects witness points using the
referenced view and returns `DimensionLayout` for renderers.

SVG export formats values in millimeters with two decimal places. Degenerate,
non-finite, missing-view, and projection-overlap cases return `OpenCadError`.

### Semantic edge dimensions (MCAD-P10-003)

```json
{
  "id": "dim:width",
  "view_id": "view:front",
  "edge_ref": "ref:edge:bracket_top_front",
  "offset_m": -0.01
}
```

This measures the chord between the referenced edge's current end vertices,
not arc length. Values use model meters regardless of view scale.
Closed/degenerate chords are rejected at `DIMENSION_LENGTH_TOLERANCE_M = 1e-9 m`.
Point fields and `edge_ref` are mutually exclusive. Semantic dimensions never
serialize cached coordinates; old point dimensions retain their wire form.

`resolve_linear_dimension` uses the referenced part's final active body and
P6-003 required edge provenance. It returns transient points plus provenance for
layout. Missing, ambiguous, consumed, duplicate or wrong-kind references fail,
naming the dimension and reference. Endpoint order is canonical.
`layout_linear_dimension` and `render_sheet_svg` refuse unresolved bindings.

CLI/Agent/MCP regeneration exposes `drawing_dimensions`, including sheet/view/
dimension IDs, `measured_length_m` and semantic `reference_provenance`. Verified
dry-run/apply, review and SVG export validate every sheet, even when rendering
only the first. Assembly semantic dimensions require a future instance-qualified
binding; legacy point dimensions remain supported there.
See [ADR-034](../adr/ADR-034-semantic-drawing-dimensions.md) and
[`semantic_bracket_drawing.ocad.d`](../../examples/semantic_bracket_drawing.ocad.d).

## Hidden lines

`classify_hidden_lines` returns `ClassifiedEdge` values with `LineVisibility`.
One source edge may return multiple ordered values when projected triangle
boundaries or triangle/edge depth crossings change visibility. Adjacent
triangles do not occlude their own edge, and same-face tessellation diagonals
are omitted.

The exported tolerance constants make units and numerical policy explicit:

- `HIDDEN_LINE_DEPTH_TOLERANCE_M = 1e-7 m`
- `HIDDEN_LINE_PROJECTION_TOLERANCE_M = 1e-9 m`
- `HIDDEN_LINE_PARAMETER_TOLERANCE = 1e-9` (dimensionless)
- `HIDDEN_LINE_BARYCENTRIC_TOLERANCE = 1e-9` (dimensionless)

Output order is deterministic by canonical source-edge key and increasing
split parameter. Equal adjacent visibility intervals are merged.

The cross-artifact SVG and review contract is fixed by the
[MCAD-P5-005 evidence manifest](golden-evidence.md).
