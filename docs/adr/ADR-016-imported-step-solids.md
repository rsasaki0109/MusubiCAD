# ADR-016: Imported STEP solids as fixed design inputs

Status: Accepted  
Date: 2026-09-25  
Roadmap: MCAD-P7-004

This ADR passes the future-geometry admission gate (ADR-012) for one feature
family: importing STEP solids. The numbered sections answer the gate's
requirements in order.

## 1. User story and non-goals

A designer places a purchased part, such as a motor, bearing, or connector
supplied as STEP, into a part. They use it as a body, join it to a body, or cut
it from a body (for example a motor pocket), so the design can be reviewed,
merged, and exported with that geometry.

Non-goals:

- editing imported geometry;
- recovering its parametric history;
- STEP assemblies with a product tree (all solids of the file become one
  body);
- colors;
- semantic topology roles for imported faces (references can still be
  assigned later with `assign_face_ref`).

## 2. Design Graph representation

The Design Graph owns the imported bytes. They are never a cache.

- **Attachments.** `OcadDocument::attachments` maps a path under `imports/`
  to the STEP bytes. Expanded documents store each attachment as a file at
  that path; `.ocad` archives store it as an archive entry. `DesignState`
  carries the same map.
- **The `imported_solid` feature.** A new `FeatureDefinition::ImportedSolid`
  has these fields:
  - `source`: the attachment path;
  - `sha256`: the hex digest of the expected bytes;
  - `transform`: a `RigidTransform` in metres;
  - `operation`: `new_body`, `join`, or `cut`;
  - `target_feature`: required for `join` and `cut`.

  OCCT handles and B-rep never enter the graph.

## 3. Units, validation, and tolerances

- **Attachment files** use millimetres, the STEP convention. The kernel
  converts them to metres (ADR for STEP exchange: `GeometryKernel::import_step`,
  MCAD-P7-003).
- **Transform.** `transform.translation_m` is in metres and `rotation` is a
  dimensionless 3×3 matrix. A rotation must be orthonormal within `1e-9`
  (dimensionless) with determinant `+1` within `1e-9`, so mirrored or scaled
  placements are rejected.
- **Paths and digests.** Attachment paths match
  `imports/[a-z0-9_]+([.-][a-z0-9_]+)*\.(step|stp)`. `sha256` is 64 lowercase
  hexadecimal characters.

## 4. Determinism, TopoRefs, and failure

- **Determinism.** Regeneration reads the attachment, verifies its SHA-256
  against the feature (failing closed on a mismatch or a missing file),
  imports it, and applies the transform, so the same bytes produce the same
  body. The regeneration cache key already covers the feature definition,
  including `sha256`.
- **TopoRefs.** Imported faces receive kernel IDs but no semantic roles.
- **Failure.** A failed import fails that feature's regeneration like any
  other feature. Structural edits are validated before commit, so the document
  and history are unchanged on failure.

## 5. Transaction and DesignPatch contract

New operations:

- `add_attachment { path, sha256, content_base64 }`: the decoded bytes must
  match `sha256`.
- `remove_attachment { path }`: fails closed while an `imported_solid` uses
  the attachment.

The imported solid itself is an ordinary `add_feature`.

The CLI command `opencad import-step <doc> <file.step> --id <feature id>`
builds that patch and applies it through the same history boundary. An MCP
tool `import_step` delegates to the same function, so the CLI, Agent API
(through `patch_apply`), and MCP stay at parity.

- **Revisions.** Revision v2 includes a path → SHA-256 map of attachments
  when any exist, so digests of documents without attachments are unchanged.
- **Diff and impact.** Semantic diffs report `attachment_added` and
  `attachment_removed`. Change impact dirties the `imported_solid` features
  that use a changed attachment.
- **Merge and rebase** treat attachments as objects keyed by path, compared
  by digest.

## 6. File format

- **No migration.** Attachments are optional files under `imports/`.
  Documents without them serialize byte-identically to before.
- **Checksums.** Attachments are included in `checksums.json`, so tampering
  is detected.
- **Schemas.** `schemas/ocad.patch.schema.json` gains the new operations and
  the `imported_solid` feature type. The document schema is unaffected,
  because attachments are opaque files.

## 7. Kernel contract

The feature uses only `GeometryKernel::import_step`, `transform_body`, and
`boolean`. The Mock backend's `import_step` returns a deterministic body
derived from the byte length, so pipeline tests run without OCCT. Real
geometry is covered by OCCT integration tests.

## 8. Evidence plan

- A pure test checks attachment and feature validation: path, digest, and
  rotation rules; fail-closed removal; digest mismatch.
- An OCCT integration test exports the bracket to STEP, imports it into a
  new part with a translation, and checks that the regenerated volume matches
  and the bounds are translated.
- Another OCCT test cuts an imported solid from a plate.
- A file round-trip test verifies the attachment is persisted under
  `imports/` and that checksums verify.
- A CLI `import-step` test and MCP tool coverage.
- `authoring_patch` round-trips a document with an attachment.

## 9. Dependencies and performance

`base64` (already in the workspace) decodes patch payloads. Importing
parses the STEP file on each non-cached regeneration of the feature. The
content-addressed regeneration cache avoids repeated imports while the digest
is unchanged. Base64 inflates patch payloads by about 33%; `import-step`
avoids hand-writing them.
