//! Parameter listing and editing for the desktop shell.

use opencad_ai::DesignPatch;
use opencad_core::Result;
use opencad_file::{
    apply_patch_with_history, read_ocad, write_ocad, DocumentHistory, DocumentHistoryState,
};
use opencad_graph::evaluate_param_graph;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterRow {
    pub id: String,
    pub name: String,
    pub expr: String,
    pub value_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_deg: Option<f64>,
    /// Short unit reminder shown under the expression field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_hint: Option<String>,
    /// Example expression used as the input placeholder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expr_hint: Option<String>,
}

fn is_angle_parameter(name: &str) -> bool {
    name.ends_with("_rad") || name.ends_with("_deg") || name.contains("angle")
}

fn parameter_hints(name: &str) -> (Option<String>, Option<String>) {
    if is_angle_parameter(name) {
        (Some("deg or rad".into()), Some("180 deg".into()))
    } else {
        (Some("mm, m, or names".into()), Some("80 mm".into()))
    }
}

pub fn list_document_parameters(path: &str) -> Result<Vec<ParameterRow>> {
    let doc = read_ocad(path)?;
    let order = doc.parameters.evaluation_order()?;
    let values = evaluate_param_graph(&doc.parameters)?;
    let mut rows = Vec::with_capacity(order.len());
    for id in order {
        let entry = doc
            .parameters
            .get(&id)
            .ok_or_else(|| opencad_core::OpenCadError::not_found(format!("parameter '{id}'")))?;
        let (unit_hint, expr_hint) = parameter_hints(&entry.name);
        rows.push(ParameterRow {
            id: entry.id.clone(),
            name: entry.name.clone(),
            expr: entry.expr.clone(),
            value_mm: if is_angle_parameter(&entry.name) {
                None
            } else {
                values.get(&entry.name).map(|meters| meters * 1000.0)
            },
            value_deg: if is_angle_parameter(&entry.name) {
                values.get(&entry.name).map(|radians| radians.to_degrees())
            } else {
                None
            },
            unit_hint,
            expr_hint,
        });
    }
    Ok(rows)
}

/// Set a parameter through the shared DesignPatch/file transaction boundary.
///
/// This compatibility wrapper intentionally does not retain history. Desktop
/// command clients should use [`set_document_parameter_with_history`] and
/// pass the returned opaque history value to later commands.
pub fn set_document_parameter(path: &str, id: &str, expr: &str) -> Result<()> {
    set_document_parameter_with_history(path, id, expr, None).map(|_| ())
}

/// Apply a parameter DesignPatch and return backend-owned undo/redo state.
///
/// `history` is passed by value because it is a transport value owned by the
/// caller. A failed validation or file write therefore cannot mutate the
/// caller's history. The history is never persisted in the `.ocad` document.
pub fn set_document_parameter_with_history(
    path: &str,
    id: &str,
    expr: &str,
    history: Option<DocumentHistory>,
) -> Result<DocumentHistoryState> {
    let mut doc = read_ocad(path)?;
    let revision = opencad_ai::design_state_revision(&opencad_file::document_design_state(&doc))?;
    let mut next_history = history.unwrap_or_default();
    crate::editor::validate_editor_parameter_expression(&doc, id, expr)?;
    let patch = DesignPatch::set_parameter(id, expr);
    let dry_run = opencad_file::dry_run_patch_document(&doc, &patch);
    opencad_ai::ensure_patch_valid(&dry_run)?;
    if dry_run.impact.no_op {
        return Ok(DocumentHistoryState::new(next_history));
    }
    apply_patch_with_history(
        &mut doc,
        &patch,
        &mut next_history,
        format!("Set parameter '{id}' to '{expr}'"),
    )?;
    crate::editor::verify_editor_document_native(&doc)?;
    if opencad_ai::design_state_revision(&opencad_file::document_design_state(&read_ocad(path)?))?
        != revision
    {
        return Err(opencad_core::OpenCadError::validation(
            "document changed during editor verification",
        ));
    }
    write_ocad(path, &doc)?;
    Ok(DocumentHistoryState::new(next_history))
}

/// Undo the latest backend history record for a document on disk.
pub fn undo_document_with_history(
    path: &str,
    history: DocumentHistory,
) -> Result<DocumentHistoryState> {
    let mut doc = read_ocad(path)?;
    let revision = opencad_ai::design_state_revision(&opencad_file::document_design_state(&doc))?;
    let mut next_history = history;
    next_history.undo(&mut doc)?;
    crate::editor::verify_editor_document_native(&doc)?;
    if opencad_ai::design_state_revision(&opencad_file::document_design_state(&read_ocad(path)?))?
        != revision
    {
        return Err(opencad_core::OpenCadError::validation(
            "document changed during editor verification",
        ));
    }
    write_ocad(path, &doc)?;
    Ok(DocumentHistoryState::new(next_history))
}

