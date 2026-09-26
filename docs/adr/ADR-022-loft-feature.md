# ADR-022: Loft feature

Status: Accepted  
Date: 2026-09-26  
Roadmap: MCAD-P7-018

This ADR passes the future-geometry admission gate (ADR-012) for one feature
family: lofting a solid through cross-sections. The numbered sections answer
the gate's requirements in order.

## 1. User story and non-goals

A designer blends between outlines at different heights, for example a
square base tapering to a smaller square or to a circle: nozzles, transition
ducts, tapered bosses. The result can be a new body, joined to a body, or
cut from a body.

Non-goals:

- guide curves;
- ruled versus smooth control;
- sections on non-parallel planes chosen by face references (sections use
  sketch workplanes);
- open (surface) lofts.

## 2. Design Graph representation

`FeatureDefinition::Loft` holds:

- `sections`: two or more `{sketch_feature, profile_ref}` pairs, in skinning
  order;
- `operation`: `new_body`, `join`, or `cut`;
- `target_feature`: the body to join or cut, when needed.

Each section is a closed sketch profile placed by its sketch's workplane.
Custom workplanes give each section its own plane.

## 3. Units, validation, and tolerances

- There must be at least two sections, with no section listed twice.
- `join` and `cut` need `target_feature`.
- Every section profile must resolve to a closed profile. This uses the same
  check as extrudes, holes, and revolves, which now share
  `FeatureDefinition::profile_inputs()`.
- Coordinates are metres through the existing sketch pipeline.

## 4. Determinism, TopoRefs, and failure

- **Section order.** Sections are skinned in the stored order. Profiles use
  the ADR-020 and ADR-021 segment representation, so circles and arcs stay
  exact.
- **Failure.** An OCCT rejection fails regeneration, and the document is
  unchanged.
- **Face roles.** Lofted faces receive no semantic roles.

## 5. Transaction and DesignPatch contract

Lofts are added, replaced, and removed with the structural feature
operations (ADR-013). Each section's `sketch_feature` and the
`target_feature` are Feature Graph inputs, under the fields `sections` and
`target_feature`.

## 6. File format

No migration is needed. `loft` is a new tagged variant in
`graph/features.json`, and the patch schema lists it.

## 7. Kernel contract

`GeometryKernel::loft(sections: &[SolvedSketch])` is added with a default
"unsupported" implementation.

- **OCCT.** Builds each section's edges with its placement and calls
  `Solid::loft` (`BRepOffsetAPI_ThruSections`, solid, smooth).
- **Mock.** Returns a deterministic stand-in body.
- **Counting kernel.** Forwards the call.
- **Booleans.** `join` and `cut` combine the lofted body with the target
  through `GeometryKernel::boolean`.

## 8. Evidence plan

- `examples/agent/author_loft_frustum_patch.json` authors a square frustum
  from an empty document with no dry-run warnings. The base is 40 mm, the
  top is 20 mm, and the top sits 30 mm higher.
- The volume equals `h/3·(A₁ + A₂ + √(A₁A₂))` = 28 000 mm³ within 0.01 mm³.
  After the top is edited to 30 mm, it equals 37 000 mm³.
- A single-section loft is rejected at dry-run.

## 9. Dependencies and performance

There are no new dependencies. A loft is one OCCT operation per
regeneration and is cached like other features.

## Related fix: custom workplane axes

Implementing lofts exposed a bug in how custom workplanes were placed. The
placement ignored the workplane normal and computed the y axis as `+y × x`.
A section on a raised XY workplane was therefore placed vertically. The
same bug affected sketches on face references. The pin sketched on the
bracket's top face in `examples/bracket_face_pin.ocad.d` stood on its side:
its top reached z = 11 mm instead of 12 mm, although the volume was the same.
The y axis is now `normal × x_axis`, so `x × y` points along the workplane
normal.
