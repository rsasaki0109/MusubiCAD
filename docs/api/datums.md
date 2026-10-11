# Parametric construction datums

`datum_plane` and `datum_axis` are non-solid Feature Graph nodes (ADR-035,
MCAD-P11-001). Add or edit them with `add_feature` and
`replace_feature_definition` DesignPatch operations. Put each datum before its
consumers in the authored feature order. Datums create no body, mass or mesh.

A plane offsets `base` along its normalized normal by signed `offset.value_si`
metres. Optional `offset_expr` is a length expression, such as `mount_height`.
`base` accepts existing global/custom/semantic-face workplanes or another datum
plane. Custom frames must be finite, non-degenerate and orthogonal. Direction norms
must be at least 1e-12; normalized frame dot products must be within 1e-9 of
zero (dimensionless).

```json
{"type":"datum_plane","base":{"type":"global","plane":"XY"},
 "offset":{"value_si":0.005},"offset_expr":"mount_height"}
```

Create a sketch with `workplane` set to
`{"type":"datum_plane","feature_id":"feature:mount_plane"}`. Extrusions
resolve the frame during regeneration, so edits move the resulting solid.
Sketch overlays use a resolved copy and retain the authored reference in `.ocad`.

An axis stores `origin_m` in metres and a dimensionless `direction`. The direction
is normalized; a zero/non-finite vector is refused. Optional `origin_expr` has
three entries (length expressions or null for a numeric coordinate).

```json
{"type":"datum_axis","origin_m":[0,0,0],"direction":[0,1,0],
 "origin_expr":["axis_x",null,null]}
```

Set a revolve's optional `axis_feature` to the datum feature ID. Its numeric
`axis_origin_m`/`axis_direction_m` fields remain required for wire compatibility
and are used only when `axis_feature` is absent. OCCT now honours the resolved
axis origin by revolving in relative coordinates and translating back.
Revolve sketches still require a global XY/YZ/XZ workplane; arbitrary or datum
plane revolve profiles are future work.

Missing, suppressed or wrong-kind datum outputs refuse regeneration. References
participate in dependency ordering, cycle checks and cache keys. Failed patches
and failed regeneration preserve previous source/output state. Old numeric-axis
and workplane documents retain their meaning; no datum association is inferred
by migration.

The shared kernel-neutral executors run for both Mock and OCCT. Mock keeps its
existing synthetic-body geometry contract; OCCT tests compare volume and bounds
against analytic solutions. `examples/datum_plate.ocad.d` is built by
`examples/agent/create_datum_plate_patch.json` and round-trip/rebuild tested.
`datum_axis_bushing` in `tools/mcp_eval_tasks.json` checks a 5 mm axis edit against
the analytic bushing volume.

Apply `examples/agent/bushing_datum_axis_patch.json` to a copy of
`examples/revolve_bushing.ocad.d` for a datum-axis solid example (expected
volume 18,849.556 mm³). Changing `axis_x` from 5 mm to 10 mm changes the
radii to 5/15 mm and the volume to 12,566.371 mm³.
