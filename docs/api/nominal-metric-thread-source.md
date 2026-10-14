# Nominal metric external-thread source

P16-007 implements [ADR-069](../adr/ADR-069-guarded-nominal-thread-source.md)
through the closed `nominal_metric_thread` FeatureDefinition and existing typed
DesignPatch operations. [ADR-068's runtime](physical-nominal-thread.md) supplies
actual geometry in explicit analytic Mock and native OCCT. The original authored
circle/distance/NewBody extrusion, checked sketch phase frame and its owned full
cylindrical side are mandatory; bounds or a similar imported body are insufficient.

All fields below are required, unknown fields refuse:

```json
{
  "type": "nominal_metric_thread",
  "target_feature": "feature:stock",
  "face_ref": "ref:face:stock_side",
  "representation": "nominal_metric_basic_flat_root",
  "size": "m6",
  "handedness": "right",
  "turns": 8,
  "ends": "square"
}
```

Sizes m3/m4/m5/m6/m8/m10/m12 select the coarse SI diameter/pitch table from
ADR-067. Hand is `right` or `left`; turns is an integer 1–32. Stock diameter
must equal the nominal major diameter, and its entire distance must equal
turns times pitch, within 1nm. Square ends truncate the actual screw contour.
The crest at local z=0/azimuth=0 uses the checked authored sketch's positive X
axis; the positive extrusion axis is X cross Y. Proper source rotations retain
that phase. Nonunit, nonorthogonal or unresolved frames refuse.

Use `add_feature` for creation and `replace_feature_definition` for size, hand
or turn edits. `set_feature_ref` supports `face_ref`; no expression fields are
accepted on the new definition. Change stock radius/distance explicitly in the
same patch as a size/turn edit. Static stock fallbacks and dimensional expressions
are validated before resolution, even with no parameters. No stock auto-resize,
missing-field inference or clearing of stale optional ref annotations occurs.

A face ref must have kind Face, creator `target_feature` and role `cylindrical`.
Optional normal/fingerprint annotations must agree with current stock. Missing
bindings can rebind only through the unique owned full-side witness; a present
cap binding refuses. Competing active solid consumers, including semantic aliases
of the target, refuse. The original stock body remains unchanged.

Every output face belongs to the new thread feature: old full side and circular
cap identities do not survive. Discovery preserves actual nonzero native IDs,
with `thread_start`, `thread_end` and `thread_surface` roles. Native split-face
count need not equal Mock's six physical classes. Surface refs cannot select a
transient subface, even with a stored ID; end refs need one current owned physical
discovery. No general downstream thread-face/edge editing or ancestry is promised.

Fresh and cached outputs recheck actual mass, area, first moments, bounded closed
face-bearing mesh and boundary screw phase against current source. Opposite hands
have equal volume: a wrong-hand cached solid must fail phase checks. Boundary
coordinate precision, rather than a loose mesh chord tolerance, bounds the phase
check; inadequate resolution refuses. Synthetic default Mock is unsupported.

CLI/MCP candidate admission forces native verification even with `--no-verify`
or `verify:false`; desktop uses its verified DesignPatch candidate gateway.
Preview/review remain read-only. Failed batches preserve document/history,
previous output and artifacts; successful edits have canonical Undo/Redo.
The new tag makes genuine old readers refuse rather than silently ignore thread
intent. Both schemas, packed/expanded/authoring round trips and no-inference
migration are updated together. Existing files are unchanged.

See the [runnable source example](../../examples/nominal_metric_thread.md),
[creation patch](../../examples/agent/create_nominal_metric_thread_patch.json)
and [left-hand edit](../../examples/agent/nominal_thread_left_patch.json).
`nominal_thread_handedness` is a real MCP eval with independently integrated
volume and exact source/hand/ownership preservation; wrong-hand/cylinder/helix
volume-matching decoys fail. Rust tests independently measure actual physical
surface phase. Source-field checking alone does not measure a surface.

This nominal **basic flat-root** representation has no production fit class,
rounded root, arbitrary pitch, partial/shouldered/runout/chamfer, multi-start or
internal thread admission. ADR-037 tap-drill remains a cylinder; a generic helix
is not a nominal thread feature. The feature is unreleased; v0.3.1 predates it.
