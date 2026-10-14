# Square-ended tapered rib plate

Build from current source; v0.3.1 predates `tapered_rib`. A 40×30×10 mm original
stock receives a 30 mm centreline rib on its extrusion end. The authored
construction line runs (5,15) to (35,15) mm on z=10 mm. Base thickness is 4 mm,
height 20 mm, signed angle 2.862405226111748 degrees (atan(0.05)), square ends.
Top thickness is 2 mm, union volume 13800 mm³, area 5061.49906367 mm²,
centre (20,15,6.81159420289855) mm. The shared internal interface disappears.

```sh
musubicad regen examples/tapered_rib_plate.ocad.d
musubicad export examples/tapered_rib_plate.ocad.d /tmp/tapered_rib_plate.stl
python tools/test_tapered_rib_mcp.py --musubicad target/debug/musubicad
```

`examples/agent/create_tapered_rib_plate_patch.json` creates the complete original
stock/sketch/owned `extrusion_end` reference and closed rib through DesignPatch.
Do not infer new angle/end fields on legacy constant-width `rib`.

Change only height to 12 mm through a reviewed patch:

```json
{"operations":[{"type":"set_parameter","id":"param:rib_height","expr":"12 mm"}]}
```

Top thickness becomes 2.8 mm, union volume 13224 mm³. Independent eval
`tapered_rib_height` also preserves explicit source, square ends, line/stock/
reference, feature order and other parameters. Matching mass with a plain rib
or altered geometry cannot pass. Typed expression fields are `base_thickness`,
`rib_height` and `draft_angle`; `face_ref` edits the owned end.

Actual stdio creates/previews/reviews/edits with native forced verification,
read-only angle/thickness previews, wrong-end/range/unit refusal, CLI no-verify
refusal and canonical Agent Undo/Redo. It makes zero model calls. End identity
is stable under source placement; output retains the annular end's stock owner,
while five new rib surfaces have the rib producer. Default Mock refuses; SDK
proof explicitly selects analytic mode.

```sh
cargo test -p opencad-file --test tapered_rib write_tapered_rib_example -- --ignored
python tools/mcp_eval.py --self-test
```

Original rectangle/distance/NewBody ends, straight stock-axis-aligned line,
contained base and square ends only. Curved/oblique/intersecting/modified target
families, unsupported units, collapse and unrepresentable precision refuse.
See [the source contract](../docs/api/tapered-rib.md). No new host or release claim.
