# M6 counterbored plate

MCAD-P16-003 / ADR-063, authored by
`examples/agent/create_counterbored_plate_patch.json`, with source in
`examples/counterbored_plate.ocad.d`. Source feature/seat tags are supported
by this implementation; published v0.3.1 predates them.

The plate is 40 × 30 × 10 mm, with an ISO273 M6 normal 6.6 mm through bore at
(12, 9) mm and an explicit 11 mm top counterbore 4 mm deep. Depths go inward
from the owned top face. Neither seat size nor manufacturing tolerances are
inferred from a fastener. No physical thread is modeled.

```sh
musubicad regen examples/counterbored_plate.ocad.d
musubicad export examples/counterbored_plate.ocad.d /tmp/counterbored_plate.stl
python tools/test_counterbore_mcp.py --musubicad target/debug/musubicad
```

Independent expectations:

- Volume: `12000 − π × (3.3² × 10 + (5.5² − 3.3²) × 4)` =
  `11414.595624930078` mm³.
- Centre: `(20.410284792773, 15.307713594580, 4.936059512815)` mm.
- Bounds: `(0,0,0)` to `(40,30,10)` mm.
- Changing only `seat_depth` to 6 mm gives `11292.95315738308` mm³.

Use typed DesignPatch (`set_parameter` or `set_feature_expr` fields
`bore_depth`, `seat_diameter`, `seat_depth`), review, then apply. Geometry cannot
be bypassed with `verify=false` / `--no-verify`. The stdio regression actually
creates the document through MCP, previews/reviews without writing, edits the
parameter, checks fresh native volume and the independent source-aware eval,
rejects wrong entry/depth without writes, and runs opaque Agent Undo/Redo.
It makes zero paid model calls. The registered eval task is
`counterbore_seat_depth`; a plain hole or altered standard/sketch/reference fails.

Only original rectangle/distance/NewBody stock and uniquely owned end faces
are admitted. Side/oblique entry, cut/imported/fused targets, out-of-stock depths
or seats, ambiguous/stale/foreign refs and unrepresentable meshes refuse.
Runtime Mock requires explicit `with_analytic_primitives()` throughout a history
session; CLI/MCP uses native OCCT. Default/no-OCCT Mock cannot supply this proof.

Regenerate the checked-in source/authoring patch with:

```sh
cargo test -p opencad-file --test seated_standard_hole write_counterbored_plate_example -- --ignored
```
