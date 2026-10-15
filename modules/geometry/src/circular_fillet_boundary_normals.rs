//! ADR-091: complete corresponding-parameter paired unit-normal chord gaps.
use crate::{
    circular_fillet_patch::norm_upper, interval::I, CircularFilletCylindricalContactBounds,
    CircularFilletCylindricalContactSnapshot, CircularFilletRationalSnapshot,
    CircularFilletRationalSnapshotBounds, CircularFilletReference,
};
use opencad_core::{Length, OpenCadError, Result};

#[derive(Debug, Clone, Copy)]
pub struct CircularFilletBoundaryNormalPrecision {
    pub maximum_position_error: Length,
    pub maximum_surface_normal_error: f64,
    pub maximum_normal_gap: f64,
    /// Per current surface, not a sample count or total case budget.
    pub max_leaves: usize,
}
/// Whole corresponding-parameter boundary bounds for four charts in chart order.
/// No exact G1, floating native evaluation, embedding or whole-BREP certificate.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletBoundaryNormalBounds {
    pub surfaces: [CircularFilletRationalSnapshotBounds; 4],
    pub cylindrical_lifts: [CircularFilletCylindricalContactBounds; 4],
    pub plane_frame_normal_error_upper: f64,
    pub cylinder_frame_normal_error_upper: [f64; 4],
    pub cylinder_normal_error_upper: [f64; 4],
    pub side_normal_gap_upper: [f64; 4],
    pub end_normal_gap_upper: [f64; 4],
    pub seam_normal_gap_upper: [f64; 4],
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("current boundary normal gap or precision not certified")
}
fn deviation(direction: [I; 3], axis: usize) -> Result<f64> {
    if direction.iter().any(|x| !x.valid()) {
        return Err(invalid());
    }
    let error = norm_upper(std::array::from_fn(|j| {
        direction[j] - I::exact(if j == axis { 1. } else { 0. })
    }));
    if !error.is_finite() {
        return Err(invalid());
    }
    Ok(error)
}
fn frame_normal(d: I) -> Result<f64> {
    if !d.valid() || d.lo < 0. || d.hi >= 1. {
        return Err(invalid());
    }
    // |a| >= 1-d > 0; |a/|a|-n| <= |1-|a||+|a-n| <= 2d.
    let bound = I::exact(2.) * d;
    if !bound.valid() {
        return Err(invalid());
    }
    Ok(bound.hi)
}
fn plane_error(axes: [[f64; 3]; 2]) -> Result<f64> {
    let x = I::exact(deviation(axes[0].map(I::exact), 0)?);
    let y = I::exact(deviation(axes[1].map(I::exact), 1)?);
    frame_normal(x + y + x * y)
}
fn cylinder_error(axes: [[I; 3]; 3]) -> Result<f64> {
    let x = I::exact(deviation(axes[0], 0)?);
    let y = I::exact(deviation(axes[1], 1)?);
    let z = I::exact(deviation(axes[2], 2)?);
    frame_normal((x + y) * (I::exact(1.) + z) + z)
}
impl CircularFilletReference {
    /// Recompute all current definitions; no external certificate can authorize
    /// a result. Capture must qualify chart order, actual owning forward faces,
    /// fixed representations and attached original-parameter boundary lines.
    /// All twelve full-span gaps must meet precision, or the whole case refuses.
    pub fn certify_boundary_normals(
        self,
        surfaces: &[CircularFilletRationalSnapshot; 4],
        cylinders: &[CircularFilletCylindricalContactSnapshot; 4],
        plane_axes: [[f64; 3]; 2],
        precision: CircularFilletBoundaryNormalPrecision,
    ) -> Result<CircularFilletBoundaryNormalBounds> {
        if !precision.maximum_normal_gap.is_finite()
            || precision.maximum_normal_gap <= 0.
            || precision.maximum_normal_gap >= 1.
        {
            return Err(invalid());
        }
        let plane = plane_error(plane_axes)?;
        let frames: [f64; 4] = cylinders
            .iter()
            .map(|c| cylinder_error(c.normal_directions()))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid())?;
        let cylinders: [CircularFilletCylindricalContactBounds; 4] = cylinders
            .iter()
            .enumerate()
            .map(|(chart, c)| {
                self.certify_cylindrical_contact_snapshot(
                    chart as u8,
                    c,
                    precision.maximum_position_error,
                )
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid())?;
        let surfaces: [CircularFilletRationalSnapshotBounds; 4] = surfaces
            .iter()
            .enumerate()
            .map(|(chart, s)| {
                self.certify_rational_snapshot(
                    chart as u8,
                    s,
                    precision.maximum_position_error,
                    precision.maximum_surface_normal_error,
                    precision.max_leaves,
                )
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid())?;
        let sum = |x, y| (I::exact(x) + I::exact(y)).hi;
        let cylinder_normal =
            std::array::from_fn(|c| sum(frames[c], cylinders[c].angle_error_upper_rad));
        let side =
            std::array::from_fn(|c| sum(surfaces[c].normal.normal_error_upper, cylinder_normal[c]));
        let end = std::array::from_fn(|c| sum(surfaces[c].normal.normal_error_upper, plane));
        let seam = std::array::from_fn(|c| {
            sum(
                surfaces[c].normal.normal_error_upper,
                surfaces[(c + 1) % 4].normal.normal_error_upper,
            )
        });
        if side
            .iter()
            .chain(&end)
            .chain(&seam)
            .any(|x| !x.is_finite() || *x > precision.maximum_normal_gap)
        {
            return Err(invalid());
        }
        Ok(CircularFilletBoundaryNormalBounds {
            surfaces,
            cylindrical_lifts: cylinders,
            plane_frame_normal_error_upper: plane,
            cylinder_frame_normal_error_upper: frames,
            cylinder_normal_error_upper: cylinder_normal,
            side_normal_gap_upper: side,
            end_normal_gap_upper: end,
            seam_normal_gap_upper: seam,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn unit(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        let n = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let norm = n.iter().map(|x| x * x).sum::<f64>().sqrt();
        n.map(|x| x / norm)
    }
    #[test]
    fn independent_analytic_support_normals_include_all_frame_axes() {
        let axes = [[1., 2e-7, 1e-7], [3e-7, 1., -2e-7], [-1e-7, 2e-7, 1.]];
        let p = plane_error([axes[0], axes[1]]).unwrap();
        let n = unit(axes[0], axes[1]);
        assert!((n[0] * n[0] + n[1] * n[1] + (n[2] - 1.).powi(2)).sqrt() <= p);
        let c = cylinder_error(axes.map(|row| row.map(I::exact))).unwrap();
        for phi in [0., 0.031, 0.7, 1.9, 3.5, 5.6, 6.23] {
            let u =
                std::array::from_fn(|j| -f64::sin(phi) * axes[0][j] + f64::cos(phi) * axes[1][j]);
            let n = unit(u, axes[2]);
            let ideal = [f64::cos(phi), f64::sin(phi), 0.];
            assert!(
                n.iter()
                    .zip(ideal)
                    .map(|(x, y)| (x - y).powi(2))
                    .sum::<f64>()
                    .sqrt()
                    <= c
            );
        }
        assert!(plane_error([[0.; 3]; 2]).is_err());
        assert!(plane_error([[f64::MAX; 3]; 2]).is_err());
        assert!(plane_error([[f64::NAN; 3]; 2]).is_err());
        assert!(cylinder_error([[I::exact(0.); 3]; 3]).is_err());
        assert!(cylinder_error([[I::exact(f64::INFINITY); 3]; 3]).is_err());
    }
}
