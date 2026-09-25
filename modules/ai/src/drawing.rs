//! Drawing queries, patches, and semantic diffs.

use std::collections::BTreeSet;

use opencad_core::{OpenCadError, Result};
use opencad_drawing::{DrawingModel, DrawingView, Sheet};
use opencad_graph::{build_summary, DesignDiff, SemanticChange};

use crate::patch::validate_stable_id;
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

fn find_sheet_mut<'a>(model: &'a mut DrawingModel, sheet_id: &str) -> Result<&'a mut Sheet> {
    model
        .sheets
        .iter_mut()
        .find(|sheet| sheet.id.as_str() == sheet_id)
        .ok_or_else(|| OpenCadError::validation(format!("unknown drawing sheet '{sheet_id}'")))
}

fn check_new_drawing_id(
    id: &str,
    prefix: &str,
    exists: bool,
    removed: &BTreeSet<String>,
    kind: &str,
) -> Result<()> {
    validate_stable_id(id, prefix)?;
    if removed.contains(id) {
        return Err(OpenCadError::validation(format!(
            "{kind} id '{id}' was removed earlier in this patch and cannot be reused"
        )));
    }
    if exists {
        return Err(OpenCadError::validation(format!(
            "{kind} '{id}' already exists"
        )));
    }
    Ok(())
}

fn validate_view_values(view: &DrawingView) -> Result<()> {
    if view.scale <= 0.0 || !view.scale.is_finite() {
        return Err(OpenCadError::validation(format!(
            "drawing view '{}' scale must be finite and > 0",
            view.id
        )));
    }
    if !view.origin_on_sheet_m.iter().all(|value| value.is_finite()) {
        return Err(OpenCadError::validation(format!(
            "drawing view '{}' origin must be finite",
            view.id
        )));
    }
    Ok(())
}

