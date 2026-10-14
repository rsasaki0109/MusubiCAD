# Physical cylinder rim queries

P16-018 implements [ADR-071](../adr/ADR-071-physical-cylindrical-rim-witnesses.md)
in explicit analytic Mock and native OCCT. The read-only Rust GeometryKernel API
`cylindrical_stock_rims(&body, axis_hint)` returns `CylindricalRimsGeometry`:

- `stock`: the ADR-065 actual full-cylinder witness, oriented along the checked
  finite unit parallel axis hint, with current start/end/lateral face IDs.
- `start_edge_id`, `end_edge_id`: distinct nonzero current complete-circle rim
  IDs shared by that cap and the lateral face. Reverse the hint to swap ends.

Call `circular_edge_geometry(&body, rim_id)` for the actual centre/plane/radius.
Rim centres are at stock origin and origin+axis*height; circumference is 2*pi*R.
All lengths and coordinates are metres. Normal sign indicates a circle plane,
not source phase or cap winding. Source sketch placement defines authored phase.
`validate_circles` checks centres within 1nm plus coordinate roundoff, radii within
1nm and plane alignment within the existing rotation tolerance; these data checks
alone do not prove ownership. The backend query supplies the physical proof.

IDs are scoped to the current body/backend. Mock stores physical rim IDs separate
from synthetic edge 5 and allocates fresh ones on transformed records. Unknown,
stale or another record's IDs refuse. Native IDs are current 1-based unique-edge
iterator indices and may numerically repeat on another body; an integer alone
does not identify its owner. Native incidence includes Location: two extrusion
caps can share a TShape and still be different edges.

Synthetic default Mock, unsupported shapes, compounds, frusta, threads, invalid
axis hints and ambiguous/precision-limited native topology refuse. Analytic Mock
now admits this typed witness and its actual complete-circle query; generic edge
discovery/endpoints/fillet selectors remain unsupported. No mesh silhouettes,
endpoint chords, closest-face fallback or fabricated topology history is used.
Queries do not change source, body properties or stored mesh/IDs.

See the [independently measured fixture](../../examples/cylindrical_rim_witness.md).
This kernel prerequisite introduces no `.ocad` field, source feature, DesignPatch
operation or MCP command. Circular variable fillet authoring remains planned in
P16-008; ADR-070's native law/contact and construction gates still apply.
