//! ADR-090: read-only whole-span gaps between CURRENT quarter-seam definitions.
use crate::{circular_fillet_patch::norm_upper, interval::I, CircularFilletRationalSnapshot};
use opencad_core::{Length, OpenCadError, Result};

/// Immutable degree-2 Bernstein edge description. Creates no native geometry.
#[derive(Clone)]
pub struct CircularFilletSeamSnapshot {
    homogeneous_m: [[I; 3]; 4],
}
/// Complete corresponding-parameter [0,1] POSITION bounds, including endpoints.
/// No native floating evaluation, normals/G1 or whole-BREP certificate.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletSeamBounds {
    pub edge_to_left_gap_upper_m: f64,
    pub edge_to_right_gap_upper_m: f64,
    pub left_to_right_gap_upper_m: f64,
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("current quarter seam or requested gap not certified")
}
fn hull(c: [I; 3]) -> I {
    I {
        lo: c.iter().map(|x| x.lo).fold(f64::INFINITY, f64::min),
        hi: c.iter().map(|x| x.hi).fold(f64::NEG_INFINITY, f64::max),
    }
}
fn gap(a: [[I; 3]; 4], b: [[I; 3]; 4]) -> Result<f64> {
    let gauge = a[3][0] / b[3][0];
    if !gauge.valid() || gauge.lo <= 0. {
        return Err(invalid());
    }
    let b: [[I; 3]; 4] = b.map(|row| row.map(|x| x * gauge));
    if a.iter().chain(&b).flatten().any(|x| !x.valid()) {
        return Err(invalid());
    }
    let w = hull(a[3]);
    let v = hull(b[3]);
    if w.lo <= 0. || v.lo <= 0. {
        return Err(invalid());
    }
    // Coefficient differences retain cancellation BEFORE the whole-span hull.
    let delta: [I; 4] =
        std::array::from_fn(|axis| hull(std::array::from_fn(|j| b[axis][j] - a[axis][j])));
    let error = std::array::from_fn(|axis| (delta[axis] - (hull(a[axis]) / w) * delta[3]) / v);
    if error.iter().any(|x| !x.valid()) {
        return Err(invalid());
    }
    let bound = norm_upper(error);
    if !bound.is_finite() {
        return Err(invalid());
    }
    Ok(bound)
}
impl CircularFilletSeamSnapshot {
    /// Raw native millimetre poles and positive weights, fixed degree two.
    /// Capture must separately establish type, identity Location and [0,1] range.
    pub fn from_millimeter_controls(poles_mm: &[[f64; 3]; 3], weights: &[f64; 3]) -> Result<Self> {
        if poles_mm.iter().flatten().any(|x| !x.is_finite())
            || weights.iter().any(|x| !x.is_finite() || *x <= 0.)
        {
            return Err(invalid());
        }
        let homogeneous_m: [[I; 3]; 4] = std::array::from_fn(|axis| {
            std::array::from_fn(|j| {
                let w = I::exact(weights[j]);
                if axis == 3 {
                    w
                } else {
                    (I::exact(poles_mm[j][axis]) / I::exact(1000.)) * w
                }
            })
        });
        if homogeneous_m.iter().flatten().any(|x| !x.valid()) {
            return Err(invalid());
        }
        Ok(Self { homogeneous_m })
    }
    /// Directly compare edge, left surface u=1 and right surface u=0. Both
    /// attached lines must be qualified as (1,t)/(0,t) on adjacent charts by
    /// capture. All three complete gaps must meet finite positive SI precision.
    /// No external bound, sampling, geometry mutation or partial result.
    pub fn certify_boundary(
        &self,
        left: &CircularFilletRationalSnapshot,
        right: &CircularFilletRationalSnapshot,
        maximum_gap: Length,
    ) -> Result<CircularFilletSeamBounds> {
        let maximum = maximum_gap.meters();
        if !maximum.is_finite() || maximum <= 0. {
            return Err(invalid());
        }
        let l = left.seam_boundary(true);
        let r = right.seam_boundary(false);
        let gaps = [
            gap(self.homogeneous_m, l)?,
            gap(self.homogeneous_m, r)?,
            gap(l, r)?,
        ];
        if gaps.iter().any(|x| *x > maximum) {
            return Err(invalid());
        }
        Ok(CircularFilletSeamBounds {
            edge_to_left_gap_upper_m: gaps[0],
            edge_to_right_gap_upper_m: gaps[1],
            left_to_right_gap_upper_m: gaps[2],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_conversion_encloses_exact_fraction_before_weighting() {
        let s = CircularFilletSeamSnapshot::from_millimeter_controls(&[[0.125; 3]; 3], &[0.5; 3])
            .unwrap();
        // Exact homogeneous coordinate = .125/1000*.5 = 1/16000 m.
        let fraction = |x: f64| {
            let bits = x.to_bits();
            let exp = ((bits >> 52) & 2047) as i32 - 1023 - 52;
            assert!((-80..0).contains(&exp));
            (
                ((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52)) as i128,
                1_i128 << (-exp),
            )
        };
        let c = s.homogeneous_m[0][0];
        let (ln, ld) = fraction(c.lo);
        let (hn, hd) = fraction(c.hi);
        assert!(ln * 16000 <= ld && hd <= hn * 16000);
    }
}
