//! ADR-075: ideal patch enclosures and interpolation bounds, not a native fit.
use crate::{interval::I, CircularFilletReference};
use opencad_core::{OpenCadError, Result};
use std::ops::{Add, Div, Mul, Neg, Sub};

fn invalid() -> OpenCadError {
    OpenCadError::validation("periodic reference patch domain or numerical enclosure failed")
}

/// Ideal component enclosures; chart derivatives have metres per dimensionless
/// coordinate. This is not a mesh, native approximation or normal-error bound.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletPatchBounds {
    pub position_lower_m: [f64; 3],
    pub position_upper_m: [f64; 3],
    pub normal_lower: [f64; 3],
    pub normal_upper: [f64; 3],
    pub du_lower_m: [f64; 3],
    pub du_upper_m: [f64; 3],
    pub dv_lower_m: [f64; 3],
    pub dv_upper_m: [f64; 3],
    /// Upper Euclidean norms of F_uu, F_uv and F_vv, in that order.
    pub second_derivative_norm_upper_m: [f64; 3],
    /// (u0,v0),(u1,v0),(u1,v1),(u0,v1); exact declared seams canonicalize.
    pub corner_points_m: [[f64; 3]; 4],
    pub corner_rounding_error_upper_m: f64,
    /// Exact-corner Taylor bound plus rounded-corner error, in metres.
    /// Applies to triangles (0,1,2),(0,2,3) with parameter correspondence.
    pub linear_interpolation_error_upper_m: f64,
}

#[derive(Debug, Clone, Copy)]
struct Jet {
    value: I,
    gradient: [I; 2],
    hessian: [[I; 2]; 2],
}
impl Jet {
    fn constant(value: I) -> Self {
        Self {
            value,
            gradient: [I::exact(0.); 2],
            hessian: [[I::exact(0.); 2]; 2],
        }
    }
    fn variable(value: I, index: usize) -> Self {
        let mut jet = Self::constant(value);
        jet.gradient[index] = I::exact(1.);
        jet
    }
    fn valid(self) -> bool {
        self.value.valid()
            && self
                .gradient
                .into_iter()
                .chain(self.hessian.into_iter().flatten())
                .all(I::valid)
    }
    fn compose(self, value: I, first: I, second: I) -> Self {
        Self {
            value,
            gradient: self.gradient.map(|d| first * d),
            hessian: std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    second * self.gradient[i] * self.gradient[j] + first * self.hessian[i][j]
                })
            }),
        }
    }
    fn reciprocal(self) -> Self {
        if !self.valid() || self.value.lo <= 0. {
            return Self::constant(I::exact(f64::NAN));
        }
        let one = I::exact(1.);
        let inverse = one / self.value;
        self.compose(
            inverse,
            -one / self.value.square(),
            I::exact(2.) / (self.value.square() * self.value),
        )
    }
    fn square(self) -> Self {
        let mut product = self * self;
        product.value = self.value.square();
        product
    }
    fn sqrt(self) -> Self {
        if !self.valid() || self.value.lo <= 0. {
            return Self::constant(I::exact(f64::NAN));
        }
        let root = self.value.sqrt();
        self.compose(
            root,
            I::exact(1.) / (I::exact(2.) * root),
            -I::exact(1.) / (I::exact(4.) * self.value * root),
        )
    }
}
impl Add for Jet {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            value: self.value + rhs.value,
            gradient: std::array::from_fn(|i| self.gradient[i] + rhs.gradient[i]),
            hessian: std::array::from_fn(|i| {
                std::array::from_fn(|j| self.hessian[i][j] + rhs.hessian[i][j])
            }),
        }
    }
}
impl Sub for Jet {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            value: self.value - rhs.value,
            gradient: std::array::from_fn(|i| self.gradient[i] - rhs.gradient[i]),
            hessian: std::array::from_fn(|i| {
                std::array::from_fn(|j| self.hessian[i][j] - rhs.hessian[i][j])
            }),
        }
    }
}
impl Neg for Jet {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            value: -self.value,
            gradient: self.gradient.map(|v| -v),
            hessian: self.hessian.map(|row| row.map(|v| -v)),
        }
    }
}
// Second-order product chain rules necessarily add derivative products.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Mul for Jet {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            value: self.value * rhs.value,
            gradient: std::array::from_fn(|i| {
                self.gradient[i] * rhs.value + self.value * rhs.gradient[i]
            }),
            hessian: std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    self.hessian[i][j] * rhs.value
                        + self.gradient[i] * rhs.gradient[j]
                        + self.gradient[j] * rhs.gradient[i]
                        + self.value * rhs.hessian[i][j]
                })
            }),
        }
    }
}
// Division is multiplication by the independently enclosed reciprocal jet.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Div for Jet {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        self * rhs.reciprocal()
    }
}

