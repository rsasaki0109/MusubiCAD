//! ADR-110: source-bound independent copies with atomic authored ownership.
use crate::editor::{
    feature_tree, verify_editor_document, verify_editor_document_native, EditorEvidence,
    FeatureEditResult,
};
use crate::sketch_editor::{sketch_editor_state_with_kernel, SketchEditorState};
use opencad_ai::{
    plan_offset, DesignPatch, FeaturePosition, OffsetAssociation, OffsetJoin, OffsetRequest,
    PatchOperation,
};
use opencad_core::{OpenCadError, Result};
use opencad_feature::{FeatureDefinition, FeatureNode, SketchFeatureDef};
use opencad_file::{
    apply_patch_to_document, apply_patch_with_history, document_design_state, read_ocad,
    write_ocad, DocumentHistory, DocumentHistoryState, OcadDocument,
};
use opencad_geometry::GeometryKernel;
use opencad_sketch::{curve_edit::EditCurve, SketchEntity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OffsetSide {
    Outward,
    Inward,
}

/// Transport intent only; the backend generates and verifies every output ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SketchOffset {
    pub document_id: String,
    pub revision: String,
    pub source_sketch_id: String,
    pub curve_ids: Vec<String>,
    pub distance: String,
    pub side: OffsetSide,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct SketchOffsetPlan {
    pub offset: SketchOffset,
    pub destination_sketch_id: String,
    pub destination_feature_id: String,
    pub destination_curve_ids: Vec<String>,
    pub destination_point_ids: Vec<String>,
    pub signed_distance_m: f64,
    pub association: OffsetAssociation,
    pub curves: Vec<EditCurve>,
    pub patch: DesignPatch,
    pub state: SketchEditorState,
    pub evidence: EditorEvidence,
}

pub fn plan_sketch_offset_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    offset: &SketchOffset,
    kernel: &K,
) -> Result<SketchOffsetPlan> {
    if offset.document_id != doc.metadata.id.as_str()
        || offset.revision != feature_tree(doc)?.revision
    {
        return Err(OpenCadError::validation("stale or foreign offset source"));
    }
    if offset.name.trim().is_empty() || offset.curve_ids.is_empty() || offset.curve_ids.len() > 256
    {
        return Err(OpenCadError::validation(
            "offset needs a name and 1..256 ordered source curves",
        ));
    }
    let has_units = offset
        .distance
        .split(|c: char| !c.is_alphabetic())
        .any(|u| matches!(u, "m" | "mm" | "cm" | "in"));
    if !has_units || !opencad_graph::parameter_names_in_expr(&offset.distance).is_empty() {
        return Err(OpenCadError::validation(
            "offset distance requires constant explicit length units; copy is independent",
        ));
    }
    let magnitude = opencad_graph::eval_length_expr_strict(&offset.distance, &Default::default())?;
    if !magnitude.is_finite() || magnitude <= 1e-9 || magnitude > 1e6 {
        return Err(OpenCadError::validation("offset magnitude must be positive, above 1 nm and at most 1e6 m; choose side explicitly"));
    }
    let signed_distance_m = match offset.side {
        OffsetSide::Outward => magnitude,
        OffsetSide::Inward => -magnitude,
    };
    let source_state = sketch_editor_state_with_kernel(doc, kernel)?;
    let source = source_state
        .sketches
        .iter()
        .find(|row| row.source.id.as_str() == offset.source_sketch_id)
        .ok_or_else(|| {
            OpenCadError::validation("offset requires an admitted authored sketch owner/workplane")
        })?;
    for id in &offset.curve_ids {
        if !source.offset_targets.contains(id) {
            return Err(OpenCadError::validation(
                "offset requires catalogued ordinary authored curves with explicit endpoints",
            ));
        }
    }
    let circle = offset.curve_ids.len() == 1
        && matches!(
            source.source.find_entity(&offset.curve_ids[0]),
            Some(SketchEntity::Circle(_))
        );
    let points = if circle {
        1
    } else {
        offset.curve_ids.len()
            + offset
                .curve_ids
                .iter()
                .filter(|id| matches!(source.source.find_entity(id), Some(SketchEntity::Arc(_))))
                .count()
    };
    let existing: std::collections::BTreeSet<_> = doc
        .sketches
        .iter()
        .flat_map(|s| &s.entities)
        .flat_map(SketchEntity::defined_ids)
        .map(|id| id.as_str())
        .collect();
    // Each occupied family consumes at least one existing source identity.
    let limit = existing
        .len()
        .saturating_add(doc.sketches.len())
        .saturating_add(doc.feature_nodes.len())
        .saturating_add(1);
    let (
        destination_sketch_id,
        destination_feature_id,
        destination_curve_ids,
        destination_point_ids,
    ) = (1..=limit)
        .find_map(|n| {
            let sketch = format!("sketch:offset_{n}");
            let feature = format!("feature:offset_{n}");
            let curves: Vec<_> = (1..=offset.curve_ids.len())
                .map(|i| format!("ent:offset_{n}_curve_{i}"))
                .collect();
            let points: Vec<_> = (1..=points)
                .map(|i| format!("ent:offset_{n}_point_{i}"))
                .collect();
            (!doc.sketches.iter().any(|s| s.id.as_str() == sketch)
                && !doc.feature_nodes.iter().any(|f| f.id == feature)
                && curves
                    .iter()
                    .chain(&points)
                    .all(|id| !existing.contains(id.as_str())))
            .then_some((sketch, feature, curves, points))
        })
        .ok_or_else(|| OpenCadError::validation("no globally free offset identity family"))?;
    let mut planned = plan_offset(
        &document_design_state(doc),
        &OffsetRequest {
            source_sketch_id: offset.source_sketch_id.clone(),
            curve_ids: offset.curve_ids.clone(),
            distance_m: signed_distance_m,
            association: OffsetAssociation::IndependentCopy,
            join: OffsetJoin::MitreTangent,
            destination_sketch_id: destination_sketch_id.clone(),
            name: offset.name.clone(),
            destination_curve_ids: destination_curve_ids.clone(),
            destination_point_ids: destination_point_ids.clone(),
        },
    )?;
    planned.patch.operations.push(PatchOperation::AddFeature {
        node: FeatureNode::new(
            &destination_feature_id,
            &offset.name,
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: destination_sketch_id.clone(),
            }),
        ),
        position: FeaturePosition::end(),
    });
    opencad_ai::ensure_patch_valid(&opencad_file::dry_run_patch_document(doc, &planned.patch))?;
    let mut candidate = doc.clone();
    apply_patch_to_document(&mut candidate, &planned.patch)?;
    let state = sketch_editor_state_with_kernel(&candidate, kernel)?;
    let evidence = verify_editor_document(&candidate, kernel)?;
    Ok(SketchOffsetPlan {
        offset: offset.clone(),
        destination_sketch_id,
        destination_feature_id,
        destination_curve_ids,
        destination_point_ids,
        signed_distance_m,
        association: planned.association,
        curves: planned.curves,
        patch: planned.patch,
        state,
        evidence,
    })
}

