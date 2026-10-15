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

P16-017 / ADR-070 defines acceptance for a future closed circular periodic law;
it enables no source tag or patch. P16-008 remains planned. The existing Mock
variable path changes IDs, not physical geometry. Do not use it as curved-contour
proof or try to create a circular variable fillet before runtime admission.

P16-018 / ADR-071 adds a kernel-only physical complete-cylinder rim query, with
current circle/cap/side ownership in analytic Mock and OCCT. It adds no MCP command
or fillet source. Keep curved variable authoring unsupported until P16-008 supplies
actual construction, guarded source and independent eval evidence.

P16-019 / ADR-072 adds an ideal analytic periodic fillet reference, with an
all-angle geometric exposure/regularity proof in a bounded domain. It creates no
Mock/OCCT solid or MCP operation and supplies no nonconstant volume quadrature.
Do not use reference arithmetic as backend mass or claim circular fillet support.

P16-020 / ADR-073 adds bounded nonconstant ideal volume with explicit m³ width
and a work budget. Unmet precision refuses; it does not certify native geometry
or introduce a source/MCP operation. Keep actual kernel mass distinct.

P16-021 / ADR-074 reproduces native contact/volume counterexamples with a constant
control and a rotated equal-volume wrong-phase body. Diagnostic success means
rejection evidence, not feature admission. A valid BREP or stored radius law
cannot enable circular variable-fillet authoring.

P16-022 / ADR-075 adds ideal patch and positional interpolation bounds with
rounded shared corners. It creates no mesh, native surface or source/MCP operation.
Never infer closure, tangent continuity, native fidelity or feature support from
that positional bound; curved variable authoring remains unsupported.

P16-023 / ADR-076 measures bounded bicubic reference patches on actual native
surfaces. These are transient reference data, with no MCP operation, physical
Mock/native solid or editable NURBS. Shared boundary poles and positional error
bounds do not establish tangent continuity or admit a curved variable fillet.

P16-024 / ADR-077 recomputes local area/orientation/normal bounds from current
reference poles, with native normal/seam diagnostic witnesses. No MCP operation,
physical body, G1 or support trimming is admitted. Normal certificates do not
validate positional metadata; edited poles require all relevant bounds anew.

P16-025 / ADR-078 supplies complete bounded reference bands with explicit
quality/budget refusal and a measured native open-shell sewing diagnostic.
Two remaining boundary loops mean this is not a closed solid. Do not infer
G1, support trimming, physical Mock bodies or curved variable-fillet authoring.

P16-026 / ADR-079 measures closed native diagnostic approximations with
polynomial side skirts and caps. Skirts are not exact cylinder supports. Native
solidity/volume refinement cannot enable product/Mock bodies, source/MCP
creation or claims of G1/support trimming. Circular authoring remains unsupported.

P16-027 / ADR-080 independently bounds signed volume/first moments of the
mathematical diagnostic boundary and compares actual native COM/moments. It
adds no MCP operation or source feature. Polynomial integral bounds and native
numerical comparisons do not prove embedding, G1, unchanged cylindrical support
or all native mass error; circular authoring remains unsupported.

P16-028 / ADR-081 bounds diagnostic band/skirt parametric area with explicit SI
precision and work limits and compares actual native curved-face areas. Caps are
excluded from this independent certificate. Refinement changes integration
partitions, not geometry or native topology. Area agreement does not prove
embedding/G1 or admit circular source/Mock/MCP creation.

P16-029 / ADR-082 creates fixed read-only rational support contact curves and
native diagnostic closed wires, with position bounds and physical phase controls.
These are not trimmed supports or fillet bodies. Do not substitute them into
existing band/cache metadata without rebuilding/re-certifying surfaces. No
editable NURBS, physical Mock/source or MCP operation is admitted; circular
variable-fillet authoring remains unsupported.

P16-030 / ADR-083 builds separate native trimmed analytic cylinder/top faces
with rational contacts, projected pcurves and measured boundary/tolerance,
retained-region/area and phase controls. Numerical gates do not certify global
pcurve error. Wire builders may copy edge identities; no semantic ownership,
compatible blend/body, physical Mock/source or MCP admission follows. Never
invent a circular-fillet command or transfer old polynomial-band certificates
to new boundaries. See docs/api/periodic-fillet-reference.md's trimming scope.

