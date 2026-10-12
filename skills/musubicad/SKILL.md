---
name: musubicad
description: Parametric 3D CAD for agents. Create and iteratively edit mechanical parts, assemblies, and drawings (brackets, enclosures, housings, robot links) as a Design Graph with named, unit-bearing parameters; every change is dry-run, regenerated, and verified before it is written; export STEP for CAD/CAM and STL for 3D printing. Use when the user asks to design, modify, resize, review, or export a mechanical part or assembly, or mentions MusubiCAD, .ocad, or .ocad.d files.
license: MIT OR Apache-2.0
---

# MusubiCAD

MusubiCAD is a parametric CAD whose source of truth is a **Design Graph**:
named parameters, constrained sketches, and features. You never write geometry
code. You send typed **DesignPatch** operations; MusubiCAD validates them,
regenerates the solid with OpenCASCADE, checks what you said the change should
do, and only then writes the document.

## 0. Check the setup

Run `musubicad version`. It must print `musubicad 0.2.0` or newer.

If the command is missing, install the prebuilt binary (no build, one file):

- macOS / Linux: `curl -fsSL https://raw.githubusercontent.com/rsasaki0109/MusubiCAD/main/install.sh | sh`
- Windows PowerShell: `irm https://raw.githubusercontent.com/rsasaki0109/MusubiCAD/main/install.ps1 | iex`

Ask the user before running an installer. If the `musubicad` MCP tools
(`patch_dry_run`, `patch_apply`, ...) are not available in this session, use
the CLI commands in section 7 instead; they enforce the same checks.

## 1. Rules

1. **Never edit files inside a `*.ocad.d/` directory by hand.** Every change is
   a DesignPatch. Hand edits break checksums and bypass verification.
2. **Make every meaningful dimension a parameter** (`param:bore_diameter =
   "8 mm"`) and drive sketches and features from it. Then the next request
   ("make the bore 10 mm") is a single `set_parameter`.
3. **Units are explicit.** Expressions carry units: `"8 mm"`, `"thickness * 2"`,
   `"30 deg"`. Raw stored numbers are SI: metres and radians (`0.005` = 5 mm).
4. **Declare intent as `expected_effects`** so the change is checked against
   what you meant, not just against "it still regenerates".
5. **Dry-run until `verification.passed` is `true`, show the user the diff,
   then apply.** `patch_apply` refuses unverified patches and leaves the
   document unchanged.

## 2. Edit an existing design (the common case)

1. Find the parameter: `query_document` with `{"kind": "list_parameters"}`
   (CLI: `musubicad params <doc> --json`). To see what a parameter drives and
   what an edit would change: `{"kind": "inspect_parameter", "id": "param:bore_diameter"}`.
2. Write the patch:

   ```json
   {
     "intent": "Reduce the bearing bore to 8 mm",
     "operations": [
       { "type": "set_parameter", "id": "param:bore_diameter", "expr": "8 mm" }
     ],
     "expected_effects": [
       { "type": "parameter_expr_equals", "id": "param:bore_diameter", "expr": "8 mm" },
       { "type": "mass_delta_kg", "min": 0.0, "max": 0.05 }
     ]
   }
   ```

   Expected effects: `parameter_expr_equals {id, expr}`,
   `mass_delta_kg {min, max}` (inclusive, kilograms; a smaller hole adds
   mass), `no_assembly_interference`, `drawing_changed {expected}`.
3. `patch_dry_run` with `{path, patch}`. Read:
   - `validation.messages`: errors name every missing or dependent ID.
   - `verification.passed`; on failure `verification.error` names the
     feature that stopped regenerating (for example
     `feature 'feature:bearing_bore': ... got 0` means the cut removed the
     whole solid) or the expected effect that did not hold. When the
     patched part does not regenerate, `verification.regeneration_failure`
     names the failing node, the features it blocks, and the body it was
     building on; the failing feature may be downstream of the one you edited.
   - If a document on disk already fails to regenerate, `inspect_regeneration`
     names the first failing sketch or feature, the features it blocks, and
     the body it was building on, without writing anything.
   - `verification.diff.summary` and `verification.diff.geometry`
     (`mass_before`/`mass_after` in kg, volumes in m³),
     `verification.geometry.before_bounds_m`/`after_bounds_m`.
