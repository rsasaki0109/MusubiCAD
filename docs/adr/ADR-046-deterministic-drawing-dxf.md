# ADR-046: Deterministic single-sheet drawing DXF

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P12-007

Export a regenerated drawing sheet as bounded ASCII-tag DXF AC1027 (AutoCAD
2013), with UTF-8 text. Model space contains right-handed paper coordinates in
millimetres: sheet metres ×1000, including the authored view scale. Set INSUNITS
4 and MEASUREMENT 1; paper extents and a paper frame preserve the sheet size.
This is a drawing output, not a replacement for exact 3D STEP or a DXF importer.
Reject documents with multiple sheets rather than silently omitting sheets.

Use a small deterministic writer in the kernel-neutral drawing module, avoiding
another geometry dependency. Reuse regenerated meshes, dimension layouts,
polygonal section contours and existing title/detail layouts. Export LINE,
LWPOLYLINE, ARC and TEXT; use fixed named layers and millimetre hidden-line dash
patterns. Angular arcs use counterclockwise DXF angles and swap clockwise ray
ends. Labels retain derived model measurements even when view scale changes.
Dimensions are rendered primitives, not associative DXF DIMENSION records.
Native contour curves retain the existing drawing wireframe approximation.

Section hatches are explicit 45-degree LINE segments. Intersect each line
x-y = n * hatch_spacing_m * sqrt(2) - sheet.height_m with all closed section
loops using half-open crossings, sort intersections along the line, and pair
using even-odd material membership. This matches the SVG hatch phase and leaves
holes empty without consumer-specific HATCH behaviour. It is exact polygon/line
clipping, not a new curve-sampling approximation. Degenerate zero-length pairs
are omitted at 1e-12 sheet metres. Reject odd intersections and invalid loops.

Canonical traversal, sequential hexadecimal handles and fixed nine-decimal
millimetre values produce repeatable bytes; no clocks, GUIDs or inferred metadata.
Bound numeric values to finite ±1e9 mm, 200,000 entities, 100,000 hatch lines and
10 million edge/line checks. Reject interpreted DXF text control syntax (%%,
backslashes and caret codes) rather than changing authored text. Normal UTF-8
and single percent signs remain supported; consumer fonts govern appearance.

Regenerate all references before rendering and construct the complete buffer
before writing an output. Exports do not mutate the Design Graph or `.ocad`
schema. Keep Mock/OCCT analytic dimensions/volume and failure/source-preservation
tests; independently read exported artifacts with ezdxf and require an audit
with zero errors and zero repairs. Full DXF input and multi-sheet layouts remain
separate roadmap work.
