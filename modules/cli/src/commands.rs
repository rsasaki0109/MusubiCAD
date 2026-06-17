use std::env;

use opencad_core::Result;
use opencad_file::{read_ocad, validate_ocad};

use crate::agent;
use crate::diff;
use crate::export;
use crate::mesh;
use crate::new;
use crate::patch;
use crate::pick;
use crate::regen;
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
        Some("regen") => cmd_regen(args.next().as_deref(), &args.collect::<Vec<_>>()),
        Some("export") => cmd_export(args.next().as_deref(), args.next().as_deref()),
        Some("mesh") => cmd_mesh(args.next().as_deref(), args.collect()),
        Some("pick") => cmd_pick(args.next().as_deref(), args.collect()),
        Some("view") => cmd_view(args.next().as_deref()),
        Some("screenshot") => cmd_screenshot(args.next().as_deref(), args.next().as_deref()),
        Some("turntable") => cmd_turntable(
            args.next().as_deref(),
            args.next().as_deref(),
            args.collect(),
        ),
        Some("patch") => cmd_patch(args.collect()),
        Some("diff") => cmd_diff(args.collect()),
        Some("agent") => cmd_agent(args.collect()),
        Some(cmd) => Err(opencad_core::OpenCadError::Other(format!(
            "unknown command '{cmd}'; run 'opencad help' for usage"
        ))),
    }
}

fn cmd_new(path: Option<&str>, extra_args: &[String]) -> Result<()> {
    let path = path.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad new <path> [bracket|boss-join|face-pin|edge-fillet|hole-row|hole-ring|pin-row|pin-ring|pin-mirror|revolve-bushing|revolve-sector]",
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
        opencad_core::OpenCadError::validation("usage: opencad export <input> <output.stl>")
    })?;
    let output = output.ok_or_else(|| {
        opencad_core::OpenCadError::validation("usage: opencad export <input> <output.stl>")
    })?;
    let summary = export::export_stl(input, output)?;
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

fn cmd_turntable(
    input: Option<&str>,
    out_dir: Option<&str>,
    extra_args: Vec<String>,
) -> Result<()> {
    let input = input.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad turntable <input> <out_dir> [--frames N] [--width W] [--height H] [--pitch DEG] [--yaw DEG] [--overlay]",
        )
    })?;
    let out_dir = out_dir.ok_or_else(|| {
        opencad_core::OpenCadError::validation(
            "usage: opencad turntable <input> <out_dir> [--frames N] [--width W] [--height H] [--pitch DEG] [--yaw DEG] [--overlay]",
        )
    })?;
    let options = parse_turntable_options(&extra_args)?;
    view::turntable_document(input, out_dir, &options)?;
    Ok(())
}

fn parse_turntable_options(args: &[String]) -> Result<view::TurntableOptions> {
    let mut options = view::TurntableOptions::default();
    let mut index = 0_usize;
    while index < args.len() {
        match args[index].as_str() {
            "--frames" => {
                options.frames = parse_u32_arg(args, index, "--frames")?;
                index += 1;
            }
            "--width" => {
                options.width = parse_u32_arg(args, index, "--width")?;
                index += 1;
            }
            "--height" => {
                options.height = parse_u32_arg(args, index, "--height")?;
                index += 1;
            }
            "--pitch" => {
                options.pitch_deg = parse_f64_arg(args, index, "--pitch")? as f32;
                index += 1;
            }
            "--yaw" => {
                options.yaw_deg = parse_f64_arg(args, index, "--yaw")? as f32;
                index += 1;
            }
            "--overlay" => options.overlay = true,
            other => {
                return Err(opencad_core::OpenCadError::validation(format!(
                    "unknown turntable option '{other}'"
                )));
            }
        }
        index += 1;
    }
    Ok(options)
}

fn cmd_patch(args: Vec<String>) -> Result<()> {
    let parsed = patch::parse_patch_args(args)?;
    patch::patch_document_with_options(&parsed)
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
    regen       Regenerate features through the geometry kernel
    export      Export the active body to STL
    mesh        Tessellate and summarize viewport scene data
    pick        Query viewport selection at a pixel coordinate
    view        Open an interactive 3D viewport
    screenshot  Render a PNG preview of the active body
    turntable   Render a 360° orbit PNG sequence for animations
    patch       Apply a DesignPatch JSON to parameters
    diff        Show semantic diff between documents or a patch preview
    agent       JSON-RPC 2.0 server on stdio for programmatic access

OPTIONS (patch):
    --dry-run   Validate and preview changes without writing
    --geometry  Include regenerated mass/volume in preview
    --json      Emit machine-readable diff output

EXAMPLES:
    opencad new bracket.ocad.d
    opencad new bracket_boss_join.ocad.d boss-join
    opencad new bracket_face_pin.ocad.d face-pin
    opencad new bracket_hole_row.ocad.d hole-row
    opencad new bracket_hole_ring.ocad.d hole-ring
    opencad new bracket_pin_row.ocad.d pin-row
    opencad new bracket_pin_ring.ocad.d pin-ring
    opencad new bracket_pin_mirror.ocad.d pin-mirror
    opencad new revolve_bushing.ocad.d revolve-bushing
    opencad validate bracket.ocad
    opencad inspect bracket.ocad.d
    opencad regen bracket.ocad
    opencad regen bracket.ocad --sync-topo-refs
    opencad export bracket.ocad bracket.stl
    opencad mesh bracket.ocad.d
    opencad mesh bracket.ocad.d --json --render
    opencad mesh bracket.ocad.d --png preview.png
    opencad pick bracket.ocad.d --x 256 --y 256 --json
    opencad view bracket.ocad.d
    opencad screenshot bracket.ocad.d preview.png
    opencad turntable bracket.ocad.d frames/ --frames 48 --width 1600 --height 900
    opencad turntable bracket_pin_row.ocad.d frames/ --frames 1 --pitch 24 --yaw 26
    opencad patch bracket.ocad.d width.patch.json
    opencad patch bracket.ocad.d combined.patch.json --dry-run --geometry
    opencad diff bracket.ocad.d --patch width.patch.json --geometry
    opencad diff before.ocad.d after.ocad.d --json
"
    );
}
