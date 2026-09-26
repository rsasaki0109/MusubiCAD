# ADR-025: Helical sweep

Status: Accepted  
Date: 2026-09-26  
Roadmap: MCAD-P7-026

This ADR passes the future-geometry admission gate (ADR-012) for sweeping a
closed profile along a helix. The numbered sections answer the gate's
requirements in order.

## 1. User story and non-goals

A designer makes springs, coils, and helical ribs or grooves: a section
moves along a helix around an axis. The result can be a new body, joined to
a body, or cut from a body.

Non-goals:

- left-handed helices;
- tapered (conical) or variable-pitch helices;
- helices from existing body edges;
- standard thread tables (ISO/UNC profiles).

## 2. Design Graph representation

`FeatureDefinition::HelixSweep` holds:

- `sketch_feature` and `profile_ref`: a closed section;
- `axis_origin_m` and `axis_direction_m`: the helix axis, in metres;
- `pitch_m` / `pitch_expr`: axial advance per turn;
- `height_m` / `height_expr`: total axial length;
- `operation` and `target_feature`, as for extrudes.

The helix has no radius field. It passes through the section's centre (the
circle centre, or the mean of the solved profile points), so its radius is
that centre's distance from the axis. A sketch constraint that moves the
section therefore also changes the coil radius, and the two cannot disagree.

## 3. Units, validation, and tolerances

- Pitch and height are lengths in metres; expressions resolve like other
  feature lengths. Each must be at least `MIN_FEATURE_LENGTH_M` (1e-9 m).
- At most `MAX_HELIX_TURNS` (1000) turns (`height / pitch`). More is almost
  certainly a unit mistake.
- The axis direction must be non-zero.
- The section centre must lie more than 1e-9 m off the axis.
- `join` and `cut` need `target_feature`.

## 4. Determinism, TopoRefs, and failure

- **Start.** The helix starts at the section centre and turns
  counterclockwise about the axis direction (right-handed), rising along it.
- **Section orientation.** `ProfileOrient::Torsion`, the Frenet frame. It is
  constant along a helix, so the section moves by an exact screw motion.
- **Approximation.** OCCT approximates the helical pipe surface with
  B-splines. Volumes agree with the screw-motion value within a relative
  5e-4. The measured error was 4.5e-5 for 4 turns and 1.3e-4 for 6 turns.
- **Failure.** A degenerate helix or an OCCT rejection fails regeneration,
  and the document is unchanged.

## 5. Transaction and DesignPatch contract

Helical sweeps use the structural feature operations. Their Feature Graph
inputs are `sketch_feature` and `target_feature`. `pitch_expr` and
`height_expr` are reported as expressions, so parameter edits dirty the
feature. `set_feature_expr` does not target these fields yet; edit the
parameters they name.

## 6. File format

No migration is needed. `helix_sweep` is a new tagged variant, and the
patch schema lists it.

## 7. Kernel contract

`GeometryKernel::helix_sweep(profile, helix: &HelixSpec)` is added with a
default "unsupported" implementation.

- **OCCT.** Builds `Edge::helix` (an exact helix on a cylinder) through the
  section centre and calls `Solid::sweep` with `ProfileOrient::Torsion`.
- **Mock.** Validates the helix and returns a deterministic body.
- **Counting kernel.** Forwards the call.

## 8. Evidence

- `examples/agent/author_coil_spring_patch.json` authors a coil spring from
  an empty document:
  - a Ø2 mm wire section on the XZ plane, 10 mm off the Z axis;
  - a 5 mm pitch over 20 mm (4 turns).

  The section sketch is fully constrained, with no dry-run warnings.
- The volume matches the screw-motion value `πa²·2πR·turns` (Pappus for a
  screw motion). After the coil radius is edited to 12 mm and the height to
  30 mm (6 turns), it matches again.
- The tessellated extents reach `R + a` in x and y, and span `−a` to
  `height + a` in z.
- A section on the axis is rejected.

## 9. Dependencies and performance

There are no new dependencies. A helical sweep is one OCCT pipe operation
per regeneration. It is slower than straight sweeps, taking a few seconds
per regeneration of the test coils in a debug build.
