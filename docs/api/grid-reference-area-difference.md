# Grid reference area difference (P16-044)

[ADR-097](../adr/ADR-097-grid-reference-area-differences.md) adds the immutable
`CircularFilletGridSolid::area_comparison(CircularFilletGridAreaPrecision)`.
The ideal reference is privately bound during mesh creation/grid admission;
there is no caller-supplied reference or metadata to trust.

Precision fields are explicit:

- `maximum_reference_interval_width_m2`: finite positive FULL ideal TOTAL area
  interval width in m², including blend, retained supports and roundoff.
- `maximum_absolute_difference_m2`: finite positive absolute discrepancy budget
  in m², applied to the complete returned difference enclosure.
- `max_cells`: 4..=262144 ideal reference leaves, unchanged from ADR-096.

The immutable `CircularFilletGridAreaComparison` exposes getters:

- `grid_unsigned_area_m2()`: outward interval for the actual stored binary64
  triangles' unsigned areas, counted with multiplicity.
- `ideal_area_m2()`: independently enclosed complete ideal reference area.
- `signed_difference_m2()`: grid minus ideal, with outward subtraction.
- `absolute_difference_upper_m2()`: maximum absolute endpoint of that interval.
- `relative_difference_upper()`: outward absolute upper bound divided by the
  strictly positive ideal lower bound; dimensionless, no midpoint substitution.
- `reference_cells()`: zero for identically authored constant radii, otherwise
  angular integration leaves.

Each actual stored triangle contributes `|cross(b-a,c-a)|/2` using private
outward IEEE-754 operations and correctly rounded sqrt. Sum in stored order.
ADR-073/096 rounding assumptions apply. This is mathematical triangle area,
not a finite-sample convergence estimate or a geometric-union/embedding proof.
Overlap would be counted with multiplicity. Existing TriangleSolid admission,
quality and resource limits are unchanged.

Invalid precision/work, unresolved facets/arithmetic, unattainable ideal width
and excessive discrepancy return an error without partial output, mutation,
coarsening or an alternate reference. Equality with the returned absolute upper
bound is accepted as an error budget. No certificate propagates through later
placement/native/f32 conversion. Actual kernel mass properties stay actual.

The existing both-kernel example and CI JSON add `area_comparison` with all
intervals/bounds/cells and `native_conversion_certified:false`,
`exact_global_embedding_certified:false`. Actual body areas and ideal component
areas remain separate. Both-kernel regression uses an explicit 1e-10 m² native
numerical allowance; this allowance is not part of the mathematical certificate.
See [example](../../examples/physical_grid_reference_bodies.md).

No authored fillet, ideal first-moment, source/schema/migration/golden, MCP
operation/eval or maturity credit is introduced; P16-008 remains planned.
