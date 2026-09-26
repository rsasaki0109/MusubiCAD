use std::env;

use opencad_core::Result;
use opencad_file::{read_ocad, validate_ocad};

use crate::agent;
use crate::animate;
use crate::diff;
use crate::export;
use crate::git_workflow;
use crate::mesh;
use crate::new;
use crate::patch;
use crate::pick;
use crate::plugin;
use crate::policy_check;
use crate::regen;
use crate::review;
use crate::view;

pub fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None | Some("help") | Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some("version") | Some("--version") | Some("-V") => {
            print_version();
            Ok(())
        }
        Some("new") => cmd_new(args.next().as_deref(), &args.collect::<Vec<_>>()),
        Some("validate") => cmd_validate(args.next().as_deref()),
        Some("inspect") => cmd_inspect(args.next().as_deref()),
        Some("intent") => cmd_intent(args.next().as_deref(), &args.collect::<Vec<_>>()),
        Some("params") => cmd_params(args.next().as_deref(), &args.collect::<Vec<_>>()),
        Some("regen") => cmd_regen(args.next().as_deref(), &args.collect::<Vec<_>>()),
        Some("export") => cmd_export(args.next().as_deref(), args.next().as_deref()),
        Some("mesh") => cmd_mesh(args.next().as_deref(), args.collect()),
        Some("pick") => cmd_pick(args.next().as_deref(), args.collect()),
        Some("view") => cmd_view(args.next().as_deref()),
        Some("screenshot") => cmd_screenshot(args.next().as_deref(), args.next().as_deref()),
        Some("animate") => {
            let input = args.next();
            let output = args.next();
            cmd_animate(input.as_deref(), output.as_deref(), args.collect())
        }
        Some("animate-features") => {
            let input = args.next();
            let output = args.next();
            cmd_animate_features(input.as_deref(), output.as_deref(), args.collect())
        }
        Some("patch") => cmd_patch(args.collect()),
        Some("plugin") => cmd_plugin(args.collect()),
        Some("diff") => cmd_diff(args.collect()),
        Some("review") => cmd_review(args.collect()),
        Some("merge") => git_workflow::merge(args.collect()),
        Some("rebase-patch") => git_workflow::rebase(args.collect()),
        Some("import-step") => crate::import::cmd_import_step(args.collect()),
        Some("merge-driver") => git_workflow::merge_driver(args.collect()),
        Some("conflicts") => git_workflow::conflicts(args.collect()),
        Some("check") => policy_check::check(args.collect()),
        Some("agent") => cmd_agent(args.collect()),
        Some("mcp") => cmd_mcp(args.collect()),
        Some(cmd) => Err(opencad_core::OpenCadError::Other(format!(
            "unknown command '{cmd}'; run 'opencad help' for usage"
        ))),
    }
}

fn cmd_new(path: Option<&str>, extra_args: &[String]) -> Result<()> {
    let path = path.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad new <path> [bracket|bearing-carrier|robot-joint|boss-join|face-pin|edge-fillet|hole-row|hole-ring|pin-row|pin-ring|pin-mirror|revolve-bushing|revolve-sector|assembly|robot-arm|drawing]",
        )
    })?;
    let template = extra_args
        .first()
        .map(|arg| new::DocumentTemplate::parse(arg))
        .transpose()?
        .unwrap_or_default();
    new::create_document(path, template)?;
    println!("created: {path} ({})", template.as_str());
    Ok(())
}

fn cmd_validate(path: Option<&str>) -> Result<()> {
    let path = path
        .ok_or_else(|| opencad_core::OpenCadError::validation("usage: opencad validate <path>"))?;
    validate_ocad(path)?;
    println!("valid: {path}");
    Ok(())
}

