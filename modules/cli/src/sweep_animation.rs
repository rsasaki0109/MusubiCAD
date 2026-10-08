//! `musubicad animate-sweep`: a GPU-free GIF of a design regenerating across
//! a parameter range.
//!
//! Every frame is a real regeneration.  Each value goes through the same
//! `set_parameter` DesignPatch validation an agent's edit does, is applied to
//! an in-memory copy, and is rebuilt by the geometry kernel; the document on
//! disk is never written.  A value that does not validate or regenerate stops
//! the sweep with an error naming it, so a clip never shows a shape the model
//! cannot produce.  Frames share one framing and carry a caption with the
//! value and the overall size.

use opencad_ai::{ensure_patch_valid, DesignPatch};
use opencad_core::{OpenCadError, Result};
use opencad_file::{apply_patch_to_document, dry_run_patch_document, read_ocad};
use opencad_geometry::MeshSet;
use opencad_render::{PreviewStyle, PreviewView};
use serde::Serialize;

use crate::preview::{aspect_height, write_preview_gif, GifFrame};

/// Most distinct values in one sweep; each is a full regeneration.
const MAX_STEPS: u32 = 120;
/// Units a sweep value may carry, longest suffix first.
const UNITS: [&str; 4] = ["deg", "rad", "mm", "m"];

const USAGE: &str = "usage: musubicad animate-sweep <document> <output.gif> \
--param NAME --from VALUE --to VALUE [--steps N] [--fps N] \
[--view iso|front|top|right] [--width N] [--height N | --aspect W:H] \
[--style studio|dark|plain] [--no-caption]
  VALUE carries a unit (mm, m, deg, rad), the same for --from and --to.
  NAME is a parameter name (upper_hub_height) or ID (param:upper_hub_height).";

/// Parsed `animate-sweep` options.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepOptions {
    pub parameter: String,
    pub from: SweepValue,
    pub to: SweepValue,
    /// Distinct values from `from` to `to`, both included.
    pub steps: u32,
    pub frames_per_second: u32,
    pub view: PreviewView,
    pub width_px: u32,
    pub height_px: u32,
    pub caption: bool,
    pub style: PreviewStyle,
}

/// A number with an explicit unit, as written.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepValue {
    pub value: f64,
    pub unit: String,
}

impl SweepValue {
    /// Parse `32mm`, `32 mm`, `0.5rad`, or `45deg`.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim();
        let unit = UNITS
            .iter()
            .find(|unit| text.ends_with(*unit))
            .ok_or_else(|| {
                OpenCadError::validation(format!(
                    "sweep value '{text}' needs a unit: mm, m, deg, or rad"
                ))
            })?;
        let value: f64 = text[..text.len() - unit.len()]
            .trim()
            .parse()
            .map_err(|_| {
                OpenCadError::validation(format!("sweep value '{text}' is not a number"))
            })?;
        if !value.is_finite() {
            return Err(OpenCadError::validation(format!(
                "sweep value '{text}' must be finite"
            )));
        }
        Ok(Self {
            value,
            unit: (*unit).to_string(),
        })
    }
}

/// Size of one regenerated frame.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SweepSample {
    /// Parameter expression applied, for example `42 mm`.
    pub expr: String,
    /// Axis-aligned size of everything drawn, millimetres.
    pub size_mm: [f64; 3],
    pub triangles: usize,
}

/// What `animate-sweep` wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SweepSummary {
    pub output: String,
    pub parameter: String,
    /// Expression the document holds; the file is not changed.
    pub original_expr: String,
    /// Regenerations, one per distinct value.
    pub samples: Vec<SweepSample>,
    /// Frames in the loop: the values forward, then back.
    pub frames: usize,
    pub frames_per_second: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub view: String,
}

/// Usage text for `musubicad animate-sweep`.
pub fn usage() -> &'static str {
    USAGE
}

