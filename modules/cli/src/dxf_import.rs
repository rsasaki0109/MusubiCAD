//! Inspect/import selected planar DXF layers through verified DesignPatch/history.
use opencad_ai::{DesignPatch, FeaturePosition, PatchOperation};
use opencad_core::{sha256_hex, DocumentKind, LengthUnit, OpenCadError, Result, SketchId};
use opencad_feature::{FeatureDefinition, FeatureNode, SketchFeatureDef};
use opencad_file::dxf::{import_dxf_sketch, inspect_dxf, DxfInspection, DXF_MAX_BYTES};
use opencad_file::{
    apply_patch_with_history, read_ocad, write_ocad, DocumentHistory, DocumentHistoryState,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportDxfRequest {
    pub path: String,
    pub dxf_path: String,
    pub sketch_id: String,
    pub feature_id: String,
    pub layers: Vec<String>,
    #[serde(default)]
    pub source_unit: Option<LengthUnit>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub history: Option<DocumentHistory>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ImportDxfSummary {
    pub source_sha256: String,
    pub source_unit: LengthUnit,
    pub layers: Vec<String>,
    pub sketch_id: String,
    pub feature_id: String,
    pub primitive_count: usize,
    pub entity_count: usize,
    pub dry_run: bool,
    pub verification: crate::review::VerificationReport,
    pub history: DocumentHistoryState,
}

fn read_dxf(path: &str) -> Result<String> {
    if std::fs::metadata(path)
        .map_err(|e| OpenCadError::Other(e.to_string()))?
        .len()
        > DXF_MAX_BYTES as u64
    {
        return Err(OpenCadError::validation("DXF input exceeds 16 MiB"));
    }
    std::fs::read_to_string(path).map_err(|e| OpenCadError::Other(format!("cannot read DXF: {e}")))
}
pub fn inspect_dxf_file(path: &str) -> Result<DxfInspection> {
    inspect_dxf(&read_dxf(path)?)
}

pub fn import_dxf(request: &ImportDxfRequest) -> Result<ImportDxfSummary> {
    let text = read_dxf(&request.dxf_path)?;
    let mut doc = read_ocad(&request.path)?;
    if doc.metadata.kind != DocumentKind::Part {
        return Err(OpenCadError::validation(
            "DXF sketch import requires a part document",
        ));
    }
    let (patch, parsed) = patch_from_text(request, &text)?;
    let verification = crate::review::verification_report(&request.path, &doc, &patch);
    if !verification.passed {
        return Err(OpenCadError::validation(format!(
            "DXF import rejected; document unchanged: {}",
            verification.failure_summary()
        )));
    }
    let mut history = request.history.clone().unwrap_or_default();
    if !request.dry_run {
        apply_patch_with_history(&mut doc, &patch, &mut history, "Import DXF sketch")?;
        write_ocad(&request.path, &doc)?;
    }
    Ok(ImportDxfSummary {
        source_sha256: sha256_hex(text.as_bytes()),
        source_unit: parsed.source_unit,
        layers: parsed.selected_layers,
        sketch_id: request.sketch_id.clone(),
        feature_id: request.feature_id.clone(),
        primitive_count: parsed.primitive_count,
        entity_count: parsed.sketch.entities.len(),
        dry_run: request.dry_run,
        verification,
        history: DocumentHistoryState::new(history),
    })
}

fn patch_from_text(
    request: &ImportDxfRequest,
    text: &str,
) -> Result<(DesignPatch, opencad_file::dxf::DxfSketch)> {
    let name = request.name.as_deref().unwrap_or("Imported DXF sketch");
    let parsed = import_dxf_sketch(
        text,
        SketchId::new(&request.sketch_id)?,
        name,
        &request.layers,
        request.source_unit,
    )?;
    let mut operations = vec![PatchOperation::AddSketch {
        id: request.sketch_id.clone(),
        name: name.into(),
        workplane: parsed.sketch.workplane.clone(),
    }];
    for entity in &parsed.sketch.entities {
        operations.push(PatchOperation::AddSketchEntity {
            sketch_id: request.sketch_id.clone(),
            entity: entity.clone(),
        });
    }
    operations.push(PatchOperation::AddFeature {
        node: FeatureNode::new(
            &request.feature_id,
            name,
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: request.sketch_id.clone(),
            }),
        ),
        position: FeaturePosition::end(),
    });
    let mut patch = DesignPatch::new(operations);
    patch.intent = Some("Import selected planar DXF layers as authored SI sketch geometry".into());
    Ok((patch, parsed))
}

