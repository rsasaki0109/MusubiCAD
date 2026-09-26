# ADR-019: Consumed references in provenance

Status: Accepted  
Date: 2026-09-26  
Roadmap: MCAD-P7-011

## Context

Reference provenance (MCAD-P6-003) classifies every semantic reference
against the final regenerated body. Some features intentionally remove a
face that a reference names. A shell opening is the first such case
(ADR-017). The removed face no longer exists, so its semantic description
can match other faces.

For example, the `top` face of the box matches both the open rim and the
inner floor. `opencad regen` then reported
`ref:face:box_top Ambiguous 2 candidates tie`, which the MCP host trial also
hit. The shell itself is correct, but the report reads as a defect and
hides the real cause.

A general fix would resolve each reference against the output of its
creating feature. That changes what every reference means. This ADR takes
the narrow path instead.

## Decision

1. **New status.** `ReferenceStatus::Consumed` is added. A reference is
   consumed when an unsuppressed feature declares that it removes the face.
   Today only a shell does this, through `open_face_refs`.
   `FeatureDefinition::consumed_face_refs()` lists these references. A new
   face-removing feature must list its inputs there.
2. **Classification.** After final-body classification, regeneration marks
   every consumed reference as `Consumed`:
   - no kernel ID is resolved;
   - the reason names the consuming feature, for example
     `opened by feature:shell`;
   - the scored candidates are kept as evidence.

   Consumption takes precedence over any other status, because a consumed
   face can only match by coincidence.
3. **Blocking.** `Consumed` counts as unresolved (`is_unresolved`). A
   `required_reference` assertion on a consumed face fails and gives the
   consuming feature as its reason. Regeneration itself is unaffected;
   provenance stays observability data.
4. **Reporting.** `opencad regen` lists a `consumed=` count, and the review
   artifact shows the status.

The status is added to a serialized enum inside regeneration reports. Such
reports are not part of `.ocad`, so no schema migration is needed.

## Consequences

- Shell openings no longer produce ambiguity reports.
- Resolving against the creating feature's output remains possible later. It
  would need its own ADR.

## Evidence plan

- The shell enclosure example regenerates with `ref:face:box_top` as
  `Consumed`, naming `feature:shell`, and with no ambiguous references.
- A suppressed shell consumes nothing.
- A `required_reference` assertion on the opened face fails with the
  consuming feature in its reason.
