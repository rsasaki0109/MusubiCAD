# ADR-112: Bounded physical rectangular-revolve parity

Status: Accepted with MCAD-P17-022 main merge
Date: 2026-10-19
Roadmap: MCAD-P17-022, prerequisite to MCAD-P17-013

## Context

Native OCCT already constructs revolves. Legacy Mock returns synthetic handles
and the physical analytic Mock refuses revolves. Neither establishes the
both-kernel physical evidence required before P17-013 exposes native controls.

## Decision

Register physical parity separately. A shared kernel-neutral admission accepts
exactly one closed, ordered, four-line right rectangle on the declared global
XY/YZ/XZ plane. An optional solved placement must equal that global frame; no
custom/face/datum sketch-plane expansion is implied. A finite nonzero axis must
lie in the profile plane and be parallel to one rectangle edge. The rectangle
must lie strictly on one radial side with inner radius above 1 nm. Native
acceptance exposed an existing sweep failure when a rectangle touches the axis;
axis-touching cylinders, crossing and near-distinct thin intervals refuse. Axis
origins may move along the same axis. Either axis direction, source winding and
either radial side retain their actual positive right-hand sweep in world space.

Require NewBody without a target, finite positive angle at most 2*pi, dimensions
above 1 nm and at most 1000 m, positions within 1e6 m and proper orthonormal
frames. Preserve the existing 1 nm / 1e-9 rotation tolerances and distinguish
arithmetic roundoff (64*epsilon*max(1,magnitude)) from geometrically distinct
gaps. Near-full partial angles inside the existing native full-circle threshold
(1e-6 rad) refuse portable admission; exact full turns remain full turns.
No profile repair, curve flattening, arbitrary axis, boolean, stock witness,
NURBS editing or serialized source/kernel-trait change is included.

Store genuine analytic annular-sector records in physical Mock. For inner/outer
radii a,b, positive sweep t and axial height h: V=(b²-a²)*t*h/2; surface area is
(b²-a²)*t+(a+b)*t*h plus 2*(b-a)*h for a partial turn. The radial centroid is
2*(b³-a³)/(3*(b²-a²)*t) times [sin(t),1-cos(t)] and axial centroid h/2.
World bounds include both radii, both axial ends, angular endpoints and each
coordinate's analytic extrema. Rigid transforms preserve exact measurements
and explicit body-owned face ancestry. Full turns have no artificial seam wall.

Display tessellation reuses ADR-063's linear/angular rule, half-step margin,
8192 angular-sample limit and existing oriented manifold builder. Pair annular
cap quads, outer/inner walls and partial-turn radial walls using
the same angular vertices. Reuse the first vertex at the full seam. Refuse
unrepresentable f32 coordinates, collapsed or inverted triangles and budget
overflow. Display approximation never supplies analytic mass. Portable native
revolve display normals come from the actual oriented triangles, preserving
the exact native solid and broader existing OCCT admission. The portable sweep
spine starts on the actual profile radial direction. This fixes the observed
negative-side 180-degree native branch error: volume was correct while COM
and angular contacts appeared on the opposite side. The generic helper start
remains available for existing nonportable native profiles.

## Acceptance

Independent tests exercise full/minor/half/major turns, both radial sides and
axis directions, reversed source winding, all global planes, translated axes
and rigid placements. Check analytic volume/area/centroid, absolute world
bounds, actual start/end and inner/outer contacts, closed outward mesh and
face ancestry. Refusals include crossing, skew/noncoplanar axis, wrong plane,
curves, invalid units/nonfinite/tiny/thin/axis-touching source, unsupported operations and
mesh budgets; preserve existing source, history and previously measured bodies.
Feature/DesignPatch/expanded/packed tests and real MCP/Agent round trips retain
source expressions, datum-axis association and canonical Undo/Redo. A fresh
native STL and source oracle reject equal-volume wrong placement/shape/axis.
Add a runnable example, guide/skill update and feasible eval. Run the four
mandatory local checks and complete exact-head CI, including unchanged native
Linux/Windows compatibility controls. UI evidence for new controls is P17-013.

## Consequences and alternatives

This prerequisite receives no maturity credit: native revolves already exist.
Synthetic mass, polygon substitution or relaxed tolerances would hide the gap.
General profiles, arbitrary oblique axes and revolve Cut/Join remain outside
physical Mock admission. No new sampler or dependency is introduced.
