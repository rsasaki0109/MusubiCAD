//! `musubicad mcp`: Model Context Protocol server on stdio (ADR-014).
//!
//! A thin adapter: every tool delegates to an existing Agent API method or to
//! a file-layer function the CLI already uses, so MCP adds no mutation path.

use std::io::{self, BufRead, Write};
use std::path::Path;

use opencad_ai::{authoring_patch, JsonRpcRequest};
use opencad_core::{DocumentId, DocumentMetadata, OpenCadError, Result};
use opencad_file::{document_design_state, read_ocad, write_ocad, OcadDocument};
use serde_json::{json, Map, Value};

use crate::agent::handle_agent_request_with_plugins;
use crate::plugin::PluginHost;
use crate::review::{generate_review, ReviewArgs};

/// Protocol versions this server can speak, newest first.
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 4] =
    ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const PATCH_SCHEMA_URI: &str = "musubicad://schema/design-patch";
const GUIDE_URI: &str = "musubicad://guide/structural-patch";
const PATCH_SCHEMA: &str = include_str!("../../../schemas/ocad.patch.schema.json");
const GUIDE: &str = include_str!("../../../docs/api/mcp-authoring-guide.md");

const INSTRUCTIONS: &str =
    "MusubiCAD is a parametric CAD whose source of truth is a Design Graph. \
Change designs only with typed DesignPatch operations: call patch_dry_run first, fix every \
reported problem until verification.passed is true, declare expected_effects (such as \
mass_delta_kg) so the change is checked against its intent, optionally call review_patch \
for a visual before/after, then patch_apply, which refuses unverified changes. \
Read the resource musubicad://guide/structural-patch before authoring new objects and \
musubicad://schema/design-patch for the exact JSON shape. Lengths are meters, angles radians.";

/// How a tool is executed.
enum Handler {
    /// Forward the arguments unchanged as params of an Agent API method.
    Agent(&'static str),
    Review,
    Authoring,
    NewDocument,
    ImportStep,
    Preview,
}

struct Tool {
    name: &'static str,
    description: &'static str,
    handler: Handler,
    schema: fn() -> Value,
}

fn path_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "path": { "type": "string", "description": "Path to a .ocad file or .ocad.d directory" } },
        "required": ["path"]
    })
}

fn patch_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Path to a .ocad file or .ocad.d directory" },
            "patch": { "type": "object", "description": format!("DesignPatch; see resource {PATCH_SCHEMA_URI}") },
            "verify": { "type": "boolean", "description": "Regenerate and check expected effects (default true). Set false only for a staged authoring step that cannot regenerate yet." }
        },
        "required": ["path", "patch"]
    })
}

fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "new_document",
            description: "Create an empty part, assembly, or drawing document at a new path. Refuses to overwrite.",
            handler: Handler::NewDocument,
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "New .ocad.d directory or .ocad file path" },
                    "kind": { "enum": ["part", "assembly", "drawing"] },
                    "document_id": { "type": "string", "description": "Stable ID such as doc:flange" },
                    "name": { "type": "string" }
                },
                "required": ["path", "kind", "document_id", "name"]
            }),
        },
        Tool {
            name: "inspect_document",
            description: "Summarize a document: kind, parameters, features, sketches.",
            handler: Handler::Agent("opencad.inspect"),
            schema: path_schema,
        },
        Tool {
            name: "validate_document",
            description: "Validate a document's schema and checksums.",
            handler: Handler::Agent("opencad.validate"),
            schema: path_schema,
        },
        Tool {
            name: "explain_document",
            description: "Explain a document's design intent in plain language.",
            handler: Handler::Agent("opencad.explain_document"),
            schema: path_schema,
        },
        Tool {
            name: "query_document",
            description: "Run a typed design query (for example list_parameters, list_features, semantic references, or inspect_parameter / inspect_reference for what drives an item and what an edit to it would change).",
            handler: Handler::Agent("opencad.query_document"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "query": { "type": "object", "description": "DesignQuery, e.g. {\"kind\": \"list_parameters\"}" }
                },
                "required": ["path", "query"]
            }),
        },
        Tool {
            name: "bom_document",
            description: "Regenerate every assembly source and count leaf part occurrences, including patterns and repeated subassemblies, with structured instance provenance. Optionally export a deterministic JSON/CSV table outside the source documents. Missing or failed children refuse output.",
            handler: Handler::Agent("opencad.bom_document"),
            schema: || json!({"type":"object","properties":{"path":{"type":"string"},"output":{"type":"string"}},"required":["path"],"additionalProperties":false}),
        },
        Tool {
            name: "inspect_dxf",
            description: "Inspect text DXF version, declared units and entity counts by layer before selecting planar geometry for import. Does not change a CAD document.",
            handler: Handler::Agent("opencad.inspect_dxf"),
            schema: || json!({"type":"object","properties":{"dxf_path":{"type":"string"}},"required":["dxf_path"],"additionalProperties":false}),
        },
        Tool {
            name: "import_stl",
            description: "Import a bounded ASCII/binary STL as one connected closed oriented solid through verified DesignPatch/history. source_unit is mandatory because STL has no unit. Refuse open, disconnected, degenerate or self-intersecting geometry. dry_run verifies without writing; returned history supports undo/redo.",
            handler: Handler::Agent("opencad.import_stl"),
            schema: || json!({"type":"object","properties":{"path":{"type":"string"},"stl_path":{"type":"string"},"feature_id":{"type":"string"},"source_unit":{"enum":["m","mm","cm","in"]},"name":{"type":"string"},"transform":{"type":"object"},"dry_run":{"type":"boolean"},"history":{"type":"object"}},"required":["path","stl_path","feature_id","source_unit"],"additionalProperties":false}),
        },
        Tool {
            name: "inspect_articulation_source",
            description: "Inspect exact child assembly document/revision and independent joint IDs before authoring parent-owned articulated copies. Read-only; rejects missing/invalid sources and reports unsupported hierarchical scopes.",
            handler: Handler::Agent("opencad.inspect_articulation_source"),
            schema: path_schema,
        },
        Tool {
            name: "inspect_joint_pose",
            description: "Inspect mechanical assembly poses using exact declared joint IDs and explicit-unit coordinates (90deg, 0.5rad, 20mm, 0.02m). Derive coupled gear/rack followers and refuse conflicts, limits or mismatched units. Returns SI positions and world placements without writing.",
            handler: Handler::Agent("opencad.inspect_joint_pose"),
            schema: || json!({"type":"object","properties":{"path":{"type":"string"},"positions":{"type":"object","additionalProperties":{"type":"string"}}},"required":["path"],"additionalProperties":false}),
        },
        Tool {
            name: "inspect_projected_sketch",
            description: "Resolve and inspect an associated projected_sketch feature from its semantic source bindings. Returns derived SI coordinates and bindings without writing the document. Generated coordinates are not authored source; edit upstream geometry or replace the projection definition through verified DesignPatch/history.",
            handler: Handler::Agent("opencad.inspect_projected_sketch"),
            schema: || json!({"type":"object","properties":{"path":{"type":"string"},"feature_id":{"type":"string"}},"required":["path","feature_id"],"additionalProperties":false}),
        },
        Tool {
            name: "plan_offset",
            description: "Plan an independent analytic offset copy of explicitly ordered convex CCW lines/tangent arcs or one circle. Signed distance_m is positive outward. Caller supplies all fresh destination IDs. Does not copy constraints or create persistent source association. Returns a revision-guarded DesignPatch; apply through verified patch tools. Concavity, collapse and non-tangent arc joins refuse.",
            handler: Handler::Agent("opencad.plan_offset"),
            schema: || json!({"type":"object","required":["path","edit"],"properties":{"path":{"type":"string"},"edit":{"type":"object","required":["source_sketch_id","curve_ids","distance_m","association","join","destination_sketch_id","name","destination_curve_ids","destination_point_ids"],"properties":{"source_sketch_id":{"type":"string"},"curve_ids":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":256,"uniqueItems":true},"distance_m":{"type":"number"},"association":{"enum":["independent_copy"]},"join":{"enum":["mitre_tangent"]},"destination_sketch_id":{"type":"string"},"name":{"type":"string"},"destination_curve_ids":{"type":"array","items":{"type":"string"},"uniqueItems":true},"destination_point_ids":{"type":"array","items":{"type":"string"},"uniqueItems":true}},"additionalProperties":false}},"additionalProperties":false}),
        },
        Tool {
            name: "plan_curve_edit",
            description: "Plan an analytic trim/extension of a stored line or arc against a finite line/circle/arc cutter in the same sketch. Preserve shared old endpoints by supplying a new point ID. Constraint removal and ambiguous intersection selection are explicit. Returns a revision-guarded DesignPatch with a solved-endpoint expected effect; review and apply through existing verified patch tools. Does not write.",
            handler: Handler::Agent("opencad.plan_curve_edit"),
            schema: || json!({"type":"object","required":["path","edit"],"properties":{"path":{"type":"string"},"edit":{"type":"object","required":["sketch_id","target_id","cutter_id","mode","end","new_point_id"],"properties":{"sketch_id":{"type":"string"},"target_id":{"type":"string"},"cutter_id":{"type":"string"},"mode":{"enum":["trim","extend"]},"end":{"enum":["start","end"]},"new_point_id":{"type":"string"},"intersection_index":{"type":"integer","minimum":0},"remove_constraints":{"type":"array","items":{"type":"string"},"uniqueItems":true}},"additionalProperties":false}},"additionalProperties":false}),
        },
        Tool {
            name: "import_dxf",
            description: "Import explicitly selected planar LINE/CIRCLE/ARC/straight LWPOLYLINE layers into an authored sketch through verified DesignPatch/history. Units must be declared or supplied and agree. Unsupported selected geometry refuses changes. dry_run verifies without writing; returned history supports undo/redo.",
            handler: Handler::Agent("opencad.import_dxf"),
            schema: || json!({"type":"object","properties":{"path":{"type":"string"},"dxf_path":{"type":"string"},"sketch_id":{"type":"string"},"feature_id":{"type":"string"},"layers":{"type":"array","items":{"type":"string"},"minItems":1,"uniqueItems":true},"source_unit":{"enum":["m","mm","cm","in"]},"name":{"type":"string"},"dry_run":{"type":"boolean"},"history":{"type":"object"}},"required":["path","dxf_path","sketch_id","feature_id","layers"],"additionalProperties":false}),
        },
        Tool {
            name: "authoring_patch",
            description: "Express an existing document as one structural DesignPatch that rebuilds it from an empty document. Use it as a worked example of patch syntax.",
            handler: Handler::Authoring,
            schema: path_schema,
        },
        Tool {
            name: "patch_dry_run",
            description: "Check a DesignPatch without writing: validation messages, semantic diff, change impact, and `verification` (regenerates before/after; reports mass, bounds, interference, declared expected_effects, and, when the patched part does not regenerate, regeneration_failure: the failing feature, the features it blocks, and the body it was building on). Fix every problem until verification.passed is true.",
            handler: Handler::Agent("opencad.patch_dry_run_document"),
            schema: patch_schema,
        },
        Tool {
            name: "review_patch",
            description: "Regenerate before/after geometry for a DesignPatch and write review.html, review.json, and (when a GPU is available) images to output_dir without changing the document.",
            handler: Handler::Review,
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "patch": { "type": "object" },
                    "output_dir": { "type": "string", "description": "Directory for review artifacts" }
                },
                "required": ["path", "patch", "output_dir"]
            }),
        },
        Tool {
            name: "patch_apply",
            description: "Validate, regenerate, and apply a DesignPatch, then write the document. Refuses (document unchanged) when validation fails, the patched model does not regenerate, or a declared expected_effect is not met. Returns the verification evidence.",
            handler: Handler::Agent("opencad.patch_apply_document"),
            schema: patch_schema,
        },
        Tool {
            name: "list_features",
            description: "Read backend feature rows, admitted length fields, authored closed profiles and current source revision for the part editor. Does not write.",
            handler: Handler::Agent("opencad.list_features"),
            schema: path_schema,
        },
        Tool {
            name: "plan_feature_edit",
            description: "Verify an explicit-unit part feature form on a disposable candidate. create_extrude, set_expression, move, suppress. Returns the concrete revision-guarded DesignPatch and SI analytic evidence without writing.",
            handler: Handler::Agent("opencad.plan_feature_edit"),
            schema: feature_edit_schema,
        },
        Tool {
            name: "apply_feature_edit",
            description: "Reverify and publish a reviewed feature edit through the same backend as Tauri. Stale revision, dependency, units, geometry and required assertion failures retain source/history. Returns tree, opaque history and SI evidence.",
            handler: Handler::Agent("opencad.apply_feature_edit"),
            schema: feature_edit_schema,
        },
        Tool { name: "inspect_sketch_creation", description: "Read part document identity, source revision, deterministic free sketch/owner ID suggestions and admitted global planes without writing.", handler: Handler::Agent("opencad.inspect_sketch_creation"), schema: path_schema },
        Tool { name: "plan_sketch_create", description: "Review atomic empty sketch and owning feature creation on explicit XY/XZ/YZ. Require current document identity/revision and stable IDs; verify candidate without writes.", handler: Handler::Agent("opencad.plan_sketch_create"), schema: sketch_create_schema },
        Tool { name: "apply_sketch_create", description: "Reverify and atomically create the reviewed global-plane sketch/owner through DesignPatch and history. Refuse stale identity/revision or ID collisions without writes.", handler: Handler::Agent("opencad.apply_sketch_create"), schema: sketch_create_schema },
        Tool { name: "inspect_sketches", description: "Read authored global-workplane sketches and backend solved SI entities, profiles and solve diagnostics without writing.", handler: Handler::Agent("opencad.inspect_sketches"), schema: path_schema },
        Tool { name: "plan_sketch_edit", description: "Plan a bounded revision-guarded sketch form batch: point, line, circle, ellipse, construction and basic constraints. Inputs require explicit units. Return solved candidate and native verified DesignPatch without writing.", handler: Handler::Agent("opencad.plan_sketch_edit"), schema: sketch_edit_schema },
        Tool { name: "apply_sketch_edit", description: "Reverify and atomically apply the same sketch commands as Tauri; preserve source/history on stale, unit, solve, profile or geometry failure.", handler: Handler::Agent("opencad.apply_sketch_edit"), schema: sketch_edit_schema },
        Tool {
            name: "inspect_selection_frame",
            description: "Rebuild a native part scene and read source/scene revisions and uniquely admitted semantic selection bindings. Does not write or rasterize. This is the same geometry/source evidence used by the Tauri preview and pick queries.",
            handler: Handler::Agent("opencad.inspect_selection_frame"),
            schema: path_schema,
        },
        Tool {
            name: "plan_selection_edit",
            description: "Plan a dimension edit from a backend selection binding returned by pick_document or inspect_selection_frame. Requires exact source/scene revisions and stable target face_ref or authored sketch/entity IDs. Missing, ambiguous, consumed, unsupported or stale bindings refuse; feature-name hints never bind a mutation. Returns a concrete verified DesignPatch without writing.",
            handler: Handler::Agent("opencad.plan_selection_edit"),
            schema: selection_edit_schema,
        },
        Tool {
            name: "apply_selection_edit",
            description: "Rebind and verify a reviewed selection dimension edit through the same backend as Tauri. Preserves source/history on failure and returns native geometry evidence, current tree and opaque history.",
            handler: Handler::Agent("opencad.apply_selection_edit"),
            schema: selection_edit_schema,
        },
        Tool {
            name: "regen_document",
            description: "Regenerate geometry and report mass, bounds, references, and assertions.",
            handler: Handler::Agent("opencad.regen_document"),
            schema: path_schema,
        },
        Tool {
            name: "inspect_regeneration",
            description: "Regenerate a part in memory without writing it and, when it fails, name the first failing sketch or feature, the error, the features that completed (with the last usable body's volume and mass), the features the failure blocks, and those never reached. Use it to find what to repair.",
            handler: Handler::Agent("opencad.inspect_regeneration_document"),
            schema: path_schema,
        },
        Tool {
            name: "import_step",
            description: "Import a STEP file (millimetres) into a part as a fixed imported solid: new body, or join/cut against target_feature. Writes the document.",
            handler: Handler::ImportStep,
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Part .ocad.d or .ocad" },
                    "step_path": { "type": "string", "description": "STEP file to import" },
                    "feature_id": { "type": "string", "description": "New feature ID, e.g. feature:motor" },
                    "name": { "type": "string" },
                    "operation": { "enum": ["new_body", "join", "cut"] },
                    "target_feature": { "type": "string" },
                    "translation_mm": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }
                },
                "required": ["path", "step_path", "feature_id"]
            }),
        },
        Tool {
            name: "preview_document",
            description: "Render a PNG image of a part or assembly (no GPU needed) and return it as an image you can look at, plus its bounds in mm. Use it to check a change visually: views iso (default), front, top, right.",
            handler: Handler::Preview,
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Part or assembly .ocad.d / .ocad" },
                    "view": { "enum": ["iso", "front", "top", "right"] },
                    "width": { "type": "integer", "minimum": 16, "maximum": 2048 },
                    "height": { "type": "integer", "minimum": 16, "maximum": 2048 },
                    "exploded_view": { "type": "string", "description": "Optional persisted assembly view ID, e.g. view:exploded; renders presentation only, not mechanical motion" },
                    "style": { "enum": ["plain", "studio", "dark"], "description": "plain (default) for checking; studio or dark for an image to share" }
                },
                "required": ["path"]
            }),
        },
        Tool {
            name: "export_document",
            description: "Regenerate and export geometry: .step/.stp (millimetre B-rep for other CAD/CAM tools), .stl (mesh), .3mf (watertight millimetre mesh for slicers; one object per assembly instance), .glb (binary glTF for viewers and the web), .svg (drawing sheet), .pdf (multi-page vector drawing with fixed embedded font), .dxf (single-sheet millimetre drawing primitives), .urdf (an assembly as a robot description with one STL per part next to it; mates become joints), or .html (an assembly as a self-contained 3D web page with one slider per joint, within its limits).",
            handler: Handler::Agent("opencad.export"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Source .ocad file or .ocad.d directory" },
                    "output": { "type": "string", "description": "Output file; the extension selects the format" }
                },
                "required": ["path", "output"]
            }),
        },
        Tool {
            name: "reach",
            description: "Can a tool point on a robot assembly reach a target? Kernel-free and fast: searches the joint limits declared in the design (mates solved first). Returns the workspace (mm), whether the target is reachable, the remaining gap in mm, each joint's angle, and a pose string for animate_joints. Use it before and after a design change that should extend reach; with gif, also writes an animation of the arm moving to that pose with the target marked.",
            handler: Handler::Agent("opencad.reach_document"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Assembly .ocad.d" },
                    "tool": { "type": "string", "description": "Instance carrying the tool, such as gripper" },
                    "point_m": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "Tool point in that part's frame, metres (default its origin)" },
                    "target_m": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "World target, metres" },
                    "tolerance_m": { "type": "number", "exclusiveMinimum": 0, "description": "default 0.0005" },
                    "gif": { "type": "string", "description": "Optional .gif path; needs target_m" },
                    "frames_per_move": { "type": "integer", "minimum": 1 },
                    "view": { "enum": ["iso", "front", "top", "right"] },
                    "style": { "enum": ["studio", "dark", "plain"], "description": "default studio" },
                    "width": { "type": "integer", "minimum": 16, "maximum": 2048 },
                    "height": { "type": "integer", "minimum": 16, "maximum": 2048 },
                    "aspect": { "type": "string", "description": "W:H such as 16:9, 1:1, 9:16; instead of height" },
                    "fps": { "type": "integer", "minimum": 1 },
                    "caption": { "type": "boolean", "description": "label frames with the values (default true)" }
                },
                "required": ["path", "tool"]
            }),
        },
        Tool {
            name: "animate_joints",
            description: "Write a GIF of a robot assembly moving through its joints (no GPU needed). Poses are keyframes such as \"shoulder=60deg,elbow=-45deg\" (deg or rad; mm or m for prismatic joints); without poses every joint swings to 60% of its limits. Poses past a declared limit are refused. Returns the frames written and the range each joint covered.",
            handler: Handler::Agent("opencad.animate_joints_document"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Assembly .ocad.d" },
                    "output": { "type": "string", "description": "Output .gif" },
                    "poses": { "type": "array", "items": { "type": "string" } },
                    "frames_per_move": { "type": "integer", "minimum": 1 },
                    "view": { "enum": ["iso", "front", "top", "right"] },
                    "style": { "enum": ["studio", "dark", "plain"], "description": "default studio" },
                    "width": { "type": "integer", "minimum": 16, "maximum": 2048 },
                    "height": { "type": "integer", "minimum": 16, "maximum": 2048 },
                    "aspect": { "type": "string", "description": "W:H such as 16:9, 1:1, 9:16; instead of height" },
                    "fps": { "type": "integer", "minimum": 1 },
                    "caption": { "type": "boolean", "description": "label frames with the values (default true)" }
                },
                "required": ["path", "output"]
            }),
        },
        Tool {
            name: "animate_sweep",
            description: "Write a GIF of a part or assembly regenerated at each value of one parameter, forward and back (no GPU needed). Every frame is a real regeneration through a validated set_parameter patch on an in-memory copy; the file is not changed. A value that does not regenerate stops the sweep and is named, which makes this a quick robustness check across a range. Returns each value with its size in mm.",
            handler: Handler::Agent("opencad.animate_sweep_document"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "output": { "type": "string", "description": "Output .gif" },
                    "param": { "type": "string", "description": "Parameter name or ID" },
                    "from": { "type": "string", "description": "Start value with a unit, such as 32mm" },
                    "to": { "type": "string", "description": "End value, same unit" },
                    "steps": { "type": "integer", "minimum": 2, "maximum": 120 },
                    "view": { "enum": ["iso", "front", "top", "right"] },
                    "style": { "enum": ["studio", "dark", "plain"], "description": "default studio" },
                    "width": { "type": "integer", "minimum": 16, "maximum": 2048 },
                    "height": { "type": "integer", "minimum": 16, "maximum": 2048 },
                    "aspect": { "type": "string", "description": "W:H such as 16:9, 1:1, 9:16; instead of height" },
                    "fps": { "type": "integer", "minimum": 1 },
                    "caption": { "type": "boolean", "description": "label frames with the values (default true)" }
                },
                "required": ["path", "output", "param", "from", "to"]
            }),
        },
        Tool {
            name: "diff_document",
            description: "Semantic diff between two documents, or between a document and a patch result.",
            handler: Handler::Agent("opencad.diff_document"),
            schema: || json!({
                "type": "object",
                "properties": {
                    "before": { "type": "string" },
                    "after": { "type": "string" },
                    "patch": { "type": "object" },
                    "geometry": { "type": "boolean" }
                },
                "required": ["before"]
            }),
        },
    ]
}

