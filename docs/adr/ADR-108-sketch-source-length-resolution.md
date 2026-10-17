# ADR-108: Resolve ordinary point and circle source lengths before solving

Status: Accepted for MCAD-P17-021; completion requires qualifying main merge
Date: 2026-10-17
Roadmap: MCAD-P17-021, prerequisite to MCAD-P17-010

## Problem

The low-level sketch literal parser splits a leading number from its suffix and
silently treats unknown suffixes as metres. An authored circle radius
`5 mm + 5 mm` therefore seeds 5 m rather than 10 mm. Ordinary point coordinates
also bypass the strict expression resolver already used for cubic controls,
ellipses and arc radii. The existing analytic trim planner consumes this solved
geometry, so exposing it through new controls would expose incorrect cuts.

## Decision

Resolve ordinary point x/y and circle radius expressions on the disposable
sketch copy before solving, using the existing strict length expression grammar
and SI parameter scope. Reject angle-named parameters in length fields and use
strict parameter-graph evaluation for regeneration containing source expressions
and for curve-edit/offset planning. This is a correction to already advertised
expression-backed fields; it does not add syntax, geometry or a solver rule.
Preserve legacy simple scientific literals for ordinary points/circles, without
expanding the existing cubic/arc strict-expression admission.

The low-level parser accepts only finite simple SI numbers or existing m/mm/cm/in
literals, and rejects unknown suffixes, arithmetic and unresolved identifiers.
Its caller must resolve arithmetic/parameters first. Radius seeding propagates
parse failures instead of silently fitting from zero. Literal numeric Coord
values retain their existing SI meaning. Existing constraints remain free to
drive the resolved seed; source expressions are not additional fixed constraints.

Keep authored expressions, entity identities, constraints, workplanes and
dependencies in the Design Graph. Inspection/planning solve copies; regeneration
uses the disposable PartModel made from the current document. Do not serialize
solved coordinates back as source or add a new persisted/resolved field. Refresh
from source when parameters change, including when reusing a regeneration cache.
Existing cubic source restoration and ellipse/arc resolved fields stay intact.

Typed desktop/Agent/MCP edits keep existing read-only review and verified atomic
DesignPatch apply/history/publication gates. Invalid source lengths, stale edits
and assertions refuse without changing source, history or published files. Raw
patches must validate source expressions before accepting a candidate, without
adding an unrelated whole-sketch constraint-admission rule.

## Acceptance

- Ordinary points and circle radii resolve arithmetic and length parameters,
  preserve exact authored source, and continue to obey existing constraints.
  Reject angle units/names, unknown units/parameters, division by zero and
  non-finite input before solving or planning intersections.
- Independent 10 mm radius / 6 mm cylinder at centre (2,-3) mm: both genuine
  analytic Mock and native OCCT, V=600*pi mm3, A=320*pi mm2, absolute bounds/COM
  and oriented closed meshes on XY/YZ/XZ and a tilted datum. Changed parameters
  regenerated from source with a shared cache must change geometry correctly.
- Explicit line/circle and line/arc intersection plans use resolved current SI
  geometry, preserve original source, and refuse ambiguous/invalid input.
- Actual MCP/Agent parity, packed/expanded source round trips and canonical
  history/refusal, exact source plus independently graded fresh native SI STL.
  Equal-volume translated output and numerically equivalent source rewrites
  must fail the grader. Example, API/architecture, MCP guide/skill and eval.
- All four mandatory local checks and complete applicable exact-head CI. Existing
  native controls retain their normal CI compatibility gate. No new UI control,
  new sampling/geometry admission, schema/migration/golden change or maturity
  increment is claimed by this prerequisite.

Analytic tolerances: volume 1e-12 m3, area 1e-10 m2, positions 3e-7 m;
meshes: 1% volume, 1e-4 m bounds/COM, 1 nm oriented closure. Intersection
tolerances remain those of the existing analytic planner (1 nm contact).
