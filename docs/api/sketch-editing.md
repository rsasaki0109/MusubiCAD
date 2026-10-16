# Interactive sketch editing (MCAD-P14-004)

Create a new empty global/datum sketch and its source owner with the separate
[P17-002/003 creation commands](sketch-creation.md), then edit it through these
existing forms. Successful unconsumed point-only drafts need no closed profile;
solid consumers still do. Datum admission follows ADR-102; arbitrary custom/face
workplane editing remains separately gated.

Tauri, Agent JSON-RPC and MCP share `modules/desktop/src/sketch_editor.rs`.
The backend owns authored entities, solve/profile diagnostics, DesignPatch,
verification, document history and publication. The SVG canvas owns disposable
local-plane presentation and unit-labelled form drafts only (ADR-058).

| MCP tool / Agent method suffix | Request | Result |
| --- | --- | --- |
| `inspect_sketches` | `path` | `document_id`, `revision`, rows with unchanged `source`, disposable SI `solved`, `workplane_label` and regenerated `resolved_frame` |
| `plan_sketch_edit` | `path`, `edit` | Concrete guarded `patch`, solved candidate `state`, verified SI `evidence`; no writes |
| `apply_sketch_edit` | `path`, `edit`, optional opaque `history` | Shared feature tree, opaque history and verified SI evidence |

Agent methods use the `opencad.` prefix. `edit` contains the current `revision`,
exact authored `sketch_id` and a batch of 1–64 typed `operations`. Unknown fields
refuse. Source IDs remain stable; clients cannot substitute generated rectangle
points, projected entities or kernel indices.

Datum-plane edits additionally require exact `edit.document_id` from inspection.
When supplied on global edits it is checked too; legacy global requests may
omit it. A foreign document with identical source revision still refuses datum
edits. A parameter change invalidates reviews; inspect and plan again. Display
frames are never accepted as mutation intent or written over source references.

| `operation` | Fields besides `operation` |
| --- | --- |
| `point` | `id`, `x`, `y`, `create` |
| `line` | New `id`, existing point `start`, `end`, `construction` |
| `circle` | `id`, point `center`, `radius`, `construction`, `create` |
| `ellipse` | `id`, point `center`, `major_radius`, `minor_radius`, `axis_angle`, `construction`, `create` |
| `construction` | Authored `id`, `construction` |
| `horizontal`, `vertical` | New constraint `id`, `line` |
| `coincident` | New constraint `id`, point `a`, `b` |
| `distance` | New constraint `id`, point `a`, `b`, length `expr` |
| `radius` | New constraint `id`, circle `target`, length `expr` |
| `remove_constraint` | Existing constraint `id` |

Coordinates and dimensions carry explicit units, for example `0 mm`, `25 mm`
and `30 deg`. Same-kind named parameters use the existing expression resolver;
point/circle source parameter expressions that the current solver cannot resolve
refuse rather than losing the expression. Radius/distance must be positive,
coordinates finite within 1e6 SI units, and ellipse admission remains unchanged.
Point replacement preserves its construction flag. Primitive replacements
preserve primitive kind; constraints are added/removed by stable ID.

Only existing authored sketches with exactly one active Sketch feature owner and
global XY/XZ/YZ or admitted source-bound datum workplanes are admitted. The canvas labels the selected plane, SI-derived origin and
local mm scale. Select points/curves by backend-authored IDs; a background click
sets an explicit-unit point draft. Two clicks draft a line with new stable point
IDs; pointer motion displays a construction-style draft overlay. A form can
create/edit a circle or ellipse, mark construction geometry or add/remove the
listed constraints. Review displays the backend-solved candidate, solve state
and verified volume; Apply rechecks the revision and verifies again.

Every admitted sketch is solved, including unconsumed construction geometry.
Underconstrained solutions are permitted and reported; failed/overconstrained
solutions refuse. Unchanged consumer profile bindings must remain valid. Native
publication also verifies assertions and tessellation before atomic file write.
Stale source, unsupported targets, unit, profile, solve and geometry failures
retain source/history and the published scene. Drafts/reviews discard stale
asynchronous replies. Undo/Redo uses existing verified backend history.

## Pointer circle intent (MCAD-P17-004)

Choose Draw circle, click a centre, move to preview and click a radius point.
Coordinates are local mm (rounded to 0.001 mm); radius is the local distance
rounded to six decimal places in mm. The preview is disposable input, not an
authoritative solved shape. Review/apply uses the same two-form batch through
Tauri and typed Agent/MCP:

```json
[
  {"operation":"point","id":"ent:circle_1_center","x":"2.000 mm","y":"-3.000 mm","create":true},
  {"operation":"circle","id":"ent:circle_1","center":"ent:circle_1_center","radius":"5.000000 mm","construction":false,"create":true}
]
```

The UI selects fresh source IDs before review; later collisions refuse without
publishing an orphan centre. One successful apply is one Undo/Redo entry.
Half gestures cannot submit the numeric form. Escape/Discard, tool/sketch/view/
file changes and upstream parameter edits invalidate drafts and late reviews.
Zero radius after coordinate rounding refuses. The backend still checks finite
explicit units, positive radius, source identity/revision, solves, required
assertions and native publication. Construction circles remain construction
geometry; empty parameter graphs resolve unit-bearing circle source without a
dummy parameter or rewriting authored source. Cold and cached regeneration
share this path.

Independent physical fixtures use r=5 mm, depth=6 mm: V=150*pi mm³ and
area=110*pi mm². Analytic Mock primitives and OCCT check absolute bounds and
centroid on XY/YZ/XZ and a tilted datum (3e-7 m bounds/centroid tolerance,
1e-12 m³ volume). Fresh native SI STL grading retains 0.1 mm placement and 1%
volume tolerances, numerical oriented closure at 1 nm and rejects an actual
translated equal-volume mesh. Actual native Linux/Windows event tests are
separate from typed parity; no external host session or new sampler is claimed.

No `.ocad` schema, migration, kernel primitive, sampling policy or dependency is
changed. Native SVG circles/ellipses are presentation only; no polygon replaces
analytic CAD geometry. Arc/rectangle/cubic/NURBS editing, arbitrary/custom/face
workplanes, new sketch/feature creation, trim/offset, constraint replacement,
datum creation/reparenting, assembly editing and cross-process file locking remain outside this form scope.
Required physical nonempty BodyCount still refuses when unavailable.

Evidence: both Mock and OCCT match ellipse volume `pi*40*15*6 mm³`, dimensions
`80×30×6 mm` (3e-7 m bounding-box tolerance accounts for OCCT's 1e-7 m face gaps),
and independently checked solved horizontal/distance constraints. Native file
preview/refusal/Undo/Redo, real MCP stdio parity and actual Linux Tauri WebView
interaction are tested. External host verification remains separate.
See `examples/agent/interactive_sketch_edit.json` and eval `interactive_sketch_edit`.