P16-031 / ADR-084 supplies immutable rational characteristic-circle reference
surfaces and bounded analytic cylinder pcurves, with actual-control position
bounds. Native diagnostic solids retain unchanged analytic supports and measure
current contacts/nets, off-knot paired normals, topology, independent volume and
transformed-solid phase. Stereographic v is not alpha/(pi/2). No old polynomial
certificate transfer, global native G1/embedding guarantee, physical Mock/source/
MCP admission or editable NURBS follows. Never invent a circular-fillet command;
see docs/api/periodic-fillet-reference.md's rational surface scope.

P16-032 / ADR-085 separately certifies unchanged rounded rational charts'
whole-domain derivative/normal errors, positive Jacobian and outward orientation,
including mathematical support/seam normal implications. Precision/work refusal
is deterministic. Live native D1/Jacobian/paired-normal samples are numerical
regressions, not global native fidelity/G1/embedding certificates. This adds no
physical Mock/runtime/source, MCP operation/eval or editable NURBS admission;
P16-008 remains planned. Use only existing documented DesignPatch operations.

P16-033 / ADR-086 recomputes whole-domain position/normal/regularity certificates
from captured current native surface definitions, including outward native-to-SI
conversion and common weight-gauge normalization. The live probe binds echoed
proof input to actual post-sewing controls, canonical domain and identity
Location. This certifies mathematical surface definitions, not every floating
evaluation, trim, contact coincidence, exact G1 or whole-BREP/global embedding.
It adds no physical Mock/runtime/mesh, source/MCP operation/eval or editable
NURBS admission. P16-008 remains planned; use documented DesignPatch operations.

P16-034 / ADR-087 also certifies entire CURRENT native 3D side/end contact curve
definitions from raw millimetre poles and weights with complete corresponding-
parameter position bounds. Strict capture/reference/kind/chart/budget binding
and real-process refusals protect the sixteen diagnostic proofs. Attached 2D
pcurve lifts/full edge-to-face gaps, derivatives, native floating evaluation and
whole-BREP fidelity remain unproved. This adds no source, Mock/runtime or MCP
operation/eval; P16-008 stays planned. Author only documented DesignPatch operations.

P16-035 / ADR-088 additionally certifies complete END/PLANE boundary position
gaps among current 3D edges, lifted attached planar pcurves and blend end charts.
Actual raw frames/UV controls and canonical attached blend lines are captured and
bound to eight recomputed proofs. This does not certify cylindrical side contacts,
paired normals/exact G1, floating evaluation or whole-BREP fidelity. No production
Mock/runtime/source or MCP operation/eval is admitted; P16-008 stays planned.

P16-036 / ADR-089 also certifies complete cylindrical SIDE boundary position
gaps from current raw pcurve controls, owning cylinder frame/radius, 3D edge and
blend definitions. Fixed unit weights/basis and actual attached (t,0) lines are
qualified; conversion, interpolation and frame errors enter every whole-span
proof. P16-035/036 cover END/SIDE position gaps, not paired normals/G1, quarter
seams, floating evaluation, global embedding or whole-BREP fidelity. No production
Mock/runtime/source or MCP operation/eval is admitted; P16-008 stays planned.

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
measurements/placement; imported-mesh shape edits/CSG/inertia refuse; ADR-060 adds bounded mesh compounds.

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

### Semantic gear and rack relationships

Use DesignPatch add/set/remove_joint_coupling. Gear has signed dimensionless
`ratio`; rack has signed SI `metres_per_radian`. Reference exact declared
`driver_joint` and `follower_joint` IDs; coordinates are displacements from
authored zero, with no inferred phase or tooth contact. At most 256, one owner
per follower, active known joints, compatible types/ranges and no cycles.
No clamping or implicit rebind. Remove dependent relationships before joints.
After dry-run/apply call `inspect_joint_pose` with
`positions:{"joint:shoulder":"90deg"}`. Use unit-bearing strings; read derived
`position_rad`/`position_m`. -0.5 ratio yields -45deg; never edit follower placement.
Followers are derived in backend pose/animation/reach/workspace. Collision
sampling checks complete posed pairs for coupled sources. Gear URDF uses mimic;
rack URDF and coupled HTML export refuse. Mock exact imported-solid intersections
are unsupported, so native OCCT supplies true collision evidence.
Coupled joint wire guards are mandatory for forward-reader refusal; preserve
`type:coupled`, association ID and underlying motion. Use the backend, not manual
JSON changes. Example/ADR-056 and the authoring guide define exact scope.

### Closed cubic spline authoring

