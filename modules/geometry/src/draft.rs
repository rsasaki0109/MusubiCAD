//! Kernel-neutral neutral-plane draft contract (ADR-036).
use crate::FacePick;
use opencad_core::{OpenCadError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct DraftInput {
    pub angle_rad: f64,
    pub neutral_origin_m: [f64; 3],
    pub neutral_normal: [f64; 3],
    pub faces: Vec<FacePick>,
}
impl DraftInput {
    pub fn validate(&self) -> Result<()> {
        let valid_direction = |v: &[f64; 3]| {
            let length = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            v.iter().all(|x| x.is_finite()) && length.is_finite() && length > 1e-12
        };
        if !self.angle_rad.is_finite()
            || self.angle_rad.abs() < 1e-8
            || self.angle_rad.abs() > std::f64::consts::FRAC_PI_4
            || !self.neutral_origin_m.iter().all(|x| x.is_finite())
            || !valid_direction(&self.neutral_normal)
            || self.faces.is_empty()
            || self
                .faces
                .iter()
                .any(|f| !valid_direction(&f.normal) || !f.point_m.iter().all(|x| x.is_finite()))
        {
            return Err(OpenCadError::validation(
                "invalid neutral-plane draft input (m/rad)",
            ));
        }
        let norm = |v: &[f64; 3]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if self.faces.iter().any(|f| {
            f.normal
                .iter()
                .zip(self.neutral_normal)
                .map(|(a, b)| a * b)
                .sum::<f64>()
                .abs()
                / (norm(&f.normal) * norm(&self.neutral_normal))
                > 1e-6
        }) {
            return Err(OpenCadError::validation(
                "draft faces must initially be perpendicular to the neutral plane",
            ));
        }
        Ok(())
    }
}
