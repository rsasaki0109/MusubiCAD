# Drawing architecture (historical implementation Phase 4)

MusubiCAD drawing documents reference regenerated 3D models and export
orthographic views to SVG. See [ADR-004](../adr/ADR-004-drawing-document-model.md).

## Data model

```
OcadDocument (kind = drawing)
└─ drawing: DrawingModel
   └─ sheets: Vec<Sheet>
      ├─ views: Vec<DrawingView>
      │  ├─ model: ModelReference   // child .ocad path + document id
      │  ├─ projection: ProjectionKind
      │  ├─ scale
      │  └─ origin_on_sheet_m
      └─ dimensions: Vec<LinearDimension>
         ├─ view_id
         ├─ start_model_m / end_model_m
         └─ offset_m
```

Drawing documents do not own B-Rep. Each view loads a child part or assembly
document at export time, tessellates it, and projects mesh edges onto the sheet.

## File layout

| Path | Content |
|---|---|
| `document.ocad.json` | `DocumentMetadata` with `kind: drawing` |
| `graph/drawings.json` | `{ "drawing": DrawingModel }` |

## Export pipeline

1. Load drawing document and first sheet.
2. For each view, resolve `ModelReference.source_path` relative to the drawing directory.
3. Tessellate the referenced model (part or assembly).
4. Project triangle edges with `ProjectionKind` and classify their visibility.
5. Place visible and hidden segments on the sheet.
6. Derive dimension values from referenced-model points and lay out witness lines.
7. Emit SVG in millimeter user units (`export_svg::render_sheet_svg`), using dashed hidden lines.

## Module boundaries

| Crate | Responsibility |
|---|---|
| `opencad-drawing` | Model, projection, wireframe layout, SVG export |
| `opencad-file` | `graph/drawings.json` serialization |
| `opencad-cli` | `musubicad new … drawing`, `musubicad export … .svg` |

## Model-driven dimensions

`LinearDimension` stores two points in model-space meters and references a
`DrawingView`. The displayed value is always derived from the 3D point distance;
it is not serialized as editable text. Projection, view scale, sheet origin, and
`offset_m` determine annotation placement. Existing drawing files migrate with
an empty `dimensions` collection through the Serde default.
## Hidden-line classification

SVG drawing views classify tessellated mesh edges using projected triangle depth.
Each projected edge is split at non-adjacent triangle boundaries and at any
interior crossing where the interpolated triangle/edge depth difference changes
visibility. The resulting intervals are classified at their midpoint. Hidden
intervals are dashed; coincident visible and hidden intervals collapse to the
visible interval. Tessellation diagonals with matching B-Rep face IDs remain
omitted.

The deterministic contract uses `1e-7 m` depth, `1e-9 m` projection,
dimensionless `1e-9` edge-parameter, and dimensionless `1e-9` barycentric
tolerances. Split parameters are sorted and tolerance-deduplicated; source edges
are traversed in canonical vertex-ID order. Tests cover visible-hidden-visible
partial occlusion, depth crossings inside one projected triangle, triangle
input-order independence, and the exact SVG under
`modules/drawing/tests/golden/partial-occlusion.svg`.

The SVG is also one leg of the cross-module MCAD-P5-005
[deterministic evidence contract](golden-evidence.md), which associates it
with the regenerated part, assembly result, Agent patch, and CLI review
artifacts.
