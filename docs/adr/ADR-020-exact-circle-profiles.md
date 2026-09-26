# ADR-020: Exact circle profiles

Status: Accepted  
Date: 2026-09-26  
Roadmap: MCAD-P7-013

## Context

Sketch profiles reach the geometry kernel as `SolvedSketch`, a list of 2D
points. A circle profile was turned into an inscribed 32-sided polygon
(`CIRCLE_SEGMENTS` in `sketch_bridge`), so every hole, boss, and revolved
bore was faceted.

The MCP evaluation harness (MCAD-P7-012) exposed the error. A Ø10 mm hole
in the 6 mm bracket removed 468.22 mm³ instead of π·5²·6 = 471.24 mm³, 0.64%
too little. The golden fixtures had recorded the polygon value as correct.
For a CAD system, a hole that is not round is a fidelity defect, not a
tessellation choice.

## Decision

1. **Exact circle in the profile.** `SolvedSketch` gains an optional
   `circle: SolvedCircle { center_m, radius_m }`, in metres, set when the
   profile is a single sketch circle. The field is optional and omitted
   when absent, so serialized `SolvedSketch` values stay compatible.
2. **Polygon kept for other kernels.** `points` still holds the inscribed
   polygon, for kernels without exact curves such as the mock kernel.
   Kernels that support curves must use `circle` when it is present.
3. **OCCT builds a true circle.** The OCCT backend builds one circular edge
   (`Edge::circle`) with the sketch placement's normal and translates it to
   the mapped centre. Extrude, hole, and revolve all go through this
   conversion.
4. **Out of scope.** Arcs inside line loops remain unsupported, as before.

## Consequences

- **Exact volumes.** Hole and boss volumes become analytic. The bracket
  now regenerates to 28 328.7611 mm³ against the analytic 28 328.7611 mm³.
- **Golden fixtures.** The bracket, chamfer, fillet, and MCAD-P5-005
  end-to-end fixtures lose exactly the material the polygon left in each
  hole: 3.02 mm³ per Ø10 × 6 mm hole. Their tolerances are unchanged.
- **Topology.** A circular hole wall is one cylindrical face instead of
  32 planar faces. Face discoveries and semantic references keep resolving
  through roles and fingerprints (ADR-018).
- **Face roles.** The render face catalog used to take a kernel face's role
  from its first triangle, which labelled a single cylindrical face `+x`.
  A kernel face whose triangles point along different axes is now
  classified as cylindrical.
- **Tessellation.** Tessellation is finer for curved faces. The robot arm
  review goes from 2 344 to 4 296 triangles, and the review goldens were
  regenerated. Only the mass, volume, and triangle-count values changed.
- **Shell.** A shell whose open face is pierced by a through hole now
  succeeds (ADR-017).

## Evidence

- The OCCT backend extrudes a circle profile to π·r²·h within `1e-12 m³`.
- `opencad regen examples/bracket.ocad.d` matches the analytic volume within
  `1e-5 mm³`.
- The MCP end-to-end test expects the analytic plate-minus-hole volume.