4. Tell the user what will change in one or two lines, with units, for
   example: "bore_diameter 18 mm → 8 mm; mass 156.00 g → 163.72 g; the part
   regenerates and the expected mass increase holds."
5. `patch_apply` with the same `{path, patch}`. It re-verifies and writes.
   If it is refused, nothing was written; fix the patch and dry-run again.

If the dimension the user names is not a parameter yet, add it and wire it in
the same patch: `add_parameter`, then `set_feature_expr` (or a sketch
constraint `expr`) that uses its name.

## 3. Create a new design

1. `new_document` with `{path, kind: "part" | "assembly" | "drawing",
   document_id: "doc:<name>", name}`. It never overwrites.
2. Read the MCP resource `musubicad://guide/structural-patch` before
   authoring, and `musubicad://schema/design-patch` for exact JSON shapes.
   `authoring_patch` on any existing document prints the patch that rebuilds
   it, which is the most reliable syntax example. `musubicad new <path>
   <template>` creates sample documents (`bracket`, `bearing-carrier`,
   `robot-joint`, `robot-arm`, `assembly`, `drawing`, ...).
3. Author in this order: parameters, sketches with entities and constraints
   (constrain every rectangle edge horizontal or vertical and dimension it
   from a parameter), then features (extrude, hole, revolve, fillet, chamfer,
   shell, patterns, mirror, loft, sweep, helix sweep).
4. You may split authoring into several patches. A step that only adds
   sketches verifies against an empty solid; that is expected.

## 4. Assemblies

Components reference part documents (`source_path`), instances place them,
and mates (`concentric`, `coincident`, `distance`, `angle`, `parallel`,
`ground`) relate connectors. Robot joints (`add_joint`, `set_joint`,
`remove_joint`) put a motion type and limits on a concentric mate, for
example a `revolute` elbow with `lower_rad`, `upper_rad`, `effort_n_m`, and
`velocity_rad_s`; zero is the current pose, and URDF export writes them. After
you add or change a robot joint, re-export the URDF and run `reach` for the
tool target so the kinematic tree and reach residual are checked from the same
Design Graph.
Add `no_assembly_interference` to the expected
effects of any patch that moves parts; `verification.geometry.after_interference_count`
reports the count. To check every joint position rather than only the current
pose, add a required `motion_interference_at_most` assertion
(`{"max_count": 0, "samples_per_joint": 13}`). A refusal names the joint's
instance, its angle, and the colliding pairs; fix the design (narrow a limit,
shorten a link) and verify again. Sampling moves one joint at a time with
others at rest; it does not certify continuous motion or combined poses.
`examples/six_axis_arm.ocad.d` includes narrowed limits and a required
78-pose clearance check; regenerate its URDF, GIF and viewer together with
`bash docs/assets/generate-six-axis.sh`.

## 5. Look, review, and export

- `preview_document` with `{path, view}` returns a PNG you can see (no GPU
  needed; views `iso`, `front`, `top`, `right`) plus bounds in mm. Look at
  the part after authoring or after a change that alters its shape, and
  describe what you see before claiming it matches the request.

- `review_patch` with `{path, patch, output_dir}` writes `review.html`,
  `review.json`, and, when a GPU is available, before/after PNGs and a GIF
  (`images_skipped` explains a missing image). Offer the HTML path to the user.
