# ADR-041: Semantic radius and diameter dimensions

- Status: Accepted
- Date: 2026-10-11
- Task: MCAD-P12-002

Drawing radius and diameter annotations bind to one semantic edge on a referenced
part. Persist only the binding, radius/diameter kind, sheet leader direction and
extension in metres. Regeneration obtains a fresh exact full-circle witness from
GeometryKernel and derives the dimension; no cached center, radius or label is
written to the drawing. Missing, duplicated and ambiguous refs fail closed.

The private native OCCT BREP query matches unique edge endpoints within 1e-8 m,
requires GeomAbs_Circle and a complete 2π parameter interval within 1e-9 rad, then
returns model-space center, unit normal and radius in metres. No sampled fit is
used. The neutral Mock provides synthetic circle edge 5 with radius half its
body side for deterministic contract tests; its box edges 1–4 remain straight.

Layouts require a finite orthographic view parallel to the circle plane (normal
projection <= 1e-9). Radius lines start at the center; diameter lines span the
circle. Leaders extend beyond the circumference in sheet metres independent of
view scale; labels report model millimetres. Resolved layouts travel separately
from serializable Sheet data. Legacy drawing SVG entrypoints refuse radial
annotations without their regenerated layouts.

Current semantic discovery remains restricted to top-Z perimeter edges. Native
arcs, ellipses, splines, assembly instance references, isometric/foreshortened
views and endpoint-ambiguous circles are unsupported and refused. SVG annotation
is a bounded initial layout, without collision avoidance or drafting-standard
arrowheads. No schema version bump: additive omitted-empty radial collection,
legacy migration and deterministic round trips retain existing bytes.
