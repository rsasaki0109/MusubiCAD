//! Model-driven drawing dimensions (Task-179).

use opencad_core::{DimensionId, OpenCadError, Result, ViewId};
use serde::{Deserialize, Serialize};

use crate::{DrawingView, ProjectionKind};

/// An aligned linear dimension driven by two points in referenced-model meters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearDimension {
    /// Stable dimension identifier.
    pub id: DimensionId,
    /// Drawing view providing projection and sheet placement.
    pub view_id: ViewId,
    /// First measurement point in model-space meters.
    pub start_model_m: [f64; 3],
    /// Second measurement point in model-space meters.
    pub end_model_m: [f64; 3],
    /// Perpendicular offset of the dimension line from the measured edge, in meters on the sheet.
    pub offset_m: f64,
}

impl LinearDimension {
    /// Validate finite coordinates and a non-degenerate measured length.
    pub fn validate(&self) -> Result<()> {
        if !self
            .start_model_m
            .iter()
            .chain(self.end_model_m.iter())
            .chain(std::iter::once(&self.offset_m))
            .all(|value| value.is_finite())
        {
            return Err(OpenCadError::validation(format!(
                "drawing dimension '{}' values must be finite",
                self.id
            )));
        }
        if self.measured_length_m() <= f64::EPSILON {
            return Err(OpenCadError::validation(format!(
                "drawing dimension '{}' measured length must be positive",
                self.id
            )));
        }
        Ok(())
    }

    /// Derived model-space distance in meters.
    pub fn measured_length_m(&self) -> f64 {
        self.start_model_m
            .iter()
            .zip(self.end_model_m.iter())
            .map(|(start, end)| (end - start).powi(2))
            .sum::<f64>()
            .sqrt()
    }
}

/// Sheet-space geometry and derived label for an aligned dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionLayout {
    /// Projected first measurement point in sheet meters.
    pub witness_start_m: [f64; 2],
    /// Projected second measurement point in sheet meters.
    pub witness_end_m: [f64; 2],
    /// First endpoint of the offset dimension line in sheet meters.
    pub line_start_m: [f64; 2],
    /// Second endpoint of the offset dimension line in sheet meters.
    pub line_end_m: [f64; 2],
    /// Text anchor at the dimension-line midpoint in sheet meters.
    pub text_position_m: [f64; 2],
    /// Derived millimeter label.
    pub label: String,
}

/// Project and place one model-driven linear dimension on its drawing view.
pub fn layout_linear_dimension(
    dimension: &LinearDimension,
    view: &DrawingView,
) -> Result<DimensionLayout> {
    dimension.validate()?;
    if dimension.view_id != view.id {
        return Err(OpenCadError::validation(format!(
            "dimension '{}' references view '{}', not '{}'",
            dimension.id, dimension.view_id, view.id
        )));
    }
    let start = place_model_point(dimension.start_model_m, view.projection, view);
    let end = place_model_point(dimension.end_model_m, view.projection, view);
    let delta = [end[0] - start[0], end[1] - start[1]];
    let projected_length = delta[0].hypot(delta[1]);
    if projected_length <= f64::EPSILON {
        return Err(OpenCadError::validation(format!(
            "dimension '{}' endpoints overlap in view '{}'",
            dimension.id, view.id
        )));
    }
    let normal = [-delta[1] / projected_length, delta[0] / projected_length];
    let offset = [
        normal[0] * dimension.offset_m,
        normal[1] * dimension.offset_m,
    ];
    let line_start = [start[0] + offset[0], start[1] + offset[1]];
    let line_end = [end[0] + offset[0], end[1] + offset[1]];
    Ok(DimensionLayout {
        witness_start_m: start,
        witness_end_m: end,
        line_start_m: line_start,
        line_end_m: line_end,
        text_position_m: [
            (line_start[0] + line_end[0]) * 0.5,
            (line_start[1] + line_end[1]) * 0.5,
        ],
        label: format!("{:.2} mm", dimension.measured_length_m() * 1000.0),
    })
}

fn place_model_point(
    point_m: [f64; 3],
    projection: ProjectionKind,
    view: &DrawingView,
) -> [f64; 2] {
    let projected = projection.project_point(point_m);
    [
        view.origin_on_sheet_m[0] + projected[0] * view.scale,
        view.origin_on_sheet_m[1] + projected[1] * view.scale,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelReference;
    use opencad_core::DocumentId;

    #[test]
    fn derives_model_length_and_sheet_layout() -> Result<()> {
        let view = DrawingView::new(
            ViewId::new("view:front")?,
            "Front",
            ModelReference::new("part.ocad.d", DocumentId::new("doc:part")?),
            ProjectionKind::Front,
            0.5,
            [0.01, 0.02],
        );
        let dimension = LinearDimension {
            id: DimensionId::new("dim:width")?,
            view_id: view.id.clone(),
            start_model_m: [0.0, 0.0, 0.0],
            end_model_m: [0.08, 0.0, 0.0],
            offset_m: 0.01,
        };
        let layout = layout_linear_dimension(&dimension, &view)?;
        assert_eq!(layout.label, "80.00 mm");
        assert!((layout.line_end_m[0] - 0.05).abs() < 1.0e-9);
        assert!((layout.line_start_m[1] - 0.03).abs() < 1.0e-9);
        Ok(())
    }

    #[test]
    fn linear_dimension_round_trip() -> Result<()> {
        let dimension = LinearDimension {
            id: DimensionId::new("dim:width")?,
            view_id: ViewId::new("view:front")?,
            start_model_m: [0.0, 0.0, 0.0],
            end_model_m: [0.08, 0.0, 0.0],
            offset_m: -0.01,
        };
        let json = serde_json::to_string(&dimension).expect("serialize");
        let restored: LinearDimension = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(dimension, restored);
        Ok(())
    }
}
