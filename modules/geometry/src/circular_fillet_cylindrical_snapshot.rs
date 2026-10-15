//! ADR-089: current cylindrical pcurve definitions, never a body or editor.
use crate::{
    circular_fillet_patch::norm_upper, interval::I, CircularFilletContactKind,
    CircularFilletContactSnapshot, CircularFilletRationalNormalBounds,
    CircularFilletRationalSnapshot, CircularFilletReference,
};
use opencad_core::{Length, OpenCadError, Result};

/// Immutable descriptive nonrational degree-3, 256-span cylindrical pcurve lift.
/// Private intervals retain original native length conversion uncertainty.
#[derive(Clone)]
pub struct CircularFilletCylindricalContactSnapshot {
    origin_m: [I; 3],
    directions: [[I; 3]; 3],
    radius_m: I,
    poles_rad_m: Vec<[I; 2]>,
}
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletCylindricalContactBounds {
    pub position_error_upper_m: f64,
    pub angle_error_upper_rad: f64,
    pub height_error_upper_m: f64,
    pub support_frame_error_upper_m: f64,
}
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletSideBoundaryPrecision {
    pub maximum_gap: Length,
    pub maximum_blend_normal_error: f64,
    pub max_leaves: usize,
}
/// Complete position gaps for E(t), cylindrical lift P(t) and B(t,0).
/// Blend normal evidence is surface-only, not paired normal/G1 agreement.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletSideBoundaryBounds {
    pub edge_position_error_upper_m: f64,
    pub cylindrical_lift: CircularFilletCylindricalContactBounds,
    pub blend_position_error_upper_m: f64,
    pub edge_to_cylinder_gap_upper_m: f64,
    pub edge_to_blend_gap_upper_m: f64,
    pub cylinder_to_blend_gap_upper_m: f64,
    pub blend_normal: CircularFilletRationalNormalBounds,
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("cylindrical contact definition or boundary precision not certified")
}
fn magnitude(x: I) -> f64 {
    x.lo.abs().max(x.hi.abs())
}
impl CircularFilletCylindricalContactSnapshot {
    pub(crate) fn normal_directions(&self) -> [[I; 3]; 3] {
        self.directions
    }

