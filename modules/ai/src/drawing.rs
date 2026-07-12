//! Drawing queries, patches, and semantic diffs.

use opencad_core::{OpenCadError, Result};
use opencad_drawing::{DrawingModel, DrawingView, Sheet};
use opencad_graph::{build_summary, DesignDiff, SemanticChange};

use crate::PatchOperation;

pub fn list_drawing_sheets(model: &DrawingModel) -> Vec<Sheet> {
    let mut sheets = model.sheets.clone();
    sheets.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    sheets
}

pub fn get_drawing_sheet(model: &DrawingModel, id: &str) -> Result<Sheet> {
    model
        .sheets
        .iter()
        .find(|sheet| sheet.id.as_str() == id)
        .cloned()
        .ok_or_else(|| OpenCadError::validation(format!("unknown drawing sheet '{id}'")))
}

pub fn list_drawing_views(model: &DrawingModel, sheet_id: &str) -> Result<Vec<DrawingView>> {
    let mut views = get_drawing_sheet(model, sheet_id)?.views;
    views.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    Ok(views)
}

pub fn get_drawing_view(
    model: &DrawingModel,
    sheet_id: &str,
    view_id: &str,
) -> Result<DrawingView> {
    get_drawing_sheet(model, sheet_id)?
        .views
        .into_iter()
        .find(|view| view.id.as_str() == view_id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown drawing view '{view_id}'")))
}

pub fn apply_drawing_patch(model: &mut DrawingModel, operations: &[PatchOperation]) -> Result<()> {
    for operation in operations {
        match operation {
            PatchOperation::SetDrawingViewScale { view_id, scale } => {
                if *scale <= 0.0 || !scale.is_finite() {
                    return Err(OpenCadError::validation(
                        "drawing view scale must be finite and > 0",
                    ));
                }
                find_view_mut(model, view_id)?.scale = *scale;
            }
            PatchOperation::SetDrawingViewOrigin {
                view_id,
                origin_on_sheet_m,
            } => {
                if !origin_on_sheet_m.iter().all(|value| value.is_finite()) {
                    return Err(OpenCadError::validation(
                        "drawing view origin must be finite",
                    ));
                }
                find_view_mut(model, view_id)?.origin_on_sheet_m = *origin_on_sheet_m;
            }
            _ => {}
        }
    }
    Ok(())
}

fn find_view_mut<'a>(model: &'a mut DrawingModel, view_id: &str) -> Result<&'a mut DrawingView> {
    model
        .sheets
        .iter_mut()
        .flat_map(|sheet| sheet.views.iter_mut())
        .find(|view| view.id.as_str() == view_id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown drawing view '{view_id}'")))
}

pub fn diff_drawing_models(before: &DrawingModel, after: &DrawingModel) -> DesignDiff {
    use std::collections::BTreeMap;

    let before_sheets: BTreeMap<_, _> = before.sheets.iter().map(|s| (s.id.as_str(), s)).collect();
    let after_sheets: BTreeMap<_, _> = after.sheets.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut changes = Vec::new();
    for id in before_sheets
        .keys()
        .chain(after_sheets.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (before_sheets.get(id), after_sheets.get(id)) {
            (Some(_), None) => changes.push(SemanticChange::DrawingSheetRemoved {
                id: (*id).to_string(),
            }),
            (None, Some(_)) => changes.push(SemanticChange::DrawingSheetAdded {
                id: (*id).to_string(),
            }),
            (Some(before_sheet), Some(after_sheet)) => {
                diff_sheet(before_sheet, after_sheet, &mut changes)
            }
            _ => {}
        }
    }
    DesignDiff::semantic(build_summary(&changes), changes)
}

fn diff_sheet(before: &Sheet, after: &Sheet, changes: &mut Vec<SemanticChange>) {
    use std::collections::{BTreeMap, BTreeSet};
    if before.name != after.name
        || before.width_m != after.width_m
        || before.height_m != after.height_m
    {
        changes.push(SemanticChange::DrawingSheetChanged {
            id: before.id.as_str().to_string(),
            before: serde_json::to_string(before).unwrap_or_default(),
            after: serde_json::to_string(after).unwrap_or_default(),
        });
    }
    let before_views: BTreeMap<_, _> = before.views.iter().map(|v| (v.id.as_str(), v)).collect();
    let after_views: BTreeMap<_, _> = after.views.iter().map(|v| (v.id.as_str(), v)).collect();
    for id in before_views
        .keys()
        .chain(after_views.keys())
        .collect::<BTreeSet<_>>()
    {
        match (before_views.get(id), after_views.get(id)) {
            (Some(_), None) => changes.push(SemanticChange::DrawingViewRemoved {
                id: (*id).to_string(),
            }),
            (None, Some(_)) => changes.push(SemanticChange::DrawingViewAdded {
                id: (*id).to_string(),
            }),
            (Some(a), Some(b)) if a != b => changes.push(SemanticChange::DrawingViewChanged {
                id: (*id).to_string(),
                before: serde_json::to_string(a).unwrap_or_default(),
                after: serde_json::to_string(b).unwrap_or_default(),
            }),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, SheetId, ViewId};
    use opencad_drawing::{ModelReference, ProjectionKind};

    fn drawing() -> DrawingModel {
        let mut sheet = Sheet::a4_portrait(SheetId::new("sheet:main").expect("sheet id"), "Main");
        sheet.views.push(DrawingView::new(
            ViewId::new("view:front").expect("view id"),
            "Front",
            ModelReference::new(
                "parts/bracket.ocad.d",
                DocumentId::new("doc:bracket").expect("document id"),
            ),
            ProjectionKind::Front,
            1.0,
            [0.05, 0.06],
        ));
        DrawingModel {
            sheets: vec![sheet],
        }
    }

    #[test]
    fn queries_views_by_sheet() {
        let model = drawing();
        let views = list_drawing_views(&model, "sheet:main").expect("views");
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].id.as_str(), "view:front");
    }

    #[test]
    fn patch_scale_produces_semantic_diff() {
        let before = drawing();
        let mut after = before.clone();
        apply_drawing_patch(
            &mut after,
            &[PatchOperation::SetDrawingViewScale {
                view_id: "view:front".into(),
                scale: 2.0,
            }],
        )
        .expect("patch");

        assert_eq!(after.sheets[0].views[0].scale, 2.0);
        assert!(diff_drawing_models(&before, &after)
            .changes
            .iter()
            .any(|change| matches!(change, SemanticChange::DrawingViewChanged { id, .. } if id == "view:front")));
    }

    #[test]
    fn patch_rejects_non_positive_scale() {
        let mut model = drawing();
        let error = apply_drawing_patch(
            &mut model,
            &[PatchOperation::SetDrawingViewScale {
                view_id: "view:front".into(),
                scale: 0.0,
            }],
        )
        .expect_err("invalid scale");
        assert!(error.to_string().contains("scale"));
    }
}
