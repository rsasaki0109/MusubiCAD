# ADR-015: Git merge driver for expanded `.ocad.d` documents

Status: Accepted  
Date: 2026-09-25  
Roadmap: MCAD-P6-005

## Context

`semantic_three_way_merge` merges complete design states by stable ID and
reports typed structural conflicts (ADR-013 §7). Git, however, merges files,
and an expanded `.ocad.d` document is a directory of JSON files:

- `graph/parameters.json`, `sketches.json`, and `features.json` hold source
  intent.
- `graph/constraints.json`, `checksums.json`, and the derived
  `feature_graph` are projections of other files.

A textual merge of these files produces JSON conflict markers, silently
inconsistent derived data, or a checksum manifest that no longer verifies. A
per-file semantic driver cannot do better on its own: merging `features.json`
correctly needs the merged sketches and references, and `checksums.json`
depends on every other merged file.

## Decision

Add `opencad merge-driver`, a Git merge driver
(`driver = opencad merge-driver %O %A %B %P`) for the files of `.ocad.d`
documents.

1. **Whole-document merge per file.**
   - From `%P`, the driver finds the enclosing `*.ocad.d` directory.
   - It reads the complete base, ours, and theirs documents from Git objects.
     Ours is `HEAD`. Theirs is the commit `git merge` exports to drivers as
     `GITHEAD_<sha>`, because `MERGE_HEAD` is not written until after the
     drivers have run; `MERGE_HEAD` is used when present. Base is the single
     merge base of the two.
   - It runs the same whole-document merge as `opencad merge`, serializes the
     merged document canonically, and writes the requested file's bytes to
     `%A`.
   - Git takes a file changed on only one branch without calling the driver.
     The merge therefore keeps a collection verbatim, including its order,
     when the merged content equals exactly one side. The persisted Feature
     Graph is likewise taken from the side whose graph inputs equal the
     merged ones, and re-derived only when both sides contributed. The
     driver-written files and the files Git took directly then form one
     checksum-consistent document.
   - Every invocation for the same document computes the same deterministic
     result, so all files of the directory agree, including `checksums.json`,
     `constraints.json`, and the re-derived Feature Graph.
2. **Fail closed when reconstruction is uncertain.** The driver exits 1 and
   leaves `%A` untouched, which Git records as a conflict, when:
   - no merged commit can be identified (rebase, cherry-pick, or apply), or
     the merge is an octopus merge;
   - there is more than one merge base;
   - the reconstructed base, ours, or theirs version of `%P` differs
     semantically from the `%O`, `%A`, or `%B` file Git supplied (compared as
     parsed JSON, so line-ending conversion does not matter);
   - the path is not a canonical file of the merged document.

   Git then falls back to its normal conflict handling, and the user runs
   `opencad merge` explicitly.
3. **Semantic conflicts.** When the merge reports conflicts, the driver
   prints them as JSON on stderr, including `add_add`, `remove_modify`,
   `order`, and `invalid_result` reasons, and exits 1. During an unfinished
   merge, `opencad conflicts <doc.ocad.d>` recomputes and prints the same
   conflict list.
4. **Setup.**
   - `opencad merge-driver install` writes the repository-local `git config`
     entries.
   - `opencad merge-driver install` also prints the `.gitattributes` lines to
     add:
     - `*.ocad.d/*.json merge=musubicad`
     - `*.ocad.d/graph/*.json merge=musubicad`
   - It never edits `.gitattributes` itself.
5. **Boundaries.**
   - Git I/O (`git merge-base`, `git ls-tree`, `git show`) lives only in the
     CLI.
   - `modules/file` exposes `parse_document_files` to build a document from
     an in-memory file map, and performs no process or network I/O.

## Consequences

- `git merge` of two branches that changed one design independently produces a
  valid, checksum-verified document with no manual step.
- Conflicting design intent stops the merge with typed, ID-level conflicts
  instead of JSON conflict markers.
- The driver runs `git` subprocesses and may run once per changed file of the
  same document. Documents are small, so this is acceptable. Caching across
  invocations is not attempted, which keeps each invocation stateless.
- Rebase and cherry-pick fall back to Git's normal conflicts; supporting them
  needs the operation-specific base and head, which is left for later.
