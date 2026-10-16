# Numerical faceted circular fillet source (ADR-101)

`faceted_circular_fillet` is a distinct Design Graph feature using the physical
ADR-100 runtime in analytic Mock and OCCT. It rounds one original circular
cylinder's complete positive end rim with the single-period cosine law
`r(theta) = (minimum + maximum)/2 + (maximum - minimum)*cos(theta)/2`.
Maximum radius lies on the positive original sketch X axis; X cross Y is the
positive extrusion direction. Seam changes do not change authored phase.

All output surfaces are planar facets, including the caps and retained side.
This explicitly named approximation has no exact G1/smooth support claim.
The source retains `representation: grid_faceted_periodic_cosine` and
`verification: canonical_grid_with_numerical_backend_checks`. Legacy `fillet`
and its linear straight-edge law retain their meanings.

Required closed fields are `target_feature`, `edge_ref`, both tags above,
`minimum_radius`, `maximum_radius`, and `quality`. Length objects contain only
`value_si` in metres. Radius expressions are initially unsupported: use typed
`replace_feature_definition` to edit radii and `set_feature_ref` / `edge_ref`
to edit the owned reference. Coordinated stock/radius edits belong in one patch.
Original stock dimensions retain strict length expressions and fallback checks.

Quality has no implicit defaults. `maximum_position_error` (Length) and
`maximum_normal_chord` apply to both raw and final stored grid. The following
absolute SI quantities bound canonical reference integration and differences:

| Reference full width | Permitted absolute stored-grid difference |
|---|---|
| `ideal_volume_width_m3` | `maximum_volume_difference_m3` |
| `ideal_area_width_m2` | `maximum_area_difference_m2` |
| `ideal_first_moment_width_m4` | `maximum_first_moment_difference_m4`, each axis |
| `ideal_centroid_width` (Length) | `maximum_centroid_difference` (Length), each axis |

`max_integration_cells` is at most 262144 per integration pass; `max_vertices`
and `max_triangles` are at most 10000. The existing 1 nm grid and intersection
work limits remain. Unreachable precision, final-grid quality or work limits
refuse without coarsening or changing the request.

Canonical outward arithmetic covers the unplaced input grid only. Native BREP
mass, placed output and f32 mesh conversion are numerical measurements. Source
admission separately verifies actual volume/area within 1e-8 relative of the
placed grid, COM within 20 nm plus f64 coordinate roundoff, and the complete
face-bearing triangle geometry/winding. Corner comparison uses
`1 nm + 8*f32::EPSILON*max_world_coordinate`; distinct corners must stay more
than twice that apart. That allowance must be no larger than minimum radius/32
and nonzero cosine amplitude/32. Large placements or unresolved variations
therefore refuse, even if the canonical grid budgets pass.

Require a unique original active exact-circle Sketch / Distance / NewBody
extrusion and a unique semantic Edge with `created_by = target_feature`, role
`extrusion_end`. Actual full-cylinder/rim/circle queries must agree with source.
Other active stock modifiers, aliases, start/foreign refs and stale hints refuse.
An optional circle fingerprint may carry only `surface_type: circle` and a
current/stale kernel edge binding: a binding to another present edge refuses;
an absent binding may rebind only after unique owned full-circle proof.
Closed-circle seam-dependent midpoint/tangent hints, face/area hints, adjacency
and fallback queries are unsupported and explicitly refuse.

Native display normals are derived from actual oriented facet triangles, with
separate display vertices at sharp boundaries. Physical coordinates, triangle
IDs and winding stay unchanged; this numerical display conversion is not a
propagated canonical certificate.

Every output physical face is owned by this feature as `faceted_surface`.
Role-only subface selection refuses, including attempts to bypass it by omitting
or renaming the role. No original cap/side/edge provenance or synthetic history
survives. Cache hits re-admit source, current rim, authored phase and actual
placed facets; cached producer lists alone are insufficient. Bounds/scalar mass
cannot stand in for the actual geometry.

Creation, edits, preview, history and MCP all use DesignPatch and transactions.
Native candidate verification is mandatory even with `verify: false` or
`--no-verify`. Failed candidates preserve source/history/output/artifacts;
review and dry-run are read-only. Both schemas require the same fields; migration
never infers the new intent and the actual pre-extension enum rejects its tag.

See [the runnable example](../../examples/faceted_circular_fillet_source.md).

Canonical integration checks are recomputed for execution and cache admission.
The example's cold regeneration measured about 10.6 seconds in this cloud debug
build; reviewed edits perform several regenerations. This initial implementation
prioritizes the explicit admission contract and does not reuse unchecked cached
certificates or silently relax requested quality.