P13-008 / ADR-057 admits a closed non-rational cubic Bezier span, not general
NURBS. Create three ordinary points and `cubic_bezier` through DesignPatch:
`control_points=[ent:p0,ent:p1,ent:p2,ent:p0]`, with stable IDs and explicit
length coordinates/expressions. Require a well-conditioned CCW control triangle;
no inferred endpoint closure. Degree-three knots and unit weights are fixed;
extra degree/knot/weight/periodic fields refuse. Constrain the control points,
not the curve. Construction excludes it from solid profiles. Use exact
`profile:bezier:<curve_id>` with normal distance/centred symmetric NewBody
extrusion. Open/mixed/multi-span/rational curves, trim/offset/projection,
loft/sweep/revolve/through-all and cubic cut/join are unsupported. Preserve
point expressions and source IDs; use `patch_dry_run`, `patch_apply`, then
`regen_document`, and retain backend history. Mock volume/bounds/centroid are
analytic; shape/topology/inertia/rendering are unsupported. Native tessellation
is only presentation. Perimeter is bounded quadrature, not a certified bound.
See `docs/api/cubic-spline.md`, `examples/cubic_plate.ocad.d`, the creation patch
and eval `cubic_edit_control`: a 40×40 mm control triangle extruded 6 mm has
volume 1440 mm³; editing a to 60 mm gives 2160 mm³ without literalizing sources.

### Phase 14 desktop editing contract

ADR-058 / P14-001 is architecture-only; feature-tree, selection-patch and
interactive sketch editors remain planned. Use existing backend DesignPatch
paths while these are implemented. Tauri/JS owns presentation and drafts;
backend owns source queries/capabilities, explicit units, semantic/source-scene
revision admission, concrete patch/effect preview, candidate regeneration,
transactions/history and persistence. Every GUI model command needs CLI or
Agent parity. Publish source/history/scene only after verified regeneration;
retain previous source/history/output on failure. Frontend name heuristics or
renderer IDs cannot bind mutations; ambiguous/missing/stale references and
unsupported geometry refuse. Sketch overlays remain drafts until backend
entity/constraint commits. Transported history stays opaque. The current
source-only parameter/history publication gap must be closed in the editor
gateway. Do not claim GUI editing, actual Tauri interactions or external-host
verification from an ADR, static handler audit or headless backend smoke alone.
See `docs/architecture/desktop-editing.md` and the Phase 14 roadmap acceptance
plan; new sampling/spline/NURBS editing stays separately gated.

### Verified part feature editor

Use `list_features` to obtain backend source IDs, admitted length fields, authored closed profiles and current revision. Call `plan_feature_edit`, review the exact change and SI evidence, then call `apply_feature_edit` with the same intent/revision and optional opaque history. Requery before each next edit. Forms admit `create_extrude` (positive NewBody distance), `set_expression`, `move`, and `suppress`; never invent source bindings from names or renderer indices. Explicit units or same-kind parameter names are required. Failed dependency/unit/geometry/assertion admission retains source/history. Empty parts are allowed when assertions permit; nonempty physical BodyCount is unavailable and fails closed. Use `examples/agent/feature_tree_edit.json` and the API contract `docs/api/feature-editor.md`. Tauri and MCP share these helpers; native publication also checks display tessellation. General definition/sketch editing and assembly editor commands remain outside this task.

### Semantic selection editing

Use `inspect_selection_frame` (native scene, no rasterization/write) or `pick_document` for authoritative stable targets and source/scene revisions. `plan_selection_edit` previews `{source_revision,scene_revision,target,field,expr}`; `apply_selection_edit` rebinds and uses the verified feature transaction, with optional unchanged opaque history. Targets are declared `face_reference` IDs or authored `sketch_entity` IDs from backend bindings; no inferred name/role, index, generated coordinate or kernel handle can authorize mutation. Missing/ambiguous/consumed/stale/unsupported bindings refuse. Initial forms edit admitted length expressions on the uniquely resolved owner. Preserve named parameters unless explicitly editing them through their separate API. Follow `examples/agent/semantic_selection_edit.json` and `docs/api/selection-editing.md`; re-query after edits. Local Linux Tauri and stdio MCP evidence are separate from actual external hosts.

