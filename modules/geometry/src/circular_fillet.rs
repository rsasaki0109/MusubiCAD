//! ADR-072's ideal regular periodic envelope, not a kernel body or mass query.
use opencad_core::{Angle, Length, OpenCadError, Result};
use std::{
    f64::consts::{FRAC_PI_2, PI, TAU},
    ops::{Add, Div, Mul, Neg, Sub},
};

fn invalid() -> OpenCadError {
    OpenCadError::validation("outside bounded periodic fillet reference admission")
}

/// Local +Z cylinder, phase maximum at +X; transient reference arithmetic only.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletReference {
    radius: f64,
    height: f64,
    minimum: f64,
    maximum: f64,
}

/// Parameter derivatives are metres per radian; normal is dimensionless.
#[derive(Debug, Clone, Copy)]
pub struct CircularFilletSample {
    pub point_m: [f64; 3],
    pub normal: [f64; 3],
    pub theta_tangent_m: [f64; 3],
    pub alpha_tangent_m: [f64; 3],
}

impl CircularFilletReference {
    pub fn new(radius: Length, height: Length, minimum: Length, maximum: Length) -> Result<Self> {
        let [radius, height, minimum, maximum] =
            [radius, height, minimum, maximum].map(|v| v.meters());
        if [radius, height, minimum, maximum]
            .iter()
            .any(|v| !v.is_finite() || *v <= 1e-9 || *v > 1000.)
            || minimum > maximum
            || maximum > radius / 4.
            || height < 4. * maximum
        {
            return Err(invalid());
        }
        Ok(Self {
            radius,
            height,
            minimum,
            maximum,
        })
    }
    pub fn cylinder_radius(self) -> Length {
        Length::from_meters(self.radius)
    }
    pub fn cylinder_height(self) -> Length {
        Length::from_meters(self.height)
    }
    pub fn min_radius(self) -> Length {
        Length::from_meters(self.minimum)
    }
    pub fn max_radius(self) -> Length {
        Length::from_meters(self.maximum)
    }

    fn theta(theta: Angle) -> Result<f64> {
        let theta = theta.radians();
        if !theta.is_finite() || !(0. ..=TAU).contains(&theta) {
            return Err(invalid());
        }
        // Only the declared closed endpoint is canonicalized, never nearby input.
        Ok(if theta >= TAU { 0. } else { theta })
    }
    pub fn radius_at(self, theta: Angle) -> Result<Length> {
        let theta = Self::theta(theta)?;
        Ok(Length::from_meters(
            self.minimum + (self.maximum - self.minimum) / 2. * (1. + theta.cos()),
        ))
    }

    /// Real-arithmetic all-angle exposure lower bound (m²), not an interval bound
    /// on floating-point evaluation, native geometry or mesh approximation.
    pub fn exposure_margin_m2(self) -> f64 {
        let q = self.maximum / self.radius;
        (2. - 7. * q + q * q / 2.) * self.radius * self.radius
    }

    pub fn sample(self, theta: Angle, alpha: Angle) -> Result<CircularFilletSample> {
        let theta = Self::theta(theta)?;
        let alpha = alpha.radians();
        if !alpha.is_finite() || !(0. ..=FRAC_PI_2).contains(&alpha) {
            return Err(invalid());
        }
        let theta = Dual {
            value: theta,
            derivative: [1., 0.],
        };
        let alpha = Dual {
            value: alpha,
            derivative: [0., 1.],
        };
        let one = Dual::constant(1.);
        let amplitude = Dual::constant((self.maximum - self.minimum) / 2.);
        let r = Dual::constant(self.minimum) + amplitude * (one + theta.cos());
        let dr = -amplitude * theta.sin();
        let radial = Dual::constant(self.radius) - r;
        let k = dr / radial;
        let q = alpha.cos() + alpha.sin();
        let k2 = k * k;
        let t = (k2 * q + (one + k2 * (q * q - one)).sqrt()) / (one + k2 * q * q);
        let nr = t * alpha.cos();
        let nz = t * alpha.sin();
        let nt = k * (t * q - one);
        let normal = [
            nr * theta.cos() - nt * theta.sin(),
            nr * theta.sin() + nt * theta.cos(),
            nz,
        ];
        let centre = [
            radial * theta.cos(),
            radial * theta.sin(),
            Dual::constant(self.height) - r,
        ];
        let point: [Dual; 3] = std::array::from_fn(|i| centre[i] + r * normal[i]);
        let sample = CircularFilletSample {
            point_m: point.map(|v| v.value),
            normal: normal.map(|v| v.value),
            theta_tangent_m: point.map(|v| v.derivative[0]),
            alpha_tangent_m: point.map(|v| v.derivative[1]),
        };
        if sample
            .point_m
            .into_iter()
            .chain(sample.normal)
            .chain(sample.theta_tangent_m)
            .chain(sample.alpha_tangent_m)
            .any(|v| !v.is_finite())
        {
            return Err(invalid());
        }
        Ok(sample)
    }

    /// Ideal constant-limit body volume only. Explicit unequal authored radii
    /// refuse even if their amplitude would be lost at evaluation precision.
    pub fn constant_limit_volume_m3(self) -> Result<f64> {
        // This compares authored parameter identity, not approximate geometry.
        if self.minimum.to_bits() != self.maximum.to_bits() {
            return Err(invalid());
        }
        let r = self.minimum;
        let removed = 2. * PI * ((self.radius - r) * (1. - PI / 4.) * r * r + r * r * r / 6.);
        Ok(PI * self.radius * self.radius * self.height - removed)
    }
}

/// Exact first-order chain rules; no finite-difference step or sampling policy.
#[derive(Clone, Copy)]
struct Dual {
    value: f64,
    derivative: [f64; 2],
}
impl Dual {
    fn constant(value: f64) -> Self {
        Self {
            value,
            derivative: [0.; 2],
        }
    }
    fn sin(self) -> Self {
        Self {
            value: self.value.sin(),
            derivative: self.derivative.map(|d| d * self.value.cos()),
        }
    }
    fn cos(self) -> Self {
        Self {
            value: self.value.cos(),
            derivative: self.derivative.map(|d| -d * self.value.sin()),
        }
    }
    fn sqrt(self) -> Self {
        let value = self.value.sqrt();
        Self {
            value,
            derivative: self.derivative.map(|d| d / (2. * value)),
        }
    }
}
impl Add for Dual {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            value: self.value + rhs.value,
            derivative: std::array::from_fn(|i| self.derivative[i] + rhs.derivative[i]),
        }
    }
}
impl Sub for Dual {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            value: self.value - rhs.value,
            derivative: std::array::from_fn(|i| self.derivative[i] - rhs.derivative[i]),
        }
    }
}
impl Neg for Dual {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            value: -self.value,
            derivative: self.derivative.map(|d| -d),
        }
    }
}
// The product rule necessarily adds the two derivative products.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Mul for Dual {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            value: self.value * rhs.value,
            derivative: std::array::from_fn(|i| {
                self.derivative[i] * rhs.value + self.value * rhs.derivative[i]
            }),
        }
    }
}
// The quotient rule necessarily includes multiplication and subtraction.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Div for Dual {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let value = self.value / rhs.value;
        Self {
            value,
            derivative: std::array::from_fn(|i| {
                (self.derivative[i] - value * rhs.derivative[i]) / rhs.value
            }),
        }
    }
}
