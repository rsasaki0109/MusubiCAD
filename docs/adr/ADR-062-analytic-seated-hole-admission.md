# ADR-062: Analytic admission for seated standard holes

Status: Accepted for MCAD-P16-002 analytic Mock infrastructure.
Date: 2026-10-13.
Roadmap: MCAD-P16-001 design gate.

Seated-hole source/schema details require their feature-specific accepted ADR
before MCAD-P16-003/004 implementation; this document does not grant that gate.

## Context

The next modeling increments need independent volume/dimension evidence from
both kernels. ADR-037 standard holes currently delegate to cylindrical cuts. The
legacy Mock ordinary wire/extrusion/Boolean path uses synthetic IDs and
synthetic cube mass; those results cannot prove physical seated-hole geometry.
ADR-060 already distinguishes tracked physical bodies from synthetic plumbing.
The accepted fixed maturity rubric and historical 50% target remain unchanged.

## Decision

Before creating a seated-hole feature, add an explicitly selected bounded
tracked analytic Mock path for exact planar rectangular stock and exact circle
extrusions. Runtime records retain SI frame, dimensions, extents and operation
provenance. The default legacy Mock remains unchanged. An explicit
analytic-primitives constructor/configuration selects the bounded physical path
in tests and supported consumers; no silent global promotion is permitted.
Unknown or unsupported records in that mode refuse physical operations rather
than use synthetic arithmetic. Never promote an unknown ID to physical data. An
admission predicate distinguishes valid supported analytic queries/operations
from legacy synthetic plumbing, without exporting OCCT types or persisting
kernel handles. Namespace allocation must avoid all existing tracked stores.

Initially admit right rectangular stock with circle-axis normal to its face,
complete radial containment, positive finite dimensions and explicitly bounded
axial intersection. Admit one concentric stepped cylindrical subtraction chain.
Use the existing SI/tolerance utilities; initial plane/containment dimensional
admission tolerance is 1e-9 m, and direction orthogonality tolerance is 1e-9 for
normalized vectors. Refuse degenerate tolerance-boundary geometry rather than
changing its intended solid. Tests state absolute/relative mass-property
tolerances independently of kernel output.

Compute the union of removed axial/radial intervals, so overlapping bore/seat
cuts are not counted twice. For bore radius r, seat radius R, bore depth L and
seat depth d within stock: removed volume is pi*r*r*L + pi*(R*R-r*r)*d. Validate
independent first moments and unchanged outer bounds, not just volume. Any
unsupported mixed shape, oblique cut, partial lateral intersection, overlapping
nonconcentric hole or ambiguous cut orientation refuses before returning an
analytic result. General Boolean support is not implied.

Mock exact topology, tessellation, intersection and inertia must either have
separately tested implementations for the admitted records or explicitly refuse;
never fall back to a synthetic cube for a tracked analytic body. Feature
regeneration uses tessellation for semantic face discovery. Therefore
MCAD-P16-002 may establish primitive/cut mass and bounds but does not itself
complete a usable seated-hole feature: actual admitted face/tessellation support
and semantic ownership must be included before MCAD-P16-003 is accepted. Any new
tessellation/sampling method requires its own documented admission first. Native
OCCT follows its existing geometry-neutral interface and is independently
checked on the same physical cases.

Use a distinct serialized seated-hole feature tag and a closed seat discriminant
when implementing the feature. Older readers must refuse the tag rather than
ignore an optional new field on StandardHole and regenerate a plain hole.
Preserve legacy StandardHole documents. Schema, migration/admission checks,
expanded/packed round trips and explicit old-reader refusal ship together.
DesignPatch uses the existing add_feature/edit_feature commands; a seated hole
retains owning target feature, profile reference, explicit standard selection,
SI depth and an owned semantic face reference. No projection-coordinate or
nearest-face fallback is admitted. Parameter expressions retain source text and
regeneration dependency ownership. Verified candidate regeneration must restore
source/history/artifacts on any admission or kernel failure.

The exact source DTO and schema version are decided in the feature task, before
implementation, against the repository's current migration policy.

Counterbores require explicit seat diameter and depth. Countersinks require
explicit seat diameter and included angle with derived frustum depth, endpoint
radii and physical volume; the latter is a separate admission extension/task. No
fallback from an unsupported seat to a cylindrical bore is allowed.

## Acceptance and consequences

MCAD-P16-002 must demonstrate real independent Mock/native primitive dimensions,
volume and first moments, concentric subtraction with non-double-counted
overlap, rigid placements where admitted, operation/query refusals and isolation
of legacy records. Zero CAD maturity credit for infrastructure alone.

MCAD-P16-003/004 require both kernels, independently calculated geometry,
parametric DesignPatch edits, semantic source/face ownership, deterministic
round trips, older-reader refusal, byte-preserving review/failure/Undo/Redo,
examples, MCP authoring guide, skill and independently graded actual MCP eval
where feasible. Give credit only after main merge with explicit
behavior/evidence in the ledger.

This bounded extension costs more than testing IDs but prevents false physical
claims and subtracting the bore overlap twice. It does not turn Mock into a
production CAD kernel. Broader stock/cut shapes require separate admission and
analytic evidence. No new curve sampling algorithm or NURBS editing is accepted.

 ## Alternatives considered

Keeping synthetic mass as evidence was rejected because it cannot detect a wrong
hole radius, placement or overlapping seat removal. Silently converting all
legacy Mock primitives to exact bodies was rejected because existing structural
fixtures exercise broader unsupported operations. Making Mock use OCCT was
rejected because it removes independent analytic evidence and violates the
kernel boundary. The explicit bounded mode preserves legacy tests and gives the
new feature a genuine physical acceptance path.
