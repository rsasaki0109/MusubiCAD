# Design Graph

The Design Graph is ForgeCAD's authoritative intermediate representation (IR). All regeneration, AI patches, and semantic diffs operate on this graph — not on cached B-Rep geometry.

## Node kinds

| Kind | Example ID | Role |
|---|---|---|
| Document | `doc:bracket_001` | Root metadata |
| Parameter | `param:width` | Named parametric value |
| Sketch | `sketch:base` | 2D profile source |
| Constraint | `con:rect_width` | Sketch constraint |
| Feature | `feature:extrude_base` | Modeling operation |
| Body | `body:base` | Solid output |
| FaceRef | `ref:face:base_top` | Semantic topology reference |
| Material | `mat:aluminum_6061` | Physical material |

## Edge kinds

| Edge | Meaning |
|---|---|
| `depends_on` | Target must be resolved after source |
| `creates` | Source produces target (e.g. extrude → body) |
| `references` | Source semantically references target |

## Sub-graphs

### `DesignGraph`

Unified node/edge store with query and dirty propagation:

```rust
graph.find_by_type(GraphNodeKind::Parameter);
graph.find_by_role("plate_thickness");
graph.mark_dirty("param:width");
graph.dependency_order()?;
```

### `ParamGraph`

Parameter expressions and parameter-to-consumer dependencies:

```
param:width → param:height (expr: "width / 2")
param:width → sketch:base/con:rect_width
```

### `FeatureGraph`

UI-ordered feature list plus dependency DAG for recomputation:

```
feature:sketch_base → feature:extrude_base → feature:hole_pattern
```

## Semantic diff

`DesignDiff` captures typed changes for Git review and AI feedback:

- `parameter_changed`
- `feature_added` / `feature_modified`
- `mass_changed` / `bbox_changed`

## Invariants

1. Graph is the source of truth.
2. `depends_on` edges must be acyclic.
3. Feature UI order must respect the dependency DAG.
4. Dirty flags propagate downstream along `depends_on` edges.

## Implementation

Crate: `modules/graph` (`opencad-graph`)

Tasks: Task-016 through Task-025
