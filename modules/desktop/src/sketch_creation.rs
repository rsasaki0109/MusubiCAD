//! Atomic, document-bound global-plane sketch creation (ADR-102).
use crate::editor::{
    feature_tree, verify_editor_document, verify_editor_document_native, EditorEvidence,
    FeatureEditResult,
};
use crate::sketch_editor::{sketch_editor_state, SketchEditorState};
use opencad_ai::{DesignPatch, FeaturePosition, PatchOperation};
use opencad_core::{OpenCadError, Result};
use opencad_feature::{FeatureDefinition, FeatureNode, SketchFeatureDef};
use opencad_file::{
    apply_patch_to_document, apply_patch_with_history, document_design_state, read_ocad,
    write_ocad, DocumentHistory, DocumentHistoryState, OcadDocument,
};
use opencad_geometry::GeometryKernel;
use opencad_sketch::{GlobalPlane, Workplane};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchCreationState {
    pub document_id: String,
    pub revision: String,
    pub suggested_sketch_id: String,
    pub suggested_feature_id: String,
    pub planes: Vec<GlobalPlane>,
}

/// Transport intent only. Source serialization reuses existing sketch semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SketchCreate {
    pub document_id: String,
    pub revision: String,
    pub sketch_id: String,
    pub feature_id: String,
    pub name: String,
    pub plane: GlobalPlane,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchCreatePlan {
    pub create: SketchCreate,
    pub patch: DesignPatch,
    pub state: SketchEditorState,
    pub evidence: EditorEvidence,
}

pub fn sketch_creation_state(doc: &OcadDocument) -> Result<SketchCreationState> {
    let tree = feature_tree(doc)?;
    let sketch_ids: std::collections::BTreeSet<_> =
        doc.sketches.iter().map(|s| s.id.as_str()).collect();
    let feature_ids: std::collections::BTreeSet<_> =
        doc.feature_nodes.iter().map(|n| n.id.as_str()).collect();
    // At most the number of existing identities can collide with these pairs.
    // Bound suggestions independently of client-controlled numeric suffixes.
    let limit = doc.sketches.len().saturating_add(doc.feature_nodes.len());
    let index = (1..=limit.saturating_add(1))
        .find(|i| {
            !sketch_ids.contains(format!("sketch:new_{i}").as_str())
                && !feature_ids.contains(format!("feature:sketch_new_{i}").as_str())
        })
        .ok_or_else(|| OpenCadError::validation("no available sketch identity"))?;
    Ok(SketchCreationState {
        document_id: doc.metadata.id.as_str().into(),
        revision: tree.revision,
        suggested_sketch_id: format!("sketch:new_{index}"),
        suggested_feature_id: format!("feature:sketch_new_{index}"),
        planes: vec![GlobalPlane::XY, GlobalPlane::XZ, GlobalPlane::YZ],
    })
}

pub fn inspect_document_sketch_creation(path: &str) -> Result<SketchCreationState> {
    sketch_creation_state(&read_ocad(path)?)
}

pub fn plan_sketch_create_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    create: &SketchCreate,
    kernel: &K,
) -> Result<SketchCreatePlan> {
    let state = sketch_creation_state(doc)?;
    if state.document_id != create.document_id || state.revision != create.revision {
        return Err(OpenCadError::validation(
            "stale sketch creation document identity or revision",
        ));
    }
    if create.name.trim().is_empty() {
        return Err(OpenCadError::validation("sketch name must not be empty"));
    }
    let patch = DesignPatch::new(vec![
        PatchOperation::AddSketch {
            id: create.sketch_id.clone(),
            name: create.name.clone(),
            workplane: Workplane::Global {
                plane: create.plane,
            },
        },
        PatchOperation::AddFeature {
            node: FeatureNode::new(
                &create.feature_id,
                &create.name,
                FeatureDefinition::Sketch(SketchFeatureDef {
                    sketch_id: create.sketch_id.clone(),
                }),
            ),
            position: FeaturePosition::end(),
        },
    ])
    .with_revision_precondition(&document_design_state(doc))?;
    opencad_ai::ensure_patch_valid(&opencad_file::dry_run_patch_document(doc, &patch))?;
    let mut candidate = doc.clone();
    apply_patch_to_document(&mut candidate, &patch)?;
    let state = sketch_editor_state(&candidate)?;
    let evidence = verify_editor_document(&candidate, kernel)?;
    Ok(SketchCreatePlan {
        create: create.clone(),
        patch,
        state,
        evidence,
    })
}

pub fn apply_sketch_create_with_kernel<K: GeometryKernel>(
    doc: &mut OcadDocument,
    create: &SketchCreate,
    history: &mut DocumentHistory,
    kernel: &K,
) -> Result<FeatureEditResult> {
    let plan = plan_sketch_create_with_kernel(doc, create, kernel)?;
    let mut candidate = doc.clone();
    let mut next = history.clone();
    apply_patch_with_history(&mut candidate, &plan.patch, &mut next, "Create sketch")?;
    let result = FeatureEditResult {
        tree: feature_tree(&candidate)?,
        history: DocumentHistoryState::new(next.clone()),
        evidence: plan.evidence,
    };
    *doc = candidate;
    *history = next;
    Ok(result)
}

pub fn plan_document_sketch_create(path: &str, create: &SketchCreate) -> Result<SketchCreatePlan> {
    let doc = read_ocad(path)?;
    let mut plan = plan_sketch_create_with_kernel(
        &doc,
        create,
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    let mut candidate = doc;
    apply_patch_to_document(&mut candidate, &plan.patch)?;
    plan.evidence = verify_editor_document_native(&candidate)?;
    Ok(plan)
}

pub fn apply_document_sketch_create(
    path: &str,
    create: &SketchCreate,
    history: Option<DocumentHistory>,
) -> Result<FeatureEditResult> {
    let mut doc = read_ocad(path)?;
    let result = apply_sketch_create_with_kernel(
        &mut doc,
        create,
        &mut history.unwrap_or_default(),
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    verify_editor_document_native(&doc)?;
    let current = read_ocad(path)?;
    if current.metadata.id.as_str() != create.document_id
        || feature_tree(&current)?.revision != create.revision
    {
        return Err(OpenCadError::validation(
            "document changed during sketch creation verification",
        ));
    }
    write_ocad(path, &doc)?;
    Ok(result)
}