pub fn cmd_import_dxf(args: Vec<String>) -> Result<()> {
    let usage = || {
        OpenCadError::validation("usage: musubicad import-dxf <part> <file.dxf> --sketch-id sketch:id --id feature:id --layers OUTLINE,HOLES [--unit m|mm|cm|in] [--name name] [--dry-run]")
    };
    if args.len() < 2 {
        return Err(usage());
    }
    let mut request = ImportDxfRequest {
        path: args[0].clone(),
        dxf_path: args[1].clone(),
        sketch_id: String::new(),
        feature_id: String::new(),
        layers: Vec::new(),
        source_unit: None,
        name: None,
        dry_run: false,
        history: None,
    };
    let mut i = 2;
    while i < args.len() {
        if args[i] == "--dry-run" {
            if request.dry_run {
                return Err(usage());
            }
            request.dry_run = true;
            i += 1;
            continue;
        }
        let value = args.get(i + 1).ok_or_else(usage)?;
        match args[i].as_str() {
            "--sketch-id" if request.sketch_id.is_empty() => request.sketch_id = value.clone(),
            "--id" if request.feature_id.is_empty() => request.feature_id = value.clone(),
            "--layers" if request.layers.is_empty() => {
                request.layers = value.split(',').map(String::from).collect()
            }
            "--unit" if request.source_unit.is_none() => {
                request.source_unit = Some(match value.as_str() {
                    "m" => LengthUnit::Meter,
                    "mm" => LengthUnit::Millimeter,
                    "cm" => LengthUnit::Centimeter,
                    "in" => LengthUnit::Inch,
                    _ => return Err(usage()),
                })
            }
            "--name" if request.name.is_none() => request.name = Some(value.clone()),
            _ => return Err(usage()),
        }
        i += 2;
    }
    if request.sketch_id.is_empty()
        || request.feature_id.is_empty()
        || request.layers.is_empty()
        || !Path::new(&request.dxf_path).is_file()
    {
        return Err(usage());
    }
    println!("{}", serde_json::to_string_pretty(&import_dxf(&request)?)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, DocumentMetadata, Length};
    use opencad_feature::{ExtrudeFeature, FeatureRegistry};
    use opencad_file::{apply_patch_to_document, OcadDocument};
    use opencad_geometry::{ExtrudeExtent, ExtrudeOperation, MockGeometryKernel};
    use opencad_sketch::{Coord, SketchEntity};
    #[test]
    fn mock_import_patch_regenerates_analytic_si_profile_and_round_trips() {
        let text = include_str!("../../../examples/import/plate_outline.dxf");
        let request = ImportDxfRequest {
            path: String::new(),
            dxf_path: String::new(),
            sketch_id: "sketch:dxf".into(),
            feature_id: "feature:sketch_dxf".into(),
            layers: vec!["OUTLINE".into()],
            source_unit: None,
            name: None,
            dry_run: false,
            history: None,
        };
        let (patch, parsed) = patch_from_text(&request, text).expect("patch");
        let patch: DesignPatch =
            serde_json::from_str(&serde_json::to_string(&patch).expect("JSON"))
                .expect("round trip");
        let mut doc = OcadDocument::new(DocumentMetadata::new(
            DocumentId::new("doc:dxf").expect("id"),
            "DXF",
        ));
        apply_patch_to_document(&mut doc, &patch).expect("apply");
        let extrude = FeatureNode::new(
            "feature:extrude",
            "Extrude",
            FeatureDefinition::Extrude(ExtrudeFeature {
                sketch_feature: "feature:sketch_dxf".into(),
                profile_ref: "sketch:dxf/profile:outer".into(),
                extent: ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                operation: ExtrudeOperation::NewBody,
                length_expr: None,
                target_feature: None,
            }),
        );
        apply_patch_to_document(
            &mut doc,
            &DesignPatch::new(vec![PatchOperation::AddFeature {
                node: extrude,
                position: FeaturePosition::end(),
            }]),
        )
        .expect("extrude patch");
        let mut model = doc.clone().into_part_model();
        model
            .regenerate(
                &MockGeometryKernel::new(),
                &FeatureRegistry::with_defaults(),
                Some(&doc.parameters),
                Some(&doc.semantic_refs),
            )
            .expect("Mock regeneration");
        assert!(model.active_body().is_some());
        let points: Vec<_> = parsed
            .sketch
            .entities
            .iter()
            .filter_map(|e| match e {
                SketchEntity::Point(p) => Some(p),
                _ => None,
            })
            .collect();
        let literal = |c: &Coord| match c {
            Coord::Literal(v) => *v,
            _ => panic!("literal"),
        };
        let area = points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .map(|(a, b)| literal(&a.x) * literal(&b.y) - literal(&a.y) * literal(&b.x))
            .sum::<f64>()
            .abs()
            / 2.;
        assert!((area - 0.08 * 0.06).abs() < 1e-12);
        assert!((area * 0.006 - 0.08 * 0.06 * 0.006).abs() < 1e-12);
    }
}
