# ADR-023: Sweep feature

Status: Accepted  
Date: 2026-09-26  
Roadmap: MCAD-P7-022

This ADR passes the future-geometry admission gate (ADR-012) for sweeping a
closed profile along a path. The numbered sections answer the gate's
requirements in order.

## 1. User story and non-goals

A designer moves a cross-section along a path of straight runs and bends:
pipes, handles, rails, and grooves. The result can be a new body, joined to
a body, or cut from a body.

Non-goals:

- 3D (non-planar) paths;
- twist and scale laws;
- guide rails;
- paths built from edges of existing bodies.

## 2. Design Graph representation

`FeatureDefinition::Sweep` holds:

- `sketch_feature` and `profile_ref`: a closed section;
- `path_sketch_feature`: a sketch whose non-construction lines and endpoint
  arcs (ADR-021) form exactly one chain, open or closed;
- `operation` and `target_feature`, as for extrudes.

## 3. Units, validation, and tolerances

- The section and the path must come from different sketches.
- `join` and `cut` need `target_feature`.
- The section profile must be closed. This uses the shared
  `profile_inputs` check.
- The path must not branch, and must be a single chain.
- Sketches may now hold only an open chain. `validate_sketch` requires some
  profile rather than a closed one, and every consumer of a closed profile
  checks closure itself.

## 4. Determinism, TopoRefs, and failure

- **Path start.** An open path starts at the end nearest the section's
  centroid, where the designer places the section, so the direction never
  depends on entity order. A closed path starts at its first segment.
- **Section orientation.** The section keeps its orientation relative to the
  path plane normal: `ProfileOrient::Up(normal)`. Straight runs have no
  curvature, so a Frenet frame is undefined there.
- **Failure.** An OCCT rejection fails regeneration, and the document is
  unchanged.

## 5. Transaction and DesignPatch contract

Sweeps use the structural feature operations. Their Feature Graph inputs are
`sketch_feature`, `path_sketch_feature`, and `target_feature`.

## 6. File format

No migration is needed. `sweep` is a new tagged variant, and the patch
schema lists it.

## 7. Kernel contract

`GeometryKernel::sweep(profile, path)` is added with a default "unsupported"
implementation.

- **OCCT.** Builds the path's line and arc edges with the path placement and
  calls `Solid::sweep`.
- **Mock.** Returns a deterministic body.
- **Counting kernel.** Forwards the call.

## 8. Evidence plan

- `examples/agent/author_sweep_elbow_patch.json` authors a pipe elbow from
  an empty document:
  - a 5 mm radius section;
  - a path that rises 30 mm, then takes a 20 mm radius quarter bend.

  Both sketches are fully constrained, with no dry-run warnings.
- OCCT matches Pappus' volume `πr²·(30 + π/2·20)` within 0.01 mm³. After the
  bend radius is edited to 30 mm, it matches again.
- The tessellated extents reach z = 55 mm and x = 20 mm.
- OCCT's bounding boxes of swept B-spline faces are padded by the control
  points (57.06 mm here), so extents come from the tessellation.

## 9. Dependencies and performance

There are no new dependencies. A sweep is one OCCT pipe operation per
regeneration.