fn cmd_inspect(path: Option<&str>) -> Result<()> {
    let path = path
        .ok_or_else(|| opencad_core::OpenCadError::validation("usage: opencad inspect <path>"))?;
    let doc = read_ocad(path)?;
    println!("document: {}", doc.metadata.id.as_str());
    println!("name: {}", doc.metadata.name);
    println!("units: {:?}", doc.metadata.units);
    println!("sketches: {}", doc.sketches.len());
    println!("features: {}", doc.feature_nodes.len());
    println!("parameters: {}", doc.parameters.evaluation_order()?.len());
    if let Some(assembly) = &doc.assembly {
        println!("kind: assembly");
        println!("components: {}", assembly.components.len());
        println!("instances: {}", assembly.instances.len());
        println!("mates: {}", assembly.mates.len());
        println!("connectors: {}", assembly.connectors.len());
        println!("patterns: {}", assembly.patterns.len());
    } else if let Some(drawing) = &doc.drawing {
        println!("kind: drawing");
        println!("sheets: {}", drawing.sheets.len());
        let views = drawing
            .sheets
            .iter()
            .map(|sheet| sheet.views.len())
            .sum::<usize>();
        println!("views: {views}");
    } else {
        println!("kind: part");
    }
    Ok(())
}

/// `opencad intent <path> <param:...|ref:...> [--json]` (MCAD-P6-006): what
/// drives a parameter or reference and what an edit to it would change.
fn cmd_intent(path: Option<&str>, extra_args: &[String]) -> Result<()> {
    let usage = || {
        opencad_core::OpenCadError::validation(
            "usage: opencad intent <path> <param:...|ref:...> [--json]",
        )
    };
    let path = path.ok_or_else(usage)?;
    let mut target = None;
    let mut json = false;
    for arg in extra_args {
        match arg.as_str() {
            "--json" => json = true,
            _ if target.is_none() && !arg.starts_with("--") => target = Some(arg.clone()),
            _ => {
                return Err(opencad_core::OpenCadError::validation(format!(
                    "unknown intent option '{arg}'"
                )))
            }
        }
    }
    let target = target.ok_or_else(usage)?;
    let query = if target.starts_with("ref:") {
        opencad_ai::DesignQuery::InspectReference { ref_id: target }
    } else {
        opencad_ai::DesignQuery::InspectParameter { id: target }
    };
    let result = opencad_ai::run_query(&read_ocad(path)?.into_query_params(query))?;
    if json {
        let text = serde_json::to_string_pretty(&result)
            .map_err(|err| opencad_core::OpenCadError::validation(err.to_string()))?;
        println!("{text}");
        return Ok(());
    }
    let list = |label: &str, items: &[String]| {
        println!(
            "{label}: {}",
            if items.is_empty() {
                "-".to_string()
            } else {
                items.join(", ")
            }
        );
    };
    match result {
        opencad_ai::QueryResult::ParameterIntent { item } => {
            let value = item
                .parameter
                .value_m
                .map(|value| format!(" = {value} m"))
                .unwrap_or_default();
            println!(
                "parameter: {} ({}) {}{value}",
                item.parameter.id, item.parameter.name, item.parameter.expr
            );
            list("driven by", &item.driven_by);
            list("drives parameters", &item.drives_parameters);
            list("sketches", &item.sketches);
            list(
                "directly affected features",
                &item.directly_affected_features,
            );
            list("predicted dirty features", &item.predicted_dirty_features);
            list("assertions", &item.assertions);
        }
        opencad_ai::QueryResult::ReferenceIntent { item } => {
            let role = item.reference.role.as_deref().unwrap_or("-");
            println!(
                "reference: {} ({}, created by {}, role {role})",
                item.reference.ref_id, item.reference.kind, item.reference.created_by
            );
            list("consuming features", &item.consuming_features);
            list("consuming mates", &item.consuming_mates);
            list("predicted dirty features", &item.predicted_dirty_features);
            list("assertions", &item.assertions);
        }
        _ => {}
    }
    Ok(())
}

