//! Private outward binary64 arithmetic shared by ADR-073 and ADR-075.
use std::ops::{Add, Div, Mul, Neg, Sub};

// Adjacent binary64 values without raising the workspace's Rust 1.78 MSRV.
// Zero checks describe signed-zero representation, not geometric equality.
pub(crate) trait AdjacentFloat {
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
pub(crate) struct I {
    pub(crate) lo: f64,
    pub(crate) hi: f64,
}
impl I {
    pub(crate) fn exact(x: f64) -> Self {
        Self { lo: x, hi: x }
    }
    pub(crate) fn pi() -> Self {
        Self {
            lo: std::f64::consts::PI.adjacent_down(),
            hi: std::f64::consts::PI.adjacent_up(),
        }
    }
    pub(crate) fn valid(self) -> bool {
        self.lo.is_finite() && self.hi.is_finite() && self.lo <= self.hi
    }
    pub(crate) fn width(self) -> f64 {
        (self.hi - self.lo).adjacent_up()
    }
    pub(crate) fn square(self) -> Self {
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
    pub(crate) fn sqrt(self) -> Self {
        if self.lo < 0. {
            return Self::exact(f64::NAN);
        }
        Self {
            lo: self.lo.sqrt().adjacent_down().max(0.),
            hi: self.hi.sqrt().adjacent_up(),
        }
    }
    pub(crate) fn atan(self) -> Self {
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