pub fn apply_sketch_offset_with_kernel<K: GeometryKernel>(
    doc: &mut OcadDocument,
    offset: &SketchOffset,
    history: &mut DocumentHistory,
    kernel: &K,
) -> Result<FeatureEditResult> {
    let plan = plan_sketch_offset_with_kernel(doc, offset, kernel)?;
    let mut candidate = doc.clone();
    let mut next = history.clone();
    apply_patch_with_history(
        &mut candidate,
        &plan.patch,
        &mut next,
        "Offset sketch (independent copy)",
    )?;
    let result = FeatureEditResult {
        tree: feature_tree(&candidate)?,
        history: DocumentHistoryState::new(next.clone()),
        evidence: plan.evidence,
    };
    *doc = candidate;
    *history = next;
    Ok(result)
}

pub fn plan_document_sketch_offset(path: &str, offset: &SketchOffset) -> Result<SketchOffsetPlan> {
    let doc = read_ocad(path)?;
    let mut plan = plan_sketch_offset_with_kernel(
        &doc,
        offset,
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    let mut candidate = doc;
    apply_patch_to_document(&mut candidate, &plan.patch)?;
    plan.evidence = verify_editor_document_native(&candidate)?;
    Ok(plan)
}

pub fn apply_document_sketch_offset(
    path: &str,
    offset: &SketchOffset,
    history: Option<DocumentHistory>,
) -> Result<FeatureEditResult> {
    let mut doc = read_ocad(path)?;
    let result = apply_sketch_offset_with_kernel(
        &mut doc,
        offset,
        &mut history.unwrap_or_default(),
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    verify_editor_document_native(&doc)?;
    let current = read_ocad(path)?;
    if current.metadata.id.as_str() != offset.document_id
        || feature_tree(&current)?.revision != offset.revision
    {
        return Err(OpenCadError::validation(
            "document changed during offset verification",
        ));
    }
    write_ocad(path, &doc)?;
    Ok(result)
}