fn cmd_params(path: Option<&str>, extra_args: &[String]) -> Result<()> {
    let path = path.ok_or_else(|| {
        opencad_core::OpenCadError::validation("usage: opencad params <path> [--json]")
    })?;
    for arg in extra_args {
        if arg != "--json" {
            return Err(opencad_core::OpenCadError::validation(format!(
                "unknown params option '{arg}'"
            )));
        }
    }
    let rows = opencad_desktop::list_document_parameters(path)?;
    if extra_args.iter().any(|arg| arg == "--json") {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for row in rows {
            let value = row
                .value_mm
                .map(|mm| format!("{mm:.6} mm"))
                .or_else(|| row.value_deg.map(|deg| format!("{deg:.6} deg")))
                .unwrap_or_else(|| "—".into());
            println!("{}: {} = {}", row.id, row.expr, value);
        }
    }
    Ok(())
}

fn cmd_regen(path: Option<&str>, extra_args: &[String]) -> Result<()> {
    let path = path.ok_or_else(|| {
        opencad_core::OpenCadError::validation("usage: opencad regen <path> [--sync-topo-refs]")
    })?;
    let sync_topo_refs = extra_args.iter().any(|arg| arg == "--sync-topo-refs");
    let summary = regen::regen_document(path, sync_topo_refs)?;
    regen::print_summary(&summary);
    if sync_topo_refs {
        println!("synced_topo_refs: true");
    }
    Ok(())
}

fn cmd_export(input: Option<&str>, output: Option<&str>) -> Result<()> {
    let input = input.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad export <input> <output.stl|output.step|output.svg>",
        )
    })?;
    let output = output.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad export <input> <output.stl|output.step|output.svg>",
        )
    })?;
    let summary = export::export_document(input, output)?;
    export::print_summary(&summary);
    Ok(())
}

fn cmd_mesh(input: Option<&str>, extra_args: Vec<String>) -> Result<()> {
    let input = input.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad mesh <input> [--json] [--render] [--png <output.png>]",
        )
    })?;
    let options = parse_mesh_options(&extra_args)?;
    let json = extra_args.iter().any(|arg| arg == "--json");
    let summary = mesh::mesh_document(input, &options)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        mesh::print_summary(&summary);
    }
    Ok(())
}

fn parse_mesh_options(args: &[String]) -> Result<mesh::MeshOptions> {
    let mut options = mesh::MeshOptions::default();
    let mut index = 0_usize;
    while index < args.len() {
        match args[index].as_str() {
            "--render" => options.render = true,
            "--png" => {
                let path = args.get(index + 1).ok_or_else(|| {
                    opencad_core::OpenCadError::validation("--png requires an output path")
                })?;
                options.png_output = Some(path.clone());
                index += 1;
            }
            "--json" => {}
            other => {
                return Err(opencad_core::OpenCadError::validation(format!(
                    "unknown mesh option '{other}'"
                )));
            }
        }
        index += 1;
    }
    Ok(options)
}

fn cmd_pick(input: Option<&str>, extra_args: Vec<String>) -> Result<()> {
    let input = input.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad pick <input> [--x <px>] [--y <px>] [--width <px>] [--height <px>] [--json]",
        )
    })?;
    let options = parse_pick_options(&extra_args)?;
    let json = extra_args.iter().any(|arg| arg == "--json");
    let summary = pick::pick_document(input, &options)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        pick::print_summary(&summary);
    }
    Ok(())
}

fn parse_pick_options(args: &[String]) -> Result<pick::PickOptions> {
    let mut options = pick::PickOptions::default();
    let mut index = 0_usize;
    while index < args.len() {
        match args[index].as_str() {
            "--x" => {
                let value = parse_f64_arg(args, index, "--x")?;
                options.x = value;
                index += 1;
            }
            "--y" => {
                let value = parse_f64_arg(args, index, "--y")?;
                options.y = value;
                index += 1;
            }
            "--width" => {
                let value = parse_u32_arg(args, index, "--width")?;
                options.width = value;
                index += 1;
            }
            "--height" => {
                let value = parse_u32_arg(args, index, "--height")?;
                options.height = value;
                index += 1;
            }
            "--json" => {}
            other => {
                return Err(opencad_core::OpenCadError::validation(format!(
                    "unknown pick option '{other}'"
                )));
            }
        }
        index += 1;
    }
    Ok(options)
}

