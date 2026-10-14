//! Nominal ISO metric basic-profile arithmetic (ADR-067), not a kernel solid.
//!
//! Flat roots, no manufactured tolerance class or rounded production root.

use std::f64::consts::{PI, TAU};

use opencad_core::{Angle, Length, OpenCadError, Result};

/// Supported nominal coarse designation; no arbitrary pitch overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricThreadSize {
    M3,
    M4,
    M5,
    M6,
    M8,
    M10,
    M12,
}

impl MetricThreadSize {
    /// Nominal major diameter and coarse pitch, both SI lengths.
    pub fn dimensions(self) -> (Length, Length) {
        let (diameter_mm, pitch_mm) = match self {
            Self::M3 => (3.0, 0.5),
            Self::M4 => (4.0, 0.7),
            Self::M5 => (5.0, 0.8),
            Self::M6 => (6.0, 1.0),
            Self::M8 => (8.0, 1.25),
            Self::M10 => (10.0, 1.5),
            Self::M12 => (12.0, 1.75),
        };
        (
            Length::from_meters(diameter_mm * 1e-3),
            Length::from_meters(pitch_mm * 1e-3),
        )
    }
}

/// Increasing angle follows increasing axial position for right-handed crests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadHandedness {
    Right,
    Left,
}

/// Immutable nominal profile with square ends and an integer number of pitches.
///
/// Transient arithmetic only: this is not a serialized feature, solid or mesh.
#[derive(Debug, Clone, Copy)]
pub struct NominalMetricThread {
    size: MetricThreadSize,
    hand: ThreadHandedness,
    turns: u32,
}

impl NominalMetricThread {
    pub fn new(size: MetricThreadSize, hand: ThreadHandedness, turns: u32) -> Result<Self> {
        if !(1..=32).contains(&turns) {
            return Err(OpenCadError::validation(
                "nominal metric thread requires 1..=32 complete pitches",
            ));
        }
        Ok(Self { size, hand, turns })
    }

    pub fn size(self) -> MetricThreadSize {
        self.size
    }

    pub fn handedness(self) -> ThreadHandedness {
        self.hand
    }

    pub fn turns(self) -> u32 {
        self.turns
    }

    pub fn major_diameter(self) -> Length {
        self.size.dimensions().0
    }

    pub fn pitch(self) -> Length {
        self.size.dimensions().1
    }

    pub fn length(self) -> Length {
        Length::from_meters(self.pitch().meters() * f64::from(self.turns))
    }

    /// Basic flat-root diameter D - 5H/4; not rounded external root D - 17H/12.
    pub fn basic_minor_diameter(self) -> Length {
        let h = 3.0_f64.sqrt() * self.pitch().meters() / 2.0;
        Length::from_meters(self.major_diameter().meters() - 5.0 * h / 4.0)
    }

    pub fn pitch_diameter(self) -> Length {
        let h = 3.0_f64.sqrt() * self.pitch().meters() / 2.0;
        Length::from_meters(self.major_diameter().meters() - 3.0 * h / 4.0)
    }

    /// Radius of the meridional screw envelope at local axial z and azimuth.
    ///
    /// The phase-zero crest is at z=0, theta=0. Both clipping planes are included.
    pub fn radius_at(self, z: Length, azimuth: Angle) -> Result<Length> {
        let z = z.meters();
        let theta = azimuth.radians();
        if !z.is_finite() || z < 0.0 || z > self.length().meters() {
            return Err(OpenCadError::validation(
                "thread axial position must be finite and within square ends",
            ));
        }
        if !theta.is_finite() || theta.abs() > 64.0 * TAU {
            return Err(OpenCadError::validation(
                "thread azimuth must be finite and within 64 full turns",
            ));
        }
        let sign = match self.hand {
            ThreadHandedness::Right => 1.0,
            ThreadHandedness::Left => -1.0,
        };
        let phase = ((z / self.pitch().meters()).rem_euclid(1.0)
            - sign * theta.rem_euclid(TAU) / TAU)
            .rem_euclid(1.0);
        Ok(Length::from_meters(self.radius_for_phase(phase)))
    }

    pub(crate) fn radius_for_phase(self, phase: f64) -> f64 {
        let phase = phase.rem_euclid(1.0);
        let distance = phase.min(1.0 - phase);
        let major = self.major_diameter().meters() / 2.0;
        if distance <= 1.0 / 16.0 {
            major
        } else if distance >= 3.0 / 8.0 {
            self.basic_minor_diameter().meters() / 2.0
        } else {
            major - 3.0_f64.sqrt() * self.pitch().meters() * (distance - 1.0 / 16.0)
        }
    }

    /// Exact ideal envelope section area in m²; not a measured kernel property.
    pub fn ideal_section_area_m2(self) -> f64 {
        let r = self.basic_minor_diameter().meters() / 2.0;
        let major = self.major_diameter().meters() / 2.0;
        PI * (11.0 * r * r + 5.0 * r * major + 8.0 * major * major) / 24.0
    }

    /// Exact ideal envelope volume in m³; no claim that a kernel body exists.
    pub fn ideal_volume_m3(self) -> f64 {
        self.ideal_section_area_m2() * self.length().meters()
    }

    /// Exact ideal exterior area, including the two noncircular square ends.
    pub fn ideal_surface_area_m2(self) -> f64 {
        let p = self.pitch().meters();
        let r = self.basic_minor_diameter().meters() / 2.0;
        let major = self.major_diameter().meters() / 2.0;
        let a = p / TAU;
        let integral = |x: f64| {
            x * (4.0 * x * x + 3.0 * a * a).sqrt() / 2.0
                + 3.0 * a * a / 4.0 * (2.0 * x / (3.0_f64.sqrt() * a)).asinh()
        };
        TAU * self.length().meters() / p
            * (p * (r / 4.0 + major / 8.0) + 2.0 / 3.0_f64.sqrt() * (integral(major) - integral(r)))
            + 2.0 * self.ideal_section_area_m2()
    }
}
