# Drawing architecture (Phase 4)

ForgeCAD drawing documents reference regenerated 3D models and export
orthographic views to SVG. See [ADR-004](../adr/ADR-004-drawing-document-model.md).

## Data model

```
OcadDocument (kind = drawing)
└─ drawing: DrawingModel
   └─ sheets: Vec<Sheet>
      └─ views: Vec<DrawingView>
         ├─ model: ModelReference   // child .ocad path + document id
         ├─ projection: ProjectionKind
         ├─ scale
         └─ origin_on_sheet_m
```

Drawing documents do not own B-Rep. Each view loads a child part or assembly
document at export time, tessellates it, and projects mesh edges onto the sheet.

## File layout

| Path | Content |
|---|---|
| `document.ocad.json` | `DocumentMetadata` with `kind: drawing` |
| `graph/drawings.json` | `{ "drawing": DrawingModel }` |

## Export pipeline (M4.1 MVP)

1. Load drawing document and first sheet.
2. For each view, resolve `ModelReference.source_path` relative to the drawing directory.
3. Tessellate the referenced model (part or assembly).
4. Project triangle edges with `ProjectionKind` and place on the sheet.
5. Emit wireframe SVG in millimeter user units (`export_svg::render_sheet_svg`).

Hidden-line removal (Task-177) and model-driven dimensions (Task-179) are deferred.

## Module boundaries

| Crate | Responsibility |
|---|---|
| `opencad-drawing` | Model, projection, wireframe layout, SVG export |
| `opencad-file` | `graph/drawings.json` serialization |
| `opencad-cli` | `opencad new … drawing`, `opencad export … .svg` |
