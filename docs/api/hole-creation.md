# Reviewed standard-hole creation

MCAD-P17-012 / [ADR-111](../adr/ADR-111-reviewed-native-hole-controls.md) exposes
the same backend-owned workflow to native Tauri, Agent and MCP. Existing kernel
and [seat admission](seated-hole-admission.md) remain bounded; no source schema
or new geometry/sampling method is added.

1. `inspect_hole_creation(path)` returns document identity/revision, a free
   feature identity and admitted stock/owned entry/existing exact-circle tuples.
2. `plan_hole_create(path, create)` returns the generated AddFeature DesignPatch,
   actual contact dimensions, removed volume and verified candidate evidence.
   Review is read-only.
3. `apply_hole_create(path, create, history?)` replans from current source,
   atomically publishes through DesignPatch/history and returns feature tree,
   opaque history and evidence. Pass `result.history.history` to the next edit.

Agent names add `opencad.`; Tauri wrappers are `inspect_hole_creation_cmd`,
`plan_hole_create_cmd`, `apply_hole_create_cmd`. Rust entry points live in
`opencad_desktop::hole_creation`, with `_with_kernel` counterparts for independent
analytic Mock and native OCCT. File entry points use actual native geometry and
display verification. `HoleCreate` and closed seat input reject unknown fields.

```json
{
  "document_id": "doc:native_hole_stock",
  "revision": "READ_CURRENT_FROM_INSPECT_HOLE_CREATION",
  "target_feature": "feature:extrude_base",
  "face_ref": "ref:face:plate_entry",
  "sketch_feature": "feature:sketch_seat",
  "profile_ref": "sketch:seat/profile:outer",
  "standard": {"type":"iso273","size":"M6","fit":"normal"},
  "depth": "bore_depth",
  "seat": {"type":"counterbore","diameter":"seat_diameter","depth":"seat_depth"},
  "name": "Reviewed M6 counterbore"
}
```

Select the whole tuple and a returned matching standard; do not substitute a
profile label for `profile_ref`, a screen face index, cached frame, guessed axis
or numeric kernel handle. The exact existing circle supplies position/diameter
and its outward plane; cuts run inward. Explicit dimensions are finite SI after
strict evaluation: constants need units, parameter references need the same
length/angle kind. Source expressions and existing symbolic workplanes remain
unchanged. Supported sizes/fits come from the existing standards table; tap-drill
is a cylinder without physical threads. The backend assigns the new feature ID.

Seat choices are `{"type":"none"}`, counterbore with explicit `diameter` and
`depth`, or countersink with explicit `diameter` and `included_angle`. The latter
derives depth `(D-d)/(2*tan(angle/2))`; it does not infer fastener head sizes.
Removal counts bore/seat overlap once and verifies physical first moments,
unchanged outer bounds and an actual closed oriented mesh. Seated features
also retain their existing runtime source/physical gate. Ordinary standard-hole
sources retain their existing executor contract; the new gateway enforces the
advertised bounded creation and physical postconditions on every plan/apply.

Only the current active original right-rectangle/distance/NewBody stock, owned
end faces and existing uniquely authored ordinary exact circle profiles are
offered. Circle workplanes must be existing custom/face/datum planes with an
outward normal. This selects an existing source; it does not add arbitrary face
sketch editing. Global-plane circle direction, projected/helper/aliased owners,
side/oblique entry, consumed/stale/foreign refs, already modified/imported/fused
stock, unsupported standards, through-all and incomplete radial containment
refuse. No source change repairs these automatically. Default synthetic Mock
cannot provide physical admission; select analytic mode explicitly in tests.

Apply clones source/history, verifies the candidate, rechecks file identity and
revision before writing, and preserves file/history/publication on refusal.
Expanded and packed files use canonical existing Undo/Redo. Native controls keep
only disposable form/review state and discard stale or late replies after input,
source, tool/view, upstream edit, cancel, Escape or reload.

See [the runnable stock and recipe](../../examples/native_hole_stock.md),
`cargo test -p opencad-desktop --test hole_creation`, the actual stdio regression
`python tools/test_native_hole_mcp.py --musubicad target/debug/musubicad`, and
`create_native_standard_hole` eval. The eval checks exact preserved source plus
a fresh native SI STL, including contact rings, bounds, oriented closure, volume
and centroid. Equal-volume wrong-entry/axis/source changes must fail. These
protocol regressions make zero model calls and do not add external-host credit.