- `export_document` with `{path, output}`: the extension picks the format.
  `.step` for other CAD/CAM tools (millimetres), `.3mf` for slicers and 3D
  printing (closed, millimetre meshes), `.stl` for older tools, `.glb` for viewers and the web,
  `.svg` / `.dxf` for single drawing sheets, `.pdf` for multi-page vector drawings, `.urdf` for an assembly as a robot
  description (simulators such as MuJoCo): the grounded instance is the root
  link; a concentric mate becomes the joint declared on it (`revolute` or
  `prismatic` with its limits, or `fixed`), or a `continuous` joint when none
  is declared; other mates become fixed joints; and one STL per part is
  written next to the URDF. Re-export after every
  design change instead of editing the URDF.
- `.html` exports an assembly as one self-contained web page with a slider
  per joint; offer it when the user wants to show the robot to someone.
- `import_step` brings a STEP solid (for example a purchased motor) into a
  part as a fixed body, a join, or a cut.

## Robots: reach and motion

- `reach` with `{path, tool, point_m, target_m}` answers whether a tool
  point (for example the gripper tip) reaches a world target within the
  declared joint limits, and how far short it falls. It needs no geometry
  kernel, so ask it before and after a change meant to extend reach. A
  longer link usually also needs its connector moved (and the downstream
  instances carried along) in the assembly.
- `animate_joints` writes a GIF of the assembly moving through poses you
  give (`"shoulder=60deg,elbow=-45deg"`); poses past a limit are refused.
- `animate_sweep` regenerates a design across one parameter's range; a
  value that breaks the model stops the sweep and is named, so it doubles
  as a robustness check. Use `style: "studio"` and `aspect: "1:1"` or
  `"16:9"` when the user wants something to share.

## Semantic drawing dimensions

For a dimension that must follow model edits, use `add_drawing_dimension` with
`{id, view_id, edge_ref: "ref:edge:<name>", offset_m}`. The edge reference belongs
to the part referenced by that drawing view; inspect that part's semantic refs
before authoring. Never use a projected segment index or invent a kernel ID.
The measurement is the distance between current edge end vertices in model
meters, not arc length. Include no fixed point coordinates with `edge_ref`.

Dry-run and apply verify every drawing sheet. Missing/ambiguous/consumed refs
fail with the dimension and ref IDs; repair the binding rather than substituting
fixed coordinates. After verified apply, inspect the SVG and
`regen_document.drawing_dimensions` for resolved lengths and provenance.
Assembly instance bindings and radius/diameter dimensions are not supported by
this linear edge measurement. See `examples/semantic_bracket_drawing.ocad.d` and
`examples/agent/semantic_bracket_dimension_patch.json`.

## 6. Report honestly

State what changed (semantic diff), whether verification passed, the mass and
size change, and which files were written. Do not claim a check that did not
run (for example interference on a single part, or images on a host without a
GPU).

## 7. Without MCP: the CLI

The CLI enforces the same verification:

```bash
musubicad params part.ocad.d --json                         # list parameters
musubicad intent part.ocad.d param:bore_diameter --json     # what it drives
musubicad intent part.ocad.d regen                          # where regeneration fails
musubicad patch part.ocad.d change.json --dry-run --geometry --json
musubicad patch part.ocad.d change.json                     # verifies, then writes
musubicad review part.ocad.d change.json --output review    # HTML review
musubicad preview part.ocad.d look.png --view iso           # PNG, no GPU needed
musubicad preview part.ocad.d share.png --style studio --aspect 1:1  # image to share
musubicad regen part.ocad.d                                 # mass, volume, assertions
musubicad export part.ocad.d part.step                      # or part.3mf / part.stl / part.glb / sheet.svg
musubicad export arm.ocad.d urdf/arm.urdf                   # assembly -> robot description
musubicad export arm.ocad.d arm.html                        # web page: 3D view + joint sliders
musubicad animate-joints arm.ocad.d arm.gif --pose shoulder=60deg,elbow=-45deg  # joint motion GIF, no GPU
musubicad animate-sweep part.ocad.d sweep.gif --param bore_diameter --from 8mm --to 20mm  # every frame regenerated
musubicad reach arm.ocad.d --tool gripper --point 0,40,7mm --target 0,330,63mm  # reachable? joint angles
```

