//! ADR-073: an outward-rounded ideal reference integral, never kernel mass.
use crate::CircularFilletReference;
use opencad_core::{OpenCadError, Result};
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    ops::{Add, Div, Mul, Neg, Sub},
};

fn invalid() -> OpenCadError {
    OpenCadError::validation("periodic reference volume domain, precision or cell budget exceeded")
}

/// Absolute enclosures in cubic metres; no midpoint is a backend mass property.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletVolumeBounds {
    pub body_lower_m3: f64,
    pub body_upper_m3: f64,
    pub removed_lower_m3: f64,
    pub removed_upper_m3: f64,
    /// Number of leaves partitioning the four angular charts; zero for constant.
    pub cells: usize,
}

// Adjacent binary64 values without raising the workspace's Rust 1.78 MSRV.
// Zero checks describe signed-zero representation, not geometric equality.
trait AdjacentFloat {
    fn adjacent_up(self) -> Self;
    fn adjacent_down(self) -> Self;
}
impl AdjacentFloat for f64 {
    fn adjacent_up(self) -> Self {
        if self.is_nan() || self >= f64::INFINITY {
            return self;
        }
        if self == 0. {
            return f64::from_bits(1);
        }
        f64::from_bits(if self > 0. {
            self.to_bits() + 1
        } else {
            self.to_bits() - 1
        })
    }
    fn adjacent_down(self) -> Self {
        -(-self).adjacent_up()
    }
}

/// Primitive operations assume IEEE-754 nearest rounding and correctly rounded
/// sqrt. Each rounded result expands by one representable value (ADR-073).
#[derive(Clone, Copy, Debug)]
struct I {
    lo: f64,
    hi: f64,
}
impl I {
    fn exact(x: f64) -> Self {
        Self { lo: x, hi: x }
    }
    fn pi() -> Self {
        Self {
            lo: std::f64::consts::PI.adjacent_down(),
            hi: std::f64::consts::PI.adjacent_up(),
        }
    }
    fn valid(self) -> bool {
        self.lo.is_finite() && self.hi.is_finite() && self.lo <= self.hi
    }
    fn width(self) -> f64 {
        (self.hi - self.lo).adjacent_up()
    }
    fn square(self) -> Self {
        let hi = self.lo.abs().max(self.hi.abs());
        let lo = if self.lo <= 0. && self.hi >= 0. {
            0.
        } else {
            self.lo.abs().min(self.hi.abs())
        };
        Self {
            lo: if lo <= 0. {
                0.
            } else {
                (lo * lo).adjacent_down().max(0.)
            },
            hi: (hi * hi).adjacent_up(),
        }
    }
    fn sqrt(self) -> Self {
        if self.lo < 0. {
            return Self::exact(f64::NAN);
        }
        Self {
            lo: self.lo.sqrt().adjacent_down().max(0.),
            hi: self.hi.sqrt().adjacent_up(),
        }
    }
    fn atan(self) -> Self {
        if !self.valid() || self.lo < 0. || self.hi > 0.125 {
            return Self::exact(f64::NAN);
        }
        let x2 = self.square();
        let mut power = self;
        let mut sum = Self::exact(0.);
        for i in 0..8 {
            let term = power / Self::exact(f64::from(2 * i + 1));
            sum = if i % 2 == 0 { sum + term } else { sum - term };
            power = power * x2;
        }
        let tail = (power / Self::exact(17.)).hi;
        sum + Self {
            lo: -tail,
            hi: tail,
        }
    }
}
impl Add for I {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self {
            lo: (self.lo + b.lo).adjacent_down(),
            hi: (self.hi + b.hi).adjacent_up(),
        }
    }
}
impl Sub for I {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self {
            lo: (self.lo - b.hi).adjacent_down(),
            hi: (self.hi - b.lo).adjacent_up(),
        }
    }
}
impl Neg for I {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            lo: -self.hi,
            hi: -self.lo,
        }
    }
}
impl Mul for I {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        if !self.valid() || !b.valid() {
            return Self::exact(f64::NAN);
        }
        let v = [
            self.lo * b.lo,
            self.lo * b.hi,
            self.hi * b.lo,
            self.hi * b.hi,
        ];
        Self {
            lo: v.into_iter().fold(f64::INFINITY, f64::min).adjacent_down(),
            hi: v
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
                .adjacent_up(),
        }
    }
}
impl Div for I {
    type Output = Self;
    fn div(self, b: Self) -> Self {
        if !self.valid() || !b.valid() || (b.lo <= 0. && b.hi >= 0.) {
            return Self::exact(f64::NAN);
        }
        let v = [
            self.lo / b.lo,
            self.lo / b.hi,
            self.hi / b.lo,
            self.hi / b.hi,
        ];
        Self {
            lo: v.into_iter().fold(f64::INFINITY, f64::min).adjacent_down(),
            hi: v
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
                .adjacent_up(),
        }
    }
}
fn dot(a: [I; 3], b: [I; 3]) -> I {
    a.into_iter()
        .zip(b)
        .fold(I::exact(0.), |s, (x, y)| s + x * y)
}

