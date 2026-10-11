# ADR-036: Neutral-plane draft by convex half-spaces

Status: Accepted for MCAD-P11-002

## Context

cadrum 0.8.5 exposes OCCT half-spaces and Booleans but no draft builder.
Introducing a second native bridge would couple builds to private cadrum
types or require another OCCT installation. Manufacturing draft needs a
well-defined angle, neutral plane and fail-closed semantic side selection.

## Decision

Add `DraftFeature` with a signed SI `Angle`, optional angle expression,
neutral `Workplane` (including datums) and mandatory unique semantic face
references. Resolve faces only on the target body. Derive all datum, body,
reference and expression dependencies through the existing graph projection.
AI creates/replaces it through structural DesignPatch; regeneration remains
transactional and cached. No OCCT types cross the backend boundary.

OCCT supports one convex planar solid with 4–64 faces. Selected faces must
initially be perpendicular to the neutral plane. Positive angle narrows the
solid toward the plane normal; negative angle widens it. The intersection
of the old side plane and neutral plane stays fixed. Rotate its normal by
`n*cos(angle) + neutral_normal*sin(angle)`; rebuild by intersecting inward
half-spaces. Angle magnitude is 1e-8 rad through 45 degrees.

Face planes use tessellated triangle normals; boundary sampling uses existing
cadrum edge tessellation (1e-6 m linear,
0.05 rad angular). Planarity requires boundary distance within 1e-7 m and
tessellated triangle normals within 1e-8 of unit dot agreement. Sampling alone
does not
certify a surface: compare the source and undrafted reconstruction using
both volumes and their Boolean intersection. Each symmetric-difference
side must be within max(1e-15 m³, source volume * 1e-7). This intentionally
rejects cavities, curved and non-convex shapes; changes below that numerical
tolerance cannot be distinguished. Clip with a guard box padded by four
source diagonals, and refuse a result that reaches it, collapses, or removes
a selected face. Source diagonals must be finite and exceed 1e-7 m. Backend
panics become errors without publishing
an output handle. No general NURBS draft or adaptive sampling is introduced.

Mock validates the same numeric contract and returns a deterministic input
hash handle. Its existing synthetic geometry is not a physical draft model;
OCCT integration tests supply physical analytic volume and dimension evidence.

## Consequences and acceptance

This delivers bounded planar draft without a dependency or ABI change.
General curved/concave draft needs a future native operation and ADR.
Rebuilt topology has no fabricated derivation history; semantic resolution
uses existing current-body discoveries and fails on ambiguity.
Tests cover inward/outward wedge volume, bounds, cache invalidation,
missing references, atomic rollback, unsupported curves and invalid angles.
Additive schema tags leave legacy files unchanged; new files round-trip and
rebuild through the authoring patch. No golden fixture changes are needed.
