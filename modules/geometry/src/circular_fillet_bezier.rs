//! ADR-076: transient bicubic reference approximation, never a kernel solid.
use crate::{
    circular_fillet_patch::{corner_derivatives, norm_upper},
    interval::I,
    CircularFilletReference,
};
use opencad_core::{OpenCadError, Result};

/// Mathematical bicubic Bezier surface on normalized local [0,1]².
/// Poles are indexed [u][v][xyz] in metres. No continuity/solid admission.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletBezierPatch {
    pub control_points_m: [[[f64; 3]; 4]; 4],
    pub reference_to_bilinear_error_upper_m: f64,
    pub control_net_deviation_upper_m: f64,
    pub position_error_upper_m: f64,
}

fn invalid() -> OpenCadError {
    OpenCadError::validation("periodic Bezier reference patch numerical enclosure failed")
}
fn midpoint(value: I) -> Result<f64> {
    if !value.valid() {
        return Err(invalid());
    }
    let x = (value.lo + (value.hi - value.lo) / 2.)
        .max(value.lo)
        .min(value.hi);
    if !x.is_finite() {
        return Err(invalid());
    }
    Ok(x)
}

impl CircularFilletReference {
    /// Construct ADR-076's bounded reference approximation. Domain matches
    /// `patch_bounds`; the returned surface uses normalized local coordinates.
    pub fn bezier_patch(
        self,
        chart: u8,
        u0: f64,
        u1: f64,
        v0: f64,
        v1: f64,
    ) -> Result<CircularFilletBezierPatch> {
        let bounds = self.patch_bounds(chart, u0, u1, v0, v1)?;
        let hu = I::exact(u1) - I::exact(u0);
        let hv = I::exact(v1) - I::exact(v0);
        let mut poles = [[[0.; 3]; 4]; 4];
        for (corner, (i, j, u, v, su, sv)) in [
            (0, 0, u0, v0, 1., 1.),
            (3, 0, u1, v0, -1., 1.),
            (3, 3, u1, v1, -1., -1.),
            (0, 3, u0, v1, 1., -1.),
        ]
        .into_iter()
        .enumerate()
        {
            let [du, dv, duv] = corner_derivatives(self, chart, u, v)?;
            let ku = I::exact(su) * hu / I::exact(3.);
            let kv = I::exact(sv) * hv / I::exact(3.);
            let inner_i = if i == 0 { 1 } else { 2 };
            let inner_j = if j == 0 { 1 } else { 2 };
            poles[i][j] = bounds.corner_points_m[corner];
            for axis in 0..3 {
                let p = I::exact(poles[i][j][axis]);
                poles[inner_i][j][axis] = midpoint(p + du[axis] * ku)?;
                poles[i][inner_j][axis] = midpoint(p + dv[axis] * kv)?;
                poles[inner_i][inner_j][axis] =
                    midpoint(p + du[axis] * ku + dv[axis] * kv + duv[axis] * ku * kv)?;
            }
        }
        // Degree elevation of the exact bilinear surface of rounded corners.
        let one = I::exact(1.);
        let mut deviation = 0_f64;
        for (i, row) in poles.iter().enumerate() {
            for (j, pole) in row.iter().enumerate() {
                let u = I::exact(i as f64) / I::exact(3.);
                let v = I::exact(j as f64) / I::exact(3.);
                let weights = [(one - u) * (one - v), u * (one - v), u * v, (one - u) * v];
                let difference = std::array::from_fn(|axis| {
                    let q = weights
                        .into_iter()
                        .zip(bounds.corner_points_m)
                        .fold(I::exact(0.), |sum, (w, p)| sum + w * I::exact(p[axis]));
                    I::exact(pole[axis]) - q
                });
                if !difference.into_iter().all(I::valid) {
                    return Err(invalid());
                }
                deviation = deviation.max(norm_upper(difference));
            }
        }
        let error = (I::exact(bounds.linear_interpolation_error_upper_m) + I::exact(deviation)).hi;
        if !deviation.is_finite() || !error.is_finite() {
            return Err(invalid());
        }
        Ok(CircularFilletBezierPatch {
            control_points_m: poles,
            reference_to_bilinear_error_upper_m: bounds.linear_interpolation_error_upper_m,
            control_net_deviation_upper_m: deviation,
            position_error_upper_m: error,
        })
    }
}