Interactive sketch forms (P14-004): `inspect_sketches`, `plan_sketch_edit`,
`apply_sketch_edit` share Tauri/Agent/MCP backend solve, DesignPatch verification
and atomic history publication. Use explicit units, stable authored IDs and the
current revision; review never writes. Existing global-workplane sketches only;
unsupported/ambiguous/stale/failed-solve candidates refuse. See
`docs/api/sketch-editing.md` and `examples/agent/interactive_sketch_edit.json`.

Reproducible editing tutorial: `docs/tutorials/editing.md` (distributed as
`EDITING-TUTORIAL.md` in CLI archives). Run `python tools/run_editing_tutorial.py`
in a built checkout, or pass the packaged `--binary` and `--fixture` paths.
The real MCP/Agent exercise checks reviews, stale/unit refusals, source identity,
Undo/Redo and native analytic dimensions/volume; it is not actual client-host
integration evidence. The inputs are in `examples/agent/editing_tutorial.json`.


## Independent articulated subassembly copies (P13-011)

CLI v0.3.1 adds `inspect_articulation_source` (published v0.2.0 predates this
API). Inspect the child for its owning document, revision and independent joint
IDs, and inspect the parent for the patch revision precondition. Author
`set_instance_articulation` with a strict `articulation` object containing
`source_doc`, `source_revision` and `positions` as an array of
`{"joint_id":"joint:hinge","position":"90 deg"}`. Dry-run, review and apply via
DesignPatch; never edit the child to change one copy. An explicit null clears
the instance configuration. Application forces regeneration and rechecks source
dependencies before writing, even with `verify:false`.

Use `examples/articulated_copies.ocad.d` and its zero-coordinate patch as the
reference. `regen_document` reports sorted structured `assembly_leaves` with
source ownership, composed SI placements, bounds and volumes. Missing/wrong
owners, stale revisions, duplicate coordinates, followers, illegal units/ranges,
nested articulated scopes and articulated pattern targets refuse. Motion, reach,
URDF and HTML mechanics also refuse before output. Static geometry, reviews,
BOM and native leaf-pair interference are supported; Mock exact mesh intersection
refuses. Parent-context constraints remain P13-012. See
[the articulated-copy API](../../docs/api/articulated-subassemblies.md) and ADR-059/060.


## Parent-context flexible constraints (P13-012)

Current main accepts `context_constraints` in `set_instance_articulation`.
Inspect exact child joint/connector IDs and revision with
`inspect_articulation_source`; the returned `connectors` retain source ownership.
Bind moving `[parent_instance, child_link]` plus owning child DocumentId and
ConnectorId, and target `[fixed_parent_part]` plus the parent DocumentId and
ConnectorId. Author the constraint in a DesignPatch, dry-run, review and apply.
Never modify a child to move one copy. The mandatory `flexible_assembly` guard
refuses older readers, including P13-011 readers.

Outer instances must be fixed; each independent flexible-child coordinate must
be explicitly saved or solved exactly once. Signed-distance constraints drive
bounded prismatic joints; signed-angle constraints drive bounded revolute joints
with explicit `turns` branch selection. Followers remain derived. Unknown,
ambiguous, singular, contradictory, unbound, wrong-unit/range or unsupported
scopes refuse. Static physical consumers share solved leaves;
`regen_document.context_dof` is zero for the admitted bound system. Hierarchical
motion/reach/URDF/HTML still refuse. See ADR-061, the articulated-subassembly API,
`examples/parent_context_slider.ocad.d`, `parent_context_hinge.ocad.d` and their
parent-only patches. Published v0.2.0 predates this API.


## Reproduce parent-context editing from a package (P15-006)

Use `python tools/run_assembly_tutorial.py --binary <trusted-native-binary>`
with the supplied slider/hinge fixtures and `examples/agent/assembly_tutorial.json`.
The walkthrough inspects exact ownership/revisions, reviews and applies only a
parent constraint, checks independent SI poses plus native dimensions/volume,
and independently integrates the exported STL's volume/centre/bounds/closedness.
It verifies child/other-copy preservation, stale/unit/range/owner/reference
refusal, protected unsupported output and opaque Agent Undo/Redo. See the
assembly tutorial in `docs/tutorials/assemblies.md` (`ASSEMBLY-TUTORIAL.md` in
v0.3.1 CLI archives). This exercises actual MCP/Agent, without paid models;
it supplies no Cursor/Gemini client-host evidence. Published v0.2.0 predates
these APIs and archive inputs; official v0.3.1 includes them and passed the native
packaged tutorial gates. See the [v0.3.1 release evidence](../../docs/plans/cli-release-v0.3.1-evidence.md).


