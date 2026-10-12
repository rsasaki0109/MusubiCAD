//! Polygonal slice views with even-odd material hatching (ADR-042).
use crate::{DrawingView, ProjectionKind};
use opencad_core::{OpenCadError, Result, ViewId};
use opencad_geometry::{SectionGeometry, SectionPlane};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionView {
    pub plane: SectionPlane,
    /// Positive hatch pitch in sheet metres, independent of model scale.
    pub hatch_spacing_m: f64,
}
impl SectionView {
    pub fn validate(&self, projection: ProjectionKind) -> Result<()> {
        self.plane.validate()?;
        let normal = projection.project_point(self.plane.normal);
        if projection == ProjectionKind::Isometric
            || normal[0].hypot(normal[1]) > 1e-9
            || !self.hatch_spacing_m.is_finite()
            || self.hatch_spacing_m <= 1e-6
            || !(self.hatch_spacing_m * 1000.).is_finite()
        {
            return Err(OpenCadError::validation("section requires a plane-parallel orthographic view and finite hatch spacing > 1e-6 sheet metres"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SectionLayout {
    pub view_id: ViewId,
    pub loops_on_sheet_m: Vec<Vec<[f64; 2]>>,
    pub hatch_spacing_m: f64,
}
pub fn layout_section(view: &DrawingView, geometry: &SectionGeometry) -> Result<SectionLayout> {
    let section = view
        .section
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("view has no cutting plane"))?;
    section.validate(view.projection)?;
    if !view.scale.is_finite()
        || view.scale <= 0.
        || !view.origin_on_sheet_m.iter().all(|v| v.is_finite())
        || geometry.loops_m.is_empty()
        || !geometry.area_m2.is_finite()
        || geometry.area_m2 <= 0.
    {
        return Err(OpenCadError::validation(
            "invalid section witness or view layout",
        ));
    }
    let loops: Vec<Vec<[f64; 2]>> = geometry
        .loops_m
        .iter()
        .map(|points| {
            points
                .iter()
                .map(|p| {
                    let projected = view.projection.project_point(*p);
                    std::array::from_fn(|i| view.origin_on_sheet_m[i] + projected[i] * view.scale)
                })
                .collect()
        })
        .collect();
    if loops.iter().any(|p| p.len() < 3)
        || loops
            .iter()
            .flatten()
            .flatten()
            .any(|v| !(v * 1000.).is_finite())
    {
        return Err(OpenCadError::validation(
            "section layout has invalid contours or overflows sheet millimetres",
        ));
    }
    Ok(SectionLayout {
        view_id: view.id.clone(),
        loops_on_sheet_m: loops,
        hatch_spacing_m: section.hatch_spacing_m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::DocumentId;
    fn view() -> DrawingView {
        let mut v = DrawingView::new(
            ViewId::new("view:section").expect("id"),
            "Section",
            crate::ModelReference::new("plate.ocad.d", DocumentId::new("doc:plate").expect("id")),
            ProjectionKind::Front,
            0.5,
            [0.05, 0.06],
        );
        v.section = Some(SectionView {
            plane: SectionPlane {
                origin_m: [0., 0., 0.003],
                normal: [0., 0., 1.],
            },
            hatch_spacing_m: 0.003,
        });
        v
    }
    #[test]
    fn section_layout_scales_contours_but_preserves_sheet_hatch_spacing() {
        let v = view();
        let geometry = SectionGeometry {
            loops_m: vec![vec![
                [0., 0., 0.003],
                [0.08, 0., 0.003],
                [0.08, 0.06, 0.003],
                [0., 0.06, 0.003],
            ]],
            area_m2: 0.0048,
        };
        let layout = layout_section(&v, &geometry).expect("layout");
        assert!((layout.loops_on_sheet_m[0][2][0] - 0.09).abs() < 1e-12);
        assert!((layout.loops_on_sheet_m[0][2][1] - 0.09).abs() < 1e-12);
        assert!((layout.hatch_spacing_m - 0.003).abs() < 1e-12);
        let json = serde_json::to_value(&v).expect("json");
        assert_eq!(v, serde_json::from_value(json.clone()).expect("round trip"));
        let mut bad = json;
        bad["section"]["cached_area_m2"] = serde_json::json!(1.);
        assert!(serde_json::from_value::<DrawingView>(bad).is_err());
        for projection in [
            ProjectionKind::Top,
            ProjectionKind::Right,
            ProjectionKind::Isometric,
        ] {
            let mut bad = v.clone();
            bad.projection = projection;
            assert!(layout_section(&bad, &geometry).is_err());
        }
        for spacing in [0., -0.1, f64::NAN, f64::INFINITY] {
            let mut bad = v.clone();
            bad.section.as_mut().expect("section").hatch_spacing_m = spacing;
            assert!(layout_section(&bad, &geometry).is_err());
        }
    }
}
