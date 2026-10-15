//! ADR-085: complete rounded rational-chart normal certificates, not native G1.
use crate::{
    bernstein::P, circular_fillet_patch::norm_upper, circular_fillet_rational::homogeneous,
    interval::I, CircularFilletRationalPatch, CircularFilletRationalSnapshotBounds,
    CircularFilletReference,
};
use opencad_core::{OpenCadError, Result};

/// Whole closed unit-square bounds for the actual mathematical control data.
/// Derivatives use the original dimensionless u,v, even after restriction.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletRationalNormalBounds {
    pub jacobian_area_lower_m2: f64,
    pub jacobian_area_upper_m2: f64,
    pub ideal_normal_alignment_lower: f64,
    pub normal_error_upper: f64,
    pub du_error_upper_m: f64,
    pub dv_error_upper_m: f64,
    pub leaves: usize,
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("rounded rational normal precision, regularity or work not certified")
}
fn hull(p: &P) -> Result<I> {
    if p.c.is_empty() || p.c.iter().any(|x| !x.valid()) {
        return Err(invalid());
    }
    Ok(I {
        lo: p.c.iter().map(|x| x.lo).fold(f64::INFINITY, f64::min),
        hi: p.c.iter().map(|x| x.hi).fold(f64::NEG_INFINITY, f64::max),
    })
}
fn magnitude(x: I) -> I {
    I::exact(x.lo.abs().max(x.hi.abs()))
}
fn cross(a: [I; 3], b: [I; 3]) -> [I; 3] {
    std::array::from_fn(|i| a[(i + 1) % 3] * b[(i + 2) % 3] - a[(i + 2) % 3] * b[(i + 1) % 3])
}
fn norm(v: [I; 3]) -> I {
    let mut squared = v.into_iter().fold(I::exact(0.), |s, x| s + x.square());
    squared.lo = squared.lo.max(0.);
    squared.sqrt()
}
fn ideal_jacobian_lower(reference: CircularFilletReference) -> Result<f64> {
    let [r, min, max] = [
        reference.cylinder_radius(),
        reference.min_radius(),
        reference.max_radius(),
    ]
    .map(|x| I::exact(x.meters()));
    let a = I::exact(((max - min) / I::exact(2.)).hi.max(0.));
    let k = a / (r - max);
    let margin = I::exact(2.) * r.square() - I::exact(7.) * r * max + max.square() / I::exact(2.);
    let j = min * margin
        / (I::exact(2.)
            * (I::exact(1.) + k.square()).sqrt()
            * (r.square() + I::exact(2.) * a.square()).sqrt());
    if !j.valid() || j.lo <= 0. {
        return Err(invalid());
    }
    Ok(j.lo)
}
// Value, u derivative, v derivative, always in original global parameters.
type Jet = [P; 3];
fn jet(p: P) -> Jet {
    [p.clone(), p.derivative(0), p.derivative(1)]
}
#[derive(Clone)]
struct Cell {
    ideal: [Jet; 4],
    delta: [Jet; 4],
    actual_den: Jet,
    depth: usize,
}
impl Cell {
    fn split(self) -> [Self; 2] {
        let axis = self.depth % 2;
        let ideal = self.ideal.map(|jet| jet.map(|p| p.split_half(axis)));
        let delta = self.delta.map(|jet| jet.map(|p| p.split_half(axis)));
        let den = self.actual_den.map(|p| p.split_half(axis));
        let pick = |p: &(P, P), side| if side == 0 { p.0.clone() } else { p.1.clone() };
        std::array::from_fn(|side| Self {
            ideal: std::array::from_fn(|i| std::array::from_fn(|j| pick(&ideal[i][j], side))),
            delta: std::array::from_fn(|i| std::array::from_fn(|j| pick(&delta[i][j], side))),
            actual_den: std::array::from_fn(|j| pick(&den[j], side)),
            depth: self.depth + 1,
        })
    }
    fn bounds(&self, global_j: f64) -> Result<CircularFilletRationalSnapshotBounds> {
        let hull_jet =
            |jet: &Jet| -> Result<[I; 3]> { Ok([hull(&jet[0])?, hull(&jet[1])?, hull(&jet[2])?]) };
        let ideal = self
            .ideal
            .iter()
            .map(hull_jet)
            .collect::<Result<Vec<_>>>()?;
        let delta = self
            .delta
            .iter()
            .map(hull_jet)
            .collect::<Result<Vec<_>>>()?;
        let actual_den = hull_jet(&self.actual_den)?;
        let w = ideal[3][0];
        let v = actual_den[0];
        if w.lo <= 0. || v.lo <= 0. {
            return Err(invalid());
        }
        let point: [I; 3] = std::array::from_fn(|i| ideal[i][0] / w);
        let tangents: [[I; 3]; 2] = std::array::from_fn(|axis| {
            std::array::from_fn(|i| (ideal[i][axis + 1] - point[i] * ideal[3][axis + 1]) / w)
        });
        let position_error: [I; 3] = std::array::from_fn(|i| {
            (magnitude(delta[i][0]) + magnitude(point[i]) * magnitude(delta[3][0])) / I::exact(v.lo)
        });
        let errors: [[I; 3]; 2] = std::array::from_fn(|axis| {
            std::array::from_fn(|i| {
                (magnitude(delta[i][axis + 1])
                    + magnitude(tangents[axis][i]) * magnitude(delta[3][0])
                    + magnitude(point[i]) * magnitude(delta[3][axis + 1])
                    + position_error[i] * magnitude(actual_den[axis + 1]))
                    / I::exact(v.lo)
            })
        });
        if point
            .into_iter()
            .chain(tangents.into_iter().flatten())
            .chain(position_error)
            .chain(errors.into_iter().flatten())
            .any(|x| !x.valid())
        {
            return Err(invalid());
        }
        let [u, t] = tangents.map(|v| I::exact(norm_upper(v)));
        let [eu, ev] = errors.map(|v| I::exact(norm_upper(v)));
        let difference = eu * t + u * ev + eu * ev;
        let area = norm(cross(tangents[0], tangents[1]));
        if !difference.valid() || !area.valid() {
            return Err(invalid());
        }
        let lower = I::exact(area.lo.max(global_j));
        let jacobian = I {
            lo: (lower - difference).lo,
            hi: (area + difference).hi,
        };
        let error = I::exact(2.) * difference / lower;
        let alignment = (lower - difference) / (lower + difference);
        if !jacobian.valid() || !error.valid() || !alignment.valid() {
            return Err(invalid());
        }
        let position_error_upper_m = norm_upper(position_error);
        if !position_error_upper_m.is_finite() {
            return Err(invalid());
        }
        Ok(CircularFilletRationalSnapshotBounds {
            position_error_upper_m,
            normal: CircularFilletRationalNormalBounds {
                jacobian_area_lower_m2: jacobian.lo,
                jacobian_area_upper_m2: jacobian.hi,
                ideal_normal_alignment_lower: alignment.lo,
                normal_error_upper: error.hi,
                du_error_upper_m: eu.hi,
                dv_error_upper_m: ev.hi,
                leaves: 1,
            },
        })
    }
}
fn certify(
    reference: CircularFilletReference,
    chart: u8,
    poles: &[[[f64; 3]; 3]; 9],
    weights: &[[f64; 3]; 9],
    maximum_error: f64,
    max_leaves: usize,
) -> Result<CircularFilletRationalNormalBounds> {
    if !maximum_error.is_finite()
        || maximum_error <= 0.
        || maximum_error >= 1.
        || !(1..=16384).contains(&max_leaves)
        || poles.iter().flatten().flatten().any(|x| !x.is_finite())
        || weights.iter().flatten().any(|x| !x.is_finite() || *x <= 0.)
    {
        return Err(invalid());
    }
    let actual: [P; 4] = std::array::from_fn(|axis| P {
        u: 8,
        v: 2,
        c: (0..9)
            .flat_map(|i| {
                (0..3).map(move |j| {
                    if axis == 3 {
                        I::exact(weights[i][j])
                    } else {
                        I::exact(poles[i][j][axis]) * I::exact(weights[i][j])
                    }
                })
            })
            .collect(),
    });
    Ok(certify_homogeneous(
        reference,
        chart,
        actual,
        f64::INFINITY,
        maximum_error,
        max_leaves,
        false,
    )?
    .normal)
}

