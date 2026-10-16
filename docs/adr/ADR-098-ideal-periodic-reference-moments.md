# ADR-098: Bound ideal periodic first moments and centroid

Status: Accepted for MCAD-P16-045's read-only ideal reference arithmetic.
Date: 2026-10-16.

## Decision and reduction

Add explicit SI volume/full first-moment/full centroid width requests and a leaf
budget, returning ideal body volume, three first moments (m⁴), centroid (m) and
separate volume/moment leaf counts. This is not a kernel mass query or source
admission. P16-008 remains planned; no score.

Use ADR-073's characteristic arc, n=o+A*cos(phi)+B*sin(phi), positive area
Jacobian r*D/v and removed-volume density Q=r²/v*(1-n_z)*n_z*D. The divergence
fields (0,0,x*(z-H)) and (0,0,(z²-H²)/2) have divergence x and z respectively.
They vanish in flux on the removed corner's top annulus and cylindrical band;
the removed region has blend outward normal -n. Therefore

```
removed_Mx = integral x*Q d_phi d_theta
T = integral r³/(2*v)*(1-n_z)²*n_z*D d_phi d_theta >= 0
body_Mx = -removed_Mx
body_Mz = H*(body_volume - stock_volume/2) + T.
```

In the rotating radial/tangential frame,
`x=(R-r+r*n_r)*cos(theta)-r*n_t*sin(theta)`. Both integrands are polynomials
in characteristic cos/sin of degree at most four. Keep ADR-073's first ten
moments, and add (c=cos(omega), s=sin(omega)):

```
integral cos⁴ = s*c³/4 + 3*(omega+s*c)/8
integral sin⁴ = -c*s³/4 + 3*(omega-s*c)/8
integral cos³*sin = (1-c⁴)/4
integral cos*sin³ = s⁴/4
integral cos²*sin² = (omega-s*c*(c²-s²))/8.
```

The original cosine law is invariant under reflection y->-y (theta->-theta,
r'->-r', tangential normal component reverses). The ideal solid and removed
region inherit that reflection, hence My and centroid_y are EXACTLY zero.
For identically authored constant radii, axial symmetry also gives Mx=0 and
the exact constant correction is
`T=pi*r³*((R-r)*(5/3-pi/2)+r/12)`. Unequal authored radii never snap constant.
These formulas follow directly by integrating sin(phi)*(1-sin(phi))² and its
cos(phi) multiple over [0,pi/2].

## Enclosure and work

Keep existing volume and area algorithms unchanged. Recompute the independent
ADR-073 volume enclosure at the explicit requested full width. Integrate the two
moment densities with deterministic outward rational-chart subdivision, normalized
by R and restored by R⁴. Use the same IEEE-754/sqrt/pi/atan assumptions as ADR-073.
Only T has a proven nonnegative integrand; never clamp the signed x contribution.
Split the largest maximum component-width cell with deterministic chart/end ties.
The leaves cover all four charts; sum endpoint replacements independently outward.

`max_cells` in 4..=262144 applies SEPARATELY to volume and moment integrations;
return both actual counts. Three finite positive requests specify full volume
width (m³), maximum full width of EACH first moment (m⁴), and maximum full width
of EACH centroid coordinate (Length). Divide moment intervals by the strictly
positive volume interval outward. Known symmetry zeros stay exact. Final admission
includes volume, stock, moment, summation and division roundoff. A coarse volume
enclosure may prevent a tight centroid request; do not silently tighten its
request, substitute a midpoint or return partial results. Invalid domains,
unattainable precision, work exhaustion or stalled refinement refuse.

## Evidence and limits

Independently integrate the original point/tangent parameterization with the
divergence fields, plus a constant meridional cross-section calculation. Test
scales, admitted radius boundary, signed x/zero y, constant rotational symmetry,
determinism, refinement, tiny amplitude and protected precision/work failures.
Show ideal intervals separately from actual both-kernel mass/centroid measurements
in the existing example/CI report. No native/placed/f32 conversion or actual-grid
moment discrepancy certificate, schema/migration/golden, new MCP operation/eval,
editable NURBS or production/source feature is introduced. Update API/architecture,
example, guide, skill and canonical plans; mandatory local and exact-head CI gate merge.
