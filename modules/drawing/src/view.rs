//! Drawing view referencing a 3D model.

use opencad_core::{OpenCadError, Result, ViewId};
use serde::{Deserialize, Serialize};

use crate::projection::ProjectionKind;
use crate::reference::ModelReference;

/// A projected view of a referenced model placed on a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingView {
    pub id: ViewId,
    pub name: String,
    pub model: ModelReference,
    pub projection: ProjectionKind,
    /// Scale factor (1.0 = full size in meters).
    pub scale: f64,
    /// View origin on the sheet in meters.
    pub origin_on_sheet_m: [f64; 2],
}

impl DrawingView {
    pub fn new(
        id: ViewId,
        name: impl Into<String>,
        model: ModelReference,
        projection: ProjectionKind,
        scale: f64,
        origin_on_sheet_m: [f64; 2],
    ) -> Self {
        Self {
            id,
            name: name.into(),
            projection,
            scale,
            origin_on_sheet_m,
            model,
        }
    }

    pub fn validate(&self, drawing_doc_id: &opencad_core::DocumentId) -> Result<()> {
        if self.scale <= 0.0 {
            return Err(OpenCadError::validation(format!(
                "view '{}' scale must be > 0",
                self.id
            )));
        }
        self.model.validate(drawing_doc_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, Result};

    #[test]
    fn drawing_view_round_trip() -> Result<()> {
        let view = DrawingView::new(
            ViewId::new("view:front")?,
            "Front",
            ModelReference::new("parts/bracket.ocad.d", DocumentId::new("doc:bracket_001")?),
            ProjectionKind::Front,
            1.0,
            [0.05, 0.05],
        );
        let json = serde_json::to_string(&view).expect("serialize");
        let restored: DrawingView = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(view, restored);
        Ok(())
    }
}
