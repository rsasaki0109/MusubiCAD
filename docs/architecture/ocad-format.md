# `.ocad` File Format

MusubiCAD stores design intent in a git-friendly container. The source of truth is JSON on disk, not cached B-Rep.

## Layout

```
bracket.ocad.d/
  manifest.ocad.json      # container manifest
  document.ocad.json      # document metadata
  checksums.json          # SHA-256 per file
  graph/
    parameters.json
    sketches.json
    constraints.json      # derived index (sketches remain canonical)
    features.json         # feature graph + nodes
    assemblies.json
    materials.json
    semantic_refs.json
  imports/                # optional attachment files (ADR-016)
    motor.step            # STEP bytes used by an imported_solid feature
```

Zip archives use the same paths inside `bracket.ocad`.

## API

```rust
use opencad_file::{OcadDocument, write_expanded_dir, read_ocad, validate_ocad};

let doc = OcadDocument::from_part_model(metadata, &part);
write_expanded_dir("bracket.ocad.d", &doc)?;
let restored = validate_ocad("bracket.ocad.d")?;
```

## Determinism

- JSON is pretty-printed with stable key order where required
- `checksums.json` covers every payload file, including `imports/` attachments
- Attachments are opaque bytes referenced by an `imported_solid` feature
  through their path and SHA-256; documents without attachments serialize
  exactly as before
- Regeneration outputs (`KernelBody`) are not stored in `.ocad`
- Equal line/radius targets use explicit `{ "line": ... }` or
  `{ "radius": ... }` objects in canonical JSON; legacy bare target strings
  remain readable as line-length targets.

## Schema

- `MusubiCAD/schemas/ocad.document.schema.json`
- Format version: `0.1.0`
