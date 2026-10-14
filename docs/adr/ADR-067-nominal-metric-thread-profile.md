# ADR-067: Nominal metric basic-profile thread contract

Status: Accepted for MCAD-P16-014; physical construction admission remains pending.
Date: 2026-10-14.

## Context and reference

ADR-037's tap-drill cylinder has no physical thread. The generic OCCT helical
sweep does not prove standard flank geometry, a connected root, clipped ends or
an independent volume. Neither receives new P16-007 credit.

The selected representation follows the 60-degree ISO metric **basic profile**
(ISO 68-1 terminology), using the same nominal/coarse size pairs as ADR-037.
Public comparison: [cq_warehouse IsoThread](https://github.com/gumyr/cq_warehouse/blob/main/src/cq_warehouse/thread.py)
was read on 2026-10-14: its `min_radius`, `apex_width` and `root_width` use a
5H/8 radial depth, P/8 crest and 3P/4 tooth base. This is a representation
comparison, not an authoritative tolerance table or an executed benchmark.
MusubiCAD's formulas/tests are independently derived; no upstream code is copied.

## Decision

Name the future representation `nominal_metric_basic_flat_root`; never describe
it as a rounded-root production external thread or a 6g/6h tolerance class.
No claim of manufactured fit, gauge compliance, stress suitability or assembly
clearance. A future rounded-root/tolerance representation requires another ADR.

P16-014 supplies a transient, immutable kernel-neutral **profile calculation**.
It creates no body/mesh, changes no document and supports no new DesignPatch.
Only M3×0.5, M4×0.7, M5×0.8, M6×1, M8×1.25, M10×1.5 and M12×1.75 mm,
single start, explicit right/left handedness, 1–32 whole pitches of axial length.
Both ends are the planes z=0 and z=L, square clips of the actual radial envelope.
No fade/runout/chamfer, fractional engagement, custom/fine pitch, inch standard,
multiple starts or implicit handedness. Length and Angle inputs carry SI units;
coordinates are local to the future source-owned rigid axis frame.

Let P be pitch, D major diameter, H=sqrt(3)P/2, R=D/2, r=R-5H/8.
Basic minor diameter is D-5H/4; pitch diameter is D-3H/4.
The **rounded external design-root** value D-17H/12 is a different model and
must not be substituted or reported for this flat-root profile.

Use q=(z/P-s*theta/(2pi)) modulo 1, s=+1 right and -1 left. A crest is centred
on q=0; increasing theta follows increasing z for a right-handed crest. With
t=min(q,1-q), radius is R for t≤1/16, r for t≥3/8, and
R-sqrt(3)P(t-1/16) between. Thus crest width P/8, root flat P/4, two flanks
5P/16 each, 60-degree included angle in the **meridional** section. There is no
normal-to-helix profile substitution. Finite z must be within [0,L]; angles
within ±64 full turns prevent enormous-phase arithmetic from posing as precision.
Invalid input refuses rather than clamping, extrapolating or inventing a size.

## Independent physical oracle

At any z, rotating the section preserves area. Integrate radius squared over
one angular period: root contributes r²/4, crest R²/8; each linear flank
contributes (5/16)(r²+rR+R²)/3. Therefore
A=pi(11r²+5rR+8R²)/24 and V=L*A. Integer-turn length makes transverse first
moments cancel; axial COM is L/2. End caps have this noncircular section area,
not pi*R² or pi*r². A full major cylinder or an unjoined spring fails the oracle.
Independent tests use numerical section integration, separately evaluated
piece integrals, dimensional constants, measured meridional slopes, screw phase,
both hands and refusal. Sharing this utility between kernels is not independent
physical proof: future tests must measure the actual native/Mock solid/mesh.

## Physical construction and source gates still required

P16-007 is incomplete until a further accepted construction/sampling ADR supplies
actual connected solid geometry in both kernels. First establish that the chosen
OCCT construction represents this meridional screw envelope and both square
clips, including kernel precision and protected failure. Any approximate/faceted
representation needs explicit deterministic error bounds, dimensions/volume
budgets, topology/winding checks and honest approximation naming. Do not silently
fall back to the existing sweep or a cylinder when a constructor fails.

Measure actual pitch, crest/root widths and diameters, pitch diameter, phase and
handedness, cap/lateral contact, independent volume/first moments, closed single
solid topology and rigid placement. Preserve original stock and refuse unsupported
consumers instead of synthetic Mock measurements. Nominal formulas alone earn
no physical-feature credit.

The later source requires original full-cylinder extrusion and owned axis/side,
explicit closed representation/hand/end fields, strict unit expressions, mandatory
physical candidate verification, semantic output ownership and cache checks.
New source tag needs both schemas, no-inference migration, true older-reader
refusal, packed/expanded roundtrips, DesignPatch transactions, rollback/Undo/Redo,
example, authoring guide/skill and independently graded actual MCP eval.
Planning/profile infrastructure earns no maturity increment; P16-007 receives
only justified partial nominal-thread credit after all deliverables merge.
