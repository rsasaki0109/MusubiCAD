# ADR-078: Bounded periodic Bezier surface bands

Status: Accepted for MCAD-P16-025's transient conforming surface band only.
Date: 2026-10-15.

## Decision

Assemble ADR-076/077's separately bounded patches into a conforming cover of the
entire periodic reference band. Require explicit positive SI position error,
dimensionless unit-normal chord error and maximum patch count. No implicit user
quality, manufacturing tolerance or product feature is selected. Measure an
actual sewn native open shell; a shell is not the closed fillet solid required
by ADR-070. P16-008 remains planned, with no maturity credit.

## Coverage, refinement and work bounds

Use all four rational charts and a common dyadic n-by-n grid per chart, n=1,2,4,
... . Each cell has exact binary64 dyadic bounds within [0,1]. Order results by
chart, u index, v index. Neighbouring cells have identical shared edge poles,
including chart3 to chart0; no T-junctions or floating seam matching is inferred.
Coverage of the parameter band follows from this explicit tensor grid, not
surface samples. Refine all cells together to retain conformity and ordering.

Require finite positive requested position error, 0<normal chord error<=2 and
4<=max_patches<=16384. The fixed hard cap bounds allocation and interval work;
this prerequisite is not a general unbounded mesher. Before attempting a level,
require 4*n*n<=max_patches using checked integer arithmetic. At each cell compute
the actual poles, positional bound and independently recomputed normal/area
certificate. Return only if every cell meets both requested bounds. Coarse
interval-certification refusal or excessive error triggers dyadic refinement,
never acceptance by samples. Once a level fails, it need not evaluate its
remaining cells. Total attempted cell work is bounded by the geometric sum of
per-level limits. Refuse if the next conforming level exceeds budget; do not
return a partial or uncertified band. No mutations or hidden I/O occur.

Return n and ordered cells with their chart/domain, poles and local certificates,
plus maximum positional/normal error. Each normal area bound is with respect to
its own normalized local parameter square. These are current generated data;
editing them invalidates the cover's certificates. Shared position curves and
local immersion do not prove exact G1, global embedding or support trimming.

## Native topology evidence

The pinned SDK diagnostic consumes actual Rust-generated poles, explicitly
converts metre coordinates to millimetres, builds native Bezier faces and sews
them. Check actual native validity, one connected shell, zero solids, expected
face count and each edge's one-or-two-face incidence. Free-edge topology must
form exactly two disjoint closed cycles with degree two at every boundary
vertex; all other edges have incidence two. Check Euler characteristic zero.
This establishes the tested native open-band topology; it is not solid closure
or a proof of global geometric embedding.

Measure actual face values against an independent Bernstein evaluator at fixed
native parameter witnesses, using an explicit numerical metre allowance. Check
boundary vertices against the top-plane/side-cylinder reference contacts within
the band's positional bound plus the allowance. Native sewing tolerance is an
explicit diagnostic parameter in millimetres, not a new model tolerance. Keep
all concrete OCCT types inside modules/kernel-occt. Native topology/evaluation
tests remain distinct from the mathematical all-cell approximation certificates.

Include constant and genuinely nonconstant fixtures. Native report-contract
negative controls must reject missing cells/faces, extra boundary loops,
nonmanifold incidence, unexpected solids or bad native surface witnesses.
Do not fabricate a physical Mock body from these reference arrays.

## Validation and remaining gates

Test exact grid coverage, shared complete edge nets, requested-error admission,
determinism, refinement, scales and invalid/unreachable budgets independently.
Run an actual native sewing diagnostic in CI and save its report. Update API,
architecture, example, guide, skill and plans. All four mandatory local checks
and all applicable exact-head workflows/jobs gate merge.

Support trimming, exact/native tangent-continuity admission, closed physical
Mock/native bodies, independent area/volume/moment bounds, mesh policy,
source/schema/cache/history and actual MCP eval remain separate P16-008 gates.
No editable NURBS, serialized source, migration or golden change is introduced.
