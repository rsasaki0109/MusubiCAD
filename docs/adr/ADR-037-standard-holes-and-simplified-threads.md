# ADR-037: Standard holes and simplified internal threads

Status: Accepted  
Date: 2026-10-11  
Roadmap: MCAD-P11-004

## Context and decision

Mechanical parts need explicit fastener designation, clearance series and
thread representation. A plain sketch cut does not distinguish a clearance
hole from a recommended tap drill. Add `standard_hole` as a separate feature,
reusing the existing circular hole cut, semantic face resolution and transaction
paths. Legacy `hole` retains its meaning.

The bounded table supports M3, M4, M5, M6, M8, M10 and M12 metric coarse sizes.
`iso273` selects `close`, `normal` or `loose` nominal clearance diameters.
`iso_metric_coarse` records the coarse designation with the required explicit
`representation: tap_drill`. Nominal diameter and coarse pitch are derived from
the designation; the recommended tap drill is nominal minus pitch. For example,
M6 gives 6 mm nominal, 1 mm pitch and a 5 mm drill cylinder. These recommended
sizes are not ISO thread tolerance limits or rounded commercial drill selection.

## Representation, units and geometry

The Design Graph stores the standard selection and the ordinary hole inputs.
Derived `StandardHoleDimensions` returns nominal diameter, pitch and cut
diameter as SI `Length`s. There are no independently stored thread dimensions
that can contradict the table. Thread information survives inspection and file
round trips in the feature definition.

A standard hole consumes one circular sketch profile. Its evaluated diameter
must match the table's cut diameter within **1e-9 m**. The sketch still defines
the center, workplane and geometry; the feature checks agreement rather than
silently overriding the sketch. To change a designation/fit, change its sketch
diameter in the same DesignPatch. Non-circular profiles and diameter mismatches
fail regeneration before cutting. Optional face refs follow the existing hole
semantic resolution contract. Distance, symmetric and through-all depths retain
the existing hole semantics; parametric depth expressions dirty the feature.

Both Mock and OCCT run the same feature validation, then execute the existing
kernel-neutral circular extrusion cut. The analytic removed volume for a blind
cylindrical hole is `π (d/2)² depth`. Geometry and mass/export represent only
the drill cylinder: **no actual helical thread, tap lead-in, thread tolerance
class, drill point or cosmetic viewport lines are generated**. The simplified
internal thread spans the hole extent without a separate engagement depth.
This representation supports ordinary CAD simplified threaded holes without
claiming physical thread geometry. Existing generic helical sweeps remain
available; modeled standard threads need a separate admission decision and
geometry/volume evidence.

## Failure and compatibility

Unsupported sizes, standards, fits, representation strings and extra standard
fields (including pitch overrides) are rejected on deserialization. Geometry
mismatches fail closed in regeneration and verified patch application; structural
staged authoring without geometry verification does not establish dimensional
agreement. Failed regeneration restores previous outputs. Agent verified
application does not persist a failed candidate.

The additive feature tag needs no version bump. Both schemas add its required
standard forms. Migration never infers standards or threads for old `hole`
records. Legacy examples, deterministic file round trips and structural
rebuilding test this policy. No dependencies or OCCT types are added.

## Evidence and limits

Mock and OCCT tests cover clearance and tap-drill geometry, diameter mismatch,
cache invalidation and rollback. Table tests cover every supported size/fit,
nominal/pitch/drill dimensions and unsupported forms. The patch-authored plate
example combines M6 normal clearance and a blind M6 tap-drill hole; OCCT volume,
parametric depth edits, verified Agent refusal, file round trips and an eval task
cover the shared paths. External threads, fine/left-hand threads, inch standards,
counterbores/countersinks, physical threads and manufacturing tolerance classes
remain future work.
