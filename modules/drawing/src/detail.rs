//! Bounded detail views linked to an ordinary parent view (ADR-043).
use crate::{DrawingView, ProjectionKind, Sheet, SheetSegment};
use opencad_core::{OpenCadError, Result, ViewId};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailView {
    pub parent_view_id: ViewId,
    /// Rectangular crop in the parent's projected model coordinates, in metres.
    pub region_min_m: [f64; 2],
    pub region_max_m: [f64; 2],
    /// Ratio of child scale to parent scale; must exceed one.
    pub magnification: f64,
}
impl DetailView {
    pub fn validate(&self) -> Result<()> {
        if !self
            .region_min_m
            .iter()
            .chain(&self.region_max_m)
            .all(|v| v.is_finite())
            || (0..2).any(|i| {
                !((self.region_max_m[i] - self.region_min_m[i]).is_finite())
                    || self.region_max_m[i] - self.region_min_m[i] <= 1e-9
            })
            || !self.magnification.is_finite()
            || self.magnification <= 1.
        {
            return Err(invalid("detail requires a finite positive rectangular region in model metres and magnification > 1"));
        }
        Ok(())
    }
    pub fn center_m(&self) -> [f64; 2] {
        std::array::from_fn(|i| {
            self.region_min_m[i] + (self.region_max_m[i] - self.region_min_m[i]) / 2.
        })
    }
}
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(message)
}
pub fn validate_detail_parent<'a>(view: &DrawingView, sheet: &'a Sheet) -> Result<&'a DrawingView> {
    let detail = view
        .detail
        .as_ref()
        .ok_or_else(|| invalid("view has no detail definition"))?;
    detail.validate()?;
    let parent = sheet
        .views
        .iter()
        .find(|v| v.id == detail.parent_view_id)
        .ok_or_else(|| invalid("detail parent is missing or outside its sheet"))?;
    let required = parent.scale * detail.magnification;
    if !view.scale.is_finite()
        || view.scale <= 0.
        || !parent.scale.is_finite()
        || parent.scale <= 0.
        || !view
            .origin_on_sheet_m
            .iter()
            .chain(&parent.origin_on_sheet_m)
            .all(|v| v.is_finite())
        || parent.id == view.id
        || parent.detail.is_some()
        || parent.section.is_some()
        || view.section.is_some()
        || parent.projection == ProjectionKind::Isometric
        || view.projection != parent.projection
        || parent.model != view.model
        || !required.is_finite()
        || required <= 0.
        || (view.scale - required).abs() > 1e-9 * required.abs()
    {
        return Err(invalid("detail needs an ordinary same-model/projection parent and scale = parent scale × magnification; nested/section details are unsupported"));
    }
    Ok(parent)
}
/// Analytic Liang–Barsky rectangle clipping; preserves visibility, no sampling.
pub fn clip_detail_segment(
    segment: &SheetSegment,
    detail: &DetailView,
) -> Result<Option<SheetSegment>> {
    detail.validate()?;
    if !segment
        .start_m
        .iter()
        .chain(&segment.end_m)
        .all(|v| v.is_finite())
    {
        return Err(invalid(
            "detail segment must have finite projected model metres",
        ));
    }
    let delta: [f64; 2] = std::array::from_fn(|i| segment.end_m[i] - segment.start_m[i]);
    if !delta.iter().all(|v| v.is_finite()) {
        return Err(invalid("detail segment extent overflows"));
    }
    let mut enter: f64 = 0.;
    let mut leave: f64 = 1.;
    for (axis, &d) in delta.iter().enumerate() {
        if d.abs() <= 1e-12 {
            if segment.start_m[axis] < detail.region_min_m[axis]
                || segment.start_m[axis] > detail.region_max_m[axis]
            {
                return Ok(None);
            }
        } else {
            let a = (detail.region_min_m[axis] - segment.start_m[axis]) / d;
            let b = (detail.region_max_m[axis] - segment.start_m[axis]) / d;
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
            if enter > leave {
                return Ok(None);
            }
        }
    }
    let start_m = std::array::from_fn(|i| segment.start_m[i] + enter * delta[i]);
    let end_m = std::array::from_fn(|i| segment.start_m[i] + leave * delta[i]);
    if (end_m[0] - start_m[0]).hypot(end_m[1] - start_m[1]) <= 1e-9 {
        return Ok(None);
    }
    Ok(Some(SheetSegment {
        start_m,
        end_m,
        visibility: segment.visibility,
    }))
}
pub fn layout_detail_segments(
    view: &DrawingView,
    sheet: &Sheet,
    segments: &[SheetSegment],
) -> Result<Vec<SheetSegment>> {
    validate_detail_parent(view, sheet)?;
    let detail = view
        .detail
        .as_ref()
        .ok_or_else(|| invalid("detail missing"))?;
    if (0..2).any(|i| {
        !((detail.region_max_m[i] - detail.region_min_m[i]) * view.scale * 1000.).is_finite()
    }) {
        return Err(invalid("detail region layout overflows sheet millimetres"));
    }
    let center = detail.center_m();
    let mut output = Vec::new();
    for segment in segments {
        if let Some(segment) = clip_detail_segment(segment, detail)? {
            let project = |p: [f64; 2]| {
                std::array::from_fn(|i| view.origin_on_sheet_m[i] + (p[i] - center[i]) * view.scale)
            };
            let start_m = project(segment.start_m);
            let end_m = project(segment.end_m);
            if !start_m
                .iter()
                .chain(&end_m)
                .all(|v| (v * 1000.).is_finite())
            {
                return Err(invalid("detail layout overflows sheet millimetres"));
            }
            output.push(SheetSegment {
                start_m,
                end_m,
                visibility: segment.visibility,
            });
        }
    }
    if output.is_empty() {
        return Err(invalid(
            "detail region contains no current projected model edges",
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineVisibility;
    fn detail() -> DetailView {
        DetailView {
            parent_view_id: ViewId::new("view:front").expect("id"),
            region_min_m: [0., 0.],
            region_max_m: [0.02, 0.01],
            magnification: 2.,
        }
    }
    fn segment(a: [f64; 2], b: [f64; 2]) -> SheetSegment {
        SheetSegment {
            start_m: a,
            end_m: b,
            visibility: LineVisibility::Hidden,
        }
    }
    #[test]
    fn analytic_clipping_crossing_inside_outside_diagonal_and_visibility() {
        for (input, a, b) in [
            (
                segment([-0.01, 0.005], [0.03, 0.005]),
                [0., 0.005],
                [0.02, 0.005],
            ),
            (
                segment([0.01, -0.01], [0.01, 0.02]),
                [0.01, 0.],
                [0.01, 0.01],
            ),
            (
                segment([0.005, 0.002], [0.015, 0.008]),
                [0.005, 0.002],
                [0.015, 0.008],
            ),
            (
                segment([-0.01, -0.01], [0.03, 0.03]),
                [0., 0.],
                [0.01, 0.01],
            ),
        ] {
            let actual = clip_detail_segment(&input, &detail())
                .expect("clip")
                .expect("visible");
            for (actual, expected) in actual
                .start_m
                .into_iter()
                .chain(actual.end_m)
                .zip(a.into_iter().chain(b))
            {
                assert!((actual - expected).abs() < 1e-12);
            }
            assert_eq!(actual.visibility, LineVisibility::Hidden);
        }
        for input in [
            segment([-0.01, 0.02], [0.03, 0.02]),
            segment([-0.01, 0.01], [0.01, 0.03]),
            segment([0.01, 0.005], [0.01, 0.005]),
        ] {
            assert!(clip_detail_segment(&input, &detail())
                .expect("clip")
                .is_none());
        }
        let mut bad = detail();
        bad.region_max_m = bad.region_min_m;
        assert!(bad.validate().is_err());
        bad = detail();
        bad.magnification = 1.;
        assert!(bad.validate().is_err());
        let json = serde_json::to_value(detail()).expect("json");
        assert_eq!(
            detail(),
            serde_json::from_value(json.clone()).expect("round trip")
        );
        let mut cache = json;
        cache["segments"] = serde_json::json!([]);
        assert!(serde_json::from_value::<DetailView>(cache).is_err());
    }
}
