# M6 countersunk plate

P16-004 / ADR-064 authors an explicit ISO273 M6 normal clearance hole on
original 40 × 30 × 10 mm rectangular stock, with a 12 mm seat diameter and
90° included angle. The exact 6.6 mm circle is at (12, 9, 10) mm; an owned
semantic entry reference is required. Head dimensions are explicit.

```sh
musubicad regen examples/countersunk_plate.ocad.d
musubicad export examples/countersunk_plate.ocad.d /tmp/countersunk_plate.stl
python tools/test_countersink_mcp.py --musubicad target/debug/musubicad
```

Use a build containing P16-004; released v0.3.1 predates seated-hole source.
The canonical expanded source is `examples/countersunk_plate.ocad.d` and its
full DesignPatch is `examples/agent/create_countersunk_plate_patch.json`.

Independent expectations (mm and mm³):

| Included angle | Derived seat depth | Remaining volume | Centre of mass |
|---|---:|---:|---|
| 90° | 2.700000000000 | 11561.691276156460 | (20.303283464936, 15.227462598702, 4.965488287834) |
| 60° | 4.676537180436 | 11491.275833221702 | (20.354163749378, 15.265622812033, 4.948898132032) |

For bore radius r, seat radius R, bore depth D and included angle A,
seat depth s = (R-r)/tan(A/2). Removed volume is
`pi*r*r*D + pi*s*(R*R+R*r-2*r*r)/3`.
The frustum and bore overlap once; the cone joins the bore without a flat
annular shoulder. Patches may change `param:seat_angle` (`seat_angle_rad`)
from `90 deg` to `60 deg`, or set feature expression `seat_included_angle`.
Direct `seat_depth` edits are refused because this depth is derived.

Independent tests check both kernels, conical contact, closed meshes, cache,
a non-axis-aligned stock/entry placement, strict units/ranges, semantic source,
packed/expanded roundtrip, reader refusal and protected Undo/Redo.
`countersink_angle` eval requires unchanged explicit feature/sketch/ref/order
source and independently regenerated volume. Actual MCP stdio regression makes
zero model calls. Default Mock cannot manufacture this source; SDK tests select
analytic Mock explicitly, and CLI/MCP verification uses native OCCT.
