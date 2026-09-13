# Design Assertions API

Executable design assertions (MCAD-P6-004) are serializable, unit-explicit
engineering intent embedded in the Design Graph. Unlike an external policy file,
assertions live in the `.ocad` document itself and run during regeneration and
patch review.

## Model

`opencad-core` defines the serializable assertion types:

```rust
use opencad_core::{Assertion, AssertionKind, AssertionSeverity};

let assertion = Assertion::new(
    "assertion:mass",
    "Actuator mass",
    AssertionSeverity::Required,
    AssertionKind::MassRange { min_kg: 0.58, max_kg: 0.65 },
);
```

Every assertion carries a stable `id`, a human `name`, a `severity`
(`required` blocks the change, `advisory` only reports), and one typed,
unit-explicit rule:

- `ParameterRange { parameter_name, min_m, max_m }` — evaluated parameter value
  inside a closed meter range.
- `MassRange { min_kg, max_kg }` — regenerated body mass inside a closed range.
- `BoundingBoxWithin { max_m }` — regenerated bounding-box size inside a limit.
- `BodyCount { expected }` — exact expected solid body count.
- `RequiredReference { ref_id }` — a semantic reference must resolve (not
  `ambiguous` or `missing`); consumes the fail-closed provenance from
  [MCAD-P6-003](topo-ref.md#reference-provenance-fail-closed-mcad-p6-003).
- `AssemblyDofAtMost { max_dof }` — solved assembly DOF must not exceed a limit.
- `InterferenceAtMost { max_count }` — assembly interference count limit.

Invalid rules (non-finite or inverted ranges) fail closed and never silently
pass.

## Persistence

`.ocad.d` documents store assertions in `graph/assertions.json`. The field is
omitted entirely when a document declares no assertions, so existing fixtures
remain byte-stable. `opencad-core` keeps the model small and serializable; the
evaluator lives in `opencad-ai`.

## Evaluation

`opencad-ai` evaluates assertions against regeneration evidence:

```rust
use opencad_ai::{evaluate_assertions, required_assertions_pass, AssertionContext};

let context = AssertionContext {
    parameter_values,          // BTreeMap<String, f64> keyed by parameter name
    mass_kg,                   // Option<f64>
    bounding_box_size_m,       // Option<[f64; 3]>
    body_count,                // Option<u32>
    reference_provenance,      // Vec<ReferenceProvenance>
    assembly_dof,              // Option<i32>
    interference_count,        // Option<usize>,
};

let results = evaluate_assertions(&doc.assertions, &context);
if !required_assertions_pass(&results) {
    // reject the change
}
```

`required_assertions_pass` only checks `required` assertions; an `advisory`
failure is reported but never blocks.

## Surfaces

- `opencad regen` evaluates the document's assertions after regeneration,
  prints `assertion <id>: PASS/FAIL (<evidence>)` per rule, and exits with an
  error when a `required` assertion fails.
- The design review artifact embeds the same assertion results and rejects the
  reviewed change when a `required` assertion fails.
- The Agent API `regen` result carries the assertion results alongside
  regeneration trace and reference provenance.

## Related

- [Semantic TopoRef API](topo-ref.md) — the provenance consumed by
  `RequiredReference` assertions.
- [Change impact and regeneration trace](change-impact-and-regeneration-trace.md)
  — the trace that assertion evaluation shares with the review.