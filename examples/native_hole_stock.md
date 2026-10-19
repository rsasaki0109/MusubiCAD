# Native standard-hole stock

`native_hole_stock.ocad.d` is an uncut 40 × 30 × 10 mm plate with two existing
outward ISO273 M6-normal circle axes at world (12, 9) mm. Its top sketch retains
the `feature:entry_plane` datum driven by `stock_height`; the bottom sketch
retains its original reversed custom frame. Owned semantic end refs and original
circle/parameter sources stay intact when a hole is created.

Open the source in the desktop. Under **Standard hole**, select the original
stock, top or bottom entry, existing circle axis and matching standard. Enter a
unit-bearing bore depth, choose plain/counterbore/countersink and explicit seat
dimensions, press **Review hole**, then **Create reviewed hole**. Review does
not write. Undo restores the uncut stock; a modified stock is no longer offered
for another independently positioned hole in this bounded workflow.

`agent/create_native_hole.json` contains the typed top counterbore intent. Read
its current revision with `inspect_hole_creation` before plan/apply. The plate's
`bore_depth`, `seat_diameter`, `seat_depth` and `seat_angle` are 10 mm, 11 mm,
4 mm and 90 deg. For a countersink choose `included_angle: "seat_angle"` instead
of seat depth; its depth is 2.2 mm. Plain bore uses `{"type":"none"}`.

Independent millimetre expectations:

- Initial stock: 12000 mm³, bounds (0,0,0) to (40,30,10) mm.
- Plain 8 mm blind bore: `12000 - pi*3.3²*8` mm³.
- Counterbore: `12000 - pi*(3.3²*10+(5.5²-3.3²)*4)` =
  11414.595624930078 mm³. Top-entry centre is
  (20.410284792773, 15.307713594580, 4.936059512815) mm.
- Countersink: subtract the bore plus the frustum's excess over its overlapping
  bore: `pi*2.2*(5.5²+5.5*3.3-2*3.3²)/3`. Bottom entry reverses the axial first
  moment; it preserves the outer bounds and same volume.

```sh
musubicad regen examples/native_hole_stock.ocad.d
python tools/test_native_hole_mcp.py --musubicad target/debug/musubicad
```

The actual MCP regression authors the stock using
`agent/create_native_hole_stock_patch.json`, then exercises all three hole forms
from both ends plus both 8 mm blind bores (eight cases), exact Agent parity,
read-only review, refusal/history and fresh native regeneration/export with
independent source/unique-entry/contact/volume/COM grading. Plain blind floors
belong to the hole; retained stock end refs stay unique after cold/warm
regeneration and depth changes. It makes zero
model calls. To regenerate a missing source example, pass
`--write-example examples/native_hole_stock.ocad.d`; existing output refuses
overwriting. See [API admission](../docs/api/hole-creation.md) and ADR-111.