/// Agent API method behind each delegating tool (for parity checks).
fn feature_edit_schema() -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"},"revision":{"type":"string"},"edit":{"type":"object","description":"FeatureEdit: operation create_extrude with id/name/sketch_feature/profile_ref/length_expr; set_expression with id/field/expr; move with id/position; suppress with id/suppressed."},"history":{"type":"object"}},"required":["path","revision","edit"],"additionalProperties":false})
}

fn selection_edit_schema() -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"},"edit":{"type":"object","required":["source_revision","scene_revision","target","field","expr"],"properties":{"source_revision":{"type":"string"},"scene_revision":{"type":"string"},"target":{"type":"object","description":"{kind:face_reference,ref_id} or {kind:sketch_entity,sketch_id,entity_id}, from selection_frame.bindings"},"field":{"type":"string"},"expr":{"type":"string"}},"additionalProperties":false},"history":{"type":"object"}},"required":["path","edit"],"additionalProperties":false})
}

fn sketch_create_schema() -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"},"create":{"type":"object","required":["document_id","revision","sketch_id","feature_id","name","plane"],"properties":{"document_id":{"type":"string"},"revision":{"type":"string"},"sketch_id":{"type":"string"},"feature_id":{"type":"string"},"name":{"type":"string","minLength":1},"plane":{"type":"string","enum":["XY","XZ","YZ"]}},"additionalProperties":false},"history":{"type":["object","null"]}},"required":["path","create"],"additionalProperties":false})
}

