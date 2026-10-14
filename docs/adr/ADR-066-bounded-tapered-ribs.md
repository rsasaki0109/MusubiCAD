# ADR-066: Bounded square-ended tapered ribs

Status: Accepted for MCAD-P16-013 / MCAD-P16-006.
Date: 2026-10-14.

P16-005 is complete in PR #110; P16-013 is registered in the roadmap before
implementation. Runtime admission and later graph-source admission are distinct.

## Context

The existing rib offsets an open line by constant thickness, extrudes the
rectangle and calls generic Union. Analytic Mock admits generic Subtract only;
its real Stock/Cylinder/Frustum records cannot prove tapered joining. Changing
only a serialized angle or using synthetic Mock volume would not supply the
physical rib family requested by P16-006.

## Decision and admission

Runtime P16-013 adds one explicit kernel-neutral rectangular-stock/rib operation;
source P16-006 later adds a distinct closed tapered_rib tag and protected authoring.
Runtime requires an actual uncut rectangular stock solid and a uniquely picked
outward rectangular face with the existing body-owned stock witness. Geometry
alone cannot distinguish authored extrusion ends from side faces or imported
boxes: runtime does not claim that provenance. P16-006 separately requires an
original authored rectangle extrusion and one of its actual extrusion ends. The straight centreline is parallel/antiparallel
to a witnessed in-plane stock axis. A full rectangular base footprint of length L
and thickness T0 is strictly inside that face, with material margins >1nm. No
oblique or curved intersection, partial/disconnected footprint, cut/modified
physical stock or general mesh-inferred stock. Source admission additionally
refuses imports, fusions and patterns even when their shape resembles a box.

The neutral base is z=0; z points outward. Explicit signed angle theta gives
T(z)=T0-2*z*tan(theta), T1=T0-2*H*tan(theta). End planes stay square at s=±L/2;
centreline stays fixed. Positive angle narrows outward, negative widens. No
implicit end/run/fillet representation. SI metres/radians, finite unit orthogonal
frames, dimensions (1nm,1000m], |theta| in [1e-8,pi/4], T1>1nm, actual resolvable
width delta >1nm, finite world vertices within 1e6m and added volume greater than 64*EPSILON times
stock volume. No author-input clamping. Axis hints select actual witnessed axes;
base centre is reconstructed on the actual plane, with arithmetic roundoff only.

Build a genuine trapezoidal prism at the stock face and an actual connected union.
Mock stores a separate RibbedStock physical record, not generic Boolean ID maths;
unsupported downstream edits on the new record refuse. Native constructs the
exact prism/union behind an OCCT-exception-catching private BREP bridge and
requires a valid single solid with 11 planar faces, 24 edges and 16 vertices.
All eight stock and eight contact/top vertices must match their exact expected
points within 1nm plus arithmetic coordinate roundoff. This prevents a kernel
tolerance from silently erasing or snapping an admitted addition. Constructor
failures publish no output handle.
Original source body remains immutable. Runtime reconstructs no fake ancestry.

## Independent measurements

Added volume L*H*(T0+T1)/2; outward axial COM H*(T0+2*T1)/(3*(T0+T1)). First
moments in the two in-plane axes remain at footprint centre. Combine stock and
rib first moments without double-counting the zero-volume shared interface.
For area, remove the base footprint L*T0 from stock exterior and omit rib bottom;
add top L*T1, two square trapezoidal ends H*(T0+T1)/2 each, and two long faces
L*sqrt(H²+((T1-T0)/2)²). Bounds include all stock and rib endpoint vertices.
Independent tests also integrate T(z), actual closed exported triangles and
contact widths, outward planes/normals and absence of internal interface faces.
Mock/native agreement alone cannot grade geometry.

## Exact planar mesh

Partition the stock cap on the footprint's exact in-plane side coordinates.
Emit the exterior rectangles around the contact hole and all outer stock faces
with matching boundary subdivisions; omit contact interior. The five exterior
rib faces use their actual endpoint vertices and compatible boundary edges.
No angular/curved sampling or near-point welding; only deterministic exact planar
partitions. Preserve existing quality validation, vertex/triangle budgets,
finite coordinates, actual f32 representability error, nondegenerate triangles,
paired opposite geometric edges and positive signed volume. Insufficient
precision or budgets refuse with no looser retry. Each in-plane grid has exactly four ordered coordinates (stock bounds and
footprint bounds), yielding at most 3×3 cells per stock cap. Omit only the
contact cell on the entry cap; partition the opposite cap and each side wall
at the same coordinates. Five unsplit rib quads share the contact rectangle
edges. This admitted family has 68 triangles before per-face vertex duplication
(and at most 204 vertices), 11 physical face IDs and no T-junctions. Reversed
face/line orientations change winding and placement, not partitions.

## Source, ownership, compatibility and acceptance

P16-006 requires original authored right-rectangle/distance/NewBody stock and original authored
straight open-line source, owned end face, explicit base thickness/height/signed
angle and square end discriminant. Strict Length/Angle kinds and expressions,
source/world witness comparisons, typed edits, graph dependencies, physically
proved output producers and cache checks use existing DesignPatch/transaction/
mandatory native verification. Distinct closed tag makes old readers refuse;
both schemas, no-inference migration, expanded/packed/authoring round trips and
canonical history accompany it. Failed source/ref/unit/range/cache/candidate
edits preserve source, previous output, history and artifacts.

P16-013 tests actual Mock/native volume/COM/area/bounds/contact/closed topology,
signed angles, all physical stock face orientations, reversed line direction, rigid placements,
source preservation and unsupported/invalid/quality failures. SDK example/API/
architecture/guide/skill mark runtime-only boundary and earn no score. P16-006
adds feature source/schema/history/example/guide/skill and independently graded
actual MCP eval, then earns partial fixed-rubric credit only after main merge.
Existing constant-width rib, generic loft/helix/planar draft and infrastructure
get no repeated credit; general curved-intersection rib remains unsupported.
