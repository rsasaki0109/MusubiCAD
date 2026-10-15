//! ADR-077: whole-patch normal certificates, not G1 or solid admission.
use crate::{
    circular_fillet_patch::norm_upper, interval::I, CircularFilletBezierPatch,
    CircularFilletReference,
};
use opencad_core::{OpenCadError, Result};

/// Bounds for the mathematical surface of the current poles. The area is for
/// normalized local dimensionless parameters; normal error is chord distance.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletNormalBounds {
    pub normal_lower: [f64; 3],
    pub normal_upper: [f64; 3],
    pub jacobian_area_lower_m2: f64,
    pub jacobian_area_upper_m2: f64,
    pub ideal_normal_alignment_lower: f64,
    pub normal_error_upper: f64,
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("Bezier reference normal regularity or orientation not certified")
}
fn hull(values: impl Iterator<Item = [I; 3]>) -> Result<[I; 3]> {
    let mut result = [I {
        lo: f64::INFINITY,
        hi: f64::NEG_INFINITY,
    }; 3];
    for v in values {
        if !v.into_iter().all(I::valid) {
            return Err(invalid());
        }
        for i in 0..3 {
            result[i].lo = result[i].lo.min(v[i].lo);
            result[i].hi = result[i].hi.max(v[i].hi);
        }
    }
    if !result.into_iter().all(I::valid) {
        return Err(invalid());
    }
    Ok(result)
}

impl CircularFilletBezierPatch {
    /// Recompute from current poles, ignoring all stored positional metadata.
    /// A positive certificate proves local immersion and matching orientation,
    /// not global embedding, position error, G1 continuity or solid admission.
    pub fn normal_bounds(
        self,
        reference: CircularFilletReference,
        chart: u8,
        u0: f64,
        u1: f64,
        v0: f64,
        v1: f64,
    ) -> Result<CircularFilletNormalBounds> {
        let ideal = reference.patch_bounds(chart, u0, u1, v0, v1)?;
        if !self
            .control_points_m
            .into_iter()
            .flatten()
            .flatten()
            .all(f64::is_finite)
        {
            return Err(invalid());
        }
        let p = self.control_points_m;
        let du = hull((0..3).flat_map(|i| {
            (0..4).map(move |j| {
                std::array::from_fn(|k| {
                    I::exact(3.) * (I::exact(p[i + 1][j][k]) - I::exact(p[i][j][k]))
                })
            })
        }))?;
        let dv = hull((0..4).flat_map(|i| {
            (0..3).map(move |j| {
                std::array::from_fn(|k| {
                    I::exact(3.) * (I::exact(p[i][j + 1][k]) - I::exact(p[i][j][k]))
                })
            })
        }))?;
        let cross: [I; 3] = std::array::from_fn(|i| {
            let j = (i + 1) % 3;
            let k = (i + 2) % 3;
            du[j] * dv[k] - du[k] * dv[j]
        });
        if !cross.into_iter().all(I::valid) {
            return Err(invalid());
        }
        let mut squared = cross.into_iter().fold(I::exact(0.), |s, x| s + x.square());
        squared.lo = squared.lo.max(0.);
        let area = squared.sqrt();
        if !area.valid() || area.lo <= 0. {
            return Err(invalid());
        }
        let normal = cross.map(|x| x / area);
        let expected: [I; 3] = std::array::from_fn(|i| I {
            lo: ideal.normal_lower[i],
            hi: ideal.normal_upper[i],
        });
        let alignment = (0..3).fold(I::exact(0.), |s, i| s + normal[i] * expected[i]);
        let difference = std::array::from_fn(|i| normal[i] - expected[i]);
        if !normal.into_iter().chain(difference).all(I::valid)
            || !alignment.valid()
            || alignment.lo <= 0.
        {
            return Err(invalid());
        }
        let error = norm_upper(difference);
        if !error.is_finite() {
            return Err(invalid());
        }
        Ok(CircularFilletNormalBounds {
            normal_lower: normal.map(|x| x.lo),
            normal_upper: normal.map(|x| x.hi),
            jacobian_area_lower_m2: area.lo,
            jacobian_area_upper_m2: area.hi,
            ideal_normal_alignment_lower: alignment.lo,
            normal_error_upper: error,
        })
    }
}