/// Redo the latest backend history record for a document on disk.
pub fn redo_document_with_history(
    path: &str,
    history: DocumentHistory,
) -> Result<DocumentHistoryState> {
    let mut doc = read_ocad(path)?;
    let revision = opencad_ai::design_state_revision(&opencad_file::document_design_state(&doc))?;
    let mut next_history = history;
    next_history.redo(&mut doc)?;
    crate::editor::verify_editor_document_native(&doc)?;
    if opencad_ai::design_state_revision(&opencad_file::document_design_state(&read_ocad(path)?))?
        != revision
    {
        return Err(opencad_core::OpenCadError::validation(
            "document changed during editor verification",
        ));
    }
    write_ocad(path, &doc)?;
    Ok(DocumentHistoryState::new(next_history))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::write_bracket_fixture_at;
    use opencad_file::read_ocad;
    use tempfile::tempdir;

    #[test]
    fn lists_bracket_parameters_in_evaluation_order() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let rows = list_document_parameters(path.to_str().expect("path")).expect("list");
        assert!(!rows.is_empty());
        let width = rows
            .iter()
            .find(|row| row.id == "param:width")
            .expect("width row");
        assert!(width.value_mm.is_some());
        let ids: Vec<_> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(
            ids.len(),
            ids.iter().collect::<std::collections::BTreeSet<_>>().len()
        );
    }

    #[test]
    fn updates_parameter_and_persists() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);
        let path = path.to_str().expect("path");

        set_document_parameter(path, "param:width", "100 mm").expect("set");

        let rows = list_document_parameters(path).expect("list");
        let width = rows
            .iter()
            .find(|row| row.id == "param:width")
            .expect("width");
        assert_eq!(width.expr, "100 mm");
        assert!((width.value_mm.expect("value") - 100.0).abs() < 1e-6);
    }

    #[test]
    fn parameter_edit_history_undoes_and_redoes_the_full_document() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);
        let path = path.to_str().expect("path");
        let before = read_ocad(path).expect("before");

        let edited =
            set_document_parameter_with_history(path, "param:width", "100 mm", None).expect("edit");
        assert!(edited.can_undo);
        assert!(!edited.can_redo);
        let after = read_ocad(path).expect("after");
        assert_ne!(before, after);

        let undone = undo_document_with_history(path, edited.history).expect("undo");
        assert!(!undone.can_undo);
        assert!(undone.can_redo);
        assert_eq!(read_ocad(path).expect("undone document"), before);

        let redone = redo_document_with_history(path, undone.history).expect("redo");
        assert!(redone.can_undo);
        assert!(!redone.can_redo);
        assert_eq!(read_ocad(path).expect("redone document"), after);
    }

    #[test]
    fn failed_parameter_edit_leaves_document_and_opaque_history_unchanged() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);
        let path = path.to_str().expect("path");

        let edited =
            set_document_parameter_with_history(path, "param:width", "100 mm", None).expect("edit");
        let before = read_ocad(path).expect("before failed edit");
        let history_json = serde_json::to_string(&edited.history).expect("history json");
        let error = set_document_parameter_with_history(
            path,
            "param:width",
            "not_a_length",
            Some(edited.history.clone()),
        )
        .expect_err("invalid expression");
        assert!(error.to_string().contains("invalid expression"));
        assert_eq!(read_ocad(path).expect("document after failed edit"), before);
        assert_eq!(
            serde_json::to_string(&edited.history).expect("history json after"),
            history_json
        );
    }

    #[test]
    fn angle_parameter_rows_include_deg_rad_hints() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("revolve.ocad.d");
        crate::template::create_revolve_bushing_document(path.to_str().expect("path"))
            .expect("create");

        let rows = list_document_parameters(path.to_str().expect("path")).expect("list");
        let angle = rows
            .iter()
            .find(|row| row.id == "param:revolve_angle")
            .expect("revolve angle");
        assert_eq!(angle.unit_hint.as_deref(), Some("deg or rad"));
        assert_eq!(angle.expr_hint.as_deref(), Some("180 deg"));
        assert!(angle.value_deg.is_some());
    }
}
