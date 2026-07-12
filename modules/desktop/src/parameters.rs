//! Parameter listing and editing for the desktop shell.

use opencad_core::Result;
use opencad_file::{read_ocad, write_ocad};
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

pub fn set_document_parameter(path: &str, id: &str, expr: &str) -> Result<()> {
    let mut doc = read_ocad(path)?;
    doc.parameters.set_expr(id, expr)?;
    doc.parameters.mark_dirty(id);
    write_ocad(path, &doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::write_bracket_fixture_at;
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