// A polynomial in cos(phi),sin(phi), total degree at most three.
type Poly = [[I; 4]; 4];
fn polynomial(coefficients: [I; 3]) -> Poly {
    let mut p = [[I::exact(0.); 4]; 4];
    p[0][0] = coefficients[0];
    p[1][0] = coefficients[1];
    p[0][1] = coefficients[2];
    p
}
fn product(a: Poly, b: Poly) -> Poly {
    let mut out = [[I::exact(0.); 4]; 4];
    for i in 0..4 {
        for j in 0..4 - i {
            for k in 0..4 - i - j {
                for l in 0..4 - i - j - k {
                    out[i + k][j + l] = out[i + k][j + l] + a[i][j] * b[k][l];
                }
            }
        }
    }
    out
}

/// Integrate the characteristic circle analytically; units are normalized R^3.
fn density(minimum: I, maximum: I, cosine: I, sine: I) -> I {
    let one = I::exact(1.);
    let two = I::exact(2.);
    let three = I::exact(3.);
    let amplitude = (maximum - minimum) / two;
    let r = minimum + amplitude * (one + cosine);
    let d = -amplitude * sine;
    let dd = -amplitude * cosine;
    let b = one - r;
    let d2 = d.square();
    let v2 = b.square() + two * d2;
    let v = v2.sqrt();
    let f = d2 / v2;
    let o = [f, -d * b / v2, f];
    let c = -d2 / (b.square() + d2);
    let sn = (one - c.square()).sqrt();
    let mut argument = d2 / (b * v);
    // Its exact value is nonnegative; retain that proven domain at zero.
    argument.lo = argument.lo.max(0.);
    let omega = I::pi() / two + argument.atan();
    let a = [one - o[0], -o[1], -o[2]];
    let ez = [I::exact(0.), I::exact(0.), one];
    let bb = std::array::from_fn(|i| (ez[i] - o[i] - c * a[i]) / sn);
    let centre_second = [-b - dd, -two * d, -dd];
    let z = polynomial([o[2], a[2], bb[2]]);
    let complement = polynomial([one - o[2], -a[2], -bb[2]]);
    let determinant = polynomial([
        b.square() + d2 - r * dd - r * dot(o, centre_second),
        -r * dot(a, centre_second),
        -r * dot(bb, centre_second),
    ]);
    let p = product(product(z, complement), determinant);
    let sn3 = sn * sn.square();
    let c3 = c * c.square();
    let integrals = [
        ((0, 0), omega),
        ((1, 0), sn),
        ((0, 1), one - c),
        ((2, 0), (omega + sn * c) / two),
        ((0, 2), (omega - sn * c) / two),
        ((1, 1), sn.square() / two),
        ((3, 0), sn - sn3 / three),
        ((2, 1), (one - c3) / three),
        ((1, 2), sn3 / three),
        ((0, 3), two / three - c + c3 / three),
    ];
    r.square() / v
        * integrals
            .into_iter()
            .fold(I::exact(0.), |sum, ((i, j), weight)| sum + p[i][j] * weight)
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    chart: u8,
    lo: f64,
    hi: f64,
    integral: I,
}
impl Cell {
    fn new(chart: u8, lo: f64, hi: f64, minimum: I, maximum: I) -> Result<Self> {
        let u = I { lo, hi };
        let one = I::exact(1.);
        let two = I::exact(2.);
        let den = one + u.square();
        let c = (one - u.square()) / den;
        let s = two * u / den;
        let (cosine, sine) = match chart {
            0 => (c, s),
            1 => (-s, c),
            2 => (-c, -s),
            _ => (s, -c),
        };
        let mut integral =
            density(minimum, maximum, cosine, sine) * two / den * (I::exact(hi) - I::exact(lo));
        if !integral.valid() {
            return Err(invalid());
        }
        // The entire removed-corner integrand is nonnegative by ADR-072/073.
        integral.lo = integral.lo.max(0.);
        if !integral.valid() {
            return Err(invalid());
        }
        Ok(Self {
            chart,
            lo,
            hi,
            integral,
        })
    }
}
impl Ord for Cell {
    fn cmp(&self, b: &Self) -> Ordering {
        self.integral
            .width()
            .total_cmp(&b.integral.width())
            .then_with(|| self.chart.cmp(&b.chart))
            .then_with(|| self.lo.total_cmp(&b.lo))
            .then_with(|| self.hi.total_cmp(&b.hi))
    }
}
impl PartialOrd for Cell {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl PartialEq for Cell {
    fn eq(&self, b: &Self) -> bool {
        self.cmp(b) == Ordering::Equal
    }
}
impl Eq for Cell {}

impl CircularFilletReference {
    /// ADR-073 ideal volume enclosure. `max_width_m3` is an absolute full width,
    /// not a relative error. Budget is 4..=262144 leaves. Exhaustion refuses.
    pub fn volume_bounds_m3(
        self,
        max_width_m3: f64,
        max_cells: usize,
    ) -> Result<CircularFilletVolumeBounds> {
        if !max_width_m3.is_finite() || max_width_m3 <= 0. || !(4..=262_144).contains(&max_cells) {
            return Err(invalid());
        }
        let r = I::exact(self.cylinder_radius().meters());
        let h = I::exact(self.cylinder_height().meters());
        let minimum = I::exact(self.min_radius().meters());
        let maximum = I::exact(self.max_radius().meters());
        let stock = I::pi() * r.square() * h;
        let scale = r.square() * r;
        let finish = |mut removed: I, cells| -> Result<CircularFilletVolumeBounds> {
            removed.lo = removed.lo.max(0.);
            let body = stock - removed;
            if !removed.valid() || !body.valid() || body.lo <= 0. || body.width() > max_width_m3 {
                return Err(invalid());
            }
            Ok(CircularFilletVolumeBounds {
                body_lower_m3: body.lo,
                body_upper_m3: body.hi,
                removed_lower_m3: removed.lo,
                removed_upper_m3: removed.hi,
                cells,
            })
        };
        // Authored identity selects the exact constant formula, not an epsilon.
        if minimum.lo.to_bits() == maximum.lo.to_bits() {
            let one = I::exact(1.);
            let removed = I::exact(2.)
                * I::pi()
                * ((r - minimum) * (one - I::pi() / I::exact(4.)) * minimum.square()
                    + minimum.square() * minimum / I::exact(6.));
            return finish(removed, 0);
        }
        let mut heap = BinaryHeap::new();
        let mut total = I::exact(0.);
        for chart in 0..4 {
            let cell = Cell::new(chart, 0., 1., minimum / r, maximum / r)?;
            total = total + cell.integral;
            heap.push(cell);
        }
        loop {
            if let Ok(bounds) = finish(total * scale, heap.len()) {
                return Ok(bounds);
            }
            if heap.len() >= max_cells {
                return Err(invalid());
            }
            let parent = heap.pop().ok_or_else(invalid)?;
            let mid = parent.lo + (parent.hi - parent.lo) / 2.;
            if mid <= parent.lo || mid >= parent.hi {
                return Err(invalid());
            }
            let left = Cell::new(parent.chart, parent.lo, mid, minimum / r, maximum / r)?;
            let right = Cell::new(parent.chart, mid, parent.hi, minimum / r, maximum / r)?;
            total.lo = ((total.lo - parent.integral.lo).adjacent_down() + left.integral.lo)
                .adjacent_down();
            total.lo = (total.lo + right.integral.lo).adjacent_down();
            total.hi =
                ((total.hi - parent.integral.hi).adjacent_up() + left.integral.hi).adjacent_up();
            total.hi = (total.hi + right.integral.hi).adjacent_up();
            if !total.valid() {
                return Err(invalid());
            }
            heap.push(left);
            heap.push(right);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contains(a: I, x: f64) {
        assert!(a.valid() && a.lo <= x && x <= a.hi, "{a:?} excludes {x}");
    }
    #[test]
    fn adjacent_values_preserve_signed_zero_subnormals_infinities_and_nan() {
        for zero in [0_f64, -0_f64] {
            assert_eq!(zero.adjacent_up().to_bits(), 1);
            assert_eq!(zero.adjacent_down().to_bits(), (1_u64 << 63) | 1);
        }
        assert_eq!(f64::from_bits(1).adjacent_down().to_bits(), 0);
        assert_eq!((-f64::from_bits(1)).adjacent_up().to_bits(), 1_u64 << 63);
        assert_eq!(f64::INFINITY.adjacent_down().to_bits(), f64::MAX.to_bits());
        assert_eq!(
            f64::NEG_INFINITY.adjacent_up().to_bits(),
            (-f64::MAX).to_bits()
        );
        assert!(f64::INFINITY.adjacent_up().is_infinite());
        assert!(f64::NEG_INFINITY.adjacent_down().is_infinite());
        assert!(f64::NAN.adjacent_up().is_nan());
        assert!(f64::NAN.adjacent_down().is_nan());
        assert!(!(I {
            lo: f64::NAN,
            hi: 1.
        } * I::exact(1.))
        .valid());
    }

    // Exact integer comparisons of normal positive binary64 rational endpoints;
    // these test values require fewer than 128 bits, with no rounded oracle.
    fn rational(x: f64) -> (u128, u128) {
        let bits = x.to_bits();
        let numerator = u128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52));
        let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 - 52;
        assert!((-63..0).contains(&exponent));
        (numerator, 1_u128 << (-exponent))
    }
    #[test]
    fn primitive_enclosures_contain_exact_rationals_without_libm_oracle() {
        for (a, b) in [(1_u32, 3_u32), (1, 10), (2, 7), (10, 3)] {
            let bounds = I::exact(f64::from(a)) / I::exact(f64::from(b));
            let (ln, ld) = rational(bounds.lo);
            let (un, ud) = rational(bounds.hi);
            assert!(ln * u128::from(b) <= ld * u128::from(a));
            assert!(un * u128::from(b) >= ud * u128::from(a));
        }
        let root = I::exact(2.).sqrt();
        let (ln, ld) = rational(root.lo);
        let (un, ud) = rational(root.hi);
        assert!(ln * ln <= 2 * ld * ld);
        assert!(un * un >= 2 * ud * ud);
    }