## Analytic Mock evidence boundary (P16-002)

`MockGeometryKernel::with_analytic_primitives()` is an explicit Rust SDK testing
mode for bounded physical rectangle/circle/concentric cuts. Default ordinary
Mock IDs remain synthetic and cannot prove physical hole dimensions or volume.
See `docs/api/analytic-mock.md` and `examples/analytic-mock/`. No new MCP command,
persisted setting or document seat feature is introduced. Counterbores use the P16-003 source/ownership gate below; countersinks use P16-004 below; do not invent seat fields or
substitute a plain bore. Use verified native DesignPatch for supported document
edits. P16-010 admits actual closed bounded meshes and body-owned stock witnesses
(ADR-063) in this explicit SDK mode; source/semantic ownership remains the
feature layer's responsibility. Edge/inertia and unsupported geometry queries
explicitly refuse; never count a synthetic fallback as physical evidence.


## Counterbored standard holes (P16-003)

Follow `docs/api/seated-hole-admission.md` / ADR-063 and
`examples/counterbored_plate.md`. Author the closed `seated_standard_hole` /
`counterbore` tags through `add_feature` or same-type
`replace_feature_definition`. Require original rectangle/distance/NewBody stock,
owned end-face reference, outward exact ISO-matching circle, strictly contained
seat and bounded inward depths. Never invent optional seat fields on old holes,
infer fastener head sizes, or substitute a plain bore.

Metres use `{value_si: ...}`. Length expressions use `set_feature_expr` fields
`bore_depth`, `seat_diameter`, `seat_depth`; parameter edits also regenerate.
Review before apply. Verification cannot be disabled for seated-hole changes;
unit/range/source/refusal preserves previous files/history/artifacts. Default
Mock is unsupported; explicit analytic Mock is SDK evidence, native is the
CLI/MCP path. Check `counterbore_seat_depth` eval / actual stdio regression;
source and geometry both matter. This adds no new MCP tool or host validation.

P16-011 supplies bounded physical countersink geometry (ADR-064); P16-004
adds the strict `seated_standard_hole` / `countersink` source. Use explicit
`diameter: {value_si: metres}` and `included_angle: {value_si: radians}`,
with optional diameter_expr/included_angle_expr. Angle parameters use names
like `seat_angle_rad` and unit-bearing expressions like `90 deg`. Length and
angle fields refuse cross-kind units/references. No head dimensions are inferred.

Depth is derived from the diameter/radius difference and half-angle tangent;
angle must be finite in (0,pi), radial/lateral margins exceed 1 nm, and seat
depth is strictly smaller than bore depth. The same owned original-stock end
face, exact standard bore circle and mandatory verified DesignPatch gate apply.
Use `seat_included_angle` for typed expression edits; `seat_depth` is
counterbore-only. Do not substitute a cylindrical counterbore or plain bore.
See `examples/countersunk_plate.md`, `countersink_angle` eval and
`tools/test_countersink_mcp.py`. Packed/expanded source and canonical Undo/Redo
are tested; old counterbore-only readers reject countersinks. Real stdio uses
zero model calls, with no additional host/release credit. v0.3.1 predates these
source tags. Explicit analytic Mock is SDK evidence; CLI/MCP uses native OCCT.

P16-005 / ADR-065 adds closed `cylindrical_draft` authoring over P16-012's
physical cylinder-to-frustum runtime. Require original exact-circle distance/
NewBody stock, owned full `cylindrical` side reference, SI signed Angle and explicit
parallel/antiparallel neutral plane within the height. Never send curved sides
to planar `draft`, use average normals or invent optional curved flags.

Create/review/apply with DesignPatch. Use angle parameter `draft_angle_rad` and
unit-bearing expressions such as `7 deg`; typed `draft_angle` edits angle_expr,
`face_ref` edits the semantic side. Radius collapse/tiny taper, invalid units/
planes, cap/foreign/missing/otherwise-consumed sides and altered stock refuse.
The changed input side is consumed; endpoint neutral-cap ownership is preserved
only with actual geometry proof. Mandatory native verification survives
verify:false/--no-verify; failure preserves source/history and cache repeats
physical ownership checks. Default Mock refuses; explicit analytic Mock is SDK
proof. See `docs/api/cylindrical-draft.md`, `examples/cylindrical_draft.md`, the
creation patch, `cylindrical_draft_angle` eval and real stdio regression
`tools/test_cylindrical_draft_mcp.py`. Sign is independently graded because a
centred-plane volume cannot distinguish opposite angles. v0.3.1 predates the tag;
no general curved/NURBS draft, additional host or release credit is claimed.

