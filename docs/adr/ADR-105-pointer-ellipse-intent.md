# ADR-105: Backend-owned three-point ellipse intent

Status: Accepted with MCAD-P17-007 qualifying main merge
Date: 2026-10-17
Roadmap: MCAD-P17-007, after P17-006 / PR #155

## Decision

Add a creation-only `ellipse_axes` form to the existing shared sketch planner
and applicator: fresh `id`, explicit local length strings `center_x`, `center_y`,
`major_x`, `major_y`, `through_x`, `through_y`, and required `construction`.
Require the current document identity and revision even on global planes.
Inputs are constant unit-bearing expressions. Preserve existing numeric and
raw parametric Ellipse editing; this form creates no associative axis helper.

The native tool takes three clicks: centre, major-axis endpoint, then a point
ON the ellipse. In the major-axis frame let `a` be the major radius and `(x,y)`
the third point relative to the centre. The backend derives
`b = abs(y) / sqrt(1 - (x/a)^2)` and the local major-axis angle. A third click
is not merely a perpendicular width. Both the tool label and disposable SVG
preview use these semantics. The frontend sends original click coordinates,
never derived radii, angle, source entities or physical results.

This follows the shared-command and point-on-curve interaction documented by
SolidCraft at snapshot `07e263d7e95cac9a6851e48bffcb79f302aabfb8`,
`crates/ui-egui/src/sketch_tools.rs` and
`crates/engine/src/cmd/sketch_create.rs` (ellipse). Source was read as a design
reference; no upstream code/assets are incorporated or application run.

Use an explicit creation ambiguity guard: `k = 1 - (x/a)^2` must be finite
and greater than `1e-9`. Zero/near-endpoint/outer-span points refuse. This
dimensionless guard bounds the new inversion; it does not relax ADR-053's
existing finite `major >= minor > 1 nm`, major at most `1e6 m`, proper-plane
and normal-extrusion admission. All input coordinates retain the editor's
finite absolute `1e6 m` bound. Refuse unit/parameter, missing/foreign identity,
stale source/datum, axis/through-point and generated-ID failures atomically.
Do not silently swap major/minor axes, clamp dimensions or repair source.

Expand one intent into an ordinary centre Point (`id_center`) and existing
Ellipse (`id`). Preserve centre expressions, store derived radii as fixed
explicit-mm strings and local angle as fixed explicit radians in `[0,2*pi)`.
No unused persistent axis points, constraints, source tag or migration is added.
Construction marks the ellipse, following existing centre-point semantics.
The existing expanded budget stays 64 source operations (32 ellipses).
One guarded DesignPatch/apply/Undo/Redo covers both additions.

Keep original workplane association, owner, required assertions, strict
profile-consuming operations, native candidate verification, file publication
and opaque history in the shared gateway. Cancel/Escape/control/file/upstream
changes and late real IPC cannot publish half a gesture or restore old review.
An empty ellipse tool cannot fall back to unrelated numeric fields.

The native regression exposed the legacy +Z placeholder in ellipse display
vertex normals. Track existing whole native ellipse prisms and their proper
rigid copies for the same oriented-triangle normal path already used by
ADR-103 sectors. Preserve exact BREP, physical triangle geometry, face ownership
and sampling settings. This is required rendering correctness for this input
path; it adds no geometry admission or new sampler. Default Mock ellipse
tessellation remains an explicit refusal.

## Acceptance

- Independent source/physical oracle: centre `(2,-3)`, major endpoint `(26,15)`,
  through point `(14,21)` mm gives `a=30`, local `(x,y)=(24,12)`, `b=20` mm,
  `cos(theta)=.8`, `sin(theta)=.6`. Perpendicular projection would give 12 mm
  and must fail. Area is `600*pi mm2`, 6 mm prism volume `3600*pi mm3`;
  planar half-bounds `12*sqrt(5),2*sqrt(145)` mm and local COM `(2,-3,3)` mm.
- Genuine existing Mock ellipse records and native OCCT mass, area, centroid,
  absolute bounds on XY/YZ/XZ and tilted/offset datum. No synthetic mass and
  no claim of default Mock ellipse tessellation, which remains explicitly
  unsupported. Native fresh SI STL must prove closed outward geometry;
  retain analytic, mesh/placement and unit tolerances separately.
  Reuse ADR-053's named 30/20 mm probe: Mock area `1e-12 m2`, native area
  `5e-9 m2` (existing OCCT default surface quadrature), volume `1e-12 m3`,
  analytic positions `3e-7 m`. Tight native display meshes use 1% volume and
  `1e-5 m` COM; fresh default SI STL grading uses 1% volume and `1e-4 m`
  bounds/COM/directional supports. A sign-reflected ellipse with the same volume,
  COM and axis-aligned bounds must fail independent major/minor support checks.
- Construction-only drafts, readonly review, atomic history, late IDs,
  expanded caps, units/identity/revision/degeneracy/required-assertion refusal,
  empty/absent parameter graphs and cold/cache source preservation, packed
  and expanded round trips, unchanged legacy numeric editing.
- Actual new Linux Tauri and Windows WebView2 gesture/preview/review/apply/
  cancellation/late real IPC/upstream stale tests. Both drivers execute new
  checks unconditionally and workflow path policy includes the helper.
- Actual same-intent MCP/Agent parity, independent intended-source plus fresh
  native export grading, wrong-axis/orientation and equal-volume wrong-placement
  controls, example/API/architecture/guide/skill and feasible eval updates.
- All four required local checks and full applicable exact-head CI inventory.

Only a qualifying main merge may earn GUI 52 to 53, weighted CAD 53.65 to
53.80 (about 54%); agent operation remains 95. Credit covers the new usable
three-click ellipse lifecycle and atomic history, with no repeat geometry,
old numeric form, workplane, external-host or kernel-parity credit. No new
sampler, editable NURBS, arbitrary plane or unsupported ellipse consumer.