fn sketch_edit_schema() -> Value {
    let forms = [
        ("point", vec!["id", "x", "y"], vec!["create"]),
        ("line", vec!["id", "start", "end"], vec!["construction"]),
        (
            "circle",
            vec!["id", "center", "radius"],
            vec!["construction", "create"],
        ),
        (
            "ellipse",
            vec!["id", "center", "major_radius", "minor_radius", "axis_angle"],
            vec!["construction", "create"],
        ),
        ("construction", vec!["id"], vec!["construction"]),
        ("horizontal", vec!["id", "line"], vec![]),
        ("vertical", vec!["id", "line"], vec![]),
        ("coincident", vec!["id", "a", "b"], vec![]),
        ("distance", vec!["id", "a", "b", "expr"], vec![]),
        ("radius", vec!["id", "target", "expr"], vec![]),
        ("remove_constraint", vec!["id"], vec![]),
    ];
    let operations: Vec<Value> = forms.into_iter().map(|(operation, strings, booleans)| {
        let mut properties=serde_json::Map::new();properties.insert("operation".into(),json!({"const":operation}));
        let mut required=vec!["operation"];
        for field in strings { properties.insert(field.into(),json!({"type":"string"}));required.push(field); }
        for field in booleans { properties.insert(field.into(),json!({"type":"boolean"}));required.push(field); }
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
    }).collect();
    json!({"type":"object","properties":{"path":{"type":"string"},"edit":{"type":"object","properties":{"revision":{"type":"string"},"sketch_id":{"type":"string"},"operations":{"type":"array","minItems":1,"maxItems":64,"items":{"oneOf":operations}}},"required":["revision","sketch_id","operations"],"additionalProperties":false},"history":{"type":["object","null"]}},"required":["path","edit"],"additionalProperties":false})
}

