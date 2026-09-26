//! `opencad import-step`: place a STEP file in a part as an imported solid
//! (ADR-016).  The command only builds a `DesignPatch`; applying it uses the
//! same validated history boundary as every other edit.

use std::path::Path;

use base64::Engine as _;
use opencad_ai::{DesignPatch, FeaturePosition, PatchOperation};
use opencad_core::{sha256_hex, OpenCadError, Result};
use opencad_feature::{FeatureDefinition, FeatureNode, ImportedSolidFeature};
use opencad_file::{apply_patch_to_document, read_ocad, write_ocad};
use opencad_geometry::{ExtrudeOperation, RigidTransform};
use serde::{Deserialize, Serialize};

/// Options for importing one STEP file.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ImportStepRequest {
    /// Part document to change.
    pub path: String,
    /// STEP file to import (millimetres).
    pub step_path: String,
    /// Stable ID of the new feature, e.g. `feature:motor`.
    pub feature_id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// `new_body` (default), `join`, or `cut`.
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub target_feature: Option<String>,
    /// Placement offset in millimetres.
    #[serde(default)]
    pub translation_mm: Option<[f64; 3]>,
}

/// Result of an import: the applied patch minus the attachment payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportStepSummary {
    pub attachment: String,
    pub sha256: String,
    pub bytes: usize,
    pub feature_id: String,
}

/// Attachment path for a STEP file name: `imports/<sanitized stem>.step`.
fn attachment_path(step_path: &str) -> String {
    let stem = Path::new(step_path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut sanitized: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.trim_matches('_').is_empty() {
        sanitized = "imported".into();
    }
    format!("imports/{sanitized}.step")
}

/// Build the patch that imports `request.step_path` into `document`.
pub fn import_step_patch(request: &ImportStepRequest) -> Result<(DesignPatch, ImportStepSummary)> {
    let bytes = std::fs::read(&request.step_path).map_err(|error| {
        OpenCadError::Other(format!("cannot read '{}': {error}", request.step_path))
    })?;
    let doc = read_ocad(&request.path)?;
    let sha256 = sha256_hex(&bytes);
    let mut path = attachment_path(&request.step_path);
    let mut operations = Vec::new();
    match doc.attachments.get(&path) {
        Some(existing) if sha256_hex(existing) == sha256 => {}
        Some(_) => {
            // Same file name, different content: keep both, suffixed by digest.
            path = path.replace(".step", &format!("_{}.step", &sha256[..8]));
            if doc.attachments.contains_key(&path) {
                return Err(OpenCadError::validation(format!(
                    "attachment '{path}' already exists with different content"
                )));
            }
        }
        None => {}
    }
    if !doc.attachments.contains_key(&path) {
        operations.push(PatchOperation::AddAttachment {
            path: path.clone(),
            sha256: sha256.clone(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        });
    }
    let operation = match request.operation.as_deref().unwrap_or("new_body") {
        "new_body" => ExtrudeOperation::NewBody,
        "join" => ExtrudeOperation::Join,
        "cut" => ExtrudeOperation::Cut,
        other => {
            return Err(OpenCadError::validation(format!(
                "operation must be new_body, join, or cut, got '{other}'"
            )))
        }
    };
    let mut transform = RigidTransform::identity();
    if let Some(mm) = request.translation_mm {
        transform.translation_m = mm.map(|value| value / 1000.0);
    }
    operations.push(PatchOperation::AddFeature {
        node: FeatureNode::new(
            request.feature_id.clone(),
            request.name.clone().unwrap_or_else(|| {
                request
                    .feature_id
                    .trim_start_matches("feature:")
                    .to_string()
            }),
            FeatureDefinition::ImportedSolid(ImportedSolidFeature {
                source: path.clone(),
                sha256: sha256.clone(),
                transform,
                operation,
                target_feature: request.target_feature.clone(),
            }),
        ),
        position: FeaturePosition::end(),
    });
    let mut patch = DesignPatch::new(operations);
    patch.intent = Some(format!(
        "Import {} as {}",
        request.step_path, request.feature_id
    ));
    Ok((
        patch,
        ImportStepSummary {
            attachment: path,
            sha256,
            bytes: bytes.len(),
            feature_id: request.feature_id.clone(),
        },
    ))
}

/// Build, validate, apply, and write the import.
pub fn import_step(request: &ImportStepRequest) -> Result<ImportStepSummary> {
    let (patch, summary) = import_step_patch(request)?;
    let mut doc = read_ocad(&request.path)?;
    apply_patch_to_document(&mut doc, &patch)?;
    write_ocad(&request.path, &doc)?;
    Ok(summary)
}

/// `opencad import-step <doc> <file.step> --id <feature id> [options]`.
pub fn cmd_import_step(args: Vec<String>) -> Result<()> {
    let usage = "usage: opencad import-step <document> <file.step> --id <feature:id> \
                 [--name <name>] [--operation new_body|join|cut] [--target <feature:id>] \
                 [--translate-mm <x,y,z>]";
    let mut positional = Vec::new();
    let mut request = ImportStepRequest {
        path: String::new(),
        step_path: String::new(),
        feature_id: String::new(),
        name: None,
        operation: None,
        target_feature: None,
        translation_mm: None,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| OpenCadError::validation(usage));
        match arg.as_str() {
            "--id" => request.feature_id = value()?,
            "--name" => request.name = Some(value()?),
            "--operation" => request.operation = Some(value()?),
            "--target" => request.target_feature = Some(value()?),
            "--translate-mm" => {
                let text = value()?;
                let parts: Vec<f64> = text
                    .split(',')
                    .map(|part| part.trim().parse::<f64>())
                    .collect::<std::result::Result<_, _>>()
                    .map_err(|_| OpenCadError::validation(usage))?;
                let [x, y, z] = parts.as_slice() else {
                    return Err(OpenCadError::validation(usage));
                };
                request.translation_mm = Some([*x, *y, *z]);
            }
            _ => positional.push(arg),
        }
    }
    let [path, step_path] = positional.as_slice() else {
        return Err(OpenCadError::validation(usage));
    };
    if request.feature_id.is_empty() {
        return Err(OpenCadError::validation(usage));
    }
    request.path = path.clone();
    request.step_path = step_path.clone();
    let summary = import_step(&request)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_paths_are_sanitized_step_names() {
        assert_eq!(
            attachment_path("C:/parts/NEMA 17 Motor.STEP"),
            "imports/nema_17_motor.step"
        );
        assert_eq!(
            attachment_path("bearing-608.stp"),
            "imports/bearing_608.step"
        );
        assert_eq!(attachment_path("___.step"), "imports/imported.step");
    }
}
