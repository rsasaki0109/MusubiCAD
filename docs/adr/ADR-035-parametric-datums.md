# ADR-035: Parametric construction datums

Status: Accepted

## Context

MCAD-P11-001 requires editable planes and axes in the Design Graph. Static
custom sketch workplanes and numeric revolve axes cannot follow a named
construction feature when parameters change.

## Decision

Add `datum_plane` and `datum_axis` feature definitions and non-body construction
outputs. A plane offsets an existing workplane along its normal by a signed
length, optionally driven by a length expression. An axis has an origin in
metres, a direction vector, and optional length expressions for its origin
coordinates. Normalize directions and reject non-finite or degenerate frames.

A sketch can name a datum plane feature. A revolve can name a datum axis;
its legacy numeric axis remains the default when the reference is absent.
Datum references are Feature Graph inputs and participate in ordering, cycle
checks, suppression refusal and cache invalidation. Missing outputs or a wrong
construction kind fail closed. Datum planes may be based on global/custom
workplanes, semantic face workplanes or other datum planes.

Datums are kernel-neutral arithmetic executed through the shared feature
registry for both Mock and OCCT. They create no solid, mass or export mesh.
Downstream solid operations still use the selected GeometryKernel.

The additive wire forms preserve existing documents without inventing datum
associations. Extend schemas, document the identity migration, and test legacy
and current round trips. All authoring uses existing DesignPatch operations.

## Consequences

Analytic tests must prove placed extrusion bounds/volume and a revolve using
a moved axis, including parameter edits and deterministic cached regeneration.
Continuous topology reassociation, interactive datum handles and advanced
plane/axis constructors are later work. Datum sketches need resolved frames
for overlays; unresolved construction references must not silently draw at XY.
