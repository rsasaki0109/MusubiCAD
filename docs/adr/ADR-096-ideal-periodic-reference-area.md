# ADR-096: Bound the ideal periodic reference surface area

Status: Accepted for MCAD-P16-043's read-only reference arithmetic only.
Date: 2026-10-16.

## Decision

Add `CircularFilletReference::surface_area_bounds_m2(max_width_m2, max_cells)`.
Return outward enclosures of the complete ideal body's unsigned surface area,
the blend, and the three retained supports separately. Use the already proved
ADR-072/073 envelope, not a variable meridional quarter-circle approximation.
This is an ideal measurement; neither kernel mass nor source admission changes.
P16-008 stays planned and no score is earned.

## Reduction and proof

ADR-073 derives the positive characteristic-circle area Jacobian `r*D/v`, where
`v=|C'|` and `D=v²-r'²-r*r''-r*n·C''`. With its existing
`n=o+A*cos(phi)+B*sin(phi)` and `0<=phi<=omega`, write
`D=D0+Dc*cos(phi)+Ds*sin(phi)`. The exact blend density per theta is
`r/v*(D0*omega+Dc*sin(omega)+Ds*(1-cos(omega)))`.
This analytically integrates the characteristic arc before bounding the remaining
one-dimensional theta integral. It includes tangential sphere-envelope motion.
Lengths normalized by R restore an R² scale. Positivity follows from ADR-072
exposure/regularity and ADR-073 orientation; intersecting with [0,+infinity)
is valid.

For `r=m+a*cos(theta)`, side contact is at height `H-r(theta)` on radius R;
end contact is at radial coordinate `R-r(theta)` in plane z=H. The other areas
(without artificial interface surfaces) are exactly

```
side = 2*pi*R*(H-m)
end = pi*((R-m)²+a²/2)
bottom = pi*R².
```

Identically authored radii use the analytic blend area
`2*pi*r*((R-r)*pi/2+r)` with zero cells. Unequal authored radii, even adjacent
binary64 values, retain the nonconstant path; no amplitude epsilon is introduced.

## Numerical contract

Reuse private outward IEEE-754 arithmetic and the four rational angular charts
from ADR-073, including its enclosed atan series, pi bracket and correctly rounded
sqrt assumption. Bisect the largest integral width, with chart and interval
endpoints as deterministic tie breakers. Accumulate both endpoints outward when
replacing a parent. The returned TOTAL area full width, including support and
roundoff contributions, must meet the finite positive absolute SI request.
`max_cells` is 4..=262144 leaves. Invalid arithmetic, unattainable width, exhausted
budget or stalled subdivision refuses without a partial result or mutation.
No libm quadrature or sample convergence is a proof.

## Validation and boundaries

Test the constant analytic formula at multiple scales, variable and admitted
boundary cases against independent two-dimensional integration of the original
reference tangents, separate support integration, deterministic refinement,
tiny positive amplitude, invalid requests and protected failures. Update the
existing both-kernel example/CI report to show ideal area separately from ACTUAL
faceted-body area. Numerical differences do not certify native conversion or
exact embedding. No area discrepancy certificate is claimed here.

Update API/architecture, examples, MCP authoring guide and skill. No serialized
change, schema/migration/golden change, editable NURBS or new MCP operation/eval.
Ideal first moments, native/placed/f32 errors, source ownership, stock consumption,
phase/history/readers and independently graded feature eval remain separate gates.
All mandatory local and applicable exact-head CI checks gate the authorized merge.
