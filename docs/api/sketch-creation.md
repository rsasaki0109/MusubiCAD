# Global-plane sketch creation (MCAD-P17-002)

Build the CLI/desktop from this checkout for these commands. Published v0.3.1
predates this API; check `tools/list` for the actual server capabilities rather
than relying on the version string alone.

The Tauri **New sketch** form and Agent/MCP commands share
`modules/desktop/src/sketch_creation.rs` under ADR-102. Choose a name and global
XY/XZ/YZ plane, review, then create. The backend suggests free stable IDs; users
do not edit graph JSON. Creation selects the new sketch for existing sketch
forms. Cancel and changed/stale reviews make no source/history entry.

| MCP tool / Agent method suffix | Request | Result |
|---|---|---|
| `inspect_sketch_creation` | `path` | `document_id`, `revision`, deterministic `suggested_sketch_id` / `suggested_feature_id`, admitted `planes` |
| `plan_sketch_create` | `path`, `create` | Exact guarded `patch`, admitted `create`, candidate sketch `state`, native verified `evidence`; no writes |
| `apply_sketch_create` | `path`, `create`, optional opaque `history` | Shared feature tree, opaque history state and native verified evidence |

Agent methods use `opencad.`. Tauri handlers add `_cmd` to these names.
The `create` DTO requires all fields, and unknown fields refuse:

```json
{"document_id":"doc:part","revision":"<current backend revision>",
 "sketch_id":"sketch:new_1","feature_id":"feature:sketch_new_1",
 "name":"Manufacturing sketch","plane":"XZ"}
```

Both planning and applying check document identity and source revision.
Suggestions are deterministic for that source; explicit valid free IDs are
also accepted. Collisions refuse without renaming. The planner produces one
DesignPatch with existing `add_sketch` and owning sketch `add_feature`, appended
to the feature order. Apply revalidates and regenerates disposable source and
history before persistence, then rechecks the file identity/revision. Failed
units, solves, required assertions, stale identity/revision, malformed choices,
namespace errors and late owner collisions preserve source/history/output.
Undo/Redo restore canonical source IDs, expressions and graph order.

A new sketch is empty, not a solid. A part with no active body has null
`volume_m3` / `bounds_size_m` and remains editable. In a part with a body,
evidence still measures that existing active body. A profile-free draft requires
one active authored global-plane owner, existing editor primitive families,
successful solve and no active consumer of that owner. Active solid consumers
still require a valid profile. Required assertions are never bypassed.

Reuse `inspect_sketches`, reviewed sketch forms and feature forms for subsequent
geometry. In local coordinates, a 20/10 mm semiaxis ellipse extruded 6 mm has
volume 1200*pi mm³. Absolute world bounds are XY: x[-20,20], y[-10,10], z[0,6]
mm; YZ: x[0,6], y[-20,20], z[-10,10] mm; XZ: x[-20,20], y[-6,0], z[-10,10]
mm. The XZ positive extrusion direction is -Y. Physical Mock and OCCT tests
check volume, bounds and centroid independently of the placement resolver.

The numerical STL grader separately allows 0.1 mm position and 1% volume error
for display tessellation, checking oriented closure at 1 nm and rejecting
equal-volume wrong-plane exports. Those mesh allowances are not native analytic
admission tolerances or an approximation certificate. Native volume tests use
1e-12 m³ and bounds/centroid 3e-7 m. No new sampling method is introduced.

Example: `examples/agent/create_global_sketch.json`. Actual protocol regression:

```sh
python tools/test_sketch_creation_mcp.py --musubicad target/debug/musubicad
```

It uses zero model turns. Eval `create_global_sketch` independently checks the
new source/owner/plane, preservation of existing source, native volume and fresh
physical STL. A protocol harness is not additional external model-host evidence.
Actual Linux/Windows Tauri tests separately operate the native forms, cancel,
late review, identity/collision refusal, all global planes and sketch-only state.

Scope: existing part documents only; no new blank-file/overwrite operation,
datum/custom/face-plane creation or editing, projected/generated sketch editing,
NURBS, or persisted GUI state. Existing `.ocad` fields and schemas are reused.
Older regeneration can refuse profile-free drafts; no migration invents a
profile or solid. Persistence retains the existing file-writer concurrency and
crash-consistency limits.
