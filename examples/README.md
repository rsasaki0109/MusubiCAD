# Examples

Ready-to-use MusubiCAD documents and Agent API requests.

## Documents

| Directory | Template | Features |
|---|---|---|
| `robot_arm_assembly.ocad.d` | `opencad new <path> robot-arm` | 4-part articulated arm: base turret, upper link, forearm link, wrist gripper; 3 concentric joints via connectors and mates |
| `robot_arm_assembly_drawing.ocad.d` | generated example | A4 front-view drawing of the arm assembly with a model-driven 160 mm upper-arm dimension |
| `robot_joint_actuator.ocad.d` | `opencad new <path> robot-joint` | 22 nodes: stepped hubs, shaft/counterbore cuts, 8-hole PCD, 6 ribs, mirrored ears and holes |
| `bearing_carrier.ocad.d` | `opencad new <path> bearing-carrier` | Base extrude, joined hub, through bore, four-hole circular cut pattern |
| `bracket.ocad.d` | `opencad new <path>` | Sketch, extrude, hole (`face_ref`) |
| `bracket_boss_join.ocad.d` | `opencad new <path> boss-join` | + extrude join onto plate |
| `bracket_face_pin.ocad.d` | `opencad new <path> face-pin` | + sketch-on-face pin (`face_ref` workplane) |
| `bracket_edge_fillet.ocad.d` | `opencad new <path> edge-fillet` | + single-edge fillet (`edge_ref`) |
| `bracket_hole_row.ocad.d` | `opencad new <path> hole-row` | + linear cut pattern, `hole_pitch` param |
| `bracket_hole_ring.ocad.d` | `opencad new <path> hole-ring` | + circular cut pattern |
| `bracket_pin_row.ocad.d` | `opencad new <path> pin-row` | + linear union pattern on plate |
| `bracket_pin_ring.ocad.d` | `opencad new <path> pin-ring` | + circular union pattern on plate |
| `bracket_pin_mirror.ocad.d` | `opencad new <path> pin-mirror` | + mirror pattern, `plane_face_ref` |
| `revolve_bushing.ocad.d` | `opencad new <path> revolve-bushing` | Revolve bushing (XY profile, Y axis, 360°) |
| `revolve_sector.ocad.d` | `opencad new <path> revolve-sector` | Half bushing sector (180°) |
| `sketch_constraints_regression.ocad.d` | solver regression fixture | Equal line/circle/arc targets, Parallel/Perpendicular combination, and under/fully/over/contradictory cases |

See [docs/examples/patterns.md](../docs/examples/patterns.md) for a full cut vs union comparison table.

### Partial-occlusion drawing golden

The synthetic two-triangle HLR example is pinned as
[`partial-occlusion.svg`](../modules/drawing/tests/golden/partial-occlusion.svg).
Its long horizontal model edge is split into visible, hidden dashed, then
visible intervals at the projected occluder boundaries. Regenerate and verify
it with:

```bash
cargo test -p opencad-drawing partially_occluded_edges_match_svg_golden
```

### Sketch regression fixture

`sketch_constraints_regression.ocad.d` is a schema-compatible, expanded
`.ocad.d` example rather than a geometry-kernel golden. It keeps the design
graph inputs and canonical checksums under version control so repeated solves
can assert identical coordinates, DOF, and diagnostics. Validate it with:

```bash
cargo test -p opencad-file --test sketch_regression
```

The fixture intentionally records the serialized golden files and their
checksums. A checksum update is expected only when the canonical fixture
serialization changes; the reason for this golden is to detect accidental
solver or serialization drift, not to hide a schema change.

```bash
cargo run -p opencad-cli -- regen examples/bracket_hole_row.ocad.d
cargo run -p opencad-cli -- inspect examples/bracket.ocad.d
cargo run -p opencad-cli -- patch examples/bracket_hole_row.ocad.d examples/agent/spacing_expr_patch.json
```

## Agent API

See `agent/` for JSON-RPC payloads. Pipe them to `opencad agent` on stdio.

## Semantic TopoRef

[`topo-ref-semantic.json`](topo-ref-semantic.json) shows the existing persisted
TopoRef shape: `ref_id` and semantic producer/role/intent are identity, while
kernel IDs and geometric hints are fallback data. P5-001 keeps this JSON
schema-compatible and documents the runtime `TopoRefTolerancePolicy` and
history/sync migration path in [`docs/api/topo-ref.md`](../docs/api/topo-ref.md).

## Atomic patch and regeneration

`bracket.ocad.d` is the representative part fixture for the atomic patch
boundary. Rust callers can apply a multi-operation `DesignPatch` and validate
part regeneration with `opencad_desktop::apply_patch_and_regenerate`; the
candidate clone is committed only after regeneration succeeds, so a failure
leaves the serialized fixture unchanged.

The same validated candidate path drives dry-run and apply for the assembly and
drawing fixtures `assembly_two_brackets.ocad.d` and
`bracket_front_view.ocad.d`. Assembly and drawing operations require their
corresponding model context and appear in the semantic diff before apply.