fn parse_f64_arg(args: &[String], index: usize, flag: &str) -> Result<f64> {
    let value = args.get(index + 1).ok_or_else(|| {
        opencad_core::OpenCadError::validation(format!("{flag} requires a numeric value"))
    })?;
    value
        .parse::<f64>()
        .map_err(|_| opencad_core::OpenCadError::validation(format!("{flag} requires a number")))
}

fn parse_u32_arg(args: &[String], index: usize, flag: &str) -> Result<u32> {
    let value = args.get(index + 1).ok_or_else(|| {
        opencad_core::OpenCadError::validation(format!("{flag} requires a positive integer"))
    })?;
    value.parse::<u32>().map_err(|_| {
        opencad_core::OpenCadError::validation(format!("{flag} requires a positive integer"))
    })
}

fn cmd_view(input: Option<&str>) -> Result<()> {
    let input = input
        .ok_or_else(|| opencad_core::OpenCadError::validation("usage: opencad view <input>"))?;
    view::view_document(input)
}

fn cmd_screenshot(input: Option<&str>, output: Option<&str>) -> Result<()> {
    let input = input.ok_or_else(|| {
        opencad_core::OpenCadError::validation("usage: opencad screenshot <input> <output.png>")
    })?;
    let output = output.ok_or_else(|| {
        opencad_core::OpenCadError::validation("usage: opencad screenshot <input> <output.png>")
    })?;
    view::screenshot_document(input, output)
}

fn cmd_animate(input: Option<&str>, output: Option<&str>, args: Vec<String>) -> Result<()> {
    let input = input.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad animate <input> <output.gif> [--frames N] [--fps N] [--orbit-deg DEG] [--pitch-deg DEG] [--show-sketch]",
        )
    })?;
    let output = output.ok_or_else(|| {
        opencad_core::OpenCadError::validation("animation output path is required")
    })?;
    let options = animate::parse_animation_options(&args)?;
    let summary = animate::animate_document(input, output, options)?;
    println!("animation: {output}");
    println!("frames: {}", summary.frame_count);
    println!("fps: {}", summary.frames_per_second);
    println!("size_px: {}x{}", summary.width_px, summary.height_px);
    Ok(())
}

fn cmd_animate_features(
    input: Option<&str>,
    output: Option<&str>,
    args: Vec<String>,
) -> Result<()> {
    let input = input.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad animate-features <input> <output.gif> [--frames N] [--fps N] [--orbit-deg DEG] [--pitch-deg DEG]",
        )
    })?;
    let output = output.ok_or_else(|| {
        opencad_core::OpenCadError::validation("feature animation output path is required")
    })?;
    let options = animate::parse_animation_options(&args)?;
    let summary = animate::animate_feature_build_document(input, output, options)?;
    println!("feature_animation: {output}");
    println!("frames: {}", summary.frame_count);
    println!("fps: {}", summary.frames_per_second);
    println!("size_px: {}x{}", summary.width_px, summary.height_px);
    Ok(())
}

fn cmd_patch(args: Vec<String>) -> Result<()> {
    let parsed = patch::parse_patch_args(args)?;
    patch::patch_document_with_options(&parsed)
}

