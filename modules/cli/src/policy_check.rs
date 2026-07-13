use std::fs;

use opencad_ai::{evaluate_policy, EngineeringMetrics, EngineeringPolicy};
use opencad_core::{OpenCadError, Result};
use opencad_desktop::{load_assembly_scene_from_document, load_view_data};
use opencad_file::read_ocad;

use crate::diff::{build_document_diff, DiffOptions};

pub fn check(args: Vec<String>) -> Result<()> {
    if args.len() != 2 {
        return Err(OpenCadError::validation(
            "usage: opencad check <document> <policy.json>",
        ));
    }
    let doc = read_ocad(&args[0])?;
    let policy_json = fs::read_to_string(&args[1]).map_err(|err| {
        OpenCadError::Other(format!("failed to read policy '{}': {err}", args[1]))
    })?;
    let policy: EngineeringPolicy = serde_json::from_str(&policy_json)
        .map_err(|err| OpenCadError::validation(format!("invalid policy JSON: {err}")))?;
    let diff = build_document_diff(
        &doc,
        &doc,
        DiffOptions {
            json: true,
            geometry: true,
        },
    )?;
    let view = load_view_data(&args[0])?;
    let extent = view.scene.bounds.extent();
    let interference = if doc.assembly.is_some() {
        Some(load_assembly_scene_from_document(&args[0], &doc)?.1)
    } else {
        None
    };
    let report = evaluate_policy(
        &policy,
        &doc.parameters,
        &EngineeringMetrics {
            mass_kg: diff.geometry.and_then(|geometry| geometry.mass_after),
            bounding_box_size_m: Some(extent.map(f64::from)),
            assembly_interference_count: interference,
        },
    );
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.passed {
        Ok(())
    } else {
        Err(OpenCadError::validation("engineering policy failed"))
    }
}
