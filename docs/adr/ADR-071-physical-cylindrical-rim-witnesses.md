# ADR-071: Physical complete-cylinder rim witnesses

Status: Accepted for MCAD-P16-018's read-only runtime witness.
Date: 2026-10-14.

## Decision and scope

ADR-070 requires a complete circular rim belonging to an actual cylinder before
a curved fillet may consume it. Analytic Mock refuses generic edge and circle
queries; the separate legacy nonanalytic Mock returns an ID-derived synthetic
circle. Native discovery
uses endpoint chords, which are not a full circle's centre or circumference.
None of those paths is an acceptable physical rim witness.

Add `GeometryKernel::cylindrical_stock_rims(body, axis_hint)` returning
`CylindricalRimsGeometry { stock, start_edge_id, end_edge_id }`. The stock is the
existing ADR-065 full-cylinder witness oriented along the checked unit axis hint.
Both nonzero distinct edge IDs name actual current complete circles shared by
the corresponding cap and full lateral face. Reverse the hint to swap the
start/end meaning, not to change the body. A hint never proves geometry.

The query is read-only and kernel-neutral. It introduces no feature tag, schema,
MCP command, body construction or new sampling method. Default/unsupported
kernels refuse. This is a prerequisite, not completion or credit for P16-008.

## Physical backend admission

Mock uses only explicit analytic `Body::Cylinder` records and their real cap/
lateral face records. Allocate two deterministic, body-scoped physical rim IDs
in a separate namespace when storing a cylinder; do not derive coordinates from
IDs or reuse the default synthetic edge-5 convention. New transformed records
receive new rim IDs; stale IDs from another record refuse even for an equal
geometric cylinder. Repeated queries do not allocate records or change IDs.
The analytic `circular_edge_geometry` query accepts only these registered current
rims and obtains its circle from the actual record. Unsupported analytic bodies
and unregistered IDs refuse rather than falling through to synthetic geometry.
The default nonanalytic circle test convention remains separate.

OCCT first proves the ADR-065 full cylinder. Within the private BREP bridge, use
`TopoDS_Shape::IsSame`, which includes Location, for cap/lateral incidence.
A real circular extrusion test showed its two cap edges can share one TShape
at different Locations: cadrum's pointer-only `Edge::id()` aliases these rims
and cannot prove membership. Require one distinct rim per cap,
shared with the lateral face and absent from the opposite cap. Never assume a
private indexed-map ID is the public iterator ID or infer ownership from a seam
point. The pinned cadrum implementation enumerates unique edges using the same
`TopExp::MapShapes` ordering; return indices in that convention and verify actual
public edge/circle queries in placed and reversed fixtures. Query the candidate's
actual complete circle through the existing private
OCCT type/2*pi-extent check, then compare centre, radius and plane to the physical
stock. Compounds, partial/modified cylinders and ambiguous topology refuse.

Circle centres must agree with stock origin and origin+axis*height within 1nm
plus 64*machine-epsilon times world-coordinate scale. Radius agrees within 1nm;
unit normals are parallel up to the existing rotation tolerance (sign does not
define circle phase or cap winding). Stock retains its existing bounded-world,
positive-dimension and native topology/volume admission. Native endpoint matching
may refuse very short or precision-limited stock; do not loosen it to admit a
nearby circle. The witness does not certify a toleranced manufactured cylinder.

Neither the circle's native parameter axis nor a Mock circle's normal supplies
authored phase. ADR-070's source sketch defines that separately. This API does
not enable analytic Mock's generic edge discovery, endpoints or fillet selectors;
consumers must use the typed physical witness and circle query. Source semantic
producer/ref admission remains P16-008 work.

## Acceptance and compatibility

- Both backends return actual rim identities and circles on original stock and
  properly placed/transformed cylinders. Reverse hints swap current rim IDs.
- Independent fixtures check circle centre/plane/radius/circumference, cap and
  lateral ownership, cylinder volume/area/COM and actual mesh face membership.
  Native IDs must work with the existing native circle query and edge iterator.
- Analytic Mock circles cannot be read using legacy edge 5 or another body's
  IDs. Stale IDs from a transformed record refuse. Counting forwards the query.
- Invalid/nonunit/nonparallel hints, unknown bodies, cuboids, frusta, threads
  and compounds refuse. Failed queries preserve original mass, bounds and meshes;
  discovery is deterministic and repeated queries do not change physical state.
- Document the API/architecture and example fixture, and update authoring guide
  and skill to keep curved variable authoring unsupported. No `.ocad` change
  means no migration or round-trip changes. No new MCP eval can exercise this
  kernel-only query yet; P16-008 must add an actual source-aware eval.

All four mandatory local checks and applicable exact-head CI/jobs must pass.
Remaining native law/contact mapping, periodic smoothness, global exposure,
actual fillet solids/meshes and source/cache/reader/history gates are unchanged
from ADR-070 and need further evidence and accepted construction decisions.