#[cfg(test)]
pub fn delegated_agent_methods() -> Vec<(&'static str, &'static str)> {
    tools()
        .into_iter()
        .filter_map(|tool| match tool.handler {
            Handler::Agent(method) => Some((tool.name, method)),
            _ => None,
        })
        .collect()
}

/// Handle one MCP message.  Returns `None` for notifications.
pub fn handle_mcp_message(message: &Value, host: &PluginHost) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let outcome = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": tools().iter().map(|tool| json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": (tool.schema)(),
            })).collect::<Vec<_>>()
        })),
        "tools/call" => call_tool(&params, host),
        "resources/list" => Ok(json!({
            "resources": [
                { "uri": PATCH_SCHEMA_URI, "name": "DesignPatch JSON Schema",
                  "mimeType": "application/schema+json" },
                { "uri": GUIDE_URI, "name": "Structural patch authoring guide",
                  "mimeType": "text/markdown" }
            ]
        })),
        "resources/read" => read_resource(&params),
        other => Err((-32601, format!("method not found: {other}"))),
    };
    Some(match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
        }
    })
}

fn initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|version| **version == requested)
        .copied()
        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false }, "resources": { "listChanged": false } },
        "serverInfo": { "name": "musubicad", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn read_resource(params: &Value) -> std::result::Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (mime, text) = match uri {
        PATCH_SCHEMA_URI => ("application/schema+json", PATCH_SCHEMA),
        GUIDE_URI => ("text/markdown", GUIDE),
        other => return Err((-32602, format!("unknown resource: {other}"))),
    };
    Ok(json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": text }] }))
}

fn call_tool(params: &Value, host: &PluginHost) -> std::result::Result<Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    let tool = tools()
        .into_iter()
        .find(|tool| tool.name == name)
        .ok_or_else(|| (-32602, format!("unknown tool: {name}")))?;
    let result = match tool.handler {
        Handler::Agent(method) => {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: json!(1),
                method: method.into(),
                params: arguments,
            };
            let response = handle_agent_request_with_plugins(&request, host);
            match (response.result, response.error) {
                (Some(result), _) => Ok(result),
                (None, Some(error)) if error.code == -32602 => {
                    return Err((-32602, error.message));
                }
                (None, Some(error)) => Err(error.message),
                (None, None) => Err("empty response".to_string()),
            }
        }
        Handler::Review => review(&arguments).map_err(|error| error.to_string()),
        Handler::Authoring => authoring(&arguments).map_err(|error| error.to_string()),
        Handler::NewDocument => new_document(&arguments).map_err(|error| error.to_string()),
        Handler::Preview => return Ok(preview_result(&arguments)),
        Handler::ImportStep => {
            serde_json::from_value::<crate::import::ImportStepRequest>(arguments)
                .map_err(|error| format!("invalid arguments: {error}"))
                .and_then(|request| {
                    crate::import::import_step(&request)
                        .and_then(|summary| Ok(serde_json::to_value(summary)?))
                        .map_err(|error| error.to_string())
                })
        }
    };
    Ok(tool_result(result))
}

