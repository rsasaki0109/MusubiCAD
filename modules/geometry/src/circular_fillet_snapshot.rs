//! ADR-086: read-only current-native surface definition, never a body/editor.
use crate::{
    bernstein::P, circular_fillet_rational_normals::certify_homogeneous, interval::I,
    CircularFilletRationalNormalBounds, CircularFilletReference,
};
use opencad_core::{Length, OpenCadError, Result};

/// Immutable descriptive degree-(8,2) snapshot. Native inputs are explicitly mm;
/// private homogeneous intervals use metres. This DTO creates no native surface.
#[derive(Clone)]
pub struct CircularFilletRationalSnapshot {
    homogeneous_m: [P; 4],
}
/// Bounds for the complete captured surface definition on original [0,1]^2.
/// Native floating evaluation, trimming, global embedding and G1 are not proved.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletRationalSnapshotBounds {
    pub position_error_upper_m: f64,
    pub normal: CircularFilletRationalNormalBounds,
}
impl CircularFilletRationalSnapshot {
    // Closed u-boundary in the ORIGINAL transverse parameter; no rounded lift.
    pub(crate) fn seam_boundary(&self, end: bool) -> [[I; 3]; 4] {
        std::array::from_fn(|axis| {
            std::array::from_fn(|j| self.homogeneous_m[axis].at(if end { 8 } else { 0 }, j))
        })
    }

    /// Read raw binary64 native poles before rounded SI export. Enclose the
    /// exact physical coordinate q_mm/1000, including conversion roundoff.
    pub fn from_millimeter_controls(
        poles_mm: &[[[f64; 3]; 3]; 9],
        weights: &[[f64; 3]; 9],
    ) -> Result<Self> {
        if poles_mm.iter().flatten().flatten().any(|x| !x.is_finite())
            || weights.iter().flatten().any(|x| !x.is_finite() || *x <= 0.)
        {
            return Err(OpenCadError::validation(
                "invalid fixed-layout rational snapshot controls",
            ));
        }
        let homogeneous_m: [P; 4] = std::array::from_fn(|axis| P {
            u: 8,
            v: 2,
            c: (0..9)
                .flat_map(|i| {
                    (0..3).map(move |j| {
                        let w = I::exact(weights[i][j]);
                        if axis == 3 {
                            w
                        } else {
                            (I::exact(poles_mm[i][j][axis]) / I::exact(1000.)) * w
                        }
                    })
                })
                .collect(),
        });
        if homogeneous_m.iter().flat_map(|p| &p.c).any(|c| !c.valid()) {
            return Err(OpenCadError::validation(
                "native snapshot conversion overflow",
            ));
        }
        Ok(Self { homogeneous_m })
    }
}
impl CircularFilletReference {
    /// Read-only proof against a validated reference and quarter chart. Position
    /// uses finite positive SI Length, normal chord error is dimensionless in
    /// (0,1), work 1..=16384 leaves. No partial certificate or geometry mutation.
    pub fn certify_rational_snapshot(
        self,
        chart: u8,
        snapshot: &CircularFilletRationalSnapshot,
        maximum_position_error: Length,
        maximum_normal_error: f64,
        max_leaves: usize,
    ) -> Result<CircularFilletRationalSnapshotBounds> {
        let maximum_position_error_m = maximum_position_error.meters();
        if !maximum_position_error_m.is_finite() || maximum_position_error_m <= 0. {
            return Err(OpenCadError::validation(
                "snapshot position precision must be finite positive metres",
            ));
        }
        certify_homogeneous(
            self,
            chart,
            snapshot.homogeneous_m.clone(),
            maximum_position_error_m,
            maximum_normal_error,
            max_leaves,
            true,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Exact binary64 fractions for these moderate nonzero fixtures; no interval
    // operations or decimal conversion are used by this independent oracle.
    fn fraction(x: f64) -> (i128, i128) {
        let bits = x.to_bits();
        let exponent = ((bits >> 52) & 2047) as i32 - 1023 - 52;
        assert!((-70..=0).contains(&exponent));
        let sign = if bits >> 63 == 0 { 1 } else { -1 };
        (
            sign * (((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52)) as i128),
            1_i128 << (-exponent),
        )
    }
    #[test]
    fn native_coordinate_intervals_contain_exact_fractions_and_overflow_refuses() {
        let raw = [-123.456, 0.1, 17.875];
        let snapshot =
            CircularFilletRationalSnapshot::from_millimeter_controls(&[[raw; 3]; 9], &[[1.; 3]; 9])
                .unwrap();
        for (axis, value) in raw.into_iter().enumerate() {
            let interval = snapshot.homogeneous_m[axis].at(0, 0);
            let (n, d) = fraction(value);
            let (ln, ld) = fraction(interval.lo);
            let (hn, hd) = fraction(interval.hi);
            assert!(ln * (d * 1000) <= n * ld && n * hd <= hn * (d * 1000));
        }
        assert!(CircularFilletRationalSnapshot::from_millimeter_controls(
            &[[[f64::MAX; 3]; 3]; 9],
            &[[f64::MAX; 3]; 9]
        )
        .is_err());
    }
}
