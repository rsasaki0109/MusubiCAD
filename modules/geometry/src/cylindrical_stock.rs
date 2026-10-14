//! ADR-065 physical cylinder witness and neutral-plane draft, all lengths metres.
use crate::{POSITION_TOLERANCE_M, ROTATION_TOLERANCE};
use opencad_core::{OpenCadError, Result};
fn invalid() -> OpenCadError {
    OpenCadError::validation("invalid bounded cylindrical stock/draft")
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn unit(a: [f64; 3]) -> bool {
    a.iter().all(|v| v.is_finite()) && (dot(a, a).sqrt() - 1.).abs() <= ROTATION_TOLERANCE
}
fn parallel(a: [f64; 3], b: [f64; 3]) -> bool {
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    dot(c, c).sqrt() <= ROTATION_TOLERANCE
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CylindricalStockGeometry {
    pub origin_m: [f64; 3],
    pub axis: [f64; 3],
    pub radius_m: f64,
    pub height_m: f64,
    pub start_face_id: u64,
    pub end_face_id: u64,
    pub lateral_face_id: u64,
}
impl CylindricalStockGeometry {
    pub fn validate(&self) -> Result<()> {
        if !unit(self.axis)
            || [self.radius_m, self.height_m]
                .iter()
                .any(|v| !v.is_finite() || *v <= POSITION_TOLERANCE_M || *v > 1000.)
            || [self.start_face_id, self.end_face_id, self.lateral_face_id].contains(&0)
            || self.start_face_id == self.end_face_id
            || self.start_face_id == self.lateral_face_id
            || self.end_face_id == self.lateral_face_id
        {
            return Err(invalid());
        }
        for i in 0..3 {
            let a = self.origin_m[i];
            let b = a + self.axis[i] * self.height_m;
            let radial = self.radius_m * (1. - self.axis[i] * self.axis[i]).max(0.).sqrt();
            if !a.is_finite()
                || !b.is_finite()
                || a.min(b) - radial < -1e6
                || a.max(b) + radial > 1e6
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
    /// Orientation only; an axis hint never establishes shape or face ownership.
    pub fn oriented(mut self, hint: [f64; 3]) -> Result<Self> {
        self.validate()?;
        if !unit(hint) || !parallel(hint, self.axis) {
            return Err(invalid());
        }
        if dot(hint, self.axis) < 0. {
            self.origin_m =
                std::array::from_fn(|i| self.origin_m[i] + self.axis[i] * self.height_m);
            self.axis = self.axis.map(|v| -v);
            std::mem::swap(&mut self.start_face_id, &mut self.end_face_id);
        }
        Ok(self)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CylindricalDraftInput {
    pub axis_hint: [f64; 3],
    pub neutral_origin_m: [f64; 3],
    pub neutral_normal: [f64; 3],
    pub angle_rad: f64,
}
impl CylindricalDraftInput {
    /// Exact endpoint radii for the admitted affine radial law.
    pub fn endpoint_radii(&self, stock: &CylindricalStockGeometry) -> Result<[f64; 2]> {
        stock.validate()?;
        if !unit(self.axis_hint)
            || !parallel(self.axis_hint, stock.axis)
            || dot(self.axis_hint, stock.axis) <= 0.
            || !unit(self.neutral_normal)
            || !parallel(self.neutral_normal, stock.axis)
            || self
                .neutral_origin_m
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e6)
            || !self.angle_rad.is_finite()
            || self.angle_rad.abs() < 1e-8
            || self.angle_rad.abs() > std::f64::consts::FRAC_PI_4
        {
            return Err(invalid());
        }
        let relative = std::array::from_fn(|i| self.neutral_origin_m[i] - stock.origin_m[i]);
        let mut z = dot(relative, stock.axis);
        let coordinate_scale = self
            .neutral_origin_m
            .into_iter()
            .chain(stock.origin_m)
            .map(f64::abs)
            .fold(1., f64::max);
        let epsilon = 64. * f64::EPSILON * stock.height_m.max(coordinate_scale);
        if z < -epsilon || z > stock.height_m + epsilon {
            return Err(invalid());
        }
        if z.abs() <= epsilon {
            z = 0.;
        } else if (z - stock.height_m).abs() <= epsilon {
            z = stock.height_m;
        }
        let k = dot(self.neutral_normal, stock.axis).signum() * self.angle_rad.tan();
        let radii = [
            stock.radius_m + k * z,
            stock.radius_m - k * (stock.height_m - z),
        ];
        if radii
            .iter()
            .any(|r| !r.is_finite() || *r <= POSITION_TOLERANCE_M || *r > 1000.)
        {
            return Err(invalid());
        }
        if (radii[0] - radii[1]).abs() <= POSITION_TOLERANCE_M {
            return Err(invalid());
        }
        // Output circles must remain in the existing representable world bound.
        let radius = radii[0].max(radii[1]);
        CylindricalStockGeometry {
            radius_m: radius,
            ..*stock
        }
        .validate()?;
        Ok(radii)
    }
}
