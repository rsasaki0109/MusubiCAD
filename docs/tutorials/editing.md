# Edit an ellipse plate, review it, and recover from a refused change

This tutorial uses the backend editing commands merged through P14-004. The
current published `latest` release was v0.1.1 when this guide was added; it lacks
these commands. Build current main, or use the v0.2.0 archive once published.
A version string alone is insufficient: the exercise checks actual MCP tool
discovery before editing. Download official releases and verify `SHA256SUMS`.

You will change an ellipse from source radii `30 mm`/`20 mm` at `30 deg` to
`40 mm`/`15 mm` at `0 deg`, then change extrusion length from `6 mm` to `10 mm`.
The final dimensions are `80×30×10 mm`; volume is
`pi*40*15*10 = 18849.555922 mm³`. The original named parameters and the exact
sketch/entity/feature/profile IDs are retained. Editing an entity expression is
different from editing its named parameter: the tutorial preserves parameters.

## Run the complete exercise

In a checkout, build the CLI with the repository's documented OCCT prerequisites:

```bash
cargo build --locked -p opencad-cli
python tools/run_editing_tutorial.py
```

Inside the official archive (Python 3.10+):

```bash
python tools/run_editing_tutorial.py --binary ./musubicad --fixture examples/ellipse_plate.ocad.d
```

Windows PowerShell uses the same command with `--binary ./musubicad.exe`.
The archive includes this guide, the script, the ellipse fixture and
`examples/agent/editing_tutorial.json`. The script copies the fixture into a
cleaned temporary directory. It never edits your original example and prints a
JSON report with measured volume, version and the checks performed. A failure
exits nonzero; read the error before continuing. No GPU or external model call
is required; the real CLI/MCP backend regenerates native OCCT geometry.

To keep a reviewable final document, add `--output my_ellipse.ocad.d`.
The output must be a new path outside the original fixture; an existing path is
refused. Open this verified copy in Tauri or run `musubicad regen` on it.

The exercise tests actual MCP stdio and separate Agent history processes. It is
not evidence that Codex, Cursor or Gemini registered or invoked the plugin. A
host/version/session must be tested separately for that claim.

## Follow the same workflow in an MCP client

1. Register `musubicad mcp` using that client's documented plugin/server setup.
   Check that `inspect_sketches`, `plan_sketch_edit`, `apply_sketch_edit`,
   `list_features`, `plan_feature_edit`, `apply_feature_edit` and
   `regen_document` are discoverable. If absent, update the executable used by
   that host rather than inventing tool names.
2. Copy `examples/ellipse_plate.ocad.d` into a working directory. Call
   `inspect_sketches` with its exact `path`; read the returned current
   `revision`, authored `sketch:ellipse` and `ent:ellipse`.
3. Call `plan_sketch_edit` with `path` and `edit` containing that revision,
   `sketch_id: "sketch:ellipse"` and the `sketch_operations` array in the included
   scenario. Inspect the concrete patch, solved candidate and measured effect.
   Review leaves source unchanged. `apply_sketch_edit` takes the same form and
   returns opaque backend history; retain that history unchanged.
4. Call `list_features` for the new revision. Call `plan_feature_edit` with
   `path`, `revision` and the scenario's `feature_edit`. Review the expected
   `18849.555922 mm³`; call `apply_feature_edit` with the same fields and
   `history` equal to the previous returned state's `history` member.
5. Reusing the old revision must refuse. A point form with `x: "10"` must also
   refuse: use `"10 mm"`, not a unitless number. Keep the verified source and
   reread the current revision before correcting a refused command.
6. Call `regen_document` and independently check volume and dimensions, source
   parameters and the profile binding `sketch:ellipse/profile:outer`.

The [sketch command reference](../api/sketch-editing.md) describes admitted forms.
In archives, see the source documentation at
https://github.com/rsasaki0109/MusubiCAD/blob/main/docs/api/sketch-editing.md.

## Follow it in Tauri

Open a copy of the ellipse example. In **Sketch editor**, select the ellipse on
the 2D canvas; enter `40 mm`, `15 mm`, `0 deg`. Choose **Review sketch draft**,
inspect solve/volume evidence and choose **Apply reviewed sketch**. In the
feature tree, edit `feature:ellipse_body`'s `length_expr` to `10 mm`, review and
apply. Use the backend Undo and Redo buttons to restore/reapply the changes.

Select a point and click the canvas to draft a local-plane position, or choose
**Draw line** and click two endpoints. Mark auxiliary lines as **Construction**
so they do not change the consumed solid profile. Review solves constraints
before saving. A missing, ambiguous, invalid-unit or unsolved draft is refused;
your current source and published preview are retained.

## What Undo/Redo proves

Opaque history snapshots belong to the backend. The current MCP catalog exposes
editing; history commands are Agent methods `opencad.history_undo_document` and
`opencad.history_redo_document`, or Tauri's Undo/Redo buttons. Do not invent MCP
history tools. The script uses real separate `musubicad agent` processes to
undo twice, compare original canonical source, redo twice and compare the final
source. JSON object ordering can change during history transport, so checksums
may be recomputed; logical source identity/expressions must remain equal.
Failed/stale edits are checked against byte-identical source/checksum files.

Global XY/XZ/YZ authored sketches and the listed primitive/constraint forms are
supported. This walkthrough does not add new sketches, custom/face workplanes,
arc/rectangle/cubic/NURBS editing, trim/offset, constraint replacement or assembly
sketch editing. See the API limits before choosing another profile.