`assembly_two_brackets.ocad.d` also exercises the P5-004 child-reference
contract: paths are relative to the assembly root, loaded document IDs and
kinds must match each component, and regenerated bodies remain disposable.
Focused assembly tests cover canonical aliases, path/symlink escape, nested
cycles, sibling reuse, localized failure, retry, and the explicit `m`/`m³`
interference policy documented in
[`docs/api/assembly.md`](../docs/api/assembly.md).

### Robot arm flagship assembly

`robot_arm_assembly.ocad.d` is a four-part articulated arm: a bolted base
pedestal, an upper link with shoulder/elbow hubs, a forearm link, and a wrist
gripper with two fingers. Its assembly declares six named connectors and three
concentric revolute joints (`mate:shoulder`, `mate:elbow`, `mate:wrist`), all
satisfied exactly at the authored pose, so the deterministic mate solver leaves
the -45° posed arm unchanged. Links are stacked along `+Z` so joint hubs touch
face-to-face; exact OCCT interference detection reports zero common volume.
Each part stays independently parametric (`upper_arm_length`, `forearm_length`,
`turret_height`, and so on). Regenerate and render it through the CLI, Agent
API, or the same `run_desktop_smoke` path as the other examples:

```bash
cargo run -p opencad-cli -- regen examples/robot_arm_assembly.ocad.d
cargo run -p opencad-cli -- screenshot examples/robot_arm_assembly.ocad.d arm.png
```

A ready-to-run agent review,
[`examples/agent/review_robot_arm_patch.json`](agent/review_robot_arm_patch.json),
reposes the elbow and wrist joints from -45° to -75° via two
`set_instance_placement` operations and checks `no_assembly_interference`. The
self-contained review bundle is pinned in `docs/assets/arm-review/`:

```bash
cargo run -p opencad-cli -- review examples/robot_arm_assembly.ocad.d \
  examples/agent/review_robot_arm_patch.json --output arm-review
```

`robot_arm_assembly_drawing.ocad.d` is a model-driven drawing of the same
assembly. Its A4 front view projects the posed arm at 0.5 scale and carries one
model-driven dimension (upper-arm reach 160 mm). The drawing references the
sibling assembly (`../robot_arm_assembly.ocad.d`) rather than copying it:

```bash
cargo run -p opencad-cli -- export \
  examples/robot_arm_assembly_drawing.ocad.d arm_front.svg
```

### Revision-guarded patches

Attach a complete-state optimistic-concurrency guard before sending a patch
through the Agent or file API:

```rust
let patch = DesignPatch::set_parameter("param:width", "100 mm")
    .with_revision_precondition(&snapshot)?;
```

`snapshot` is the immutable `DesignState` used to author the patch. The guard
is serialized as a `revision_equals` precondition and is refreshed by
`rebase_patch` when the patch is moved to a newer state.

The desktop parameter toolbar, `opencad patch`, and
`opencad.patch_apply_document` all cross the same validated
`DesignPatch`/file boundary; the command-parity test exercises the desktop
path against a direct patch transaction.

## Plugin API contract example

[`plugin-example`](plugin-example) is a buildable linked feature-plugin crate.
Its checked-in manifest demonstrates the explicit Rust API version and schema used by
[`opencad-plugin-api`](../docs/api/plugin-api.md). The contract tests can be
run without OCCT, filesystem, or network service:

```bash
cargo test -p opencad-plugin-api
cargo test -p opencad-plugin-example
```

Feature and importer implementations return validated `DesignPatch` DTOs;
exporters receive immutable serializable state and return bytes. The example
declares `feature_patch`, so it passes the deterministic P4-002 registry policy.
Product integration is available through `opencad plugin list`, `opencad plugin
invoke`, `opencad.plugin_list`, and `opencad.plugin_invoke`. Copy the bracket
fixture before invoking a mutating example; the checked-in Agent invoke request
uses dry-run mode and points at `work/bracket.ocad.d`.

## Semantic-reference regeneration examples

[`bracket.ocad.d`](bracket.ocad.d),
[`bracket_edge_fillet.ocad.d`](bracket_edge_fillet.ocad.d), and
[`bracket_hole_row.ocad.d`](bracket_hole_row.ocad.d) exercise the source,
fillet, and pattern shapes used by the P5-002 reference-stability contract.
The OCCT integration harness also constructs the corresponding boolean-hole
and chamfer models, edits their unit-bearing parameters, and verifies that the
same semantic `ref_id` resolves to a face in the regenerated body:

```bash
cargo test -p opencad-feature --test occt_regen \
  occt_semantic_toporef_survives_boolean_fillet_chamfer_and_linear_pattern_edits
```

## Backend history

Desktop parameter edits use the same validated file-layer `DesignPatch` path
and return a serializable opaque `DocumentHistoryState`. Pass its `history`
field unchanged to the backend undo/redo operation; the complete Design Graph
document is restored, while viewport camera and selection remain outside the
history value. The focused round-trip and failure coverage lives in
`modules/file/src/history.rs`, `modules/desktop/src/parameters.rs`, and the
desktop/Agent command parity tests.
