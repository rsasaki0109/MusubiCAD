# Move one child mechanism with a parent constraint

This walkthrough uses the current-main P13-011/012 APIs. Published CLI v0.2.0
predates them. Build current main or use a future archive that contains this
`ASSEMBLY-TUTORIAL.md`, the runner and both mechanisms; a package filename or
version string alone does not establish support.

The slider and hinge each contain two copies of one child assembly plus a fixed
parent target part. Every physical part is a closed 10 × 20 × 30 mm box:
6000 mm³ each, five parts total 30000 mm³. The parent owns the constraint; the
child document is unchanged. The right copy stays independent.

## Run the complete exercise

From the checkout with a native build:

```sh
cargo build -p opencad-cli
python tools/run_assembly_tutorial.py --binary target/debug/musubicad
```

From an extracted future Linux/macOS archive:

```sh
python tools/run_assembly_tutorial.py --binary ./musubicad
```

On Windows replace the binary path with `.\musubicad.exe`. Python 3.10+ is
required; no Python package, paid model, graphical client or GPU is required.
The runner copies the fixtures to temporary directories, executes real MCP and
Agent processes, prints a JSON report, then removes its copies and reviews.
It fails on any missing tool, unexpected refusal or inconsistent evidence.
The supplied binary must be trusted before executing it.

## Follow the operations

1. Inspect the child with `inspect_articulation_source`. Require the exact
   document/revision, admitted `joint:hinge` driver and `connector:link_tip`.
   Inspect the parent separately for its revision precondition.
2. Read the existing `instance:left.articulation`. Retain its source revision,
   empty saved positions, `mate:parent_context`, driver and exact moving/target
   connector references. The moving path is `[instance:left, instance:link]`
   owned by the child assembly; the target `[instance:target]` is owned by the
   parent assembly. IDs inside different copies remain distinct.
3. Change only the relation in `set_instance_articulation`: the slider's
   signed distance goes from `25 mm` to `0 mm`; the hinge's signed angle goes
   from `180 deg` to `90 deg`, retaining explicit `turns:0`.
4. Dry-run and generate a review outside the source. Review must leave every
   source/checksum byte unchanged and report no interference. Apply through
   DesignPatch. `verify:false` still requires verification for this operation.
5. Regenerate and inspect five scoped physical leaves plus `context_dof:0`.
   The right copy, named connectors, authored placements, child graph/assets
   and unrelated parent source must remain unchanged. BOM counts five parts.
6. Exercise stale revision, unitless/range, wrong-owner and missing-connector
   refusal, then opaque Agent Undo/Redo. Both canonical source states and
   child bytes must survive. URDF/HTML requests must refuse before replacing
   existing output files because hierarchical mechanics are not supported.

The scenario in `examples/agent/assembly_tutorial.json` names both fixtures and
targets. Equivalent standalone patches are in `examples/agent/`; editing those
on your own should use a disposable fixture copy and a fresh parent precondition.
The runner constructs its patch from inspected source instead of guessing IDs.

## Independent geometry checks

Native regeneration checks each 6000 mm³ leaf, total volume, dimensions and
separately specified SI frames. It also exports real binary STL, interprets its
coordinates in metres, and independently integrates signed tetrahedra to obtain
volume and centre. A 0.1 µm weld grid checks closed, consistently oriented mesh
edges and five disconnected physical members. This check is for these separated
fixtures, not general mesh repair or a Boolean union of overlapping solids.

| Case / state | Native total, mm³ | Exported centre, m | Exported Y minimum, m |
|---|---:|---|---:|
| Slider before | 30000 | (0.119, 0.080, 0) | -0.055 |
| Slider after | 30000 | (0.119, 0.085, 0) | -0.030 |
| Hinge before | 30000 | (0.114, 0.083, -0.009) | -0.020 |
| Hinge after | 30000 | (0.113, 0.086, -0.009) | 0 |

Native volume tolerance is `1e-11 m³`; exported f32 STL tolerance is
`5e-11 m³`; physical centres/bounds allow `3e-7 m`; pose frames allow `1e-9`.
A wrong internal placement can preserve total volume and outer bounds; the
independent centre check deliberately catches that failure.

The report distinguishes actual MCP/Agent execution from Codex/Cursor/Gemini
host integration (`actual_client_host:false`, `model_turns:0`). It proves neither
a client registration nor autonomous model selection. Fixed outer placements,
admitted single-coordinate connector constraints and explicit branch selection
are the supported scope; free/nonlinear/deeper systems and hierarchical motion,
reach, URDF/HTML remain refused. See ADR-061 and the articulated-subassembly API.