fn cmd_plugin(args: Vec<String>) -> Result<()> {
    let parsed = plugin::parse_cli_args(&args)?;
    let host = plugin::PluginHost::with_builtins()?;
    match parsed {
        plugin::PluginCliCommand::List { json } => {
            let manifests = host.list();
            if json {
                println!("{}", serde_json::to_string_pretty(&manifests)?);
            } else {
                for manifest in manifests {
                    let capabilities = manifest
                        .capabilities
                        .iter()
                        .map(|capability| capability.as_str())
                        .collect::<Vec<_>>()
                        .join(",");
                    println!(
                        "{}\t{:?}\t{}.{}\t{}",
                        manifest.id,
                        manifest.kind,
                        manifest.api_version.major,
                        manifest.api_version.minor,
                        capabilities
                    );
                }
            }
            Ok(())
        }
        plugin::PluginCliCommand::Invoke {
            plugin_id,
            doc_path,
            request_path,
            dry_run,
            output,
            json,
        } => {
            let request = plugin::read_request_file(&request_path)?;
            let result = plugin::invoke_plugin_on_path(
                &host,
                &plugin_id,
                &doc_path,
                request,
                dry_run,
                output.as_deref(),
                None,
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("plugin: {}", result.invocation.plugin_id);
                if result.dry_run {
                    println!("dry-run: ok");
                } else if result.applied {
                    println!("patched: {doc_path}");
                }
                if let Some(path) = result.output_path {
                    println!("output: {path}");
                } else if let Some(data) = result.invocation.data {
                    println!("bytes: {}", data.len());
                }
            }
            Ok(())
        }
    }
}

fn cmd_diff(args: Vec<String>) -> Result<()> {
    let parsed = diff::parse_diff_args(args)?;
    let options = parsed.options;
    let diff = diff::diff_documents_at_paths(&parsed)?;
    let has_changes = !diff.is_empty();
    diff::print_diff(&diff, options)?;
    if has_changes {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_review(args: Vec<String>) -> Result<()> {
    let args = review::parse_review_args(&args)?;
    let artifact = review::generate_review(&args)?;
    println!("review: {}/review.html", args.output_dir);
    println!("document: {}", artifact.document_id);
    println!("changes: {}", artifact.diff.changes.len());
    review::ensure_expected_effects_pass(
        &artifact,
        &format!("{}/github-summary.md", args.output_dir),
    )?;
    Ok(())
}

fn cmd_agent(args: Vec<String>) -> Result<()> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_agent_help();
        return Ok(());
    }
    if !args.is_empty() {
        return Err(opencad_core::OpenCadError::validation(
            "usage: opencad agent   (reads JSON-RPC lines from stdin)",
        ));
    }
    agent::serve_stdio()
}

fn cmd_mcp(args: Vec<String>) -> Result<()> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            r"opencad mcp — Model Context Protocol server on stdio (ADR-014)

USAGE:
    opencad mcp

Register it with an MCP host, for example:
    claude mcp add musubicad -- opencad mcp

Tools delegate to the Agent API; every change is a validated DesignPatch."
        );
        return Ok(());
    }
    if !args.is_empty() {
        return Err(opencad_core::OpenCadError::validation(
            "usage: opencad mcp   (serves MCP over stdin/stdout)",
        ));
    }
    crate::mcp::serve_stdio()
}

fn print_agent_help() {
    println!(
        r"opencad agent — JSON-RPC 2.0 server on stdio

USAGE:
    opencad agent < request.jsonl

Each input line is one JSON-RPC request. One JSON response is written per line.

IN-MEMORY METHODS:
    opencad.patch_dry_run
    opencad.patch_apply
    opencad.diff
    opencad.regen

DOCUMENT METHODS:
    opencad.inspect
    opencad.validate
    opencad.patch_dry_run_document
    opencad.patch_apply_document
    opencad.regen_document

PLUGIN METHODS:
    opencad.plugin_list
    opencad.plugin_invoke

See OpenCAD/docs/api/agent.md
"
    );
}

fn print_version() {
    println!("opencad {}", env!("CARGO_PKG_VERSION"));
    #[cfg(feature = "occt")]
    if let Some(version) = opencad_kernel_occt::version() {
        println!("{version}");
    }
}

