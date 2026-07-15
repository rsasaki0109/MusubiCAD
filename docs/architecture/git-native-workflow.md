# Git-native CAD workflow

MusubiCAD reviews design intent instead of opaque B-Rep files. A `DesignPatch` can carry an
`intent`, `rationale`, stale-state `preconditions`, and machine-checkable `expected_effects`.
The Design Graph remains the source of truth; review geometry is regenerated and disposable.

## Review

```bash
opencad review examples/bracket.ocad.d examples/agent/review_width_patch.json --output review
```

The command performs an in-memory dry run and writes deterministic `review.json`,
`review.html`, `github-summary.md`, `before.png`, `after.png`, and `comparison.gif`. The report
includes semantic parameter/feature changes, regenerated mass and volume, bounds, triangle counts,
and expected effect results. It does not mutate the input document. After writing the complete
review bundle, the command exits unsuccessfully if any declared expected effect failed.

The repository's `Design Review` GitHub Actions workflow runs this command against the flagship
bracket patch. It appends `github-summary.md` to the job summary and uploads the full bundle as a
14-day workflow artifact. Summary and artifact publication use `always()` so failed expected-effect
checks retain their review evidence. GitHub I/O remains in the workflow; the CLI only writes local
files.

Assembly reviews render placed instances and report exact solid-interference counts. Drawing
reviews add Before/After SVG sheets alongside the referenced model geometry comparison.

## Merge and rebase

```bash
opencad merge base.ocad.d ours.ocad.d theirs.ocad.d merged.ocad.d
opencad rebase-patch old-base.ocad.d new-base.ocad.d change.json rebased.json
```

Three-way merge compares parameters and features by stable semantic ID. Independent edits are
merged; divergent edits to the same ID return structured base/ours/theirs conflicts. Structural
feature and parameter additions/removals currently require manual resolution because their graph
edges must be reviewed together. Rebase refuses to move a patch when a parameter it edits changed
since the old base.

## Agent approval boundary

`IntentProvider` receives an immutable `DesignState` and selection IDs and may only return a
serializable proposal. MusubiCAD validates the selection and dry-runs the patch. Mutation requires
the exact deterministic approval ID, verifies that the proposal did not change, and dry-runs again
against current state. Providers therefore cannot silently bypass DesignPatch validation.

## Engineering policy

`evaluate_policy` is a deterministic, I/O-free CI gate. It supports required parameter expressions,
maximum mass in kilograms, bounding-box limits in metres, and zero assembly interference. Missing
metrics fail closed rather than silently passing.

```bash
opencad check examples/bracket.ocad.d examples/agent/bracket_policy.json
```

The command prints a JSON report and exits unsuccessfully when any finding fails.