/// Preview as MCP image content plus a text summary, or `isError`.
fn preview_result(arguments: &Value) -> Value {
    let outcome = serde_json::from_value::<crate::preview::PreviewParams>(arguments.clone())
        .map_err(|error| format!("invalid arguments: {error}"))
        .and_then(|params| {
            crate::preview::preview_document(&params).map_err(|error| error.to_string())
        });
    match outcome {
        Ok((png, summary)) => {
            use base64::Engine as _;
            let structured = serde_json::to_value(&summary).unwrap_or(Value::Null);
            json!({
                "content": [
                    {
                        "type": "image",
                        "data": base64::engine::general_purpose::STANDARD.encode(png),
                        "mimeType": "image/png"
                    },
                    { "type": "text", "text": serde_json::to_string_pretty(&structured).unwrap_or_default() }
                ],
                "structuredContent": structured,
                "isError": false
            })
        }
        Err(message) => tool_result(Err(message)),
    }
}

/// Wrap a tool outcome: structured JSON plus a text copy, or `isError`.
fn tool_result(result: std::result::Result<Value, String>) -> Value {
    match result {
        Ok(value) => {
            let text = serde_json::to_string_pretty(&value).unwrap_or_default();
            let structured = if value.is_object() {
                value
            } else {
                json!({ "result": value })
            };
            json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": structured,
                "isError": false
            })
        }
        Err(message) => json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true
        }),
    }
}