fn circle(coordinate: Jet) -> (Jet, Jet) {
    let one = Jet::constant(I::exact(1.));
    let two = Jet::constant(I::exact(2.));
    let denominator = one + coordinate.square();
    (
        (one - coordinate.square()) / denominator,
        two * coordinate / denominator,
    )
}
fn evaluate(
    reference: CircularFilletReference,
    chart: u8,
    u: I,
    v: I,
) -> Result<([Jet; 3], [Jet; 3])> {
    let (c, s) = circle(Jet::variable(u, 0));
    let (ct, st) = match chart {
        0 => (c, s),
        1 => (-s, c),
        2 => (-c, -s),
        _ => (s, -c),
    };
    let (ca, sa) = circle(Jet::variable(v, 1));
    let one = Jet::constant(I::exact(1.));
    let scale = I::exact(reference.cylinder_radius().meters());
    let minimum = I::exact(reference.min_radius().meters()) / scale;
    let maximum = I::exact(reference.max_radius().meters()) / scale;
    let amplitude = Jet::constant((maximum - minimum) / I::exact(2.));
    let r = Jet::constant(minimum) + amplitude * (one + ct);
    let radial = one - r;
    let k = -amplitude * st / radial;
    let q = ca + sa;
    let k2 = k.square();
    let t = (k2 * q + (one + k2 * (q.square() - one)).sqrt()) / (one + k2 * q.square());
    let nr = t * ca;
    let nz = t * sa;
    let nt = k * (t * q - one);
    let normal = [nr * ct - nt * st, nr * st + nt * ct, nz];
    let centre = [
        radial * ct,
        radial * st,
        Jet::constant(I::exact(reference.cylinder_height().meters()) / scale) - r,
    ];
    let point = std::array::from_fn(|i| (centre[i] + r * normal[i]) * Jet::constant(scale));
    if !point.into_iter().chain(normal).all(Jet::valid) {
        return Err(invalid());
    }
    Ok((point, normal))
}
pub(super) fn norm_upper(values: [I; 3]) -> f64 {
    let mut squared = values.into_iter().fold(I::exact(0.), |sum, v| {
        sum + I::exact(v.lo.abs().max(v.hi.abs())).square()
    });
    // A sum of real squares is nonnegative, even at the subnormal floor.
    squared.lo = squared.lo.max(0.);
    squared.sqrt().hi
}

/// Canonical corner derivatives, expressed in the caller's original chart.
pub(super) fn corner_derivatives(
    reference: CircularFilletReference,
    chart: u8,
    u: f64,
    v: f64,
) -> Result<[[I; 3]; 3]> {
    let seam = u >= 1.; // Declared parameter endpoint, not geometric equality.
    let chart = if seam { (chart + 1) % 4 } else { chart };
    let u = if seam || u <= 0. { 0. } else { u };
    let v = if v <= 0. { 0. } else { v };
    let (point, _) = evaluate(reference, chart, I::exact(u), I::exact(v))?;
    // d(u_next)/d(u_original)=1/2 at the quarter-circle seam.
    let scale = I::exact(if seam { 0.5 } else { 1. });
    Ok([
        point.map(|p| p.gradient[0] * scale),
        point.map(|p| p.gradient[1]),
        point.map(|p| p.hessian[0][1] * scale),
    ])
}

