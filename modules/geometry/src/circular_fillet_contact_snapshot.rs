//! ADR-087: read-only native 3D contact definitions, never a body or editor.
use crate::{
    circular_fillet_patch::norm_upper, interval::I, CircularFilletContactKind,
    CircularFilletRationalNormalBounds, CircularFilletRationalSnapshot, CircularFilletReference,
};
use opencad_core::{Length, OpenCadError, Result};

/// Immutable single-span degree-2 side or degree-4 end definition. Raw native
/// inputs use millimetres; private homogeneous intervals use metres.
#[derive(Clone)]
pub struct CircularFilletContactSnapshot {
    kind: CircularFilletContactKind,
    homogeneous_m: [Vec<I>; 4],
}
/// Complete corresponding-parameter bound on the closed original [0,1] domain.
/// Does not certify native floating evaluation, derivatives or attached pcurves.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletContactSnapshotBounds {
    pub position_error_upper_m: f64,
}
/// ADR-088: immutable affine lift of an end-contact pcurve, including original
/// native frame and UV conversion uncertainty. Creates no native plane or curve.
#[derive(Clone)]
pub struct CircularFilletPlanarContactSnapshot {
    curve: CircularFilletContactSnapshot,
}
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletEndBoundaryPrecision {
    pub maximum_gap: Length,
    pub maximum_blend_normal_error: f64,
    pub max_leaves: usize,
}
/// Complete pairwise position bounds at corresponding t in [0,1]. The blend
/// boundary is B(t,1); actual attached line qualification belongs to capture.
/// Blend normal bounds are not a plane/blend normal or exact G1 certificate.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletEndBoundaryBounds {
    pub edge_position_error_upper_m: f64,
    pub plane_lift_position_error_upper_m: f64,
    pub blend_position_error_upper_m: f64,
    pub edge_to_plane_gap_upper_m: f64,
    pub edge_to_blend_gap_upper_m: f64,
    pub plane_to_blend_gap_upper_m: f64,
    pub blend_normal: CircularFilletRationalNormalBounds,
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("native contact definition or position precision not certified")
}
fn hull(coefficients: &[I]) -> I {
    I {
        lo: coefficients
            .iter()
            .map(|x| x.lo)
            .fold(f64::INFINITY, f64::min),
        hi: coefficients
            .iter()
            .map(|x| x.hi)
            .fold(f64::NEG_INFINITY, f64::max),
    }
}
impl CircularFilletPlanarContactSnapshot {
    /// Capture a fixed clamped degree-4 pcurve and actual affine support. All
    /// lengths are ORIGINAL mm; X/Y directions are dimensionless. Separately
    /// establish native plane type, identity Location, degree/knots/range and
    /// nonperiodicity. Constructor acceptance is not geometric admission.
    pub fn from_millimeter_controls(
        origin_mm: [f64; 3],
        x_direction: [f64; 3],
        y_direction: [f64; 3],
        poles_mm: &[[f64; 2]; 5],
        weights: &[f64; 5],
    ) -> Result<Self> {
        if origin_mm
            .iter()
            .chain(&x_direction)
            .chain(&y_direction)
            .any(|x| !x.is_finite())
            || poles_mm.iter().flatten().any(|x| !x.is_finite())
            || weights.iter().any(|x| !x.is_finite() || *x <= 0.)
        {
            return Err(invalid());
        }
        let homogeneous_m: [Vec<I>; 4] = std::array::from_fn(|axis| {
            (0..5)
                .map(|i| {
                    let w = I::exact(weights[i]);
                    if axis == 3 {
                        w
                    } else {
                        let si = |x| I::exact(x) / I::exact(1000.);
                        (si(origin_mm[axis])
                            + si(poles_mm[i][0]) * I::exact(x_direction[axis])
                            + si(poles_mm[i][1]) * I::exact(y_direction[axis]))
                            * w
                    }
                })
                .collect()
        });
        if homogeneous_m.iter().flatten().any(|x| !x.valid()) {
            return Err(invalid());
        }
        Ok(Self {
            curve: CircularFilletContactSnapshot {
                kind: CircularFilletContactKind::End,
                homogeneous_m,
            },
        })
    }
}
impl CircularFilletContactSnapshot {
    pub(crate) fn kind(&self) -> CircularFilletContactKind {
        self.kind
    }
    /// Fixed clamped [0,1] Bernstein representation. The caller must establish
    /// the actual native degree/knots/multiplicities, nonperiodicity and identity
    /// Location separately. Constructor acceptance is not geometric admission.
    pub fn from_millimeter_controls(
        kind: CircularFilletContactKind,
        poles_mm: &[[f64; 3]],
        weights: &[f64],
    ) -> Result<Self> {
        let count = match kind {
            CircularFilletContactKind::Side => 3,
            CircularFilletContactKind::End => 5,
        };
        if poles_mm.len() != count
            || weights.len() != count
            || poles_mm.iter().flatten().any(|x| !x.is_finite())
            || weights.iter().any(|w| !w.is_finite() || *w <= 0.)
        {
            return Err(invalid());
        }
        let homogeneous_m: [Vec<I>; 4] = std::array::from_fn(|axis| {
            weights
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    if axis == 3 {
                        I::exact(*w)
                    } else {
                        (I::exact(poles_mm[i][axis]) / I::exact(1000.)) * I::exact(*w)
                    }
                })
                .collect()
        });
        if homogeneous_m.iter().flatten().any(|x| !x.valid()) {
            return Err(invalid());
        }
        Ok(Self {
            kind,
            homogeneous_m,
        })
    }
}
impl CircularFilletReference {
    /// Recompute whole edge, affine plane-lift and blend-definition bounds.
    /// Their ideal end contacts agree; outward pairwise sums therefore bound
    /// the entire corresponding-parameter boundary. No external certificates,
    /// samples, native mutation or partial result. Side contacts are refused.
    pub fn certify_end_boundary(
        self,
        chart: u8,
        edge: &CircularFilletContactSnapshot,
        plane: &CircularFilletPlanarContactSnapshot,
        blend: &CircularFilletRationalSnapshot,
        precision: CircularFilletEndBoundaryPrecision,
    ) -> Result<CircularFilletEndBoundaryBounds> {
        if edge.kind != CircularFilletContactKind::End {
            return Err(invalid());
        }
        let e = self
            .certify_contact_snapshot(chart, edge, precision.maximum_gap)?
            .position_error_upper_m;
        let p = self
            .certify_contact_snapshot(chart, &plane.curve, precision.maximum_gap)?
            .position_error_upper_m;
        let b = self.certify_rational_snapshot(
            chart,
            blend,
            precision.maximum_gap,
            precision.maximum_blend_normal_error,
            precision.max_leaves,
        )?;
        let sum = |x, y| (I::exact(x) + I::exact(y)).hi;
        let gaps = [
            sum(e, p),
            sum(e, b.position_error_upper_m),
            sum(p, b.position_error_upper_m),
        ];
        if gaps
            .iter()
            .any(|x| !x.is_finite() || *x > precision.maximum_gap.meters())
        {
            return Err(invalid());
        }
        Ok(CircularFilletEndBoundaryBounds {
            edge_position_error_upper_m: e,
            plane_lift_position_error_upper_m: p,
            blend_position_error_upper_m: b.position_error_upper_m,
            edge_to_plane_gap_upper_m: gaps[0],
            edge_to_blend_gap_upper_m: gaps[1],
            plane_to_blend_gap_upper_m: gaps[2],
            blend_normal: b.normal,
        })
    }
    /// Enclose the entire native 3D curve against the ideal contact on the given
    /// quarter chart, with outward SI conversion and positive common gauge.
    /// Fixed at most five controls; no samples, subdivision or partial result.
    pub fn certify_contact_snapshot(
        self,
        chart: u8,
        snapshot: &CircularFilletContactSnapshot,
        maximum_position_error: Length,
    ) -> Result<CircularFilletContactSnapshotBounds> {
        let maximum = maximum_position_error.meters();
        if !maximum.is_finite() || maximum <= 0. {
            return Err(invalid());
        }
        let reference = self.rational_contact_curve(snapshot.kind, chart)?;
        let supplied: [Vec<I>; 4] = std::array::from_fn(|axis| {
            reference
                .weights()
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    if axis == 3 {
                        I::exact(*w)
                    } else {
                        I::exact(reference.control_points_m()[i][axis]) * I::exact(*w)
                    }
                })
                .collect()
        });
        let gauge = supplied[3][0] / snapshot.homogeneous_m[3][0];
        if !gauge.valid() || gauge.lo <= 0. {
            return Err(invalid());
        }
        let actual: [Vec<I>; 4] = std::array::from_fn(|axis| {
            snapshot.homogeneous_m[axis]
                .iter()
                .map(|x| *x * gauge)
                .collect()
        });
        if supplied.iter().chain(&actual).flatten().any(|x| !x.valid()) {
            return Err(invalid());
        }
        let w = hull(&supplied[3]);
        let v = hull(&actual[3]);
        if w.lo <= 0. || v.lo <= 0. {
            return Err(invalid());
        }
        // Form coefficient differences BEFORE hulls: this preserves cancellation
        // and bounds the complete Bernstein polynomial, including endpoints.
        let delta: [I; 4] = std::array::from_fn(|axis| {
            hull(
                &actual[axis]
                    .iter()
                    .zip(&supplied[axis])
                    .map(|(b, a)| *b - *a)
                    .collect::<Vec<_>>(),
            )
        });
        let error: [I; 3] =
            std::array::from_fn(|axis| (delta[axis] - (hull(&supplied[axis]) / w) * delta[3]) / v);
        if error.iter().any(|x| !x.valid()) {
            return Err(invalid());
        }
        let bound = (I::exact(norm_upper(error)) + I::exact(reference.position_error_upper_m())).hi;
        if !bound.is_finite() || bound > maximum {
            return Err(invalid());
        }
        Ok(CircularFilletContactSnapshotBounds {
            position_error_upper_m: bound,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn affine_lift_encloses_exact_fraction_without_rounded_point_intermediate() {
        // Exact x: (.125 + .25*.5 + .375*.25)/1000 = 11/32000 m.
        let snapshot = CircularFilletPlanarContactSnapshot::from_millimeter_controls(
            [0.125, 0., 0.],
            [0.5, 1., 0.],
            [0.25, 0., 1.],
            &[[0.25, 0.375]; 5],
            &[1.; 5],
        )
        .unwrap();
        let c = snapshot.curve.homogeneous_m[0][0];
        let fraction = |x: f64| {
            let bits = x.to_bits();
            let exp = ((bits >> 52) & 2047) as i32 - 1023 - 52;
            assert!((-70..0).contains(&exp));
            (
                ((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52)) as i128,
                1_i128 << (-exp),
            )
        };
        let (ln, ld) = fraction(c.lo);
        let (hn, hd) = fraction(c.hi);
        assert!(ln * 32000 <= 11 * ld && 11 * hd <= hn * 32000);
        assert!(
            CircularFilletPlanarContactSnapshot::from_millimeter_controls(
                [0.; 3],
                [f64::MAX; 3],
                [0.; 3],
                &[[f64::MAX, 0.]; 5],
                &[f64::MAX; 5]
            )
            .is_err()
        );
    }
    #[test]
    fn raw_si_interval_encloses_exact_binary64_fraction() {
        // 0.1 = 3602879701896397 / 2^55 exactly. Cross products below are
        // integer rational comparisons, independent of interval arithmetic.
        let raw = 0.1_f64;
        let snapshot = CircularFilletContactSnapshot::from_millimeter_controls(
            CircularFilletContactKind::Side,
            &[[raw; 3]; 3],
            &[1.; 3],
        )
        .unwrap();
        let interval = snapshot.homogeneous_m[0][0];
        let fraction = |x: f64| {
            let bits = x.to_bits();
            let exponent = ((bits >> 52) & 2047) as i32 - 1023 - 52;
            assert!((-70..0).contains(&exponent));
            (
                ((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52)) as i128,
                1_i128 << (-exponent),
            )
        };
        let (n, d) = fraction(raw);
        let (ln, ld) = fraction(interval.lo);
        let (hn, hd) = fraction(interval.hi);
        assert!(ln * (d * 1000) <= n * ld && n * hd <= hn * (d * 1000));
    }
}