fn string_arg<'a>(arguments: &'a Value, key: &str) -> Result<&'a str> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| OpenCadError::validation(format!("missing string argument '{key}'")))
}

fn authoring(arguments: &Value) -> Result<Value> {
    let doc = read_ocad(string_arg(arguments, "path")?)?;
    let patch = authoring_patch(&document_design_state(&doc))?;
    Ok(json!({ "patch": serde_json::to_value(patch)? }))
}

fn new_document(arguments: &Value) -> Result<Value> {
    let path = string_arg(arguments, "path")?;
    if Path::new(path).exists() {
        return Err(OpenCadError::validation(format!(
            "'{path}' already exists; new_document never overwrites"
        )));
    }
    let id = DocumentId::new(string_arg(arguments, "document_id")?)?;
    let name = string_arg(arguments, "name")?;
    let metadata = match string_arg(arguments, "kind")? {
        "part" => DocumentMetadata::new(id, name),
        "assembly" => DocumentMetadata::new_assembly(id, name),
        "drawing" => DocumentMetadata::new_drawing(id, name),
        other => {
            return Err(OpenCadError::validation(format!(
                "unknown document kind '{other}'; expected part, assembly, or drawing"
            )))
        }
    };
    write_ocad(path, &OcadDocument::new(metadata))?;
    Ok(json!({ "created": path }))
}

fn review(arguments: &Value) -> Result<Value> {
    let path = string_arg(arguments, "path")?;
    let output_dir = string_arg(arguments, "output_dir")?;
    let patch = arguments
        .get("patch")
        .ok_or_else(|| OpenCadError::validation("missing argument 'patch'"))?;
    // Validate the patch shape before touching the file system.
    let patch: opencad_ai::DesignPatch = serde_json::from_value(patch.clone())
        .map_err(|error| OpenCadError::validation(format!("invalid patch: {error}")))?;
    std::fs::create_dir_all(output_dir)
        .map_err(|error| OpenCadError::Other(format!("cannot create '{output_dir}': {error}")))?;
    let patch_path = Path::new(output_dir).join("patch.json");
    std::fs::write(&patch_path, serde_json::to_vec_pretty(&patch)?)
        .map_err(|error| OpenCadError::Other(format!("cannot write patch: {error}")))?;
    let artifact = generate_review(&ReviewArgs {
        document_path: path.to_string(),
        patch_path: patch_path.to_string_lossy().to_string(),
        output_dir: output_dir.to_string(),
    })?;
    Ok(json!({
        "review_html": Path::new(output_dir).join("review.html").to_string_lossy(),
        "review": serde_json::to_value(&artifact)?,
    }))
}

