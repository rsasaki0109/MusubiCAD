# ADR-054: Semantic cross-sketch projection

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P13-005

## Context and ownership

External geometry must follow an explicit source dependency. Copying projected
coordinates into authored sketches loses that association. SolidCraft's shared
command/source identity practices inform this graph-owned feature design.

## Decision

Add a `projected_sketch` feature storing a destination sketch ID, explicit
Global/Custom workplane and ordered source curve bindings. Each binding names
an upstream sketch feature and stable entity ID, a fresh destination curve ID,
explicit destination point IDs and a construction flag. The graph owns these
bindings; generated coordinates are derived FeatureOutput.sketch only, never
saved in OcadDocument.sketches. Existing consumer features read this output via
RegenContext.sketch_for_feature. Source-feature dependencies are explicit,
including chained projected sketches; cycles fail before execution.

Initial sources are stored/prepared lines and full circles/ellipses in explicit
Global/Custom planes. Body-edge picks, Datum/FaceRef planes, arcs and spline
projection are deferred. Source lookup must find exactly one curve and each
point; missing/ambiguous/suppressed sources refuse. Rectangle-generated edges
retain their caller-defined semantic IDs. Destination point sharing requires
the same semantic source vertex identity; coincident different vertices do not
merge. Duplicate destination curve IDs and collapsed projections refuse.

Projection is orthogonal world-to-plane mapping in metres. Circle/ellipse
projection uses the exact 2x2 covariance eigensystem, with minor eigenvalue
computed from determinant squared / major eigenvalue to avoid cancellation.
A tilted circle becomes an exact ellipse; edge-on conics refuse. No sampling,
closest-pick inference or topology repair. Explicit plane frames must be
finite, nondegenerate and orthogonal. Spatial tolerance is 1 nm; no coordinate
rounding becomes document truth. Generated curves keep stable destination IDs.

Both kernels consume the existing line/circle/ellipse neutral wire. A projected
ellipse uses the real analytic Mock prism and exact native ellipse extrusion;
other legacy Mock solids remain synthetic and must be disclosed in evidence.
Source parameter/plane/ID edits invalidate the dependency cache. Failure must
preserve previous outputs and verified document/history transactions.

All authoring/editing uses existing DesignPatch AddFeature/ReplaceFeatureDefinition
commands. Additive feature/derived-output tags require schema/migration and old/
new round trips. Resolved coordinates are not a migration artefact or source.
No OCCT types, hidden I/O or new dependencies enter sketch/graph modules.

## Named validation

A 30 mm source circle projected to a plane tilted 30 degrees has semiaxes
30 mm and 30*cos(30) mm; 6 mm extrusion volume is
pi*.03^2*cos(pi/6)*.006 m3. Verify dimensions/volume in Mock and OCCT, source
radius/plane changes with warm cache and stable IDs, deterministic repeats,
line-loop point identity, chained projections, missing/duplicate/suppressed
sources, cycles, edge-on/invalid-plane refusal and rollback. Add an example,
MCP guide/skill/eval and independent source-binding/volume grading.

## Limits

This is associated cross-sketch geometry, not general BREP external projection
or an editable copy of the generated sketch. Edit its source or graph binding.
Freeform output editing and additional source/plane kinds need scoped follow-up.