P16-006 / ADR-066 adds closed `tapered_rib` authoring over P16-013 physical
runtime. Require original rectangle/distance/NewBody target, authored complete
straight centreline (construction allowed), matching original end plane and
owned named `extrusion_start`/`extrusion_end` face. Require SI base_thickness,
height, signed Angle and explicit ends:square, with strict optional expressions.
T(z)=T0-2*z*tan(angle), neutral base, square ends; reject invalid units/static
fallbacks, missing/foreign/ambiguous/stale/consumed refs, collapse/tiny additions,
curved/oblique/modified/intersecting families. Never invent optional angle/end
fields on old `rib` or infer source provenance from physical box geometry.

Create/review/apply via DesignPatch. Typed base_thickness/rib_height/draft_angle
edit expressions; face_ref edits ownership. Mandatory native verification
survives verify:false/--no-verify, previews remain read-only, failed edits preserve
source/history. Six surviving stock planes retain source/end identity, five rib
surfaces have the rib producer; shared interface is absent and the whole entry
ref is not consumed. Cache repeats physical/source ownership checks. Default
Mock refuses, explicit analytic Mock is SDK evidence; CLI/MCP uses native OCCT.

See docs/api/tapered-rib.md, examples/tapered_rib_plate.md, creation patch,
source-aware tapered_rib_height eval and tools/test_tapered_rib_mcp.py. Real stdio
creation/edit/typed previews/refusal and canonical Agent history use zero model
calls. Both schemas, additive migration/roundtrips and old-reader refusal ship;
v0.3.1 predates the tag. No generic picker named-end, arbitrary curved/multiple
rib, additional host or release capability is claimed.

### Nominal metric thread profile boundary (P16-014)

ADR-067 supplies geometry SDK arithmetic for coarse nominal metric flat-root
basic profiles, explicit right/left hand, whole pitches and square clips.
See docs/api/nominal-metric-thread.md and examples/nominal_metric_thread_profile.md.
It creates no physical body, serialized feature or MCP operation. ADR-037
tap_drill remains a cylinder; a generic helix is not independently verified
standard thread geometry. Ideal volume is only a later measurement target.
Do not claim rounded production roots or tolerance-class fit. P16-015 supplies
physical construction; P16-007 supplies protected source/schema/patch/history/eval
below. Feature credit is conditional on its complete main merge.

P16-015 / ADR-068 now supplies actual nominal basic-flat-root runtime in explicit
analytic Mock and OCCT, with independent physical checks. See
docs/api/physical-nominal-thread.md and examples/physical_nominal_thread.md.
The SDK consumes a proved full cylinder, explicit phase/hand and whole pitches;
unsupported geometry/quality/precision refuses. Runtime alone exposes no `.ocad`
feature: P16-007 supplies source below. Never invent create_thread or relabel a
generic helix/tap-drill.
Runtime infrastructure earns no maturity increment or manufactured-fit claim.

P16-007 / ADR-069 now exposes guarded `nominal_metric_thread` source through
existing typed DesignPatch. Require original authored circle/Distance/NewBody
stock, owned cylindrical face_ref and checked sketch phase; required fields
representation=nominal_metric_basic_flat_root, size=m3/m4/m5/m6/m8/m10/m12,
handedness=right|left, turns=integer1..32, ends=square. Add with add_feature;
use replace_feature_definition's `id` for categorical edits, set_feature_ref for
face_ref. Coordinate stock diameter/length changes explicitly; no new expression
fields, auto-resize, missing-intent inference or create_thread MCP operation.
All output producers are thread-owned, not original circular caps/full side.
Never select a thread surface by transient native subface index; end refs need
unique current owned geometry. Strict source/cache/physical phase checks and
mandatory native candidate verification apply even with verify:false/no-verify.
See docs/api/nominal-metric-thread-source.md, examples/nominal_metric_thread.md,
creation/left-hand patches and tools/test_nominal_thread_mcp.py. Source-aware
nominal_thread_handedness eval rejects wrong-hand/cylinder/helix decoys despite
equal claimed volume. Physical phase is independently measured in Rust tests;
no model host was impersonated. Preview/review/failure and canonical history
use the existing shared backend. No rounded production root, fit class,
partial/internal/multi-start/runout or general thread-face refinement claim.
