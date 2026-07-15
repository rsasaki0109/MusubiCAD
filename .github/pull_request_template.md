## Linked task or issue

Task-XXX / Closes #XXX

## Summary and user impact

- What changed?
- Why does it matter to a MusubiCAD user or contributor?

## Implementation notes

- Which Design Graph, transaction, DesignPatch, geometry, file, or UI boundaries are involved?
- What tolerance and unit assumptions apply?

## Tests added

- [ ] Pure data model / regression test as applicable
- [ ] Sketch, geometry, file-format, or AI patch test as applicable
- [ ] Feature example under `examples/`, or not applicable with explanation

Commands run:

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

## Docs updated

- [ ] `docs/api/` for a public API change
- [ ] `docs/architecture/` and ADR for an architecture change
- [ ] `schemas/`, migration code, and round-trip tests for a schema change
- [ ] Not applicable, with explanation

## Known limitations

- List intentional limitations and follow-up work.

## Invariant checklist

- [ ] The Design Graph remains the source of truth.
- [ ] Model mutations use transactions and AI mutations use DesignPatch.
- [ ] Failed regeneration cannot corrupt the document.
- [ ] Numeric values have explicit units and geometry checks use tolerances.
- [ ] No OCCT types escape `modules/kernel-occt`.
