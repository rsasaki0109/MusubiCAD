# ADR-092: Construct bounded closed periodic reference triangle meshes

Status: Accepted for a read-only closed diagnostic triangle shell only.
Date: 2026-10-15.

## Decision and scope

Register P16-039 before implementation. Construct an actual indexed f64 SI
triangle shell around the admitted ideal rolling-sphere reference, including
unchanged cylindrical side and planar top/bottom regions. This supplies a bounded
mesh construction prerequisite; it does NOT register physical Mock/runtime,
source, editable NURBS or a production fillet. P16-008 remains planned; no score.
Triangle orientation/manifold topology and actual native solid measurements do
not establish global embedding, exact G1 or whole native floating/integration
fidelity. Preserve the existing rational native diagnostic and all its proofs.

## Conforming layout and bounded refinement

Use four existing ADR-075 charts theta=c*pi/2+2*atan(u), alpha=2*atan(v).
Uniform dyadic n=1,2,...,64 subdivisions per chart direction give K=4n angular
stations and 2n+1 profile rows: n side-cylinder intervals from z=0 to the side
contact, followed by n fillet intervals to the end-plane contact. Reuse actual
rounded fillet corners from ADR-075; share their vertex IDs with the side/caps.
Canonical quarter/period endpoints use exactly the same corner evaluator.
Two centre vertices close top and bottom with triangle fans. Never infer welds
from proximity or route through the STL reader's unrelated 1e-9 m grid.

Vertex count is 4n(2n+1)+2; triangle count is 16n^2+8n. Explicit requested vertex
and triangle budgets must fit before allocating a level; hard maxima are 33026
vertices / 66048 triangles. All cells must meet finite positive SI position
precision and a unit-normal chord bound strictly in (0,1). Failed coarse
interval bounds or quality trigger complete dyadic refinement; exhausted work
refuses with no partial shell. Order vertices angular/profile, triangles by
angular/profile cell then top/bottom, fixed diagonal (0,1,2),(0,2,3). Diagnostic
face groups 1..4 fillet,5..8 cylinder,9 top,10 bottom are NOT semantic/native IDs.
No f32 export is certified by this method.

## Whole position bounds

For fillet cells reuse ADR-075's complete triangle Taylor/Hessian bound with
actual rounded corners; verify the shared vertex data have the same binary64
representation as those corners (signed zero denotes the same zero).
For side F(u,w)=(R*cos(theta),R*sin(theta),w*(H-r(theta))), theta_u<=2,
abs(theta_uu)<=4, amplitude a=(rmax-rmin)/2, hence

    norm(F_uu)<=8*sqrt(R^2+a^2), norm(F_uw)<=2*a, F_ww=0.

For hu=hw=1/n the existing two-triangle Taylor inequality therefore gives
(sqrt(R^2+a^2)+a/2)/n^2, plus the maximum outward vertex-rounding error.
Enclose original side-contact coordinates using their ADR-075 rounding radius;
propagate actual stored z=w*z_contact with outward operations. Bottom contact
XY and top contact vertices retain their original errors.

For cap fans use barycentric correspondence w=beta0+beta1,
t=beta1/w (w>0), ideal point w*E(u0+t*hu)+(1-w)*centre. At w=0 all parameters
map to the same exact centre. The one-dimensional linear interpolation error
is at most norm(E_uu)*hu^2/8 plus endpoint error, uniformly in w. Bounds are
8R for the bottom circle and 8R+16a for the top E=(R-r)*e_r. Thus cap errors
are R/n^2 and (R+2a)/n^2 plus current endpoint rounding. Do not claim these
barycentric parameters are the fillet's rational characteristic-circle parameter.
All differences/products/divisions/norms and budget comparisons are outward.

## Whole normal and topology checks

Compute each actual facet cross product and normalized direction with outward
interval arithmetic from its exact stored f64 vertices; require a positive
squared-norm lower bound. Compare to the complete ideal normal enclosure over
its cell: ADR-075 fillet normals, rational-circle cylinder normal enclosure,
and +/-Z cap normals. Require positive lower dot alignment and the requested
whole-cell normal chord bound. Samples cannot authorize a triangle.

The known periodic ring/cap layout must have finite vertices, nondegenerate
facets, all ten groups, connected face adjacency, exactly two opposite uses of
every edge and Euler characteristic two. These are shell/topology/local
orientation statements, not a global intersection proof. Expose immutable f64
vertices/triangles/groups plus bounds and actual signed boundary measurements
(volume flux, unsigned facet area and first moments). Do not relabel those
integrals as a physical Mock body or insert ideal mass into native output.

## Independent and actual native evidence

Tests independently recompute incidence, connectivity, normals, signed tetrahedron
integrals, bounding boxes and constant-limit polygon-frustum volume/area/z moment;
exercise variable laws, scale, deterministic refinement, error/work refusal and
independent ideal samples within position/normal bounds. Compare refinement to
ADR-073's independent ideal volume and actual retained analytic support geometry.

A separate pinned OCCT diagnostic consumes actual generated f64 triangles,
converts metres to millimetres explicitly, constructs/sews triangular planar
faces and measures real closed connected positive-volume solids without winding
repair. Check expected faces/edges/vertices/incidence/Euler, current native
vertices and independent triangle/native volume/area/COM. Constant and variable
fixtures must be distinguished; negative report/process controls refuse missing,
changed, reversed or overclaimed data. The native mesh diagnostic is faceted, separate from ADR-084's unchanged
analytic-support rational BREP. This is native numerical evidence, not
a certificate of arbitrary kernel floating evaluation or global embedding.
Keep concrete OCCT types under kernel-occt, retain existing diagnostics unchanged,
and add the new actual probe to CI/report metadata. Update API, architecture,
example, MCP guide, skill and plans. All four mandatory local checks and all
exact-head applicable workflows/full job inventories must pass before merge.
No source/schema/golden/new MCP operation or runnable eval applies here.
