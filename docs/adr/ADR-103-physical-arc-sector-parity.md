# ADR-103: Bounded physical arc-sector kernel parity

Status: Accepted with MCAD-P17-020 main merge
Date: 2026-10-16
Roadmap: MCAD-P17-020, prerequisite to MCAD-P17-006

## Context

ADR-021 already supplies exact line/three-point-arc profiles to OCCT. The
explicitly selected physical analytic Mock admits rectangles and circles but
rejects curved profiles. Its legacy synthetic ID geometry or the 16-point
compatibility polygon cannot substantiate independent physical arc tests.
P17-006 must have genuine both-kernel evidence before exposing pointer arcs.

## Decision

Add a private analytic Mock sector profile/body record for exactly one arc and
two straight radii in an ordered closed loop. Existing OCCT exact edges remain
the native implementation. This portable subset is a prerequisite, rather than
an implicit geometry expansion inside a GUI task. A shared kernel-neutral validation function identifies the portable subset.
Native sector extrusion/rigid-placement records select oriented triangle-derived
display normals, fixing the legacy +Z placeholder for these faces. This does not
alter the exact native arc solid. No new public kernel trait method,
persisted source field, schema or tool is needed.

Resolve source Arc radius and angle expressions with the existing strict
length/angle evaluators before solving, including empty/absent parameter
graphs. Otherwise the solver's optional radius seed can silently discard a
wrong-unit parse error and fit another radius from endpoints. Only disposable
regeneration/inspection copies resolve; authored expressions remain unchanged.

The two lines must meet at a unique circle centre, and their other endpoints
must be the arc endpoints. All three arc points must lie at the same finite
positive radius from that centre. Validate continuous closure; identify shared
points using arithmetic roundoff, without snapping distinct thin gaps. Radius
consistency is bounded by the minimum of the existing 1 nm position tolerance
and the existing arithmetic-roundoff bound (64*epsilon*max(1,r) in SI). Reject duplicate arc
points, ambiguous zero/full turns, nonfinite values and invalid placement.
Use the existing 1000 m dimension / 1e6 m position / proper orthonormal frame
bounds. Clockwise and counterclockwise input, including unambiguous major arcs,
are distinguished by the through-point; no shortest-arc heuristic is allowed.

Blind and symmetric normal NewBody extrusion follow existing extent semantics.
Oblique/through-all extrusion, sector booleans/cuts/joins, chord segments,
general mixed line-arc loops and new stock witnesses are outside Mock admission
and refuse. Existing broader OCCT capability is preserved. Do not identify a
sector as a cuboid or a complete cylinder from its bounding box.

For radius r, positive sweep magnitude t and height h, exact physical values
are V=r²*t*h/2, A=r²*t+r*(t+2)*h. The in-plane centroid is
4*r*sin(t/2)/(3*t) along the selected arc bisector; height is h/2.
World bounds include the centre, endpoints and analytic coordinate extrema
on the selected arc at both extrusion ends. Rigid transforms preserve these
records and stable local face keys. In physical Mock, a transformed body owns
new face IDs with explicit one-to-one ancestry to its source; it does not reuse
another body's owned IDs. Native retains its body-scoped enumeration convention.
No sampled-polygon mass is reported.

Display tessellation reuses ADR-063's existing linear/angular deflection rule,
half-step margin and 8192 angular-sample budget. Restrict that angular grid to
the selected arc and include both endpoints. Pair cap fans, curved-wall quads
and two radial quads through the existing oriented-manifold builder. Faces
remain two caps, one curved wall and two radial walls. Display approximation
never changes analytic measurements. No adaptive sampler or general polygon
triangulator is introduced; unattainable settings and collapsed float meshes
refuse using the existing checks.

## Acceptance

- Independent quarter-, half- and major-sector volume, surface area, centroid
  and absolute world bounds in physical Mock and OCCT. Include reversed loop
  direction, translated/tilted frames, positive/symmetric extents and rigid
  placement; declare native and display tolerances independently.
- Fresh closed outward meshes, density scaling, deterministic repeated queries,
  owned face history and explicit refusal of unsupported witnesses/operations.
  Invalid requests must preserve existing bodies and their measurements.
- Regeneration from authored Arc/Point/Line source, cold/cached and explicit-unit
  source preservation; verified DesignPatch/Agent/MCP history and refusal,
  expanded/packed round trips, an example, guide/skill updates and a feasible
  independent fresh native export eval. Equal volume with wrong placement or
  the wrong source/arc branch must fail.
- All four required pre-push checks and applicable exact-head CI. This task
  introduces no native control; new arc pointer acceptance belongs to P17-006.

## Consequences and alternatives

The native implementation is reused without narrowing its existing admission.
Only the explicit physical Mock gains this portable subset; legacy Mock behavior
is unchanged. Arbitrary rounded profiles remain future work. Using synthetic
mass, substituting a polygon for exact native arcs or loosening tolerances would
hide the parity gap. P17-020 earns no maturity credit because exact native arc
extrusion already existed. NURBS editing remains gated by ADR-012.
