# Inspect and repair a failed regeneration

A scripted scenario on the robot joint housing flagship
(`examples/robot_joint_actuator.ocad.d`), part of MCAD-P6-006. An edit breaks
the part; four commands find which feature fails, connect it to the edit,
and repair it. Every step reads the Design Graph or regenerates in memory,
and only the two `patch` steps write the document.

The test `the_flagship_inspect_and_repair_scenario` in
`modules/cli/src/regen.rs` runs the same steps and checks each result.

Work on a copy:

```bash
cp -r examples/robot_joint_actuator.ocad.d joint.ocad.d
```

## 1. The edit that breaks the part

[`repair_shaft_bore_break_patch.json`](../../examples/agent/repair_shaft_bore_break_patch.json)
widens the shaft bore to 80 mm. A verified patch refuses it, because the bore
is wider than the 72 mm lower hub and splits the part in two:

```text
$ musubicad patch joint.ocad.d examples/agent/repair_shaft_bore_break_patch.json
error: validation failed: patch rejected; the document was not changed: patched document does not regenerate: feature 'feature:radial_ribs': OCCT error: Expected exactly one resulting Solid, got 2; blocks feature:mounting_ears, feature:mounting_holes
```

A dry run (`patch_dry_run` through MCP, or `opencad.patch_dry_run_document`)
returns the same failure as `verification.regeneration_failure`, so an agent
sees the failing feature before anything is written.

The scenario forces it through with `--no-verify`, the way an unverified
staged edit or a hand edit leaves a document that no longer regenerates:

```bash
musubicad patch joint.ocad.d examples/agent/repair_shaft_bore_break_patch.json --no-verify
```

## 2. Where does regeneration stop?

```text
$ musubicad intent joint.ocad.d regen
kernel: OCCT 8.0.0 (cadrum static)
status: failed at feature feature:radial_ribs
error: feature 'feature:radial_ribs': OCCT error: Expected exactly one resulting Solid, got 2
completed features: feature:sketch_counterbore, ..., feature:shaft_bore, feature:counterbore, feature:pcd_fasteners
blocked features: feature:mounting_ears, feature:mounting_holes
not reached: -
suppressed: -
upstream body: feature:pcd_fasteners
volume_m3: 0.00010131926186235493
mass_kg: 0.2735620070283583
```

The failing feature is `feature:radial_ribs`, not the bore that was edited.
The bore cut itself succeeded and left two solids. The ribs are the first
feature that needs a single solid. The ears and mounting holes depend on the
ribs, so they cannot regenerate. The document on disk is not written.

## 3. Is the edit the cause?

```text
$ musubicad intent joint.ocad.d param:shaft_diameter
parameter: param:shaft_diameter (shaft_diameter) 80 mm = 0.08 m
driven by: -
drives parameters: -
sketches: sketch:shaft_bore
directly affected features: feature:sketch_shaft_bore
predicted dirty features: feature:sketch_shaft_bore, feature:shaft_bore, feature:counterbore, feature:pcd_fasteners, feature:radial_ribs, feature:mounting_ears, feature:mounting_holes
assertions: -
```

`feature:radial_ribs` is among the features `shaft_diameter` regenerates, so
the 80 mm bore is the cause.

## 4. Repair

[`repair_shaft_bore_fix_patch.json`](../../examples/agent/repair_shaft_bore_fix_patch.json)
narrows the bore to 32 mm, the largest round bore that leaves a 10 mm wall in
the 52 mm upper hub. Verification runs before the patch is written:

```text
$ musubicad patch joint.ocad.d examples/agent/repair_shaft_bore_fix_patch.json
verified: regenerates
patched: joint.ocad.d

$ musubicad intent joint.ocad.d regen
kernel: OCCT 8.0.0 (cadrum static)
status: regenerated
body: feature:mounting_holes
volume_m3: 0.00021838602584240514
mass_kg: 0.5896422697744939
```

The document before the repair does not regenerate, so the repair patch
cannot declare a `mass_delta_kg` effect. Verification checks that the part
regenerates again, and that `shaft_diameter` is now 32 mm.

## The same steps through MCP

| Step | MCP tool | Arguments |
|---|---|---|
| 2 | `inspect_regeneration` | `{ "path": "joint.ocad.d" }` |
| 3 | `query_document` | `{ "path": "joint.ocad.d", "query": { "kind": "inspect_parameter", "id": "param:shaft_diameter" } }` |
| 4 | `patch_dry_run`, then `patch_apply` | `{ "path": "joint.ocad.d", "patch": <repair patch> }` |
| 4 | `inspect_regeneration` | `{ "path": "joint.ocad.d" }` |
