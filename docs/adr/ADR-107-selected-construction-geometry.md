# ADR-107: Source-selected construction controls and profile consequences

Status: Accepted for MCAD-P17-009 implementation; completion requires qualifying main merge
Date: 2026-10-17
Roadmap: MCAD-P17-009, after P17-008 / PR #157

## Decision

Expose a disposable backend catalogue of admitted authored point, line, circle,
ellipse and arc IDs with their kinds and current construction flags. Share
admission with the existing typed Construction form. Exclude cubic entities,
cubic control points, projected/derived geometry and stored rectangle helpers.
This catalogue is distinct from ADR-106's constraint targets. No new source
schema, geometry admission, solver rule or mutation operation is introduced.

Canvas selection or an accessible source dropdown selects exactly one entity.
Offer an explicit reviewed Make construction / Make ordinary operation according
to its current source flag. Emit only SetSketchConstruction through the existing
SketchEdit, DesignPatch, transaction, verification, publication and opaque history
gateways. Preserve all IDs, references, coordinates, expressions, constraints,
owners and workplanes. Never reconstruct a circle/ellipse from displayed numeric
values merely to change its flag. Keep existing raw forms and pointer tools.

The backend plan exposes before/after closed profile references and exact entity
members, plus the existing consumers of the selected sketch's profiles. The UI
shows these actual consequences and the verified candidate solid volume (or no
solid). Construction curves are omitted from profiles; point flags only mark the
point and do not exclude curves that reference it. Do not label every ordinary
point or open curve as a closed profile. Existing unchanged-consumer binding
guards still refuse a changed/missing loop, including silent outer-loop rebinding.
No silent consumer removal, suppression or reassociation is offered here.

Review is read-only; Apply rechecks current identity/revision, solve, profile,
assertions and native geometry before atomic publication/history. Target, sketch,
tool, span, file and upstream changes, Escape/discard and late real IPC cancel
old reviews and selections. Candidate-only IDs never become selectable. Use
backend profile detection rather than a frontend solver/profile inference.

## Acceptance

- Both genuine analytic Mock and native OCCT: convert a construction circle
  with authored radius `1 cm` and centre (2,-3) mm to ordinary, then
  extrude 6 mm. Independent V=600*pi mm3, A=320*pi mm2; absolute XY/YZ/XZ
  and tilted datum bounds/COM and genuine oriented closed display meshes.
  No synthetic Mock ID arithmetic. Preserve source expressions throughout.
- Point-flag changes preserve the consumed circle loop and its solid. An
  unconsumed ellipse/line/closed rectangle/arc sector can change profile eligibility
  without altering stored geometry; removing a consumed circle/ellipse/loop
  refuses with complete source/history/publication preservation. No-op, stale
  identity/revision, missing/unsupported targets, required assertions, and
  packed/expanded Undo/Redo have explicit regression evidence.
- Actual new Linux Tauri and Windows WebView2 controls: source selection,
  current state, separate make-construction/make-ordinary intent, candidate
  profile/solid consequences, read-only review/apply, canonical history,
  cancel/late IPC, upstream invalidation and consumed-profile refusal. Append
  unconditional new helper groups after the existing 44 groups in both drivers;
  include helper in workflow path policy. No fake IPC or manufactured results.
- Actual typed MCP/Agent parity and independently graded exact source flags,
  unchanged expressions/dependencies/prior source and fresh native SI STL
  volume/bounds/COM/oriented closure. Equal-volume wrong placement and a
  wrong unconsumed construction flag must fail even when solid mass agrees.
  Example, API/architecture, authoring guide, skill and feasible eval updates.
- All four mandatory local checks, complete applicable exact-head CI inventory,
  actual new Windows controls, then one task/one merge-commit PR from main.

Physical tolerances: analytic volume 1e-12 m3, area 1e-10 m2, positions 3e-7 m;
display meshes 1% volume, 1e-4 m bounds/COM and 1 nm oriented closure.
Existing geometry admission/tolerances remain unchanged. No golden changes.

Only the qualifying main merge may earn GUI 54 to 55, weighted CAD 53.95 to
54.10 (about 54%); agent operation remains 95. Credit covers the new usable
selected-flag lifecycle and explicit backend profile consequences. Existing
construction operations, raw checkbox, kernels, solver, workplanes and host
coverage receive no repeated credit; the original rubric/weights stay fixed.
