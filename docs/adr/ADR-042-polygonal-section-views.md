# ADR-042: Exact polygonal section views

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P12-003

A drawing view may author a `section` containing a cutting plane (finite model
origin in metres and unit normal) and positive `hatch_spacing_m` in sheet metres.
Existing add/remove drawing-view DesignPatch operations author/edit the view;
verified regeneration must succeed before persistence. No section contour cache
is serialized. Legacy views default to no section, without a schema version bump.

GeometryKernel returns an exact polygonal section witness. OCCT intersects its
current solid with a plane using BRepAlgoAPI_Section with approximation disabled,
accepts only native line curves and returns their endpoints. The neutral loop
builder stitches within 1e-8 m, refuses open/branched/touching/intersecting contours,
canonicalizes edge/loop order and derives material area with even-odd nesting.
Area cross-products use a local contour origin to avoid translation cancellation.
There is a 4096-edge limit; positive loop/material areas must exceed 1e-16 m².
The plane must strictly cross body bounds; native coplanar-face cuts are refused,
including boundary cuts and conservatively some step-face cuts.
Mock computes exact axis-aligned box slices; neither path introduces sampling.

SVG renders the slice outlines and diagonal material hatch using even-odd fill,
leaving holes clear. Hatch perpendicular spacing is in sheet metres, independent
of view scale. Layout requires a plane-parallel orthographic view (normal projected
magnitude <=1e-9), finite coordinates, and positive hatch spacing >1e-6 m.
Resolved geometry/layout travels separately from authored Sheet data. Render APIs
refuse section views without their regenerated layouts. Section area/loop/edge
counts are exposed as fresh regeneration evidence in SI units.

This initial feature is a slice view: it does not draw projected geometry beyond
the plane, cutting arrows/labels on a parent view or drafting-standard cutaway
views. Curved contours, assembly sections, isometric/foreshortened views, and
annotations on section geometry are unsupported and refused. Section dimensions
need distinct semantic contour references in a future task; existing part edge
references must not silently dimension a different slice. No physical-host
validation is implied by the automated Linux tests.
