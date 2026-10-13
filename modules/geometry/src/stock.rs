//! Transient, kernel-neutral witness for a physical rectangular stock face.
use crate::{FacePick, POSITION_TOLERANCE_M, ROTATION_TOLERANCE};
use opencad_core::{OpenCadError, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectangularStockGeometry {
    pub entry_face_id: u64,
    pub origin_m: [f64; 3],
    pub u_axis: [f64; 3],
    pub v_axis: [f64; 3],
    pub outward_normal: [f64; 3],
    pub face_spans_m: [f64; 2],
    pub inward_thickness_m: f64,
}
impl RectangularStockGeometry {
    /// Strict finite, outward and in-face matching; never a nearest-face search.
    pub fn matches_pick(&self, pick: FacePick) -> Result<bool> {
        let dot = |a: [f64; 3], b: [f64; 3]| a.into_iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
        if pick
            .point_m
            .iter()
            .chain(pick.normal.iter())
            .any(|v| !v.is_finite())
            || (dot(pick.normal, pick.normal).sqrt() - 1.).abs() > ROTATION_TOLERANCE
        {
            return Err(OpenCadError::validation(
                "stock pick requires finite point and unit normal",
            ));
        }
        let axes = [self.u_axis, self.v_axis, self.outward_normal];
        if self
            .origin_m
            .iter()
            .chain(axes.iter().flatten())
            .any(|v| !v.is_finite())
            || axes
                .iter()
                .any(|a| (dot(*a, *a) - 1.).abs() > ROTATION_TOLERANCE)
            || (0..3).any(|i| (0..i).any(|j| dot(axes[i], axes[j]).abs() > ROTATION_TOLERANCE))
            || self
                .face_spans_m
                .into_iter()
                .chain([self.inward_thickness_m])
                .any(|v| !v.is_finite() || v <= POSITION_TOLERANCE_M)
        {
            return Err(OpenCadError::validation(
                "invalid rectangular stock witness frame or dimensions",
            ));
        }
        let n = self.outward_normal;
        let c = [
            n[1] * pick.normal[2] - n[2] * pick.normal[1],
            n[2] * pick.normal[0] - n[0] * pick.normal[2],
            n[0] * pick.normal[1] - n[1] * pick.normal[0],
        ];
        let p = std::array::from_fn(|i| pick.point_m[i] - self.origin_m[i]);
        Ok(dot(n, p).abs() <= POSITION_TOLERANCE_M
            && dot(c, c).sqrt() <= ROTATION_TOLERANCE
            && dot(n, pick.normal) > 0.
            && [dot(p, self.u_axis), dot(p, self.v_axis)]
                .into_iter()
                .zip(self.face_spans_m)
                .all(|(v, span)| v >= -POSITION_TOLERANCE_M && v <= span + POSITION_TOLERANCE_M))
    }
}
