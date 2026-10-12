# ADR-048: Explicit-unit planar DXF sketch import

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P12-009

Parse bounded text DXF at the file-format boundary and author ordinary sketch
entities through DesignPatch. Support planar LINE, CIRCLE, ARC and straight
LWPOLYLINE geometry from explicitly selected layers. Inspection reports version,
declared units and layer/entity counts before import. Selected unsupported
entities, nonzero thickness/width/bulge, nonplanar elevation or non-default OCS
fail; unselected layers are omitted only by the caller's explicit selection.
No INSERT expansion, spline/ellipse approximation, binary DXF or style import.
Numeric admission tolerances: 1 nm planarity/thickness/width, 1e-12 OCS and
1e-15 dimensionless zero-bulge tolerance (at the coordinate bounds below).

Accept metre/mm/cm/inch INSUNITS codes. Missing/unassigned units require an explicit
source unit; a supplied unit conflicting with declared units fails. Convert all
coordinates/radii to SI metres and DXF degrees to radians exactly once. Imported
coordinates are literal design inputs; units in the destination metadata do not
rescale them. Drawing DXF output contains scaled paper coordinates, so importing
it does not recover the original model scale or semantic dimension bindings.

Use a fixed 1e-9 metre grid for shared point identity and coordinate normalization.
This explicit endpoint quantization joins repeated line/polyline/arc endpoints,
not curve sampling. Retain analytic circles/arcs, with CCW arc spans and endpoint
IDs; reject collapsed segments/arcs rather than silently losing geometry.
Stable IDs derive from the caller's sketch ID and canonical input record order.
Bound to 16 MiB input, 500,000 tag pairs, 50,000 selected primitives and finite
coordinates/radii within 1e6 m. No network/system-font or implicit file access in
parsing. Malformed section/tag/unit/geometry data fails before document changes.

Build AddSketch/AddSketchEntity/AddFeature operations, then use existing candidate
validation, regeneration/required-assertion verification and transactional apply
before persistence. Bare sketches are valid staged parts; closed/open profiles
are consumed under existing feature rules. No new `.ocad` representation or
migration is required: source geometry becomes authored sketch data, not a linked
external DXF cache. JSON round trips and authoring replay retain the contract.

Test analytic SI dimensions and circle/arc geometry, unit conversions/conflicts,
shared-point closure, unsupported/ambiguous input refusal, Mock/OCCT extrusion
volume/dimensions, failed import isolation, CLI/Agent/MCP and deterministic examples.
Update guide/skill and add a feasible eval task.
