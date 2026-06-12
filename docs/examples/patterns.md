# Bracket Pattern Examples

Committed samples under `examples/` demonstrate every pattern type with both **cut** and **union** operations.

## Comparison

| Template | Example path | Pattern | Operation | `target_feature` | Parametric field |
|---|---|---|---|---|---|
| `bracket` | `bracket.ocad.d` | — | hole cut | `face_ref` | `hole_diameter` |
| `hole-row` | `bracket_hole_row.ocad.d` | linear | cut | `feature:extrude_base` | `spacing_expr: hole_pitch` |
| `hole-ring` | `bracket_hole_ring.ocad.d` | circular | cut | `feature:extrude_base` | — |
| `pin-row` | `bracket_pin_row.ocad.d` | linear | union | `feature:extrude_base` | `spacing_expr: hole_pitch` |
| `pin-ring` | `bracket_pin_ring.ocad.d` | circular | union | `feature:extrude_base` | — |
| `pin-mirror` | `bracket_pin_mirror.ocad.d` | mirror | union | `feature:extrude_base` | `plane_face_ref` |

## Create samples

```bash
cargo run -p opencad-cli -- new examples/bracket.ocad.d
cargo run -p opencad-cli -- new examples/bracket_hole_row.ocad.d hole-row
cargo run -p opencad-cli -- new examples/bracket_hole_ring.ocad.d hole-ring
cargo run -p opencad-cli -- new examples/bracket_pin_row.ocad.d pin-row
cargo run -p opencad-cli -- new examples/bracket_pin_ring.ocad.d pin-ring
cargo run -p opencad-cli -- new examples/bracket_pin_mirror.ocad.d pin-mirror
```

## Cut vs union

- **Cut** patterns subtract the source tool (and translated/rotated copies) from `target_feature`.
- **Union** patterns fuse the source tool onto `target_feature` one instance at a time. This avoids OCCT compound issues when copies are not mutually touching.

Mirror patterns may use `plane_face_ref` instead of explicit `plane_origin_m` / `plane_normal_m`. See [feature modeling](../architecture/feature-modeling.md) and [Agent API](../api/agent.md).

## Agent patches

| Goal | Example patch |
|---|---|
| Parametric hole row spacing | `examples/agent/spacing_expr_patch.json` |
| Semantic mirror plane | `examples/agent/plane_face_ref_patch.json` |
| Persisted top face ref | `examples/agent/assign_face_ref_patch.json` |

## Further reading

- [Feature modeling](../architecture/feature-modeling.md)
- [Examples README](../../examples/README.md)
