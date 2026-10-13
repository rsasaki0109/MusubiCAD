# Interactive sketch editing (MCAD-P14-004)

Tauri, Agent JSON-RPC and MCP share `modules/desktop/src/sketch_editor.rs`.
The backend owns authored entities, solve/profile diagnostics, DesignPatch,
verification, document history and publication. The SVG canvas owns disposable
local-plane presentation and unit-labelled form drafts only (ADR-058).

| MCP tool / Agent method suffix | Request | Result |
| --- | --- | --- |
| `inspect_sketches` | `path` | `revision`, rows containing unchanged `source` and disposable SI `solved` sketches |
| `plan_sketch_edit` | `path`, `edit` | Concrete guarded `patch`, solved candidate `state`, verified SI `evidence`; no writes |
| `apply_sketch_edit` | `path`, `edit`, optional opaque `history` | Shared feature tree, opaque history and verified SI evidence |

Agent methods use the `opencad.` prefix. `edit` contains the current `revision`,
exact authored `sketch_id` and a batch of 1–64 typed `operations`. Unknown fields
refuse. Source IDs remain stable; clients cannot substitute generated rectangle
points, projected entities or kernel indices.

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
global XY/XZ/YZ workplanes are admitted. The canvas labels the selected plane and
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

No `.ocad` schema, migration, kernel primitive, sampling policy or dependency is
changed. Native SVG circles/ellipses are presentation only; no polygon replaces
analytic CAD geometry. Arc/rectangle/cubic/NURBS editing, arbitrary/custom/face
workplanes, new sketch/feature creation, trim/offset, constraint replacement,
assembly editing and cross-process file locking remain outside this form scope.
Required physical nonempty BodyCount still refuses when unavailable.

Evidence: both Mock and OCCT match ellipse volume `pi*40*15*6 mm³`, dimensions
`80×30×6 mm` (3e-7 m bounding-box tolerance accounts for OCCT's 1e-7 m face gaps),
and independently checked solved horizontal/distance constraints. Native file
preview/refusal/Undo/Redo, real MCP stdio parity and actual Linux Tauri WebView
interaction are tested. External host verification remains separate.
See `examples/agent/interactive_sketch_edit.json` and eval `interactive_sketch_edit`.