fn print_help() {
    println!(
        r"opencad — AI-native parametric CAD CLI

USAGE:
    opencad <COMMAND> [ARGS]

COMMANDS:
    help        Show this help
    version     Show version
    new         Create a sample bracket document
    validate    Validate a .ocad or .ocad.d document
    inspect     Show document summary
    intent      Show what drives a parameter or reference and what it changes
    params      List document parameters
    regen       Regenerate features through the geometry kernel
    export      Export the active body to STL or a drawing to SVG
    mesh        Tessellate and summarize viewport scene data
    pick        Query viewport selection at a pixel coordinate
    view        Open an interactive 3D viewport
    screenshot  Render a PNG preview of the active body
    animate     Render a deterministic presentation orbit GIF
    animate-features  Render Feature Graph body milestones as a deterministic GIF
    patch       Apply a DesignPatch JSON to parameters
    plugin      List or invoke linked feature/importer/exporter plugins
    diff        Show semantic diff between documents or a patch preview
    review      Generate a self-contained DesignPatch review directory
    merge       Semantically merge base/ours/theirs Design Graph documents
    rebase-patch Rebase a DesignPatch onto a newer document state
    import-step Import a STEP file into a part as a fixed imported solid
    merge-driver Git merge driver for .ocad.d documents (`merge-driver install` to set up)
    conflicts   Typed semantic conflicts of an unfinished git merge
    check       Evaluate an engineering policy as a CI gate
    agent       JSON-RPC 2.0 server on stdio for programmatic access
    mcp         Model Context Protocol server on stdio for agent hosts

OPTIONS (patch):
    --dry-run   Validate and preview changes without writing
    --geometry  Include regenerated mass/volume in preview
    --json      Emit machine-readable diff output

EXAMPLES:
    opencad new bracket.ocad.d
    opencad new bearing_carrier.ocad.d bearing-carrier
    opencad new robot_joint.ocad.d robot-joint
    opencad new bracket_boss_join.ocad.d boss-join
    opencad new bracket_face_pin.ocad.d face-pin
    opencad new bracket_hole_row.ocad.d hole-row
    opencad new bracket_hole_ring.ocad.d hole-ring
    opencad new bracket_pin_row.ocad.d pin-row
    opencad new bracket_pin_ring.ocad.d pin-ring
    opencad new bracket_pin_mirror.ocad.d pin-mirror
    opencad new revolve_bushing.ocad.d revolve-bushing
    opencad new assembly_two_brackets.ocad.d assembly
    opencad new robot_arm_assembly.ocad.d robot-arm
    opencad new bracket_front_view.ocad.d drawing
    opencad validate bracket.ocad
    opencad inspect bracket.ocad.d
    opencad params bracket.ocad.d --json
    opencad regen bracket.ocad
    opencad regen bracket.ocad --sync-topo-refs
    opencad export bracket.ocad bracket.stl
    opencad export bracket_front_view.ocad.d bracket_front.svg
    opencad mesh bracket.ocad.d
    opencad mesh bracket.ocad.d --json --render
    opencad mesh bracket.ocad.d --png preview.png
    opencad pick bracket.ocad.d --x 256 --y 256 --json
    opencad view bracket.ocad.d
    opencad screenshot bracket.ocad.d preview.png
    opencad animate bracket.ocad.d showcase.gif --frames 48 --fps 12 --orbit-deg 220
    opencad animate-features robot_joint.ocad.d build.gif --frames 54 --fps 9
    opencad patch bracket.ocad.d width.patch.json
    opencad patch bracket.ocad.d combined.patch.json --dry-run --geometry
    opencad diff bracket.ocad.d --patch width.patch.json --geometry
    opencad diff before.ocad.d after.ocad.d --json
    opencad review bracket.ocad.d width.patch.json --output review
    opencad merge base.ocad.d ours.ocad.d theirs.ocad.d merged.ocad.d
    opencad rebase-patch old.ocad.d new.ocad.d change.json rebased.json
    opencad check bracket.ocad.d engineering-policy.json
"
    );
}
