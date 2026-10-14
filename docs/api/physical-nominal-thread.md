# Physical nominal metric thread runtime

`GeometryKernel::cut_nominal_metric_thread(body, &NominalThreadInput)` constructs
ADR-067's nominal basic flat-root envelope after [ADR-068](../adr/ADR-068-physical-nominal-metric-threads.md)
admission. The source body must have the actual uncut full-cylinder witness,
diameter equal to the selected coarse major and height equal to1–32 whole pitches
within1nm. Supported coarse sizes M3/M4/M5/M6/M8/M10/M12, explicit right/left hand
and square ends. No arbitrary pitch, rounded root, tolerance class or runout.

`NominalThreadInput` has an immutable `NominalMetricThread` profile, unit
`axis_hint` selecting the witnessed axis sign, and perpendicular unit
`radial_axis` fixing the phase-zero crest. Origin/actual axis come from the
body-owned witness, not bounds/mean normals. An opposite hint starts at the old
end and reverses the axis. Invalid body/dimensions/frames refuse before output;
source remains immutable and no reconstruction ancestry is invented.

`input.admit(&CylindricalStockGeometry)` gives immutable `NominalThreadGeometry`.
`point_at(Length,Angle)` evaluates its world envelope; `mass_properties(density)`
returns exact ideal volume/area/COM with SI kg/m³; `bounding_box` solves support
extrema on knot helices/cap contours, including tilted placements. `transformed`
admits proper finite rigid transforms, enforcing the world/axis bounds again.
`NominalMetricThread::ideal_surface_area_m2` adds the exact ideal exterior area.

`geometry.mesh_plan(&TessellationSettings)` yields angular count, interpolation
bound in metres and normal bound in radians. N is divisible16 and at least64;
side/cap error reserves half linear deflection for actual f32 representability.
Angular quality below the unsubdivided-profile normal floor refuses. Integer
band clipping and canonical keys produce real closed Mock triangles and six
physical surface classes. Defaults are supported through32 pitches; fine quality
can exceed the N8192/100000-vertex/200000-triangle budget and refuses. Both
backends preflight quality; actual f32 coordinate/normal errors and native output
budgets are checked, with no looser retries. Mock triangle integrals approximate
its exact analytic mass; mesh values are not interchangeable with ideal properties.

Native constructs in local mm with explicit precision, a connected per-turn
spine, Frenet valley tool, deterministic cut/square clipping and proper placement.
Left hand reflects the proved local right geometry before world placement.
Actual native properties use adaptive BRepGProp integration epsilon1e-9; ideal
profile values never replace a measured BREP. Thread-containing bounds use the
existing tight native query. Translating/rigidly transforming threads preserves
their transient physical metadata and rejects improper/unresolved placements.
Use `transform_body` for rotations; legacy direct `rotate_body`/`mirror_body`
edits of these runtime records refuse. The left-hand construction reflection
does not enable arbitrary reflected output placements.

Default synthetic Mock refuses. Explicit analytic Mock has a separate physical
thread record; unsupported generic Booleans/refinements/inertia do not fall through
to synthetic measurements. Native has actual OCCT surfaces/face IDs, which need
not have Mock's cardinality. Neither record owns document/source semantics.
See the runnable [both-kernel example](../../examples/physical_nominal_thread.md).

P16-015 is runtime only. P16-007 now provides the separate
[source feature](nominal-metric-thread-source.md), proving original authored source/owned side,
semantic/cache ownership, mandatory DesignPatch verification, schema/migration/
old-reader refusal/roundtrips, history and actual source-aware MCP eval. Runtime
work earns no maturity credit. The source feature is unreleased. v0.3.1 predates this runtime.
