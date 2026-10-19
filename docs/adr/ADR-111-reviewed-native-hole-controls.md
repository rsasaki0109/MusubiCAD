# ADR-111: Reviewed native controls for admitted standard holes

Status: Accepted for MCAD-P17-012.
Date: 2026-10-18.

## Context

ADR-037/063/064 implement standard bores and explicit counterbore/countersink
seats. Native creation still requires authored patches. ADR-058/102 require
shared backend admission, reviewed DesignPatch and protected history for usable
controls. A plausible volume or a frontend face index cannot establish entry
ownership, inward direction, contact dimensions or stock preservation.

## Decision

Expose a read-only, document/revision-bound catalogue of the current original
right-rectangle distance/NewBody stock, owned semantic end entries and existing
authored exact circle profiles. The selected circle is the axis and diameter;
its regenerated plane must coincide with the selected entry and point outward.
Use existing custom/face/datum workplanes without editing or copying them.
Global-plane circles are excluded because the legacy plain-hole direction
contract differs. Exclude projected/helper/aliased/suppressed owners, consumed
entries, already modified or nonactive stock, side/oblique entry and unsupported
profiles. Do not invent an axis, face reference, workplane or diameter.

Factor the existing seated-hole input guard into a shared read-only admission
query. Require actual rectangular-stock and unique semantic face witnesses,
source/physical dimensions, complete radial containment and bounded positive
depth. This introduces no geometry or sampling admission. Both kernels retain
their independent existing implementations; default synthetic Mock refuses.

The transport intent selects target, face reference, sketch owner/profile,
explicit existing standard, name, unit-bearing distance and a closed optional
seat choice: none, counterbore (diameter/depth) or countersink (diameter/included
angle). Numeric source fallbacks are resolved in SI while original expressions
remain source. Parameter kinds are strict; constants require units. Backend
assigns a globally free feature identity. One AddFeature DesignPatch appends the
existing standard_hole or seated_standard_hole source without rewriting stock,
axis, references or parameters. No persisted schema changes are required.

Review verifies the whole source, required assertions, actual candidate body,
display mesh, independently specified removal volume and first moments. Expose
entry/axis in metres, inward unit vector, bore/seat contact dimensions and removed
volume. Plain holes additionally use the same source/ownership guard and physical
postconditions as seated holes. Missing evidence refuses. Apply replans from
the current source, clones source/history, uses the shared transaction and
rechecks file identity/revision before publishing. Expanded/packed files and
canonical Undo/Redo use the existing contracts.

Native controls list backend choices and labelled units, never raw patch JSON.
All form/source/view/tool/upstream/reload/cancel changes invalidate review;
sequence/path/revision checks discard late real IPC results. No frontend geometry
or cached witness becomes mutation authority. Tauri, Agent and MCP call the same
inspect/plan/apply functions. Existing controls keep their compatibility gates.

## Acceptance and consequences

Independently measure bore, annular counterbore and frustum-minus-bore removal,
first moments, unchanged stock bounds, actual closed oriented mesh and radial/
axial contacts on both analytic Mock and native OCCT, both end entries and rigid
placements. Exercise parametric edits, exact original source, globally free IDs,
stale/foreign/unsupported/units/depth/containment/assertion refusals, packed and
expanded read-only review/failure/history and actual MCP/Agent parity. Independent
eval must reject equal-volume wrong-axis/entry/source changes and use a fresh
native export. Include an executable example, guide and skill updates, actual
new Linux and Windows native form interactions, all four mandatory checks,
complete exact-head CI and a main merge before completion/credit.

The admitted controls do not create arbitrary face sketches, infer fastener head
dimensions, model physical threads or machine already-cut stock. Broader geometry
needs a registered prerequisite and ADR. Only newly usable native creation earns
conditional GUI credit; existing standards/kernel/host evidence earns none.