`musubicad patch` refuses a patch that does not regenerate or misses its
expected effects and prints the reason; `--no-verify` exists only for staged
authoring steps that cannot regenerate yet.

## Parametric construction datums

Use `add_feature` / `replace_feature_definition` to author `datum_plane` and
`datum_axis` nodes before their consumers. Planes store `base`, signed
`offset.value_si` in metres and optional `offset_expr`; bind a new sketch with
`{"type":"datum_plane","feature_id":"feature:plane"}`. Axes store
`origin_m`, normalized nonzero `direction` and optional `origin_expr` (three
length expressions/nulls). Set a revolve's `axis_feature` to the axis ID; retain
its legacy numeric axis fields. Verify parameter edits and use the generated
geometry as evidence. Missing/suppressed/wrong-kind datums refuse regeneration.
Datums are construction outputs, so they do not add bodies or meshes. Revolve
profiles still require global planes; use datum planes with extrusions.
See `docs/api/datums.md` and `examples/agent/create_datum_plate_patch.json`.

### Neutral-plane draft

For planar convex parts, add a `draft` feature through DesignPatch with
`target_feature`, unit-explicit `angle: {value_si: radians}`, optional
`angle_expr`, `neutral_plane` Workplane, and mandatory unique semantic
`face_refs`. A datum neutral plane is supported. Positive angle narrows along
its normal; negative angle widens. Preserve its neutral intersection. Use
`examples/agent/create_draft_plate_patch.json` and `docs/api/draft.md` as the
contract. Reject ambiguous/missing faces, curved or non-convex solids,
holes/cavities, nonperpendicular side faces and collapsed extents; report the
backend error. Always verify through dry-run/apply/regeneration and compare
analytic volume/dimensions. Mock checks the contract; physical evidence comes
from OCCT (ADR-036).

### Centerline ribs

Author `rib` features through DesignPatch when the user needs a parametric
reinforcing web. Create a sketch line for the rib centreline, then add a rib
feature with `sketch_feature`, `centerline_ref`, `target_feature`, positive
`thickness`/`thickness_expr`, and positive distance `extent`/`length_expr`.
The backend expands the line into a rectangular profile, extrudes along the
sketch workplane normal, and unions the result into the target. Verify with
regeneration and compare analytic volume when possible. Do not replace an
ambiguous or missing centreline with a different entity.

### Standard holes and simplified internal threads

For MCAD-P11-004, author `standard_hole` through DesignPatch with existing hole
inputs and an explicit `standard`. Use `iso273` + size M3/M4/M5/M6/M8/M10/M12 +
fit `close`/`normal`/`loose` for clearance. Use `iso_metric_coarse` + size +
`representation: tap_drill` for an internal coarse thread represented by its
recommended nominal-minus-pitch drill cylinder. M6 normal clearance is 6.6 mm;
M6 coarse has 1 mm pitch and a 5 mm tap drill. This does not create physical
thread helices or manufacturing tolerance classes.

The selected circle's evaluated diameter must agree with the cut diameter to
1e-9 m. Change standard and sketch dimensions in the same patch and run verified
regeneration before applying. Unsupported sizes, pitch overrides and non-circular
profiles fail closed. Preserve the explicit representation in saved documents.
Reference: `examples/agent/create_standard_hole_plate_patch.json` and ADR-037.

### Variable and face fillets

For MCAD-P11-005, use a `fillet` with one semantic `edge_ref`, start `radius` /
`radius_expr`, and `radius_law: {type: linear, end_radius: {value_si: ...},
end_radius_expr: ...}`. The start/end order follows increasing world-coordinate
lexicographic order, independent of kernel edge orientation. Radii are finite
SI lengths >= 1e-9 m; edit their parameters through DesignPatch.