// One complete cover for supplied SI controls and captured native definitions.
// Infinity is used only internally by the old normal-only API.
pub(super) fn certify_homogeneous(
    reference: CircularFilletReference,
    chart: u8,
    mut actual: [P; 4],
    maximum_position_error_m: f64,
    maximum_error: f64,
    max_leaves: usize,
    normalize_gauge: bool,
) -> Result<CircularFilletRationalSnapshotBounds> {
    if maximum_position_error_m.is_nan()
        || maximum_position_error_m <= 0.
        || !maximum_error.is_finite()
        || maximum_error <= 0.
        || maximum_error >= 1.
        || !(1..=16384).contains(&max_leaves)
        || actual
            .iter()
            .any(|p| p.u != 8 || p.v != 2 || p.c.len() != 27 || p.c.iter().any(|c| !c.valid()))
    {
        return Err(invalid());
    }
    let (n, w) = homogeneous(reference, chart)?;
    let ideal = [n[0].clone(), n[1].clone(), n[2].clone(), w];
    if normalize_gauge {
        let anchor = actual[3].at(0, 0);
        if anchor.lo <= 0. {
            return Err(invalid());
        }
        let scale = ideal[3].at(0, 0) / anchor;
        if !scale.valid() || scale.lo <= 0. {
            return Err(invalid());
        }
        for p in &mut actual {
            for c in &mut p.c {
                *c = *c * scale;
            }
        }
    }
    let delta = std::array::from_fn(|i| actual[i].difference(&ideal[i]));
    let global_j = ideal_jacobian_lower(reference)?;
    let root = Cell {
        ideal: ideal.map(jet),
        delta: delta.map(jet),
        actual_den: jet(actual[3].clone()),
        depth: 0,
    };
    let mut stack = vec![root];
    let mut cover_leaves = 1;
    let mut result = CircularFilletRationalSnapshotBounds {
        position_error_upper_m: 0.,
        normal: CircularFilletRationalNormalBounds {
            jacobian_area_lower_m2: f64::INFINITY,
            jacobian_area_upper_m2: 0.,
            ideal_normal_alignment_lower: 1.,
            normal_error_upper: 0.,
            du_error_upper_m: 0.,
            dv_error_upper_m: 0.,
            leaves: 0,
        },
    };
    while let Some(cell) = stack.pop() {
        let bounds = cell.bounds(global_j)?;
        let b = bounds.normal;
        if bounds.position_error_upper_m <= maximum_position_error_m
            && b.jacobian_area_lower_m2 > 0.
            && b.ideal_normal_alignment_lower > 0.
            && b.normal_error_upper <= maximum_error
        {
            result.position_error_upper_m = result
                .position_error_upper_m
                .max(bounds.position_error_upper_m);
            result.normal.jacobian_area_lower_m2 = result
                .normal
                .jacobian_area_lower_m2
                .min(b.jacobian_area_lower_m2);
            result.normal.jacobian_area_upper_m2 = result
                .normal
                .jacobian_area_upper_m2
                .max(b.jacobian_area_upper_m2);
            result.normal.ideal_normal_alignment_lower = result
                .normal
                .ideal_normal_alignment_lower
                .min(b.ideal_normal_alignment_lower);
            result.normal.normal_error_upper =
                result.normal.normal_error_upper.max(b.normal_error_upper);
            result.normal.du_error_upper_m = result.normal.du_error_upper_m.max(b.du_error_upper_m);
            result.normal.dv_error_upper_m = result.normal.dv_error_upper_m.max(b.dv_error_upper_m);
            result.normal.leaves += 1;
        } else {
            if cover_leaves >= max_leaves || cell.depth >= 28 {
                return Err(invalid());
            }
            cover_leaves += 1;
            let [left, right] = cell.split();
            stack.push(right);
            stack.push(left);
        }
    }
    if result.normal.leaves != cover_leaves
        || !result.normal.jacobian_area_lower_m2.is_finite()
        || !result.normal.jacobian_area_upper_m2.is_finite()
        || result.normal.jacobian_area_lower_m2 <= 0.
        || result.normal.jacobian_area_lower_m2 > result.normal.jacobian_area_upper_m2
    {
        return Err(invalid());
    }
    Ok(result)
}
impl CircularFilletRationalPatch {
    /// Certify all original (u,v) in [0,1]^2 against this chart's privately bound
    /// construction reference. Precision is dimensionless normal chord error;
    /// work is 1..=16384 cover leaves. No partial certificate or geometry change.
    pub fn normal_bounds(
        &self,
        maximum_normal_error: f64,
        max_leaves: usize,
    ) -> Result<CircularFilletRationalNormalBounds> {
        certify(
            self.reference,
            self.chart(),
            self.control_points_m(),
            self.weights(),
            maximum_normal_error,
            max_leaves,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::Length;

    fn patch() -> CircularFilletRationalPatch {
        CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(0.0015),
        )
        .unwrap()
        .rational_patch(0)
        .unwrap()
    }

    #[test]
    fn actual_collapsed_reversed_and_locally_deformed_controls_refuse() {
        let original = patch();
        let weights = *original.weights();
        let mut reverse = *original.control_points_m();
        let mut reverse_weights = weights;
        for row in &mut reverse {
            row.reverse();
        }
        for row in &mut reverse_weights {
            row.reverse();
        }
        let mut bulge = *original.control_points_m();
        bulge[4][1][2] += 0.001;
        // Actual controls are evaluated anew, regardless of the patch's valid
        // positional metadata. No public geometry-mutation API is introduced.
        for (poles, weights) in [
            ([[[0.; 3]; 3]; 9], weights),
            (reverse, reverse_weights),
            (bulge, weights),
        ] {
            assert!(certify(original.reference, 0, &poles, &weights, 1e-9, 64).is_err());
        }
        assert!(original.normal_bounds(1e-9, 4096).is_ok());
    }

    #[test]
    fn invalid_actual_weights_poles_and_chart_refuse() {
        let original = patch();
        for value in [0., -1., f64::NAN, f64::INFINITY] {
            let mut weights = *original.weights();
            weights[4][1] = value;
            assert!(certify(
                original.reference,
                0,
                original.control_points_m(),
                &weights,
                1e-9,
                64
            )
            .is_err());
        }
        for value in [f64::NAN, f64::INFINITY] {
            let mut poles = *original.control_points_m();
            poles[4][1][2] = value;
            assert!(certify(original.reference, 0, &poles, original.weights(), 1e-9, 64).is_err());
        }
        assert!(certify(
            original.reference,
            4,
            original.control_points_m(),
            original.weights(),
            1e-9,
            64
        )
        .is_err());
    }
}
