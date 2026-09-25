# ADR-017: Shell feature

Status: Accepted  
Date: 2026-09-25  
Roadmap: MCAD-P7-007

This ADR passes the future-geometry admission gate (ADR-012) for one feature
family: shelling (hollowing) a solid. The numbered sections answer the gate's
requirements in order.

## 1. User story and non-goals

A designer turns a solid into a thin-walled housing of uniform wall thickness,
leaving chosen faces open. A typical case is an enclosure whose top face is
removed.

Non-goals:

- per-face thicknesses;
- shelling several bodies at once;
- outward (positive) walls in this first scope, because the wall always grows
  inward;
- automatic face picking by geometry.

## 2. Design Graph representation

A new `FeatureDefinition::Shell` holds:

- `target_feature`: the body to shell;
- `thickness`: a `Length`;
- `thickness_expr`: an optional parametric expression, resolved like other
  `*_expr` fields;
- `open_face_refs`: semantic face references (`ref:face:*`) that become
  openings.

The persisted input is the semantic references, never kernel face IDs.

## 3. Units, validation, and tolerances

- `thickness` is in metres. It must be finite and greater than
  `SHELL_MIN_THICKNESS_M = 1e-6 m`.
- `open_face_refs` must be unique, existing semantic face references, and at
  least one is required.

   Shelling with no opening builds a sealed solid with an internal void. That
   is rarely intended and cannot be reviewed visually, so it is rejected.

## 4. Determinism, TopoRefs, and failure

- **References fail closed.** Each open face reference is resolved to a
  current kernel face through the existing reference resolution: kernel
  history, then fingerprint discoveries. The discoveries are computed on the
  target body itself (`RegenContext::face_discoveries_on`), because the
  session's discoveries describe the most recently regenerated body, whose
  face IDs need not exist on an earlier target. If any reference cannot be
  resolved, regeneration fails instead of guessing. This is stricter than
  fillet's role fallback, because opening the wrong face silently changes the
  part.
- **OCCT failures.** OCCT rejections, such as self-intersecting offsets at
  sharp corners, fail the feature's regeneration, and the document is
  unchanged. For a wall that does not fit, OCCT can return the input
  unchanged instead of failing. The backend therefore requires a relative
  volume loss above `1e-9` and fails otherwise.
- **Known limitation.** OCCT cannot shell a body whose open face is pierced
  by a through hole. Users shell first and cut the hole afterwards.
- **New faces.** Faces created by the shell receive no semantic roles.

## 5. Transaction and DesignPatch contract

- **Operations.** A shell is added, replaced, or removed with the structural
  feature operations (ADR-013). `set_feature_expr` supports the field
  `thickness_expr`.
- **Validation.** Final-state validation checks the thickness, the
  expression's parameters, and that every open face reference exists.
- **Graph edges.** `target_feature` is a Feature Graph input. Each open face
  reference adds an edge from its creating feature unless that feature is
  already upstream.

## 6. File format

No schema migration is needed. The feature is a new tagged variant inside the
existing `graph/features.json`. `schemas/ocad.patch.schema.json` lists
`shell` among feature types.

## 7. Kernel contract

`GeometryKernel::shell_body(body, thickness_m, open_faces: &[FacePick])` is
added with a default "unsupported" implementation. A `FacePick` is a point on
the face (or in its plane) in metres plus the outward unit normal. The shell
executor takes both from the face discovery that the reference resolved to.

Faces are picked by geometry rather than by kernel face ID, for two reasons:

- In OCCT, an extrusion's top and bottom faces share one underlying
  `TShape`, so the TShape-based face ID does not tell them apart.
- Tessellation reads face IDs from a deep copy of the solid, so those IDs do
  not name faces of the stored solid.

- **OCCT.** Wraps `cadrum::Solid::shell` (`BRepOffsetAPI_MakeThickSolid`)
  with a negative thickness, so the wall grows inward. Each pick selects
  the stored solid's planar face whose boundary lies in the pick plane:
  - every sampled point of the face's boundary edges must lie within
    `1e-6 m` of the plane through the pick point with the pick normal
    (edges are sampled at `1e-5 m` chordal deflection);
  - among matching faces, the one whose boundary-sample centroid is nearest
    the pick point wins.

  If no face matches, or two faces are equally near, the pick fails.
  `Face::project` is not used, because cadrum panics when OCCT cannot
  project a point onto a face (observed on cylindrical hole faces).
- **Mock.** Returns a deterministic body derived from the inputs, so pipeline
  tests run without OCCT.

Wrapper kernels, such as the counting kernel, must forward the method
explicitly. A regression test guards this.

## 8. Evidence plan

- **Pure tests:** thickness and open-face validation, and graph edges.
- **OCCT integration.** A 60 × 40 × 20 mm box, authored by
  `examples/agent/add_shell_patch.json`, is shelled 2 mm with its top face
  open. The volume must equal the analytic
  `60·40·20 − 56·36·18 = 11 712 mm³` within `1e-9 m³`. A feature-level test
  shells an 80 × 60 × 20 mm plate to `80·60·20 − 76·56·18 = 19 392 mm³` and
  checks that the bounds are unchanged. It also checks that the centre of
  mass lies below mid-height. Opening the bottom face would give the same
  volume and bounds, so only the centre of mass shows that the top face was
  opened.
- **Failure paths:** an unresolved open face reference fails closed, and an
  over-thick wall fails regeneration.
- **Patch round trip:** a structural patch adds the shell, and dry-run
  predicts it as dirty.

## 9. Dependencies and performance

There are no new dependencies. Shelling is one OCCT offset operation per
regeneration, and the content-addressed cache reuses it while inputs are
unchanged.
