//! Semantic selection admission. Renderer/name hints never authorize edits.
use crate::editor::{
    feature_tree, plan_feature_edit_with_kernel, FeatureEdit, FeatureEditPlan, FeatureField,
};
use opencad_core::{sha256_hex, OpenCadError, Result};
use opencad_feature::FeatureDefinition;
use opencad_file::{document_design_state, read_ocad, OcadDocument};
use opencad_geometry::{GeometryKernel, ReferenceProvenance, ReferenceStatus, TopoRefKind};
use opencad_render::{RenderScene, SketchOverlay};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SelectionTarget {
    FaceReference {
        ref_id: String,
    },
    SketchEntity {
        sketch_id: String,
        entity_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SelectionHit {
    FaceGroup { index: usize },
    SketchLine { index: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionBinding {
    pub target: SelectionTarget,
    pub feature_id: String,
    pub fields: Vec<FeatureField>,
    pub hits: Vec<SelectionHit>,
}

/// Disposable backend scene evidence, not part of the source format. Clients
/// can read bindings but cannot submit this frame to the mutation API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionFrame {
    pub source_revision: String,
    pub scene_revision: String,
    pub bindings: Vec<SelectionBinding>,
    #[serde(skip)]
    pub provenance: Vec<ReferenceProvenance>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionEdit {
    pub source_revision: String,
    pub scene_revision: String,
    pub target: SelectionTarget,
    pub field: String,
    pub expr: String,
}

fn resolved(record: &ReferenceProvenance) -> bool {
    matches!(
        record.status,
        ReferenceStatus::Exact | ReferenceStatus::Derived | ReferenceStatus::Fingerprint
    ) && record.resolved_kernel_id.is_some()
}

/// Resolve source ownership without feature-name/role guesses. A selected
/// authored sketch entity must belong to exactly one active profile consumer.
pub fn resolve_selection_source(
    doc: &OcadDocument,
    target: &SelectionTarget,
    provenance: &[ReferenceProvenance],
) -> Result<SelectionBinding> {
    let tree = feature_tree(doc)?;
    let feature_id = match target {
        SelectionTarget::FaceReference { ref_id } => {
            let refs: Vec<_> = doc
                .semantic_refs
                .iter()
                .filter(|r| r.ref_id.as_str() == ref_id && r.kind == TopoRefKind::Face)
                .collect();
            if refs.len() != 1 {
                return Err(OpenCadError::validation(
                    "missing or ambiguous declared face reference",
                ));
            }
            let records: Vec<_> = provenance
                .iter()
                .filter(|r| &r.ref_id == ref_id && r.kind == TopoRefKind::Face)
                .collect();
            if records.len() != 1 || !resolved(records[0]) {
                return Err(OpenCadError::validation(
                    "missing, ambiguous or consumed current face binding",
                ));
            }
            let record = records[0];
            if record.created_by != refs[0].semantic.created_by
                || provenance
                    .iter()
                    .filter(|r| {
                        r.kind == TopoRefKind::Face
                            && resolved(r)
                            && r.resolved_kernel_id == record.resolved_kernel_id
                    })
                    .count()
                    != 1
            {
                return Err(OpenCadError::validation(
                    "ambiguous semantic aliases or source owner",
                ));
            }
            record.created_by.clone()
        }
        SelectionTarget::SketchEntity {
            sketch_id,
            entity_id,
        } => {
            let sketch = doc
                .sketches
                .iter()
                .find(|s| s.id.as_str() == sketch_id)
                .ok_or_else(|| OpenCadError::validation("missing authored sketch binding"))?;
            if !sketch.entities.iter().any(|e| e.id().as_str() == entity_id) {
                return Err(OpenCadError::validation(
                    "missing authored sketch entity; generated coordinates are not editable source",
                ));
            }
            let owners: Vec<_> = doc.feature_nodes.iter().filter(|n| !n.suppressed && matches!(&n.definition, FeatureDefinition::Sketch(d) if &d.sketch_id == sketch_id)).collect();
            if owners.len() != 1 {
                return Err(OpenCadError::validation(
                    "missing or ambiguous authored sketch owner",
                ));
            }
            let mut prepared = sketch.clone();
            opencad_feature::sketch_bridge::prepare_sketch(&mut prepared)?;
            let consumers: Vec<_> = doc
                .feature_nodes
                .iter()
                .filter(|n| {
                    !n.suppressed
                        && n.definition
                            .profile_inputs()
                            .iter()
                            .any(|(owner, profile)| {
                                *owner == owners[0].id
                                    && opencad_feature::sketch_bridge::resolve_sketch_profile(
                                        &prepared, profile,
                                    )
                                    .is_some_and(|p| {
                                        p.entity_ids.iter().any(|id| id.as_str() == entity_id)
                                    })
                            })
                })
                .collect();
            if consumers.len() != 1 {
                return Err(OpenCadError::validation(
                    "missing or ambiguous active profile consumer",
                ));
            }
            consumers[0].id.clone()
        }
    };
    let row = tree
        .rows
        .iter()
        .find(|r| r.id == feature_id && !r.suppressed && !r.fields.is_empty())
        .ok_or_else(|| {
            OpenCadError::validation("selection owner has no admitted dimension form")
        })?;
    Ok(SelectionBinding {
        target: target.clone(),
        feature_id,
        fields: row.fields.clone(),
        hits: Vec::new(),
    })
}

pub fn build_selection_frame(
    doc: &OcadDocument,
    scene: &RenderScene,
    overlay: &SketchOverlay,
    provenance: &[ReferenceProvenance],
) -> Result<SelectionFrame> {
    let source_revision = opencad_ai::design_state_revision(&document_design_state(doc))?;
    // Hash actual geometry and source bindings, without disposable kernel IDs,
    // camera state or debug formatting. No new sampling is introduced.
    let meshes: Vec<_> = scene
        .meshes
        .iter()
        .map(|m| (&m.positions, &m.normals, &m.indices))
        .collect();
    let lines: Vec<_> = overlay
        .lines
        .iter()
        .map(|l| {
            (
                l.start,
                l.end,
                l.construction,
                &l.sketch_id,
                &l.entity_id,
                l.segment_index,
            )
        })
        .collect();
    let mut bindings: Vec<SelectionBinding> = Vec::new();
    let mut add = |target: SelectionTarget, hit: SelectionHit| {
        if let Ok(mut binding) = resolve_selection_source(doc, &target, provenance) {
            if let Some(existing) = bindings.iter_mut().find(|b| b.target == target) {
                existing.hits.push(hit);
            } else {
                binding.hits.push(hit);
                bindings.push(binding);
            }
        }
    };
    for group in &scene.face_catalog.groups {
        if let Some(kernel_id) = group.kernel_face_id {
            let candidates: Vec<_> = provenance
                .iter()
                .filter(|p| {
                    p.kind == TopoRefKind::Face
                        && resolved(p)
                        && p.resolved_kernel_id == Some(kernel_id)
                })
                .collect();
            if candidates.len() == 1 {
                add(
                    SelectionTarget::FaceReference {
                        ref_id: candidates[0].ref_id.clone(),
                    },
                    SelectionHit::FaceGroup { index: group.index },
                );
            }
        }
    }
    for (index, line) in overlay.lines.iter().enumerate() {
        if let (Some(sketch_id), Some(entity_id)) = (&line.sketch_id, &line.entity_id) {
            add(
                SelectionTarget::SketchEntity {
                    sketch_id: sketch_id.clone(),
                    entity_id: entity_id.clone(),
                },
                SelectionHit::SketchLine { index },
            );
        }
    }
    bindings.sort_by(|a, b| a.target.cmp(&b.target));
    let scene_revision = sha256_hex(&serde_json::to_vec(&(
        "musubicad.selection-scene.v1",
        doc.metadata.id.as_str(),
        &source_revision,
        meshes,
        lines,
        &bindings,
    ))?);

    Ok(SelectionFrame {
        source_revision,
        scene_revision,
        bindings,
        provenance: provenance.to_vec(),
    })
}

pub fn plan_selection_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    frame: &SelectionFrame,
    edit: &SelectionEdit,
    kernel: &K,
) -> Result<FeatureEditPlan> {
    let revision = opencad_ai::design_state_revision(&document_design_state(doc))?;
    if edit.source_revision != revision
        || frame.source_revision != revision
        || edit.scene_revision != frame.scene_revision
    {
        return Err(OpenCadError::validation(
            "stale source or scene revision; select again",
        ));
    }
    let binding = resolve_selection_source(doc, &edit.target, &frame.provenance)?;
    if !frame
        .bindings
        .iter()
        .any(|b| b.target == edit.target && b.feature_id == binding.feature_id)
    {
        return Err(OpenCadError::validation(
            "selection is absent from the current scene",
        ));
    }
    if !binding.fields.iter().any(|f| f.field == edit.field) {
        return Err(OpenCadError::validation(
            "unsupported selection dimension field",
        ));
    }
    plan_feature_edit_with_kernel(
        doc,
        &revision,
        FeatureEdit::SetExpression {
            id: binding.feature_id,
            field: edit.field.clone(),
            expr: edit.expr.clone(),
        },
        kernel,
    )
}

pub fn plan_document_selection_edit(path: &str, edit: &SelectionEdit) -> Result<FeatureEditPlan> {
    let doc = read_ocad(path)?;
    if opencad_ai::design_state_revision(&document_design_state(&doc))? != edit.source_revision {
        return Err(OpenCadError::validation(
            "stale source revision; select again",
        ));
    }
    let data = crate::load_view_data(path)?;
    let frame = data
        .selection_frame
        .ok_or_else(|| OpenCadError::validation("selection editing requires a part scene"))?;
    let plan = plan_selection_with_kernel(
        &doc,
        &frame,
        edit,
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )?;
    crate::plan_document_feature_edit(path, &plan.revision, plan.edit)
}

pub fn apply_document_selection_edit(
    path: &str,
    edit: &SelectionEdit,
    history: Option<opencad_file::DocumentHistory>,
) -> Result<crate::FeatureEditResult> {
    let plan = plan_document_selection_edit(path, edit)?;
    crate::apply_document_feature_edit(path, &plan.revision, plan.edit, history)
}
