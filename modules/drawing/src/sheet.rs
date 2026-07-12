//! Drawing sheet containing placed views.

use opencad_core::{OpenCadError, Result, SheetId};
use serde::{Deserialize, Serialize};

use crate::view::DrawingView;

/// ISO A4 portrait sheet size in meters.
pub const A4_WIDTH_M: f64 = 0.210;
pub const A4_HEIGHT_M: f64 = 0.297;

/// One sheet in a drawing document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub id: SheetId,
    pub name: String,
    pub width_m: f64,
    pub height_m: f64,
    pub views: Vec<DrawingView>,
}

impl Sheet {
    pub fn a4_portrait(id: SheetId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            width_m: A4_WIDTH_M,
            height_m: A4_HEIGHT_M,
            views: Vec::new(),
        }
    }

    pub fn validate(&self, drawing_doc_id: &opencad_core::DocumentId) -> Result<()> {
        if self.width_m <= 0.0 || self.height_m <= 0.0 {
            return Err(OpenCadError::validation(format!(
                "sheet '{}' must have positive width and height",
                self.id
            )));
        }
        for view in &self.views {
            view.validate(drawing_doc_id)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::Result;

    #[test]
    fn sheet_round_trip() -> Result<()> {
        let sheet = Sheet::a4_portrait(SheetId::new("sheet:a4")?, "Sheet 1");
        let json = serde_json::to_string(&sheet).expect("serialize");
        let restored: Sheet = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sheet, restored);
        Ok(())
    }
}