    #[test]
    fn interval_operations_enclose_exact_cases_and_rounding_boundaries() {
        let one = I::exact(1.);
        let tiny = I::exact(f64::from_bits(1));
        let sum = one + I::exact(f64::EPSILON / 2.);
        assert!(sum.lo <= 1. && sum.hi >= 1_f64.adjacent_up());
        contains(one / I::exact(3.), 1. / 3.);
        contains((I { lo: -2., hi: 3. }).square(), 0.);
        contains((I { lo: -2., hi: 3. }).square(), 9.);
        contains(I::exact(4.).sqrt(), 2.);
        contains(-I { lo: 2., hi: 3. }, -3.);
        contains(tiny / I::exact(2.), 0.);
        assert!((tiny / I::exact(2.)).hi >= tiny.hi);
        assert!(!(one / I { lo: -1., hi: 1. }).valid());
        assert!(!I::exact(-1.).sqrt().valid());
        assert!(!I::exact(0.2).atan().valid());
        for x in [0., 0.001, 1. / 36., 0.125] {
            contains(I::exact(x).atan(), x.atan());
        }
        for x in [-0.3, 0., 0.7] {
            for y in [0.1, 2., 10.] {
                contains(I::exact(x) * I::exact(y), x * y);
                contains(I::exact(x) / I::exact(y), x / y);
            }
        }
    }
}