    /// Original origin/radius/height use mm; directions are dimensionless,
    /// pcurve controls are [angle_rad,height_mm]. Exactly 769 controls. Caller
    /// separately establishes current cylinder type/identity Location and the
    /// fixed clamped unit-weight degree-3 basis with knots j/256 and unit range.
    /// Constructor acceptance is not geometric or production admission.
    pub fn from_millimeter_controls(
        origin_mm: [f64; 3],
        directions: [[f64; 3]; 3],
        radius_mm: f64,
        poles_rad_mm: &[[f64; 2]],
    ) -> Result<Self> {
        if poles_rad_mm.len() != 769
            || origin_mm
                .iter()
                .chain(directions.iter().flatten())
                .any(|x| !x.is_finite())
            || !radius_mm.is_finite()
            || radius_mm <= 0.
            || poles_rad_mm.iter().flatten().any(|x| !x.is_finite())
        {
            return Err(invalid());
        }
        let si = |x| I::exact(x) / I::exact(1000.);
        let out = Self {
            origin_m: origin_mm.map(si),
            directions: directions.map(|d| d.map(I::exact)),
            radius_m: si(radius_mm),
            poles_rad_m: poles_rad_mm
                .iter()
                .map(|p| [I::exact(p[0]), si(p[1])])
                .collect(),
        };
        if out
            .origin_m
            .iter()
            .chain(out.poles_rad_m.iter().flatten())
            .any(|x| !x.valid())
            || !out.radius_m.valid()
            || out.radius_m.lo <= 0.
        {
            return Err(invalid());
        }
        Ok(out)
    }
}
impl CircularFilletReference {
    /// Whole fixed-basis cylindrical lift bound against the ideal side contact.
    /// Uses control-difference convex hulls, supplied analytic Hermite remainder,
    /// global circle Lipschitz and actual frame/radius bounds; no samples/refining.
    pub fn certify_cylindrical_contact_snapshot(
        self,
        chart: u8,
        snapshot: &CircularFilletCylindricalContactSnapshot,
        maximum_position_error: Length,
    ) -> Result<CircularFilletCylindricalContactBounds> {
        let maximum = maximum_position_error.meters();
        if !maximum.is_finite() || maximum <= 0. {
            return Err(invalid());
        }
        let reference = self.cylindrical_contact_pcurve(chart)?;
        let mut delta = [0_f64; 2];
        let mut zmax = 0_f64;
        for (actual, supplied) in snapshot
            .poles_rad_m
            .iter()
            .zip(reference.control_points_rad_m())
        {
            for axis in 0..2 {
                let d = actual[axis] - I::exact(supplied[axis]);
                if !d.valid() {
                    return Err(invalid());
                }
                delta[axis] = delta[axis].max(magnitude(d));
            }
            zmax = zmax.max(magnitude(actual[1]));
        }
        let angle = I::exact(reference.angle_error_upper_rad()) + I::exact(delta[0]);
        let height = I::exact(reference.height_error_upper_m()) + I::exact(delta[1]);
        let deviations: [f64; 3] = std::array::from_fn(|axis| {
            norm_upper(std::array::from_fn(|coordinate| {
                snapshot.directions[axis][coordinate]
                    - I::exact(if axis == coordinate { 1. } else { 0. })
            }))
        });
        let frame = I::exact(norm_upper(snapshot.origin_m))
            + I::exact(snapshot.radius_m.hi) * (I::exact(deviations[0]) + I::exact(deviations[1]))
            + I::exact(zmax) * I::exact(deviations[2]);
        let radius = I::exact(self.cylinder_radius().meters());
        let difference = snapshot.radius_m - radius;
        if !difference.valid() || !angle.valid() || !height.valid() || !frame.valid() {
            return Err(invalid());
        }
        let horizontal = I::exact(magnitude(difference)) + radius * angle;
        let positional = frame + I::exact(norm_upper([horizontal, height, I::exact(0.)]));
        if !positional.valid() || positional.hi > maximum {
            return Err(invalid());
        }
        Ok(CircularFilletCylindricalContactBounds {
            position_error_upper_m: positional.hi,
            angle_error_upper_rad: angle.hi,
            height_error_upper_m: height.hi,
            support_frame_error_upper_m: frame.hi,
        })
    }
    /// Recompute all three current definitions against the same ideal side
    /// contact, then require every outward pairwise sum to meet the SI budget.
    /// Actual attached blend-line qualification as (t,0) is a capture obligation.
    pub fn certify_side_boundary(
        self,
        chart: u8,
        edge: &CircularFilletContactSnapshot,
        cylinder: &CircularFilletCylindricalContactSnapshot,
        blend: &CircularFilletRationalSnapshot,
        precision: CircularFilletSideBoundaryPrecision,
    ) -> Result<CircularFilletSideBoundaryBounds> {
        if edge.kind() != CircularFilletContactKind::Side {
            return Err(invalid());
        }
        let e = self
            .certify_contact_snapshot(chart, edge, precision.maximum_gap)?
            .position_error_upper_m;
        let p =
            self.certify_cylindrical_contact_snapshot(chart, cylinder, precision.maximum_gap)?;
        let b = self.certify_rational_snapshot(
            chart,
            blend,
            precision.maximum_gap,
            precision.maximum_blend_normal_error,
            precision.max_leaves,
        )?;
        let sum = |a, c| (I::exact(a) + I::exact(c)).hi;
        let gaps = [
            sum(e, p.position_error_upper_m),
            sum(e, b.position_error_upper_m),
            sum(p.position_error_upper_m, b.position_error_upper_m),
        ];
        if gaps
            .iter()
            .any(|g| !g.is_finite() || *g > precision.maximum_gap.meters())
        {
            return Err(invalid());
        }
        Ok(CircularFilletSideBoundaryBounds {
            edge_position_error_upper_m: e,
            cylindrical_lift: p,
            blend_position_error_upper_m: b.position_error_upper_m,
            edge_to_cylinder_gap_upper_m: gaps[0],
            edge_to_blend_gap_upper_m: gaps[1],
            cylinder_to_blend_gap_upper_m: gaps[2],
            blend_normal: b.normal,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_height_conversion_encloses_exact_binary64_fraction() {
        let snapshot = CircularFilletCylindricalContactSnapshot::from_millimeter_controls(
            [0.; 3],
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            10.,
            &[[0., 0.1]; 769],
        )
        .unwrap();
        let interval = snapshot.poles_rad_m[0][1];
        let fraction = |x: f64| {
            let bits = x.to_bits();
            let exp = ((bits >> 52) & 2047) as i32 - 1023 - 52;
            assert!((-70..0).contains(&exp));
            (
                ((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52)) as i128,
                1_i128 << (-exp),
            )
        };
        let (n, d) = fraction(0.1);
        let (ln, ld) = fraction(interval.lo);
        let (hn, hd) = fraction(interval.hi);
        assert!(ln * (d * 1000) <= n * ld && n * hd <= hn * (d * 1000));
    }
}
