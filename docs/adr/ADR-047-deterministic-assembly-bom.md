# ADR-047: Deterministic flattened assembly bill of materials

- Status: Accepted
- Date: 2026-10-12
- Task: MCAD-P12-008

Expose a read-only BOM query through CLI, Agent and MCP, with optional canonical
JSON or CSV table output. Count placed leaf part occurrences, including patterns
and repeated subassemblies, regardless of fixed/movable status. Unused declared
components do not become purchased quantities. Report declared root component
count, expanded top-level instance count, unique leaf-source count and total leaf
quantity separately.

The assembly module counts from a caller-supplied immutable source catalog; no
implicit file access or kernel types enter the BOM data. Each source includes
verified document identity/name and, for an assembly, its model plus component
ID to resolved-source-key mapping. Rows are keyed by canonical root-relative
source path and document identity. Copies at different resolved files remain
separate rows even when their document UUIDs match; do not merge potentially
changed copies. Structured root-to-leaf instance-ID paths preserve provenance
without delimiter ambiguity. Canonically sort sources, rows and occurrence paths.

The CLI boundary loads used references through existing lexical/canonical path
containment checks, verifies document kind/identity and builds a snapshot catalog.
Detect cycles using active canonical source path or document ID, as regeneration
does. Bound depth to 64, source count to 10,000 and expanded occurrence visits to
100,000; check pattern budgets before expansion and use checked count arithmetic.

Validate regeneration of every unique assembly snapshot, requiring every instance
to succeed. Checking only the root compound is insufficient: existing regeneration
can return a parent compound when a nested assembly has partial child failures.
Load all regeneration payloads from the same snapshot; failed/missing parts or
unsupported reference graphs fail before output. BOM generation never writes the
source model. It adds no `.ocad` schema, inferred metadata or material/cost rules.

JSON quantities are dimensionless counts with explicit field names; CSV quotes
all string fields and escapes quotes per RFC 4180. Return the same structured
report on Agent/MCP. Build the complete optional output before writing. Verify
analytic nested/pattern quantities, deterministic source permutations, cycles,
missing/kind/identity failures and existing-output/source preservation. Mock and
OCCT tests compare analytic solid dimensions/volume before and after BOM queries.
Examples, authoring guidance/skill and feasible eval tasks accompany the feature.

Native integration exposed a required backend gap: `transform_body` handled only
single solids, so placement of a multi-solid subassembly failed with a missing
body handle. Resolve compound members deterministically, bound traversal to 64
levels/100,000 members, and apply the existing exact rigid transform to every
solid. Store compounds with flattened member handles so bounds/mass/tessellation
consumers retain all members without Boolean union. Validate source handles
before compound insertion; preserve repeated occurrences and source solids.
Analytic native compound/nested-assembly dimensions and volume cover this BOM
verification prerequisite. No OCCT type or sampling method enters the public API.

Use the existing tempfile dependency for complete-buffer, same-directory atomic
output replacement. Reject output inside any expanded source document (or equal
to a packed input). Replacement preserves an input even when the requested
external output previously hard-linked it; validation failure preserves output.
