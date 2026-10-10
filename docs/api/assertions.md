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
- `MotionInterferenceAtMost { max_count, samples_per_joint }` — interfering
  instance pairs at every sampled joint pose must not exceed the limit
  (MCAD-P10-002, [ADR-033](../adr/ADR-033-motion-interference.md)). Each
  movable joint sweeps `samples_per_joint` (2–360) evenly spaced positions,
  both limits included and a full turn for a continuous joint, while the
  other joints stay at the authored pose.

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

### Assembly documents (MCAD-P10-001)

Assembly documents declare assertions in the same `graph/assertions.json` and
evaluate them against their own regeneration. `assembly_assertion_context`
builds the context from an `AssemblyRegenReport`:

```rust
use opencad_ai::{assembly_assertion_context, evaluate_assertions};

let context = assembly_assertion_context(&doc.parameters, &report, interference_count);
let results = evaluate_assertions(&doc.assertions, &context);
```

| Metric | Assembly evidence |
|---|---|
| `mass_kg`, `bounding_box_size_m` | All placed instance bodies (density 2700 kg/m³) |
| `body_count` | Instances that regenerated to a body; a failed instance is not counted |
| `assembly_dof` | `AssemblyRegenReport::dof`: the mate solver's remaining DOF, or 6 per movable instance when there are no mates |
| `interference_count` | Exact instance-pair count with the default explicit tolerances (`1e-9 m` bounds, `1e-12 m³` common volume) |
| `parameter_values` | The assembly document's own parameters |
| `reference_provenance` | Empty: semantic references are part-level, so `required_reference` fails closed on an assembly |
| `motion_interference` | `motion_interference_evidence(kernel, model, scene, assertions)`: one joint sweep per distinct `samples_per_joint`, computed only when a motion assertion asks for it |

A motion assertion's message names the worst pose, for example
`max interference count 5 <= 0 over 78 poses (6 joints × 13 samples); worst
instance:forearm at 61.7 deg: instance:base × instance:gripper, …`. A sweep
that cannot run (no grounded instance, an instance that did not regenerate)
fails the assertion with the reason.

An assembly with no instances yet reports zero bodies, zero DOF, and zero
interference, with no mass or bounds.

## Surfaces

- `musubicad regen` evaluates the document's assertions after regeneration,
  prints `assertion <id>: PASS/FAIL (<evidence>)` per rule, and exits with an
  error when a `required` assertion fails. The error names each failed
  required assertion and its evidence, for example
  `assertion:clearance (assembly interference count 1 <= 0)`. Assemblies
  compute interference only when they declare assertions.
- `verify_patch` (Agent API and MCP `patch_dry_run` / `patch_apply`,
  `musubicad patch`, and `musubicad review`) evaluates the patched document's
  assertions, for parts and assemblies alike. A failed `required` assertion
  fails verification with the same message, and apply leaves the file
  unchanged.
- The design review artifact embeds the same assertion results and rejects the
  reviewed change when a `required` assertion fails.
- The Agent API `regen` result carries the assertion results alongside
  regeneration trace and reference provenance.

## Related

- [Semantic TopoRef API](topo-ref.md) — the provenance consumed by
  `RequiredReference` assertions.
- [Change impact and regeneration trace](change-impact-and-regeneration-trace.md)
  — the trace that assertion evaluation shares with the review.