Variable fillets support one straight edge only. The linear law uses 65 fixed
radius samples with native OCCT interpolation, so treat it as an approximation.
Curved/tangent-chain and ambiguous/missing selections fail closed. For a constant
face-perimeter fillet, supply `face_ref` and omit both `edge_ref` and `radius_law`.
Never supply both reference types. Run verified dry-run/apply; the native BREP
transfer has no exact face-history relay and downstream references need geometry
provenance. Reference: `examples/agent/create_variable_fillet_plate_patch.json`
and ADR-038.

### Existing-body Boolean features (MCAD-P11-006)

Use `body_boolean` with `source_feature`, `target_feature` and `operation`
(`union`, `subtract`, `intersect`) through DesignPatch. Subtraction is target
minus source. Inputs must be distinct existing solid-producing feature outputs;
never use sketches/datums, suppressed inputs or a cycle. Feature dependencies
are derived from both IDs; no GUI or direct kernel mutation is needed.
OCCT refuses empty/non-finite-volume outputs. Use verified dry-run before apply,
and review provenance, especially for source-derived faces whose exact history
is not recorded. Preserve both upstream branches when editing the operation.
Example: `examples/agent/create_body_boolean_patch.json` and
`examples/body_boolean.ocad.d`. The 80 × 60 × 6 mm plate and overlapping
30 × 20 × 8 mm tool yield union/subtract/intersect volumes of
30000/25200/3600 mm³. Editing source_height to 10 mm gives union 31200 mm³.

### Angular annotations (MCAD-P12-001)

Use `add_angular_dimension` with `sheet_id`, a new `dim:` ID, `view_id`, two
`edge_refs`, explicit `ray_ends` (`start`/`end` in canonical world x/y/z order),
and positive finite `arc_radius_m` in sheet metres. References belong to the
view's part document and must resolve to native straight edges; current semantic
discovery covers the top-Z perimeter. Never infer an unavailable edge or use
curved, ambiguous, parallel/skew or degenerate geometry. Use an orthographic
view parallel to the angle plane; isometric/foreshortened angles are refused.
The backend derives radians and SVG displays degrees, independent of view scale.
Explicit non-associative point rays use `vertex_model_m`, `first_model_m`,
`second_model_m` instead of references; never mix those forms. Review
`drawing_angular_dimensions` and both provenances from regeneration. Remove
annotations through `remove_drawing_dimension` before removing their view.
Example: `examples/agent/create_angular_drawing_patch.json` and
`examples/angular_plate_drawing.ocad.d`; verify dry-run/apply and SVG export.

### Radius and diameter dimensions

Use `add_radial_dimension` via DesignPatch, with `sheet_id`, and a dimension
containing a new `dim:` ID, `view_id`, semantic `edge_ref`, `kind` (`radius` or
`diameter`), nonzero finite `leader_direction` in sheet coordinates and positive
`leader_extension_m` in sheet metres. Bind an exact complete circular edge on the
referenced part; inspect its discoveries. Current semantic discovery covers only
top-Z perimeter. Never author radius, center or label caches. Regenerate/dry-run
and verify `drawing_radial_dimensions` SI measurements/provenance before applying
and exporting SVG. Use a plane-parallel orthographic view; unsupported arcs,
ellipses, ambiguous/missing refs, assembly bindings and foreshortened/isometric
views must fail. Remove/add with a new ID to edit; remove dimensions before views.
Example: `examples/agent/create_radial_drawing_patch.json`; eval:
`drawing_semantic_radial_layout`. See ADR-041 and the MCP authoring guide.

### Polygonal section slice views

Author `add_drawing_view` through DesignPatch with optional `section`:
`plane.origin_m` in model metres, unit `plane.normal`, and positive sheet
`hatch_spacing_m` (>1e-6 m). Choose a plane-parallel orthographic projection
(front = XY, top = XZ, right = YZ). Use verified dry-run/apply and inspect fresh
`drawing_sections` SI area/loop/edge counts before SVG export. Only exact line
contours are accepted; outside/coplanar-face cuts, curved or invalid contours,
assembly sources and isometric/foreshortened views must fail. Do not author caches
or attach existing part-edge dimensions to sections. Edit via remove/add with a
fresh view ID. This initial slice view renders material hatch with clear holes,
without projected geometry beyond the plane or parent cutting arrows.
Example: `examples/agent/create_section_drawing_patch.json`; eval:
`drawing_polygonal_section`. See ADR-042 and the MCP authoring guide.

