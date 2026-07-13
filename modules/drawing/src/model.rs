//! Top-level drawing document model.

use opencad_core::{DocumentId, OpenCadError, Result};
use serde::{Deserialize, Serialize};

use crate::sheet::Sheet;

/// Drawing design graph: sheets and projected model views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DrawingModel {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sheets: Vec<Sheet>,
}

impl DrawingModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sheet(&self, id: &opencad_core::SheetId) -> Option<&Sheet> {
        self.sheets.iter().find(|sheet| &sheet.id == id)
    }

    pub fn sorted_deterministic(mut self) -> Self {
        self.sheets.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        for sheet in &mut self.sheets {
            sheet.views.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            sheet
                .dimensions
                .sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        }
        self
    }

    pub fn validate(&self, drawing_doc_id: &DocumentId) -> Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        for sheet in &self.sheets {
            if !seen.insert(sheet.id.as_str().to_string()) {
                return Err(OpenCadError::validation(format!(
                    "duplicate sheet id '{}'",
                    sheet.id
                )));
            }
            sheet.validate(drawing_doc_id)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::DrawingView;
    use crate::{ModelReference, ProjectionKind};
    use opencad_core::{SheetId, ViewId};

    #[test]
    fn drawing_model_round_trip() -> Result<()> {
        let model = DrawingModel {
            sheets: vec![Sheet {
                id: SheetId::new("sheet:a4")?,
                name: "Sheet 1".into(),
                width_m: 0.210,
                height_m: 0.297,
                views: vec![DrawingView::new(
                    ViewId::new("view:front")?,
                    "Front",
                    ModelReference::new(
                        "parts/bracket.ocad.d",
                        DocumentId::new("doc:bracket_001")?,
                    ),
                    ProjectionKind::Front,
                    1.0,
                    [0.05, 0.05],
                )],
                dimensions: Vec::new(),
            }],
        };
        let json = serde_json::to_string(&model).expect("serialize");
        let restored: DrawingModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(model, restored);
        Ok(())
    }
}