/// Final-state checks for structural drawing operations: dimensions must
/// stay on a view of their own sheet, and a removed view may not leave a
/// dimension behind.
fn structural_drawing_failures(model: &DrawingModel, operations: &[PatchOperation]) -> Vec<String> {
    let mut failures = BTreeSet::new();
    for operation in operations {
        let PatchOperation::RemoveDrawingView { view_id } = operation else {
            continue;
        };
        let users: BTreeSet<String> = model
            .sheets
            .iter()
            .flat_map(|sheet| sheet.dimensions.iter())
            .filter(|dimension| dimension.view_id.as_str() == view_id)
            .map(|dimension| format!("dimension {}", dimension.id))
            .collect();
        if !users.is_empty() {
            failures.insert(format!(
                "cannot remove '{view_id}': still used by {}",
                users.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    if failures.is_empty() {
        for sheet in &model.sheets {
            for dimension in &sheet.dimensions {
                if !sheet.views.iter().any(|view| view.id == dimension.view_id) {
                    failures.insert(format!(
                        "dimension '{}' references view '{}' outside sheet '{}'",
                        dimension.id, dimension.view_id, sheet.id
                    ));
                }
            }
        }
    }
    failures.into_iter().collect()
}

pub fn apply_drawing_patch(model: &mut DrawingModel, operations: &[PatchOperation]) -> Result<()> {
    let mut removed = BTreeSet::new();
    for operation in operations {
        match operation {
            PatchOperation::AddSheet { sheet } => {
                let id = sheet.id.as_str();
                let exists = model.sheets.iter().any(|item| item.id == sheet.id);
                check_new_drawing_id(id, "sheet", exists, &removed, "drawing sheet")?;
                if !sheet.views.is_empty() || !sheet.dimensions.is_empty() {
                    return Err(OpenCadError::validation(format!(
                        "sheet '{id}' must be added empty; add views and dimensions with add_drawing_view and add_drawing_dimension"
                    )));
                }
                if !(sheet.width_m > 0.0 && sheet.height_m > 0.0)
                    || !sheet.width_m.is_finite()
                    || !sheet.height_m.is_finite()
                {
                    return Err(OpenCadError::validation(format!(
                        "sheet '{id}' must have finite, positive width_m and height_m"
                    )));
                }
                model.sheets.push(sheet.clone());
            }
            PatchOperation::RemoveSheet { id } => {
                let index = model
                    .sheets
                    .iter()
                    .position(|sheet| sheet.id.as_str() == id)
                    .ok_or_else(|| {
                        OpenCadError::validation(format!("unknown drawing sheet '{id}'"))
                    })?;
                // A sheet owns its views and dimensions; they leave with it.
                let sheet = model.sheets.remove(index);
                removed.insert(id.clone());
                removed.extend(sheet.views.iter().map(|view| view.id.as_str().to_string()));
                removed.extend(
                    sheet
                        .dimensions
                        .iter()
                        .map(|dimension| dimension.id.as_str().to_string()),
                );
            }
            PatchOperation::AddDrawingView { sheet_id, view } => {
                let id = view.id.as_str();
                let exists = model
                    .sheets
                    .iter()
                    .flat_map(|sheet| sheet.views.iter())
                    .any(|item| item.id == view.id);
                check_new_drawing_id(id, "view", exists, &removed, "drawing view")?;
                validate_view_values(view)?;
                find_sheet_mut(model, sheet_id)?.views.push(view.clone());
            }
            PatchOperation::RemoveDrawingView { view_id } => {
                let sheet = model
                    .sheets
                    .iter_mut()
                    .find(|sheet| sheet.views.iter().any(|view| view.id.as_str() == view_id))
                    .ok_or_else(|| {
                        OpenCadError::validation(format!("unknown drawing view '{view_id}'"))
                    })?;
                sheet.views.retain(|view| view.id.as_str() != view_id);
                removed.insert(view_id.clone());
            }
            PatchOperation::AddDrawingDimension {
                sheet_id,
                dimension,
            } => {
                let id = dimension.id.as_str();
                let exists = model
                    .sheets
                    .iter()
                    .flat_map(|sheet| sheet.dimensions.iter())
                    .any(|item| item.id == dimension.id);
                check_new_drawing_id(id, "dim", exists, &removed, "drawing dimension")?;
                dimension.validate()?;
                find_sheet_mut(model, sheet_id)?
                    .dimensions
                    .push(dimension.clone());
            }
            PatchOperation::RemoveDrawingDimension { id } => {
                let sheet = model
                    .sheets
                    .iter_mut()
                    .find(|sheet| {
                        sheet
                            .dimensions
                            .iter()
                            .any(|dimension| dimension.id.as_str() == id)
                    })
                    .ok_or_else(|| {
                        OpenCadError::validation(format!("unknown drawing dimension '{id}'"))
                    })?;
                sheet
                    .dimensions
                    .retain(|dimension| dimension.id.as_str() != id);
                removed.insert(id.clone());
            }
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
    let failures = structural_drawing_failures(model, operations);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(OpenCadError::validation(failures.join("; ")))
    }
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
    diff_items_by_id(
        &before.dimensions,
        &after.dimensions,
        |item| item.id.as_str(),
        |id| SemanticChange::DrawingDimensionAdded { id },
        |id| SemanticChange::DrawingDimensionRemoved { id },
        |id, before, after| SemanticChange::DrawingDimensionChanged { id, before, after },
        changes,
    );
}

/// Report added, removed, and changed items by stable ID, in ID order.
fn diff_items_by_id<T: serde::Serialize + PartialEq>(
    before: &[T],
    after: &[T],
    id_of: impl Fn(&T) -> &str,
    added: impl Fn(String) -> SemanticChange,
    removed: impl Fn(String) -> SemanticChange,
    changed: impl Fn(String, String, String) -> SemanticChange,
    changes: &mut Vec<SemanticChange>,
) {
    let before: std::collections::BTreeMap<&str, &T> =
        before.iter().map(|item| (id_of(item), item)).collect();
    let after: std::collections::BTreeMap<&str, &T> =
        after.iter().map(|item| (id_of(item), item)).collect();
    let ids: std::collections::BTreeSet<&str> =
        before.keys().chain(after.keys()).copied().collect();
    for id in ids {
        match (before.get(id), after.get(id)) {
            (Some(_), None) => changes.push(removed(id.to_string())),
            (None, Some(_)) => changes.push(added(id.to_string())),
            (Some(old), Some(new)) if old != new => changes.push(changed(
                id.to_string(),
                serde_json::to_string(old).unwrap_or_default(),
                serde_json::to_string(new).unwrap_or_default(),
            )),
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
