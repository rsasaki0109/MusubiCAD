# ADR-040: Semantic angular dimensions from straight-edge rays

Status: Accepted  
Date: 2026-10-11  
Roadmap: MCAD-P12-001

Angular annotations reference two semantic edges and explicit canonical endpoint
choices (`start`/`end`, sorted lexicographically in model coordinates). Their
supporting straight lines must meet within 1e-8 m and define an angle strictly
between 0 and pi (1e-9 rad degeneracy tolerance). Parallel/skew lines, coincident
selected ray endpoints, missing/ambiguous references and curved edges fail.
Native OCCT curve classification is exact (`GeomAbs_Line`), queried through the
existing private in-memory BREP bridge with unique endpoint matching, never
through polyline fitting or new sampling. The current backend semantic discovery exposes the top-Z perimeter; other
edge bindings are refused rather than inferred. Explicit point rays can measure
other orthographic planes. Mock supplies deterministic straight
box-edge endpoints. No OCCT type escapes the kernel and no body is modified.

The authored `angular_dimensions` sheet collection is additive, separate from
legacy linear dimensions. Semantic measurements persist only two references and
ray choices. Explicit point rays are also supported, with the same unit and
geometry validation; resolved semantic points are transient drawing build data.
The angle is derived in radians; SVG labels show degrees. Arc radius is an
explicit length in sheet metres, independent of view scale. Only orthographic
Front/Top/Right projections of the measurement plane are accepted; isometric or
foreshortened angle displays are refused. SVG arcs use the minor angle, with a
fixed analytic midpoint label and no editable label override.

All authoring/removal uses DesignPatch and existing drawing transactions. New
angular IDs share the dimension ID namespace with linear dimensions. Removing
views with angular users fails; removing a dimension or sheet removes owned
annotations. Existing sheets default to an empty angular collection, migration
never infers annotations, and old serialized/golden data remains unchanged.

Evidence includes analytic right/oblique angles with Mock and OCCT, exact curved
edge refusal, canonical ray orientation, view placement/scale, model edits,
missing/ambiguous reference refusal, schema/file/patch round trips and verified
Agent atomicity. Radius/diameter annotations are a separate MCAD-P12-002 task.
