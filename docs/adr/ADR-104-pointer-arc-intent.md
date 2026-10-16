# ADR-104: Backend-owned pointer arc intent

Status: Accepted with MCAD-P17-006 main merge
Date: 2026-10-16
Roadmap: MCAD-P17-006, after P17-020 / PR #154

## Decision

Extend ADR-102's existing sketch planner/applicator with a creation-only `arc`
form: fresh `id`, explicit local length strings `center_x`, `center_y`,
`start_x`, `start_y`, `end_direction_x`, `end_direction_y`, and required boolean
`clockwise`, `close_sector`, `construction`. Inputs are constant unit-bearing
expressions; named parameters refuse because this creation intent derives
ordinary source and does not create an associative helper. Existing parametric
Arc source through DesignPatch / ADR-024 remains supported.

The native tool takes three clicks: centre, radius/start point, then end
direction. Its labelled CW/CCW control selects the branch, including major arcs.
The end click specifies a direction from the centre, not a new radius. The
backend alone computes radius, angles and the endpoint on that circle. The
frontend computes only disposable SVG preview/input intent; it never publishes
derived source or a physical measurement.

Store three ordinary points (`_center`, `_start`, `_end`) and one Arc (`id`)
in one guarded DesignPatch. Preserve centre/start input expressions; derived
endpoint/radius use explicit metre/millimetre strings with sufficient fixed
precision, and angles use explicit radians. Existing Arc source is CCW: for a
CW gesture, swap its endpoint references and canonical source angles to encode
the same selected curve. No persisted direction flag, new entity or schema is
introduced. `_start` still names the first clicked point. Zero/full/ambiguous
turns refuse; never infer the shortest arc.

Optional **Close with radii** adds `_start_ray` (centre to canonical Arc start)
and `_end_ray` (canonical Arc end to centre) in that same patch. These share
actual point IDs and yield exactly the ADR-103 physical sector subset without
asking the user to write IDs or JSON. Construction marks the Arc/radii; points
retain existing point-form semantics. There is no implicit constraint group or
associative arc helper after later point edits.

Use ADR-103 admission for the derived local fan: finite positions within 1e6 m,
radius (1 nm,1000 m], distinct endpoints/through-point above 1 nm, existing
angular/roundoff tolerances and exact selected branch. Reject missing identity,
stale source/datum, any ID collision, wrong/missing units, unsupported expressions
and expanded batches over 64 source operations. Four additions for an open arc,
six for a sector; late failures publish none of them.

Admit a successfully solved, otherwise admitted unconsumed construction draft
with bounded explicit-endpoint arcs. Profile consumers remain strict. Required
assertions, native candidate verification, file publication and opaque history
stay in the shared gateway. Cancel/Escape/control/file/upstream changes discard
half gestures/reviews, including late real IPC responses. One apply/Undo/Redo
handles every addition. NURBS/generated source stays outside editing admission.

## Acceptance

- Actual Linux Tauri and Windows WebView2 three-click/branch/closure controls,
  read-only review, atomic apply/history, cancel and stale refusal. Both native
  scripts execute the new helper unconditionally; workflow path policy includes it.
- Genuine analytic Mock and OCCT sector mass, surface area, centroid, absolute
  bounds and closed outward meshes, including all global planes and a tilted
  datum; cold/cached empty-parameter regeneration and packed/expanded history.
- Independent minor/major/CW/CCW branch evidence. For centre (2,-3) mm, start
  (12,-3) mm, end direction (2,17) mm and 6 mm depth: CCW quarter V=150*pi mm³,
  A=80*pi+120 mm², planar centroid (2+d,-3+d) mm, d=40/(3*pi) mm. CW major has
  V=450*pi mm³, A=240*pi+120 mm², centroid (2-d/3,-3-d/3) mm. Both kernels and
  native controls must distinguish them, rather than choosing the minor branch.
- Actual same-intent MCP/Agent parity, independent exact source and fresh native
  SI export eval, refusal preserving bytes/history/output, wrong-branch/source
  and equal-volume wrong-placement controls, example/API/guide/skill updates.
- All four required local checks and full applicable exact-head CI inventories.

Only a qualifying main merge with new native acceptance may earn GUI credit;
existing arc/sector geometry, workplanes, old forms and physical parity receive
no repeat credit. Current weighted CAD 53.50 / GUI 51 / agent 95 remain unchanged
until that merge. No new sampler, spline/NURBS editing or external host claim.