fn corner(
    reference: CircularFilletReference,
    mut chart: u8,
    mut u: f64,
    mut v: f64,
) -> Result<([f64; 3], f64)> {
    // Exact endpoint identities, including signed-zero representation only.
    if u >= 1. {
        chart = (chart + 1) % 4;
        u = 0.;
    } else if u <= 0. {
        u = 0.;
    }
    if v <= 0. {
        v = 0.;
    }
    let (point, _) = evaluate(reference, chart, I::exact(u), I::exact(v))?;
    let representative = point.map(|p| {
        (p.value.lo + (p.value.hi - p.value.lo) / 2.)
            .max(p.value.lo)
            .min(p.value.hi)
    });
    let error = norm_upper(std::array::from_fn(|i| {
        I::exact(representative[i]) - point[i].value
    }));
    Ok((representative, error))
}
impl CircularFilletReference {
    /// ADR-075 rational rectangle: chart 0..3, 0<=u0<u1<=1, 0<=v0<v1<=1.
    /// theta=chart*pi/2+2*atan(u), alpha=2*atan(v); coordinates dimensionless.
    /// No tessellation policy, native fit, solid or source admission is implied.
    pub fn patch_bounds(
        self,
        chart: u8,
        u0: f64,
        u1: f64,
        v0: f64,
        v1: f64,
    ) -> Result<CircularFilletPatchBounds> {
        if chart >= 4
            || [u0, u1, v0, v1]
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || u0 >= u1
            || v0 >= v1
        {
            return Err(invalid());
        }
        let (point, normal) = evaluate(self, chart, I { lo: u0, hi: u1 }, I { lo: v0, hi: v1 })?;
        let norms =
            [(0, 0), (0, 1), (1, 1)].map(|(i, j)| norm_upper(point.map(|p| p.hessian[i][j])));
        let mut corners = [[0.; 3]; 4];
        let mut rounding = 0_f64;
        for (i, (u, v)) in [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
            .into_iter()
            .enumerate()
        {
            let (p, e) = corner(self, chart, u, v)?;
            corners[i] = p;
            rounding = rounding.max(e);
        }
        let hu = I::exact(u1) - I::exact(u0);
        let hv = I::exact(v1) - I::exact(v0);
        let linear = (I::exact(norms[0]) * hu.square()
            + I::exact(2.) * I::exact(norms[1]) * hu * hv
            + I::exact(norms[2]) * hv.square())
            / I::exact(8.);
        let error = (linear + I::exact(rounding)).hi;
        if !norms
            .into_iter()
            .chain([rounding, error])
            .all(|v| v.is_finite() && v >= 0.)
        {
            return Err(invalid());
        }
        Ok(CircularFilletPatchBounds {
            position_lower_m: point.map(|p| p.value.lo),
            position_upper_m: point.map(|p| p.value.hi),
            normal_lower: normal.map(|n| n.value.lo),
            normal_upper: normal.map(|n| n.value.hi),
            du_lower_m: point.map(|p| p.gradient[0].lo),
            du_upper_m: point.map(|p| p.gradient[0].hi),
            dv_lower_m: point.map(|p| p.gradient[1].lo),
            dv_upper_m: point.map(|p| p.gradient[1].hi),
            second_derivative_norm_upper_m: norms,
            corner_points_m: corners,
            corner_rounding_error_upper_m: rounding,
            linear_interpolation_error_upper_m: error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::Length;
    fn contains(i: I, x: f64) {
        assert!(i.lo <= x && x <= i.hi, "{i:?} excludes {x}");
    }
    #[test]
    fn constant_contact_chart_derivatives_and_hessians_match_exact_solutions() {
        let reference = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.001),
            Length::from_meters(0.001),
        )
        .unwrap();
        let (p, n) = evaluate(reference, 0, I::exact(0.), I::exact(0.)).unwrap();
        for i in 0..3 {
            contains(p[i].gradient[0], [0., 0.02, 0.][i]);
            contains(p[i].gradient[1], [0., 0., 0.002][i]);
            contains(p[i].hessian[0][0], [-0.04, 0., 0.][i]);
            contains(p[i].hessian[0][1], 0.);
            contains(p[i].hessian[1][1], [-0.004, 0., 0.][i]);
            contains(n[i].value, [1., 0., 0.][i]);
        }
    }
    #[test]
    fn composed_second_order_chain_rules_match_independent_rational_values() {
        assert!(norm_upper([I::exact(0.); 3]).is_finite());
        let x = Jet::variable(I::exact(2.), 0);
        let y = Jet::variable(I::exact(3.), 1);
        let f = x.square() * y + x / y;
        contains(f.value, I::exact(12.).lo + 2. / 3.);
        contains(f.gradient[0], 12. + 1. / 3.);
        contains(f.gradient[1], 4. - 2. / 9.);
        contains(f.hessian[0][0], 6.);
        contains(f.hessian[0][1], 4. - 1. / 9.);
        contains(f.hessian[1][1], 4. / 27.);
        assert!(!Jet::constant(I { lo: -1., hi: 1. }).reciprocal().valid());
        assert!(!Jet::constant(I::exact(0.)).sqrt().valid());
    }
}
