# Nominal metric thread profile SDK

`opencad_geometry::NominalMetricThread` is immutable arithmetic for
[ADR-067](../adr/ADR-067-nominal-metric-thread-profile.md), not a physical body.
`new(MetricThreadSize, ThreadHandedness, turns)` admits only coarse M3/M4/M5/M6/
M8/M10/M12 and 1–32 whole pitches. Handedness is explicit. No arbitrary/fine
pitch, lead/runout, tolerance class or rounded-root representation is inferred.

`major_diameter`, `pitch`, `length`, `basic_minor_diameter` and `pitch_diameter`
return SI `Length`s. The basic minor diameter refers to a flat-root nominal
profile, not rounded external d3. `radius_at(z: Length, azimuth: Angle)` returns
the radius of the meridional screw envelope, phase-zero crest at z=theta=0.
Finite z must lie between the square end planes; finite angles are bounded to
±64 full turns. Invalid values return `OpenCadError::Validation` without clamping.

`ideal_section_area_m2` and `ideal_volume_m3` are exact **ideal-profile** targets
in m² and m³, not kernel measurements. See the independent integration tests
and runnable SDK [example](../../examples/nominal_metric_thread_profile.md).
For M6×1, eight turns: ideal volume 183.657632385 mm³. Both end sections have
the same noncircular polar area; a full major cylinder is not this envelope.

No new GeometryKernel operation, Mock/OCCT solid, document source tag, patch
field, serializer, export or MCP eval is introduced here. Physical P16-007 must
first satisfy its construction/sampling admission and actual measured geometry,
then ship source/semantic/schema/history/authoring/eval evidence in both kernels.