/// Parse the flags after `<document> <output.gif>`.
pub fn parse_sweep_args(args: &[String]) -> Result<SweepOptions> {
    let mut parameter = None;
    let mut from = None;
    let mut to = None;
    let mut steps = 16;
    let mut frames_per_second = 12;
    let mut view = PreviewView::Iso;
    let (mut width_px, mut height_px) = (720, 540);
    let mut caption = true;
    let mut style = PreviewStyle::STUDIO;
    let mut aspect: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if flag == "--no-caption" {
            caption = false;
            index += 1;
            continue;
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| OpenCadError::validation(format!("{flag} needs a value\n{USAGE}")))?;
        let number = || -> Result<u32> {
            value
                .parse()
                .map_err(|_| OpenCadError::validation(format!("{flag} needs an integer")))
        };
        match flag {
            "--param" => parameter = Some(value.clone()),
            "--from" => from = Some(SweepValue::parse(value)?),
            "--to" => to = Some(SweepValue::parse(value)?),
            "--steps" => steps = number()?,
            "--fps" => frames_per_second = number()?,
            "--width" => width_px = number()?,
            "--height" => height_px = number()?,
            "--view" => view = PreviewView::parse(value)?,
            "--style" => style = PreviewStyle::parse(value)?,
            "--aspect" => aspect = Some(value.clone()),
            _ => {
                return Err(OpenCadError::validation(format!(
                    "unknown option '{flag}'\n{USAGE}"
                )))
            }
        }
        index += 2;
    }
    let missing = |what: &str| OpenCadError::validation(format!("{what} is required\n{USAGE}"));
    let (parameter, from, to) = (
        parameter.ok_or_else(|| missing("--param"))?,
        from.ok_or_else(|| missing("--from"))?,
        to.ok_or_else(|| missing("--to"))?,
    );
    if from.unit != to.unit {
        return Err(OpenCadError::validation(format!(
            "--from and --to must use the same unit ({} vs {})",
            from.unit, to.unit
        )));
    }
    if !(2..=MAX_STEPS).contains(&steps) {
        return Err(OpenCadError::validation(format!(
            "--steps must be between 2 and {MAX_STEPS}"
        )));
    }
    if frames_per_second == 0 {
        return Err(OpenCadError::validation("--fps must be positive"));
    }
    if let Some(aspect) = aspect {
        if args.iter().any(|arg| arg == "--height") {
            return Err(OpenCadError::validation(
                "use --height or --aspect, not both",
            ));
        }
        height_px = aspect_height(width_px, &aspect)?;
    }
    Ok(SweepOptions {
        parameter,
        from,
        to,
        steps,
        frames_per_second,
        view,
        width_px,
        height_px,
        caption,
        style,
    })
}

/// Swept values, eased so the ends linger: `steps` values from `from` to `to`.
/// Values between the ends are rounded to a hundredth of the span's order of
/// magnitude (0.1 mm for a 32 mm sweep) so captions read cleanly; the ends
/// are exact.
fn sweep_values(from: f64, to: f64, steps: u32) -> Vec<f64> {
    let span = (to - from).abs();
    let quantum = 10f64.powf((span / 100.0).log10().floor());
    (0..steps)
        .map(|step| {
            if step == 0 || span == 0.0 {
                return from;
            }
            if step == steps - 1 {
                return to;
            }
            let t = f64::from(step) / f64::from(steps - 1);
            let value = from + (to - from) * t * t * (3.0 - 2.0 * t);
            (value / quantum).round() * quantum
        })
        .collect()
}

