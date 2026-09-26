# ADR-021: Profiles of lines and arcs

Status: Accepted  
Date: 2026-09-26  
Roadmap: MCAD-P7-015

## Context

A closed sketch profile could only be a single circle or a loop of lines.
Profile detection traced loops through the point entities that lines
share. An arc stored only its centre, radius, and start and end angles, so
it could not join a loop. Slots, rounded rectangles, and D-shapes, which are
among the most common outlines in real parts, could not be drawn.

## Decision

1. **Arc endpoints.** `ArcEntity` gains optional `start_point` and
   `end_point` point IDs. Both fields are omitted when absent, so existing
   documents are unchanged. The arc still runs counterclockwise from
   `start_angle` to `end_angle`.
2. **Solver.** For each endpoint point, the solver adds two residuals, in
   metres:
   - `x − (cx + r·cos θ)`;
   - `y − (cy + r·sin θ)`.

   Here θ is the endpoint's angle. Angles must be literals or constant
   angle expressions such as `90 deg`; parameter-driven arc angles are not
   supported yet. The radius stays a solver variable, so a radius or
   parameter edit moves the endpoint points with the arc and keeps the loop
   closed.
3. **Profile detection.** Lines, and arcs with both endpoints, are traced as
   loop segments through shared point IDs. A loop closes with three
   segments, or with two when one is an arc (a half-disc). Arcs without
   endpoints are not loop segments, as before.
4. **Kernel input.** A loop that contains an arc reaches the kernel as
   ordered `ProfileSegment`s in `SolvedSketch.segments`:
   - `Line { start_m, end_m }`;
   - `Arc { start_m, mid_m, end_m }`, where the mid point follows the loop
     direction.

   `points` holds a polygon with 16 samples per arc, for kernels without
   curves. The OCCT backend builds exact line and three-point arc edges
   (`Edge::line` and `Edge::arc_3pts`). Line-only loops keep the polygon
   path, so existing results are unchanged.
5. **Validation.** Arc endpoint IDs are point references, so patch validation
   checks that they exist, and removing a referenced point fails closed.

## Consequences

- Slots and other line–arc outlines extrude and cut with exact areas.
- Arc angles did not follow parameters at first. ADR-024 resolves
  parametric arc angles before solving.

## Evidence

- A sketch test checks that a half-disc (chord plus arc) closes, and that an
  arc without endpoints does not.
- A solver test checks that endpoint points land on the arc at 0° and 90°.
- `examples/agent/author_slot_plate_patch.json` authors a 60 × 40 × 5 mm
  plate with a 30 × 8 mm slot from an empty document. The patch is fully
  constrained, with no dry-run warnings. OCCT regenerates the analytic
  volume within `1e-12 m³`, both as authored and after widening the slot to
  10 mm.
