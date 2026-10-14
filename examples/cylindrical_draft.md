# Cylindrical neutral-plane draft

Build from current source; v0.3.1 predates `cylindrical_draft`. The example's
original circle has centre (12,9) mm, radius 20 mm and height 30 mm. A +7 degree
full-side draft has a neutral plane at z=15 mm with +z normal. Endpoint radii are
21.8417684135 and 18.1582315865 mm. Exact volume is 37805.6781496 mm³ and axial
centre of mass is 14.0817115734 mm. The unchanged neutral circle is internal;
both caps and the conical side belong to the draft producer.

Create an empty part using MCP `new_document` with document ID
`doc:cylindrical_draft` and name `Cylindrical neutral-plane draft`, then:

```sh
musubicad patch drafted.ocad.d examples/agent/create_cylindrical_draft_patch.json
musubicad regen drafted.ocad.d
```

Or regenerate the checked-in `examples/cylindrical_draft.ocad.d` directly. The
creation patch supplies original sketch/stock, an owned `cylindrical` side ref,
SI angle and explicit neutral plane. It uses DesignPatch/transactions.

Change only the angle parameter through a reviewed patch:

```json
{"operations":[{"type":"set_parameter","id":"param:draft_angle","expr":"-5 deg"}]}
```

The output volume becomes 37753.2166629 mm³. Negative angle widens toward +z;
the neutral circle still stays fixed. The independent `cylindrical_draft_angle`
eval also checks the signed expression because +5 degrees has the same volume
with this centred plane. Typed `set_feature_expr` field `draft_angle` is available
for direct expression edits. Refuse length units, invalid angles, cap/foreign/
missing side refs, consumed sides and unsupported target/plane geometry.

```sh
python tools/mcp_eval.py --self-test
python tools/test_cylindrical_draft_mcp.py --musubicad target/debug/musubicad
```

Actual MCP preview/review is read-only; `verify:false` cannot bypass native
geometry. Failed edits preserve source/history; Agent Undo/Redo reproduces
canonical authored JSON and fresh measurements. Default Mock refuses this
feature; the SDK explicitly selects analytic Mock for independent physical
proof. No additional host, release or arbitrary curved-face support is claimed.
See [the source/admission contract](../docs/api/cylindrical-draft.md).
