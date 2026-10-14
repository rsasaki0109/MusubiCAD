# Nominal M6x1 external thread

The example is an authored 6mm-diameter, 8mm-long exact circle extrusion, cut to
8 whole right-hand pitches of the nominal metric basic flat-root envelope.
The circle centre is (12mm,9mm) in XY; phase zero uses authored positive X.
Both source and optional annotations are explicit; no hidden diameter/length
inference or stock resizing occurs.

```sh
musubicad regen examples/nominal_metric_thread.ocad.d
musubicad patch examples/nominal_metric_thread.ocad.d examples/agent/nominal_thread_left_patch.json --dry-run
```

To work on a copy, create an empty part and apply
`examples/agent/create_nominal_metric_thread_patch.json` through `patch_apply`,
then preview/review/apply `examples/agent/nominal_thread_left_patch.json`.
These use existing typed DesignPatch operations, not a new MCP command.
`verify:false` and CLI `--no-verify` still require native candidate validation.

Independent flat/root/flank integration predicts 183.657632385mm³ for either
hand, compared with the original cylinder's 226.194671058mm³. Equal volume is
insufficient to prove the requested hand: source-aware eval
`nominal_thread_handedness` checks the complete definition, authored stock and
side ownership. Both-kernel tests independently check the actual screw surface.
All output faces are newly produced by the thread; native subface numbering is
not a persistent selection contract. Unique physical ends can be referenced;
general thread-face/edge refinement remains outside admission.

Regenerate the committed example through its canonical authoring patch with:

```sh
cargo test -p opencad-file --test nominal_metric_thread write_nominal_thread_example -- --ignored
python tools/test_nominal_thread_mcp.py --musubicad target/debug/musubicad
```

The native stdio regression tests read-only preview/review, forced verification,
left-hand edit, independently graded eval, failed-batch/artifact protection and
canonical Agent Undo/Redo. No model host is impersonated.

This is a nominal basic flat-root shape with square clips. It has no production
fit class, rounded root, runout, internal/multi-start or partial-thread support.
See [source contract](../docs/api/nominal-metric-thread-source.md).
