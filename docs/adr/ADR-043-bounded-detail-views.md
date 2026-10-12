# ADR-043: Bounded parent-linked detail views

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P12-004

Add optional `DrawingView.detail`: parent view ID, rectangular min/max in projected
model metres, and explicit magnification >1. Parent must be an ordinary view on the
same sheet, with the same model reference and orthographic projection. Nested or
section parents are unsupported. Existing required view `scale` is retained for
legacy API consistency and validated against parent.scale × magnification (1e-9
relative tolerance); parent scale edits must update child scales in the same
DesignPatch. Rendering never silently rewrites authored values.

Project existing Mock/OCCT meshes through the current wireframe/visibility path,
clip line segments analytically with Liang–Barsky and preserve visibility. Center
the crop on the child sheet origin and apply its scale. No new geometry sampling
or curve fitting is introduced. Empty crops, missing/invalid parents, incompatible
projection/source and scale mismatches fail verified regeneration before applying.
Crop sides must be finite and >1e-9 model metres; zero-length clipped edges <=1e-9 m
are omitted. SVG shows the parent's crop rectangle and derived magnification label.

Author/edit/remove through existing drawing-view DesignPatch operations. Parent
removal with a remaining child is refused; child removal then parent succeeds.
State/snapshots/authoring preserve links and derive fresh segment counts. Schemas
and migration remain additive, defaulting old views to no detail; legacy bytes
are unchanged. Detail dimensions need explicit future semantic contour bindings,
so existing dimensions on detail views are refused. Curves inherit the current
mesh wireframe approximation; circular regions, arbitrary camera projections,
nested details and section-detail combinations are outside this bounded task.
