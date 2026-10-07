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
     whole solid) or the expected effect that did not hold.
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
`ground`) relate connectors. Add `no_assembly_interference` to the expected
effects of any patch that moves parts; `verification.geometry.after_interference_count`
reports the count.

## 5. Review and export

- `review_patch` with `{path, patch, output_dir}` writes `review.html`,
  `review.json`, and, when a GPU is available, before/after PNGs and a GIF
  (`images_skipped` explains a missing image). Offer the HTML path to the user.
- `export_document` with `{path, output}`: the extension picks the format.
  `.step` for other CAD/CAM tools (millimetres), `.3mf` for slicers and 3D
  printing (closed, millimetre meshes; Bambu Studio, PrusaSlicer, OrcaSlicer
  open it directly), `.stl` for older tools, `.glb` for viewers and the web,
  `.svg` for drawing sheets, `.urdf` for an assembly as a robot
  description (simulators such as MuJoCo): the grounded instance is the root
  link, concentric mates become continuous joints, other mates fixed joints,
  and one STL per part is written next to the URDF. Re-export after every
  design change instead of editing the URDF.
- `import_step` brings a STEP solid (for example a purchased motor) into a
  part as a fixed body, a join, or a cut.

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
musubicad patch part.ocad.d change.json --dry-run --geometry --json
musubicad patch part.ocad.d change.json                     # verifies, then writes
musubicad review part.ocad.d change.json --output review    # HTML review
musubicad regen part.ocad.d                                 # mass, volume, assertions
musubicad export part.ocad.d part.step                      # or part.3mf / part.stl / part.glb / sheet.svg
musubicad export arm.ocad.d urdf/arm.urdf                   # assembly -> robot description
```

`musubicad patch` refuses a patch that does not regenerate or misses its
expected effects and prints the reason; `--no-verify` exists only for staged
authoring steps that cannot regenerate yet.