/// Serve MCP over stdio until end of input.
pub fn serve_stdio() -> Result<()> {
    let host = PluginHost::with_builtins()?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line =
            line.map_err(|error| OpenCadError::Other(format!("failed to read stdin: {error}")))?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle_mcp_message(&message, &host),
            Err(error) => Some(json!({
                "jsonrpc": "2.0", "id": null,
                "error": { "code": -32700, "message": format!("parse error: {error}") }
            })),
        };
        if let Some(response) = response {
            writeln!(stdout, "{response}")
                .and_then(|()| stdout.flush())
                .map_err(|error| OpenCadError::Other(format!("failed to write stdout: {error}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> PluginHost {
        PluginHost::with_builtins().expect("plugin host")
    }

    fn call(method: &str, params: Value) -> Value {
        handle_mcp_message(
            &json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params }),
            &host(),
        )
        .expect("response")
    }

    fn bracket() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/bracket.ocad.d").to_string()
    }

    #[test]
    fn initialize_negotiates_supported_versions() {
        let known = call("initialize", json!({ "protocolVersion": "2025-06-18" }));
        assert_eq!(known["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(known["result"]["serverInfo"]["name"], "musubicad");
        let unknown = call("initialize", json!({ "protocolVersion": "1999-01-01" }));
        assert_eq!(
            unknown["result"]["protocolVersion"],
            SUPPORTED_PROTOCOL_VERSIONS[0]
        );
    }

    #[test]
    fn notifications_get_no_response_and_unknown_methods_error() {
        let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(handle_mcp_message(&notification, &host()).is_none());
        assert_eq!(call("nope", json!({}))["error"]["code"], -32601);
        assert_eq!(
            call("tools/call", json!({ "name": "nope", "arguments": {} }))["error"]["code"],
            -32602
        );
    }

    #[test]
    fn every_tool_is_listed_with_an_object_schema() {
        let listed = call("tools/list", json!({}));
        let tools = listed["result"]["tools"].as_array().expect("tools");
        assert_eq!(tools.len(), 39);
        assert!(tools.iter().any(|tool| tool["name"] == "bom_document"));
        for tool in tools {
            assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
            assert!(tool["description"]
                .as_str()
                .is_some_and(|text| !text.is_empty()));
        }
    }

    #[test]
    fn preview_returns_a_png_image_and_its_bounds() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/bracket.ocad.d");
        let response = call(
            "tools/call",
            json!({ "name": "preview_document", "arguments": { "path": path, "view": "top", "width": 120, "height": 90 } }),
        );
        let result = &response["result"];
        assert_eq!(result["isError"], false, "{response}");
        assert_eq!(result["content"][0]["type"], "image");
        assert_eq!(result["content"][0]["mimeType"], "image/png");
        use base64::Engine as _;
        let png = base64::engine::general_purpose::STANDARD
            .decode(result["content"][0]["data"].as_str().expect("data"))
            .expect("base64");
        assert_eq!(&png[1..4], b"PNG");
        assert_eq!(result["structuredContent"]["view"], "top");
        assert_eq!(result["content"][1]["type"], "text");

        let bad = call(
            "tools/call",
            json!({ "name": "preview_document", "arguments": { "path": path, "view": "bottom" } }),
        );
        assert_eq!(bad["result"]["isError"], true);
    }

    #[test]
    fn delegated_tools_map_to_real_agent_methods() {
        for (tool, method) in delegated_agent_methods() {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: json!(1),
                method: method.into(),
                params: json!({}),
            };
            let response = handle_agent_request_with_plugins(&request, &host());
            let error = response.error.map(|error| error.code);
            assert_ne!(
                error,
                Some(-32601),
                "{tool} -> {method} is not an Agent method"
            );
        }
    }

    #[test]
    fn dry_run_returns_structured_results_and_readable_failures() {
        let ok = call(
            "tools/call",
            json!({ "name": "patch_dry_run", "arguments": {
                "path": bracket(),
                "patch": { "operations": [
                    { "type": "add_parameter", "id": "param:rib", "name": "rib", "expr": "4 mm" }
                ] }
            } }),
        );
        assert_eq!(ok["result"]["isError"], false);
        assert!(ok["result"]["structuredContent"]["validation"].is_object());

        let failed = call(
            "tools/call",
            json!({ "name": "patch_dry_run", "arguments": {
                "path": bracket(),
                "patch": { "operations": [ { "type": "remove_parameter", "id": "param:thickness" } ] }
            } }),
        );
        let text = failed["result"].to_string();
        assert!(text.contains("cannot remove parameter"), "{text}");
    }

    #[test]
    fn resources_serve_the_schema_and_guide() {
        let listed = call("resources/list", json!({}));
        assert_eq!(listed["result"]["resources"].as_array().unwrap().len(), 2);
        let schema = call("resources/read", json!({ "uri": PATCH_SCHEMA_URI }));
        let text = schema["result"]["contents"][0]["text"].as_str().unwrap();
        let schema = serde_json::from_str::<Value>(text).expect("schema is JSON");
        assert!(
            schema["$defs"]["feature_definition"]["properties"]["type"]["enum"]
                .as_array()
                .expect("advertised feature types")
                .contains(&json!("body_boolean"))
        );
        assert!(schema["$defs"]["operation"]["properties"]["type"]["enum"]
            .as_array()
            .expect("operation types")
            .contains(&json!("add_angular_dimension")));
        let guide = call("resources/read", json!({ "uri": GUIDE_URI }));
        assert!(guide["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("add_feature"));
        assert_eq!(
            call("resources/read", json!({ "uri": "musubicad://nope" }))["error"]["code"],
            -32602
        );
    }

    #[test]
    fn new_document_refuses_to_overwrite() {
        let result = call(
            "tools/call",
            json!({ "name": "new_document", "arguments": {
                "path": bracket(), "kind": "part", "document_id": "doc:x", "name": "X"
            } }),
        );
        assert_eq!(result["result"]["isError"], true);
        assert!(result["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("never overwrites"));
    }
}
