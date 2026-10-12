//! Explicit-unit STL import through verified DesignPatch and history.
use std::{fs::File, io::Read};

use base64::Engine as _;
use opencad_ai::{DesignPatch, FeaturePosition, PatchOperation};
use opencad_core::{sha256_hex, DocumentKind, LengthUnit, OpenCadError, Result};
use opencad_feature::{FeatureDefinition, FeatureNode, ImportedMeshFeature};
use opencad_file::{
    apply_patch_with_history, read_ocad, write_ocad, DocumentHistory, DocumentHistoryState,
};
use opencad_geometry::{read_stl_solid, RigidTransform};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportStlRequest {
    pub path: String,
    pub stl_path: String,
    pub feature_id: String,
    pub source_unit: LengthUnit,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub transform: Option<RigidTransform>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub history: Option<DocumentHistory>,
}

#[derive(Debug, Serialize)]
pub struct ImportStlSummary {
    pub attachment: String,
    pub source_sha256: String,
    pub source_unit: LengthUnit,
    pub feature_id: String,
    pub triangle_count: usize,
    pub dry_run: bool,
    pub verification: crate::review::VerificationReport,
    pub history: DocumentHistoryState,
}

pub fn import_stl(request: &ImportStlRequest) -> Result<ImportStlSummary> {
    let mut bytes = Vec::new();
    File::open(&request.stl_path)
        .map_err(|e| OpenCadError::Other(e.to_string()))?
        .take(opencad_geometry::triangle_solid::MAX_STL_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| OpenCadError::Other(e.to_string()))?;
    let solid = read_stl_solid(&bytes, request.source_unit)?;
    let mut doc = read_ocad(&request.path)?;
    if doc.metadata.kind != DocumentKind::Part {
        return Err(OpenCadError::validation(
            "STL import requires a part document",
        ));
    }
    let sha256 = sha256_hex(&bytes);
    let attachment = format!("imports/mesh_{sha256}.stl");
    let def = ImportedMeshFeature {
        source: attachment.clone(),
        sha256: sha256.clone(),
        source_unit: request.source_unit,
        transform: request.transform.unwrap_or_else(RigidTransform::identity),
    };
    def.validate()?;
    solid.transformed(def.transform)?;
    let mut operations = Vec::new();
    match doc.attachments.get(&attachment) {
        Some(existing) if existing != &bytes => {
            return Err(OpenCadError::validation("STL attachment digest collision"))
        }
        Some(_) => {}
        None => operations.push(PatchOperation::AddAttachment {
            path: attachment.clone(),
            sha256: sha256.clone(),
            content_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        }),
    }
    operations.push(PatchOperation::AddFeature {
        node: FeatureNode::new(
            &request.feature_id,
            request.name.as_deref().unwrap_or("Imported STL solid"),
            FeatureDefinition::ImportedMesh(def),
        ),
        position: FeaturePosition::end(),
    });
    let mut patch = DesignPatch::new(operations);
    patch.intent = Some("Import an explicit-unit connected closed STL solid".into());
    let verification = crate::review::verification_report(&request.path, &doc, &patch);
    if !verification.passed {
        return Err(OpenCadError::validation(format!(
            "STL import rejected; document unchanged: {}",
            verification.failure_summary()
        )));
    }
    let mut history = request.history.clone().unwrap_or_default();
    if !request.dry_run {
        apply_patch_with_history(&mut doc, &patch, &mut history, "Import STL solid")?;
        write_ocad(&request.path, &doc)?;
    }
    Ok(ImportStlSummary {
        attachment,
        source_sha256: sha256,
        source_unit: request.source_unit,
        feature_id: request.feature_id.clone(),
        triangle_count: solid.triangles().len(),
        dry_run: request.dry_run,
        verification,
        history: DocumentHistoryState::new(history),
    })
}

pub fn cmd_import_stl(args: Vec<String>) -> Result<()> {
    let usage = || {
        OpenCadError::validation("usage: musubicad import-stl <part> <file.stl> --id feature:id --unit m|mm|cm|in [--name name] [--dry-run]")
    };
    if args.len() < 2 {
        return Err(usage());
    }
    let mut id = None;
    let mut unit = None;
    let mut name = None;
    let mut dry_run = false;
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--dry-run" && !dry_run {
            dry_run = true;
            i += 1;
            continue;
        }
        let value = args.get(i + 1).ok_or_else(usage)?;
        match args[i].as_str() {
            "--id" if id.is_none() => id = Some(value.clone()),
            "--unit" if unit.is_none() => {
                unit = Some(match value.as_str() {
                    "m" => LengthUnit::Meter,
                    "mm" => LengthUnit::Millimeter,
                    "cm" => LengthUnit::Centimeter,
                    "in" => LengthUnit::Inch,
                    _ => return Err(usage()),
                })
            }
            "--name" if name.is_none() => name = Some(value.clone()),
            _ => return Err(usage()),
        }
        i += 2;
    }
    let result = import_stl(&ImportStlRequest {
        path: args[0].clone(),
        stl_path: args[1].clone(),
        feature_id: id.ok_or_else(usage)?,
        source_unit: unit.ok_or_else(usage)?,
        name,
        transform: None,
        dry_run,
        history: None,
    })?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
