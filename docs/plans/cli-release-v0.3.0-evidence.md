# MusubiCAD CLI v0.3.0 failed release gate

Task: MCAD-P15-008. Observed on 2026-10-13 UTC.

This is a failed tag attempt, not an official release or a downloaded-archive
success. The tag is retained without replacement; recovery uses v0.3.1.

- Exact main source: `75ac288fb97402433310d1d513041821deb7a434` (PR #99 merge).
- [Main CI 38377113003](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38377113003)
  passed Rust and desktop workflow contract.
- [Main Desktop 38377113021](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38377113021)
  passed all four native platforms and headless/packaged Linux smoke.
- The annotated `v0.3.0` tag was pushed only after those exact-source gates and
  all four required local checks succeeded (1042 Rust tests, zero failures,
  31 ignored; plugin and release-version contracts passed).
- [Tag Release 38383290027](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38383290027):
  Windows job `115252088597` built the native CLI, passed version/OCCT/diff,
  packaged it and verified its archive contents. It failed at **Execute tutorial
  from the packaged native archive**. Upload was skipped. Linux and Apple Silicon
  archive jobs succeeded when the Windows failure was observed.

The release publisher requires every native build. No local binaries are uploaded
to bypass that boundary, and no v0.3.0 official asset inventory or downloaded
tutorial success is asserted. API check annotations initially only exposed
`Process completed with exit code 1`. PR #100 added real failure annotations and
reproduced the Windows failure in [Release 38390841664](https://github.com/rsasaki0109/MusubiCAD/actions/runs/38390841664),
job `115274075763`. The child part's imported **ASCII STL** bytes had SHA-256
`666cdf8ccad22547437de036752f88a5f47e643d39278a0d92dadbc98d8edfa0`
while the feature required
`129172c9ac8ff189ad01b4e620c9b7d29f4996f4198cb27d7e9f905dc2e6e7be`.
The original 1133-byte LF asset has the required hash; LF-to-CRLF conversion
produces exactly the observed bad Windows hash. JSON-only CRLF conversion did
not change the child revision; the affected data is the opaque STL attachment.

Recovery marks every `*.stl` as Git `binary`, preserving opaque asset bytes even
when ASCII content would otherwise be transformed by Windows `core.autocrlf`.
A real temporary Git repository, index and `core.autocrlf=true` checkout test
verifies original bytes and required SHA-256; no STL, feature checksum or geometry
assertion is rewritten. Hash validation continues to reject changed attachments.
Recovery also emits bounded, escaped real Python failure traces as Actions check
annotations while preserving nonzero exit and normal stderr. Release-input PRs
now exercise Windows and Linux extracted-archive tutorials; tags still require
all four platforms. Assertions, source guards, units, tolerances and native
verification remain unchanged. Candidate v0.3.1 is not a publication claim.

No additional CAD capability, actual host session, signature or maturity point
is earned by the failed tag or its diagnosis. General CAD remains 50.10 (about
50%), ecosystem 14 and agent operation 90. Cursor/authenticated Gemini editing
remain unverified; the separate protected desktop trust policy is unchanged.
