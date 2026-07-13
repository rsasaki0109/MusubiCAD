# Drawing API

`opencad-drawing` stores drawing sheets as part of the Design Graph and derives
SVG output from referenced model meshes.

## Linear dimensions

`LinearDimension` defines an aligned measurement using explicit meter units:

- `id: DimensionId` (`dim:` prefix)
- `view_id: ViewId`
- `start_model_m` and `end_model_m`: referenced-model coordinates in meters
- `offset_m`: perpendicular annotation offset in sheet meters

`LinearDimension::measured_length_m` derives the 3D measurement. Serialized
dimension labels are intentionally unsupported, so displayed values cannot drift
from the model. `layout_linear_dimension` projects witness points using the
referenced view and returns `DimensionLayout` for renderers.

SVG export formats values in millimeters with two decimal places. Degenerate,
non-finite, missing-view, and projection-overlap cases return `OpenCadError`.

## Hidden lines

`classify_hidden_lines` returns `ClassifiedEdge` values with `LineVisibility`.
Depth comparisons use `HIDDEN_LINE_DEPTH_TOLERANCE_M` (`1e-7 m`).
