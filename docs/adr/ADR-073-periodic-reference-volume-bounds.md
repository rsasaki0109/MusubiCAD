# ADR-073: Bounded periodic reference volume

Status: Accepted for MCAD-P16-020's transient reference integration only.
Date: 2026-10-14.

## Decision

Extend ADR-072's ideal reference with an explicit absolute volume enclosure in
cubic metres. Reduce its characteristic-arc integral analytically, then enclose
one remaining angular integral using deterministic interval subdivision. This
admits neither kernel solids nor native approximation, meshes, source, MCP or
feature credit. P16-008 remains planned. Keep the existing constant-only method's
refusal semantics; add a separately named bounded method.

## Independent reduction

For the removed corner use the divergence field (0,0,z-H), with divergence 1.
The top annulus and cylindrical band contribute zero; on the blend the removed
region's outward normal is -n. Hence

```
V_removed = integral r^2/v * integral (1-n_z)*n_z*D d_phi d_theta
v^2 = |C'|^2 = (R-r)^2 + 2*r'^2
D = v^2-r'^2-r*r''-r*n dot C''
C'' = (-(R-r)-r'', -2*r', -r'')  in (e_r,e_theta,e_z).
```

To obtain the Jacobian, differentiate the envelope equation to get
C' dot X_theta = D and C' dot X_phi=0. Write T=C'/v, q=-r'/v,
s=sqrt(1-q^2). The normal's characteristic circle is n=q*T+s*(U*cos(phi)+W*sin(phi)).
Its tangent has length s; projection on T perpendicular to n has length s too.
Consequently the outward area Jacobian is r*D/v. ADR-072 proves its positivity.

Let b=R-r, d=r', f=d^2/(b^2+2*d^2), and define local vectors

```
o = (f,-d*b/(b^2+2*d^2),f)
A = e_r-o
c = -d^2/(b^2+d^2)
s_omega = sqrt(1-c^2)
B = (e_z-o-c*A)/s_omega
n = o+A*cos(phi)+B*sin(phi)
omega = pi/2 + atan(d^2/(b*sqrt(b^2+2*d^2))).
```

The end normals are e_r and e_z; phi runs from 0 to omega. The inner integrand
is a polynomial in cos(phi),sin(phi) of degree at most 3. Integrate its ten
monomials exactly using omega, c and s_omega:

| powers (cos,sin) | definite integral |
|---|---|
| (0,0) | omega |
| (1,0), (0,1) | s_omega, 1-c |
| (2,0), (0,2), (1,1) | (omega+s_omega*c)/2, (omega-s_omega*c)/2, s_omega^2/2 |
| (3,0), (2,1) | s_omega-s_omega^3/3, (1-c^3)/3 |
| (1,2), (0,3) | s_omega^3/3, 2/3-c+c^3/3 |

This is not a meridional variable quarter-circle approximation. It retains the
sphere envelope's tangential displacement. H cancels from removal; body volume
is pi*R^2*H minus removal.

## Numerical enclosure and work admission

Normalize lengths by R and restore R^3 through outward interval operations.
Cover theta with four rational quarter-circle charts, u in [0,1]:
cos= (1-u^2)/(1+u^2), sin=2*u/(1+u^2), rotated by j*pi/2;
d_theta/du=2/(1+u^2). No transcendental sine/cosine evaluator is used in the
integrator. All authored floating-point dimensions are treated as exact inputs.

Use private binary64 interval operations: next_down/next_up after each elementary
addition, subtraction, multiplication, division and correctly rounded square
root. Division/square root require valid domains; nonfinite intermediates refuse.
The enclosure assumes IEEE-754 round-to-nearest primitive arithmetic and correctly
rounded sqrt, without fast-math/reassociation. No native or external libm accuracy
assumption enters. Bracket pi with adjacent binary64 values around Rust's PI;
these contain the classical decimal bounds
3.14159265358979323846264338327950288 < pi <
3.14159265358979323846264338327950289.
The atan argument is nonnegative and at most 1/36 in the admitted geometric
range. Evaluate its first eight alternating odd-power terms with interval
arithmetic and enclose the tail by +/-x^17/17. Refuse an interval argument beyond
1/8 rather than rely on an unproved series domain; coarse boxes remain below it.

Each chart interval bounds the whole density range, multiplied by its parameter
width. Its integral therefore lies in that product; intersect the lower bound
with zero using the proven positivity. Sum endpoint bounds with directed rounding.
Bisect the largest contribution-width cell; break ties by chart and dyadic
endpoints. Update lower and upper sums independently with directed rounding,
not by subtracting an interval from itself. The leaves always partition all four
charts, so there is no unbounded truncation estimate. Budget at most 262144
leaves, including four initial cells. Refuse a split whose midpoint rounds to an
endpoint, invalid requested widths/budgets, nonfinite arithmetic, or exhaustion
before the requested final body-volume width is achieved. Never return the last
coarse estimate as a successful precise result. No implicit tolerance/default
sampling policy or global state is introduced.

An identical authored radius pair can use an outward-rounded constant formula.
An explicitly positive amplitude is never snapped constant. Return lower/upper
body and removal volumes, with leaf count; do not present a midpoint as an exact
mass property. Width admission uses an upward-rounded endpoint difference and
includes normalization, constants, summation and stock-volume roundoff.

## Evidence and boundaries

Test interval primitives against exact rational cases and rounding boundaries;
compare the reduced density to independent ADR-072 point/analytic-tangent surface
integration. Check nonconstant volume with an independent two-dimensional
surface integral, constant limits, different scales, explicit tiny amplitudes,
refinement, deterministic repetition and protected budget/precision refusal.
These independent test quadratures are numerical oracles with stated convergence
tolerances, not the source of the enclosure proof. They do not certify OCCT.

Update API, architecture, authoring guide, skill, example and plans. No serialized
feature changes, migration or runtime MCP eval applies. All four mandatory checks
and applicable exact-head CI jobs still gate merge. Actual Mock/native construction,
contact/phase/periodicity fidelity, mesh admission and guarded authoring remain
separate decisions. No kernel mass may be replaced by this ideal interval.