### Parent-linked rectangular details

Use `add_drawing_view` through DesignPatch with `detail.parent_view_id`,
`region_min_m`/`region_max_m` in projected model metres and `magnification` >1.
Match an ordinary same-sheet parent's model/projection; set child scale to parent
scale × magnification. Child sheet origin centers the crop. Never author segment
caches. Dry-run/apply must regenerate nonempty clipped edges and expose fresh
`drawing_details` evidence. Missing parents, mismatched scale/model/projection,
empty crops, nested/section details and dimensions on detail geometry are refused.
Update parent and child scales together; remove children before their parent.
Edit crop/magnification via remove/add with a new view ID, then export SVG and
verify the source rectangle and derived magnification label. Example:
`examples/agent/create_detail_drawing_patch.json`; eval `drawing_bounded_detail`.
See ADR-043 and the MCP authoring guide.

### Explicit sheet title blocks

Use `set_title_block` through DesignPatch with `sheet_id` and an explicitly supplied
`title_block`: title (≤50 characters), drawing_number (≤24), revision (≤8), optional
date (≤16), author/material (≤24) and scale_note (≤50). Required fields are nonempty;
all text is one line. Never infer dates/authors/material/scale or insert caches.
Dry-run/apply validates the 180 × 48 mm frame at 5 mm margins; existing geometry and
annotations are preserved. Export SVG and verify escaped fields. Clear only with
explicit null; a missing update field must fail. Small sheets, invalid text or
unknown sheet IDs fail atomically. Use `examples/agent/create_title_drawing_patch.json`
and eval `drawing_title_block_revision`; see ADR-044 and the authoring guide.

### Printable vector PDF export

Use the existing export command/`opencad.export` with `.pdf` output after verified
regeneration. Drawing sheets become pages in canonical sheet-ID order, preserving
SI paper dimensions, view scales, annotations/hatches/title blocks. Results expose
`pages` and `bytes`; no model patch is required for read-only export. A bundled
DejaVu font makes output deterministic; missing glyphs or unsupported resources,
invalid semantic refs/layouts and overflow must fail before overwriting an old
PDF. Do not silently replace glyphs or infer timestamps/fonts. Correct model/text
through DesignPatch and retry. No PDF/A certification, images/filters or system
font lookup is claimed. Example: `examples/printable_plate_drawing.ocad.d` and its
PDF; eval `drawing_pdf_revision`. See ADR-045 and the MCP authoring guide.

### Millimetre drawing DXF export

Export a verified **single-sheet drawing** to `.dxf` through the existing export
command/Agent/MCP API. Output is UTF-8 AC1027 with millimetre paper coordinates,
explicit units, named layers, contour-clipped section hatch and derived labels.
View scale is applied: an 80 mm model edge at 1:2 spans 40 mm in DXF paper space,
but its label measures the model. Use STEP for exact 3D/model exchange.
Multiple sheets, invalid references/numbers, limits and text containing `%%`,
backslashes or caret codes fail before replacing output. Normal Unicode/single
percent signs remain supported; fonts depend on the reader. This adds output,
not DXF input or associative DXF dimensions. Example artifacts: title/section
plate DXFs; independent checker `tools/test_drawing_dxf.py` requires ezdxf 1.4.4.
Eval `drawing_dxf_revision`; see ADR-046 and the MCP authoring guide.

### Verified assembly bill of materials

