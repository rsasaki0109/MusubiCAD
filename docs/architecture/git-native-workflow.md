# Git-native CAD workflow

MusubiCAD reviews design intent instead of opaque B-Rep files. A `DesignPatch` can carry an
`intent`, `rationale`, stale-state `preconditions`, and machine-checkable `expected_effects`.
The Design Graph remains the source of truth; review geometry is regenerated and disposable.

## Review

```bash
opencad review examples/robot_joint_actuator.ocad.d \
  examples/agent/review_robot_joint_patch.json --output review
```

The command performs an in-memory dry run and writes deterministic `review.json`,
`review.html`, `github-summary.md`, `before.png`, `after.png`, and `comparison.gif`. The report
includes semantic parameter/feature changes, regenerated mass and volume, bounds, triangle counts,
and expected effect results. It does not mutate the input document. After writing the complete
review bundle, the command exits unsuccessfully if any declared expected effect failed.

The repository's `Design Review` GitHub Actions workflow runs this command against the flagship
22-feature robot-joint housing patch. It appends `github-summary.md` to the job summary and uploads the full bundle as a
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

Three-way merge compares every collection of the design by stable semantic ID: parameters,
sketches, features and their display order, semantic references, assertions, and assembly and
drawing objects. Independent edits, including additions and removals, are merged. Divergent edits
return structured base/ours/theirs conflicts, with a `reason` for structural cases (`add_add`,
`remove_modify`, `order`, `invalid_result`). The merged design must pass whole-state validation.
Rebase uses canonical serialized source values for each patch target, so it never relies on
geometry floating-point equality. It drops additions the new base already contains, reports
missing feature anchors, and requires the rebased patch to apply to the new base. A
`RevisionEquals` precondition is updated to the new complete-state digest after a successful
rebase.

## Git merge driver

Expanded `.ocad.d` documents merge through plain `git merge` with the MusubiCAD merge driver
([ADR-015](../adr/ADR-015-git-merge-driver.md)):

```bash
opencad merge-driver install      # registers merge.musubicad in this repository's git config
```

Add the printed lines to `.gitattributes`:

```text
*.ocad.d/*.json merge=musubicad
*.ocad.d/graph/*.json merge=musubicad
```

For each changed file of a document, the driver reconstructs the complete base, ours, and theirs
documents from Git, runs the semantic merge, and writes that file of the merged result. All files
of the directory therefore stay consistent and `checksums.json` verifies. When the design intent
conflicts, the merge stops like any Git conflict and prints typed conflicts. List them again with:

```bash
opencad conflicts path/to/design.ocad.d
```

Resolve them with a `DesignPatch` or `opencad merge`, then commit. The driver refuses to guess:
rebase, cherry-pick, octopus and criss-cross merges, or any input that does not match its
reconstruction fall back to Git's ordinary conflict handling.

## Agent approval boundary

`IntentProvider` receives an immutable `DesignState` and selection IDs and may only return a
serializable proposal. MusubiCAD validates the selection and dry-runs the patch. Mutation requires
the exact deterministic approval ID, verifies that the proposal did not change, and dry-runs again
against current state. Providers therefore cannot silently bypass DesignPatch validation or a
complete-state revision precondition.

## Engineering policy

`evaluate_policy` is a deterministic, I/O-free CI gate. It supports required parameter expressions,
maximum mass in kilograms, bounding-box limits in metres, and zero assembly interference. Missing
metrics fail closed rather than silently passing.

```bash
opencad check examples/bracket.ocad.d examples/agent/bracket_policy.json
```

The command prints a JSON report and exits unsuccessfully when any finding fails.
