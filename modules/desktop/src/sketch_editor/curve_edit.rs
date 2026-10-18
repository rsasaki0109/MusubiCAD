//! ADR-109: source-bound discovery and endpoint-verified curve edit forms.
use super::*;
use opencad_ai::{plan_curve_edit, CurveEditPlan, CurveEditRequest};
use opencad_sketch::curve_edit::{CurveEditEnd, CurveEditIntersection, CurveEditMode};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchCurveTargets {
    pub targets: Vec<String>,
    pub cutters: Vec<String>,
    pub suggested_point_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SketchCurveEditQuery {
    pub document_id: String,
    pub revision: String,
    pub sketch_id: String,
    pub target_id: String,
    pub cutter_id: String,
    pub mode: CurveEditMode,
    pub end: CurveEditEnd,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchCurveEditOptions {
    pub query: SketchCurveEditQuery,
    pub new_point_id: String,
    pub intersections: Vec<CurveEditIntersection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchCurveEditEffect {
    pub options: SketchCurveEditOptions,
    pub selected_index: usize,
    pub solved_position_m: [f64; 2],
}

fn kind(sketch: &Sketch, entity: &SketchEntity) -> Option<&'static str> {
    let point = |id: &EntityId| point_constraint_allowed(sketch, id.as_str());
    match entity {
        SketchEntity::Line(line) if point(&line.start) && point(&line.end) => Some("line"),
        SketchEntity::Circle(circle) if point(&circle.center) => Some("circle"),
        SketchEntity::Arc(arc)
            if point(&arc.center)
                && [&arc.start_point, &arc.end_point]
                    .into_iter()
                    .flatten()
                    .all(point) =>
        {
            Some("arc")
        }
        _ => None,
    }
}

pub(super) fn targets(doc: &OcadDocument, sketch: &Sketch) -> Result<SketchCurveTargets> {
    let ids: std::collections::BTreeSet<_> = doc
        .sketches
        .iter()
        .flat_map(|s| &s.entities)
        .flat_map(SketchEntity::defined_ids)
        .map(|id| id.as_str())
        .collect();
    let suggested_point_id = (1..=ids.len() + 1)
        .map(|n| format!("ent:trim_end_{n}"))
        .find(|id| !ids.contains(id.as_str()))
        .ok_or_else(|| OpenCadError::validation("no free curve endpoint identity"))?;
    let mut targets = Vec::new();
    let mut cutters = Vec::new();
    for entity in &sketch.entities {
        if let Some(kind) = kind(sketch, entity) {
            if kind != "circle" {
                targets.push(entity.id().as_str().into());
            }
            cutters.push(entity.id().as_str().into());
        }
    }
    Ok(SketchCurveTargets {
        targets,
        cutters,
        suggested_point_id,
    })
}

fn plan(
    doc: &OcadDocument,
    query: &SketchCurveEditQuery,
    new_point_id: &str,
    intersection_index: usize,
    remove_constraints: &[String],
) -> Result<CurveEditPlan> {
    if query.document_id != doc.metadata.id.as_str()
        || query.revision != feature_tree(doc)?.revision
    {
        return Err(OpenCadError::validation(
            "stale or foreign curve edit source",
        ));
    }
    let source = authored(doc, &query.sketch_id)?;
    let catalog = targets(doc, source)?;
    if !catalog.targets.contains(&query.target_id) || !catalog.cutters.contains(&query.cutter_id) {
        return Err(OpenCadError::validation(
            "curve edit requires admitted authored target/cutter IDs",
        ));
    }
    if doc
        .sketches
        .iter()
        .flat_map(|s| &s.entities)
        .flat_map(SketchEntity::defined_ids)
        .any(|id| id.as_str() == new_point_id)
    {
        return Err(OpenCadError::validation(
            "curve endpoint ID must be globally free",
        ));
    }
    plan_curve_edit(
        &document_design_state(doc),
        &CurveEditRequest {
            sketch_id: query.sketch_id.clone(),
            target_id: query.target_id.clone(),
            cutter_id: query.cutter_id.clone(),
            mode: query.mode,
            end: query.end,
            new_point_id: new_point_id.into(),
            intersection_index: Some(intersection_index),
            remove_constraints: remove_constraints.to_vec(),
        },
    )
}

pub fn sketch_curve_edit_options_with_kernel<K: GeometryKernel>(
    doc: &OcadDocument,
    query: &SketchCurveEditQuery,
    kernel: &K,
) -> Result<SketchCurveEditOptions> {
    let state = sketch_editor_state_with_kernel(doc, kernel)?;
    let row = state
        .sketches
        .iter()
        .find(|row| row.source.id.as_str() == query.sketch_id)
        .ok_or_else(|| {
            OpenCadError::validation("curve source is not an admitted authored sketch")
        })?;
    let new_point_id = row.curve_edit_targets.suggested_point_id.clone();
    // A read-only probe obtains all intersections. Neither its patch nor its
    // index is returned as an accepted choice; the user must choose explicitly.
    let probe = plan(doc, query, &new_point_id, 0, &[])?;
    Ok(SketchCurveEditOptions {
        query: query.clone(),
        new_point_id,
        intersections: probe.intersections,
    })
}

pub fn inspect_document_curve_edit(
    path: &str,
    query: &SketchCurveEditQuery,
) -> Result<SketchCurveEditOptions> {
    sketch_curve_edit_options_with_kernel(
        &read_ocad(path)?,
        query,
        &opencad_kernel_occt::OcctGeometryKernel::new(),
    )
}

fn form_query(edit: &SketchEdit) -> Result<SketchCurveEditQuery> {
    let [SketchForm::CurveEdit {
        target_id,
        cutter_id,
        mode,
        end,
        ..
    }] = edit.operations.as_slice()
    else {
        return Err(OpenCadError::validation(
            "curve edit requires exactly one typed operation",
        ));
    };
    Ok(SketchCurveEditQuery {
        document_id: edit
            .document_id
            .clone()
            .ok_or_else(|| OpenCadError::validation("curve edit requires document identity"))?,
        revision: edit.revision.clone(),
        sketch_id: edit.sketch_id.clone(),
        target_id: target_id.clone(),
        cutter_id: cutter_id.clone(),
        mode: *mode,
        end: *end,
    })
}

pub(super) fn patch(doc: &OcadDocument, edit: &SketchEdit) -> Result<DesignPatch> {
    let query = form_query(edit)?;
    let SketchForm::CurveEdit {
        new_point_id,
        intersection_index,
        remove_constraints,
        ..
    } = &edit.operations[0]
    else {
        return Err(OpenCadError::validation("curve form required"));
    };
    let patch = plan(
        doc,
        &query,
        new_point_id,
        *intersection_index,
        remove_constraints,
    )?
    .patch;
    if patch.operations.len() > 64 {
        return Err(OpenCadError::validation(
            "curve edit exceeds 64 expanded source operations",
        ));
    }
    Ok(patch)
}

pub(super) fn effect(
    doc: &OcadDocument,
    edit: &SketchEdit,
    state: &SketchEditorState,
) -> Result<Option<SketchCurveEditEffect>> {
    if !edit
        .operations
        .iter()
        .any(|form| matches!(form, SketchForm::CurveEdit { .. }))
    {
        return Ok(None);
    }
    let query = form_query(edit)?;
    let SketchForm::CurveEdit {
        new_point_id,
        intersection_index,
        remove_constraints,
        ..
    } = &edit.operations[0]
    else {
        return Err(OpenCadError::validation("curve form required"));
    };
    let original = plan(
        doc,
        &query,
        new_point_id,
        *intersection_index,
        remove_constraints,
    )?;
    let row = state
        .sketches
        .iter()
        .find(|row| row.source.id.as_str() == edit.sketch_id)
        .ok_or_else(|| OpenCadError::validation("solved curve source unavailable"))?;
    let Some(SketchEntity::Point(point)) = row.solved.find_entity(new_point_id) else {
        return Err(OpenCadError::validation(
            "solved curve endpoint unavailable",
        ));
    };
    let (Coord::Literal(x), Coord::Literal(y)) = (&point.x, &point.y) else {
        return Err(OpenCadError::validation(
            "curve endpoint was not solved in SI",
        ));
    };
    let expected = original.intersections[*intersection_index].position_m;
    if !x.is_finite() || !y.is_finite() || (x - expected[0]).hypot(y - expected[1]) > 1e-9 {
        return Err(OpenCadError::validation("retained constraints move the selected curve intersection; explicitly remove the offending constraints"));
    }
    Ok(Some(SketchCurveEditEffect {
        options: SketchCurveEditOptions {
            query,
            new_point_id: new_point_id.clone(),
            intersections: original.intersections,
        },
        selected_index: *intersection_index,
        solved_position_m: [*x, *y],
    }))
}