Use `bom_document` / Agent `opencad.bom_document` / `musubicad bom <assembly>`.
Optional `output` (`--output` in CLI) writes JSON or CSV outside all source
models; the query also returns the structured report. Leaf quantities include
patterns and repeated nested assemblies, with root-to-leaf instance-ID paths.
Fixed/movable instances count equally; unused declarations have zero quantity.
Verified source paths distinguish changed copies sharing a document UUID.
Check declared root components, expanded top-level instances and leaf totals
separately. Counts are dimensionless; geometry and metadata remain authored.
Missing/kind/identity/cycle/expansion or any nested regeneration failure refuses
output, preserving source and previous output. Correct sources with DesignPatch,
verify and query again. No inferred material/cost or exclusion rule is supported.
Example: `bom_nested_assembly` totals seven plates (2×3+1); eval
`assembly_bom_add_subassembly` checks ten. See ADR-047 and the authoring guide.

### Explicit-unit DXF sketch import

Use `inspect_dxf` to inspect `dxf_path` units and layer/entity counts, then
`import_dxf` with `path`, `dxf_path`, new `sketch_id`/`feature_id` and explicit
`layers`. Verify with `dry_run:true`, apply with false and retain returned opaque
history for undo/redo. Missing units require explicit `source_unit` (m/mm/cm/in);
it must agree with declared units. Destination display units never rescale input.
Planar LINE/CIRCLE/ARC/straight LWPOLYLINE become authored SI sketch geometry,
with analytic curves and 1 nm point identity. Selected unsupported geometry,
nonplanar/non-default OCS, bulges/widths, ambiguous data or units refuse changes.
Select geometry layers deliberately; text/styles/blocks/splines/ellipses are not
silently converted. Drawing DXF coordinates already include view scale. Bare
sketches can be staged, then consumed through ordinary validated features.
Example: `import/plate_outline.dxf` and `dxf_import_plate.ocad.d`; ADR-048 and eval
`dxf_import_copy_sketch`. CLI: `inspect-dxf` / `import-dxf`; Agent equivalents exist.

### Explicit-unit STL solid import

Call `import_stl` with `path`, `stl_path`, a new `feature_id`, and mandatory
`source_unit` (`m`/`mm`/`cm`/`in`). Dry-run first. This initial path accepts one
closed connected outward-oriented manifold shell, at most 4 MiB/10,000 triangles;
open scans, disconnected shells, self-intersections and repairs refuse. Never
infer units from the document or label an STL asset as STEP. Optional rigid
placement uses metres and a dimensionless proper rotation. The backend retains
original bytes and SHA-256 through AddAttachment/AddFeature DesignPatch/history.
For undo use the returned `history.history` payload. See
`examples/stl_import_plate.ocad.d`, `examples/import/closed_plate_mm.stl` and
`examples/agent/create_stl_import_plate_patch.json`. Mock preserves accurate mesh
measurements/placement; imported-mesh shape edits/CSG/compounds/inertia refuse.

### Construction geometry editing

Use `{ "type": "set_sketch_construction", "sketch_id": "sketch:base",
"entity_id": "ent:guide_line", "construction": true }` in a verified DesignPatch.
Use an explicit boolean, not a context-sensitive toggle. Preserve stored IDs and
constraints; rectangle helpers target the helper's own ID. Construction curves
remain in the solver but cannot form manufacturing profiles. A lost consumed
profile or `profile:outer` switching to another loop refuses; explicitly
rebind/remove consumers in the same patch. Dry-run, apply with history, then
regenerate. See `examples/construction_plate.ocad.d` and
`examples/agent/create_construction_plate_patch.json`.

### Sketch trim and extend

Call `plan_curve_edit` with `path` and `edit` containing stored sketch/target/cutter
IDs, explicit `mode`, `end`, and fresh `new_point_id`. Targets are lines/arcs;
cutters are finite lines/circles/arcs in the same sketch. Explicitly select
`intersection_index` if needed. Review/dry-run/apply the returned DesignPatch,
retaining its revision guard and solved-point expected effect. Old shared points
remain intact. Constraint removal is explicit (`remove_constraints`); conflicts
that move the solved endpoint refuse rather than undo the requested trim.
Close/rebind consumed profiles in the same transaction if necessary. No tangent,
coincident, rectangle-derived or sampled intersections. Angles edited by the
planner are fixed source values, not an association to the cutter. See
`examples/trim_extend_plate.ocad.d` and its authoring patch.