/// `value` with at most three decimals and no trailing zeros.
fn format_number(value: f64) -> String {
    let text = format!("{:.3}", (value * 1000.0).round() / 1000.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

/// Indices of the loop: forward through every value, then back without
/// repeating either end.
fn ping_pong(count: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count).collect();
    order.extend((1..count.saturating_sub(1)).rev());
    order
}

fn size_mm(meshes: &[MeshSet]) -> [f64; 3] {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for position in meshes.iter().flat_map(|mesh| &mesh.positions) {
        for axis in 0..3 {
            min[axis] = min[axis].min(f64::from(position[axis]));
            max[axis] = max[axis].max(f64::from(position[axis]));
        }
    }
    std::array::from_fn(|axis| {
        let size = (max[axis] - min[axis]).max(0.0) * 1000.0;
        (size * 10.0).round() / 10.0
    })
}

/// Regenerate `input` at every swept value and write the GIF.
pub fn animate_sweep(input: &str, output: &str, options: &SweepOptions) -> Result<SweepSummary> {
    if !output.to_ascii_lowercase().ends_with(".gif") {
        return Err(OpenCadError::validation("sweep output must use .gif"));
    }
    let doc = read_ocad(input)?;
    if doc.drawing.is_some() {
        return Err(OpenCadError::validation(
            "animate-sweep renders parts and assemblies, not drawings",
        ));
    }
    let entry = doc
        .parameters
        .get(&options.parameter)
        .or_else(|| doc.parameters.find_by_name(&options.parameter))
        .ok_or_else(|| {
            OpenCadError::validation(format!(
                "no parameter named '{}'; parameters: {}",
                options.parameter,
                doc.parameters
                    .entries()
                    .map(|entry| entry.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?
        .clone();

    let unit = &options.from.unit;
    let mut samples = Vec::with_capacity(options.steps as usize);
    let mut regenerated = Vec::with_capacity(options.steps as usize);
    for value in sweep_values(options.from.value, options.to.value, options.steps) {
        let expr = format!("{} {unit}", format_number(value));
        let patch = DesignPatch::set_parameter(entry.id.clone(), expr.clone());
        let at = |error: OpenCadError| error.with_context(format!("{} = {expr}", entry.name));
        ensure_patch_valid(&dry_run_patch_document(&doc, &patch)).map_err(at)?;
        let mut after = doc.clone();
        apply_patch_to_document(&mut after, &patch).map_err(at)?;
        let meshes: Vec<MeshSet> = crate::export::document_meshes(input, after)
            .map_err(at)?
            .into_iter()
            .map(|(_, mesh)| mesh)
            .collect();
        let size = size_mm(&meshes);
        samples.push(SweepSample {
            expr,
            size_mm: size,
            triangles: meshes.iter().map(MeshSet::triangle_count).sum(),
        });
        regenerated.push(meshes);
    }

    let frames: Vec<GifFrame> = ping_pong(regenerated.len())
        .into_iter()
        .map(|index| GifFrame {
            meshes: regenerated[index].clone(),
            accents: Vec::new(),
            caption: if options.caption {
                let [x, y, z] = samples[index].size_mm;
                vec![
                    format!("{} = {}", entry.name, samples[index].expr),
                    format!(
                        "size {} × {} × {} mm",
                        format_number(x),
                        format_number(y),
                        format_number(z)
                    ),
                ]
            } else {
                Vec::new()
            },
        })
        .collect();
    let frame_count = write_preview_gif(
        &frames,
        options.view,
        &options.style,
        options.width_px,
        options.height_px,
        options.frames_per_second,
        output,
    )?;
    Ok(SweepSummary {
        output: output.to_string(),
        parameter: entry.id,
        original_expr: entry.expr,
        samples,
        frames: frame_count,
        frames_per_second: options.frames_per_second,
        width_px: options.width_px,
        height_px: options.height_px,
        view: options.view.name().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    fn example(name: &str) -> String {
        format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn sweep_values_need_matching_units() {
        assert_eq!(
            SweepValue::parse("32mm").expect("mm"),
            SweepValue {
                value: 32.0,
                unit: "mm".into()
            }
        );
        assert_eq!(SweepValue::parse("0.5 rad").expect("rad").unit, "rad");
        assert_eq!(SweepValue::parse("1.2m").expect("m").unit, "m");
        assert!(SweepValue::parse("32").is_err());
        assert!(SweepValue::parse("NaNmm").is_err());
        let args = |to: &str| {
            strings(&[
                "--param",
                "bore_diameter",
                "--from",
                "8mm",
                "--to",
                to,
                "--steps",
                "4",
            ])
        };
        let options = parse_sweep_args(&args("20mm")).expect("options");
        assert_eq!((options.steps, options.view), (4, PreviewView::Iso));
        assert_eq!(options.style, PreviewStyle::STUDIO);
        let vertical = parse_sweep_args(&[args("20mm"), strings(&["--aspect", "9:16"])].concat())
            .expect("9:16");
        assert_eq!((vertical.width_px, vertical.height_px), (720, 1280));
        assert!(parse_sweep_args(&args("2m")).is_err());
        assert!(parse_sweep_args(&strings(&["--param", "x", "--from", "1mm"])).is_err());
        assert!(parse_sweep_args(&[args("20mm"), strings(&["--steps", "1"])].concat()).is_err());
    }

    #[test]
    fn values_ease_between_the_ends_and_loop_without_repeats() {
        let values = sweep_values(10.0, 20.0, 5);
        assert_eq!(values.first(), Some(&10.0));
        assert_eq!(values.last(), Some(&20.0));
        assert!((values[2] - 15.0).abs() < 1e-12);
        // A 32 mm span rounds interior values to 0.1 mm.
        let hub = sweep_values(32.0, 64.0, 14);
        assert!(hub
            .iter()
            .all(|value| ((value * 10.0).round() - value * 10.0).abs() < 1e-9));
        assert_eq!(sweep_values(6.25, 6.25, 3), [6.25, 6.25, 6.25]);
        assert!(values.windows(2).all(|pair| pair[1] > pair[0]));
        assert_eq!(ping_pong(4), [0, 1, 2, 3, 2, 1]);
        assert_eq!(ping_pong(2), [0, 1]);
        assert_eq!(format_number(42.0), "42");
        assert_eq!(format_number(6.25), "6.25");
        assert_eq!(format_number(-0.0001), "0");
    }

    #[test]
    fn sweeps_a_real_part_without_touching_the_file() {
        let input = example("bearing_carrier.ocad.d");
        let before =
            std::fs::read_to_string(format!("{input}/graph/parameters.json")).expect("parameters");
        let output =
            std::env::temp_dir().join(format!("musubicad-sweep-{}.gif", std::process::id()));
        let options = parse_sweep_args(&strings(&[
            "--param",
            "boss_height",
            "--from",
            "14mm",
            "--to",
            "30mm",
            "--steps",
            "3",
            "--width",
            "160",
            "--height",
            "120",
        ]))
        .expect("options");
        let summary =
            animate_sweep(&input, output.to_str().expect("utf-8"), &options).expect("sweep");
        assert_eq!(summary.parameter, "param:boss_height");
        assert_eq!(summary.original_expr, "14 mm");
        let exprs: Vec<&str> = summary.samples.iter().map(|s| s.expr.as_str()).collect();
        assert_eq!(exprs, ["14 mm", "22 mm", "30 mm"]);
        assert_eq!(summary.frames, 4);
        // The 96 × 72 mm plate is unchanged; the boss grows by 16 mm in Z.
        let height = |index: usize| summary.samples[index].size_mm[2];
        assert!(
            (height(2) - height(0) - 16.0).abs() < 0.2,
            "{:?}",
            summary.samples
        );
        assert_eq!(summary.samples[0].size_mm[..2], [96.0, 72.0]);
        let gif = std::fs::read(&output).expect("gif");
        assert_eq!(&gif[..3], b"GIF");
        let _ = std::fs::remove_file(&output);
        assert_eq!(
            std::fs::read_to_string(format!("{input}/graph/parameters.json")).expect("after"),
            before
        );
    }

    #[test]
    fn a_value_that_destroys_the_part_stops_the_sweep_and_names_itself() {
        let options = parse_sweep_args(&strings(&[
            "--param",
            "bore_diameter",
            "--from",
            "18mm",
            "--to",
            "200mm",
            "--steps",
            "2",
            "--width",
            "64",
            "--height",
            "64",
        ]))
        .expect("options");
        let output =
            std::env::temp_dir().join(format!("musubicad-sweep-fail-{}.gif", std::process::id()));
        let error = animate_sweep(
            &example("bearing_carrier.ocad.d"),
            output.to_str().expect("utf-8"),
            &options,
        )
        .expect_err("200 mm bore cannot regenerate");
        assert!(
            error.to_string().contains("bore_diameter = 200 mm"),
            "{error}"
        );
        assert!(!output.exists());
        let unknown = SweepOptions {
            parameter: "bore".into(),
            ..options
        };
        let error = animate_sweep(
            &example("bearing_carrier.ocad.d"),
            output.to_str().expect("utf-8"),
            &unknown,
        )
        .expect_err("unknown parameter");
        assert!(error.to_string().contains("bore_diameter"), "{error}");
    }
}
