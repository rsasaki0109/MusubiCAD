//! ADR-078: complete conforming periodic reference band, not a kernel body.
use crate::{CircularFilletBezierPatch, CircularFilletNormalBounds, CircularFilletReference};
use opencad_core::{Length, OpenCadError, Result};

/// A current generated cell; editing any field invalidates its certificates.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletBandCell {
    pub chart: u8,
    /// Dimensionless [u_min,u_max,v_min,v_max] in the rational chart.
    pub chart_domain: [f64; 4],
    pub patch: CircularFilletBezierPatch,
    pub normals: CircularFilletNormalBounds,
}
/// Ordered chart/u/v cover of the entire periodic reference band.
#[derive(Debug, Clone)]
pub struct CircularFilletBezierBand {
    pub divisions_per_chart_axis: usize,
    pub cells: Vec<CircularFilletBandCell>,
    pub position_error_upper_m: f64,
    pub normal_error_upper: f64,
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("periodic Bezier band quality or patch budget not certified")
}
fn level(
    reference: CircularFilletReference,
    n: usize,
    count: usize,
    position: f64,
    normal: f64,
) -> Result<CircularFilletBezierBand> {
    let mut result = CircularFilletBezierBand {
        divisions_per_chart_axis: n,
        cells: Vec::with_capacity(count),
        position_error_upper_m: 0.,
        normal_error_upper: 0.,
    };
    for chart in 0..4 {
        for i in 0..n {
            for j in 0..n {
                // n is dyadic and <=64, so these authored parameter endpoints are exact.
                let [u0, u1, v0, v1] = [
                    i as f64 / n as f64,
                    (i + 1) as f64 / n as f64,
                    j as f64 / n as f64,
                    (j + 1) as f64 / n as f64,
                ];
                let patch = reference.bezier_patch(chart, u0, u1, v0, v1)?;
                let normals = patch.normal_bounds(reference, chart, u0, u1, v0, v1)?;
                if patch.position_error_upper_m > position || normals.normal_error_upper > normal {
                    return Err(invalid());
                }
                result.position_error_upper_m = result
                    .position_error_upper_m
                    .max(patch.position_error_upper_m);
                result.normal_error_upper =
                    result.normal_error_upper.max(normals.normal_error_upper);
                result.cells.push(CircularFilletBandCell {
                    chart,
                    chart_domain: [u0, u1, v0, v1],
                    patch,
                    normals,
                });
            }
        }
    }
    Ok(result)
}
impl CircularFilletReference {
    /// ADR-078: refine a uniform dyadic cover until all cells meet explicit
    /// position (metres) and normal chord-distance bounds, or refuse at budget.
    /// No partial cover, native body, G1 or manufacturing tolerance is implied.
    pub fn bezier_band(
        self,
        position_error: Length,
        normal_error: f64,
        max_patches: usize,
    ) -> Result<CircularFilletBezierBand> {
        let position = position_error.meters();
        if !position.is_finite()
            || position <= 0.
            || !normal_error.is_finite()
            || normal_error <= 0.
            || normal_error > 2.
            || !(4..=16384).contains(&max_patches)
        {
            return Err(invalid());
        }
        let mut n = 1_usize;
        loop {
            let count = 4_usize
                .checked_mul(n)
                .and_then(|v| v.checked_mul(n))
                .ok_or_else(invalid)?;
            if count > max_patches {
                return Err(invalid());
            }
            if let Ok(band) = level(self, n, count, position, normal_error) {
                return Ok(band);
            }
            // A coarse interval refusal never supplies acceptance by samples.
            n = n.checked_mul(2).ok_or_else(invalid)?;
        }
    }
}