### Bounded offset copies

`plan_offset` / `opencad.plan_offset` / CLI `plan-offset` is read-only. Specify
source sketch, ordered CCW convex line/tangent-arc IDs or one circle,
`distance_m` (outward positive), `join: mitre_tangent`, mandatory
`association: independent_copy`, a new destination sketch/name and all fresh
curve/point IDs. Points follow start-vertex order then arc-centre order; circle
uses one centre. See MCP authoring guide and ADR-052. No constraints are copied
and coordinate/parameter changes in the source do not update the copy. Existing
semantic workplane references still retain plane dependencies. Refuse concavity,
non-tangent joins, collapsed insets and ambiguous topology; never sample or
repair. Review the returned DesignPatch, explicitly append consumer features if
needed, dry-run and apply through verified patch/history. Example:
`offset_plate.ocad.d` and `agent/create_offset_plate_patch.json`.

### Analytic ellipse authoring

Create a centre point then an `ellipse` entity through DesignPatch. Required:
`id`, `center`, `major_radius`, `minor_radius`, `axis_angle`; radii are semiaxes
and literal SI metres, angle is literal radians. Expressions/parameters are
supported; name angle parameters with `_rad`/`_deg`/`angle`, e.g.
`axis_angle_rad: 30 deg`. Major >= minor > 1 nm; never swap axes or guess units.
Source expressions stay authoritative; resolved parameters are not persisted.
Centre point constraints work; ellipse radius/diameter/equal-radius/tangency,
elliptical arcs/mixed loops and ellipse loft/sweep/helix/revolve/trim/offset
refuse. Construction excludes the curve from profiles; unchanged consumers may
not silently rebind their outer loop. Use exact normal extrusion with native
OCCT; Mock has analytic new-body measures/rigid matrix placement but refuses
shape/topology edits and rendering. Dry-run/apply with verified patches/history.
See ADR-053, MCP authoring guide and `ellipse_plate.ocad.d` authoring example.

### Associated geometry projection

Use graph-owned `projected_sketch` via DesignPatch AddFeature or
ReplaceFeatureDefinition, not an authored coordinate copy. Specify upstream
sketch feature/entity IDs, a unique destination sketch ID, Global/Custom plane,
fresh curve/point IDs and construction flags. Lines need two endpoint IDs,
full circles/ellipses one centre; shared points must retain the same semantic
source vertex. Missing/ambiguous/suppressed sources, cycles, invalid planes and
collapsed curves refuse. Curves regenerate after source edits; generated
coordinates are derived output only. Body-edge/Datum/FaceRef/arc/spline sources
remain unsupported. Inspect with `inspect_projected_sketch` (Agent and CLI
counterparts); edit the source or binding, dry-run and apply through verified
patch/history. See ADR-054 and `projected_circle_plate.ocad.d` authoring example.

### Explicit exploded presentations

Use `add_exploded_view`, `set_exploded_view` or `remove_exploded_view` through
DesignPatch. Offset fields are `instance_id` and world-space `translation_m`;
never edit mechanical placements to fake an exploded view. Stable `view:` IDs, nonempty
names, 1..256 unique declared top-level instances and finite bounded SI metres
are required. A child assembly moves as one whole; internal paths and generated
pattern targets refuse. Remove view consumers before instance deletion.
Dry-run/apply, then call `preview_document` with `exploded_view:"view:exploded"`;
CLI equivalent exports PNG with `preview ... --exploded-view view:exploded`.
The default view, physical interference, BOM and URDF stay mechanical. No
exploded drawing/STEP/STL/GLB output or automatic spacing is admitted yet.
See ADR-055 and `examples/agent/add_arm_exploded_view_patch.json`. An example
already containing that ID needs `set_exploded_view`, not a duplicate add.
