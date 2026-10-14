//! ADR-068 physical nominal screw envelope, phase frame and exact ideal properties.
use std::f64::consts::{PI, TAU};

use crate::TessellationSettings;
use opencad_core::{Angle, Length, OpenCadError, Result};

use crate::{
    BoundingBox, CylindricalStockGeometry, MassProperties, NominalMetricThread, RigidTransform,
    ThreadHandedness, ROTATION_TOLERANCE,
};

fn invalid() -> OpenCadError {
    OpenCadError::validation("invalid physical nominal metric thread frame/stock")
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f64; 3]) -> bool {
    a.iter().all(|x| x.is_finite()) && (dot(a, a) - 1.).abs() <= ROTATION_TOLERANCE
}

#[derive(Debug, Clone, Copy)]
pub struct NominalThreadInput {
    pub profile: NominalMetricThread,
    pub axis_hint: [f64; 3],
    /// Unit perpendicular radial direction of the phase-zero crest.
    pub radial_axis: [f64; 3],
}

impl NominalThreadInput {
    pub fn admit(&self, stock: &CylindricalStockGeometry) -> Result<NominalThreadGeometry> {
        stock.validate()?;
        if !unit(self.axis_hint) || !unit(self.radial_axis) {
            return Err(invalid());
        }
        let stock = stock.oriented(self.axis_hint)?;
        if (stock.radius_m - self.profile.major_diameter().meters() / 2.).abs() > 1e-9
            || (stock.height_m - self.profile.length().meters()).abs() > 1e-9
            || dot(self.radial_axis, stock.axis).abs() > ROTATION_TOLERANCE
        {
            return Err(invalid());
        }
        let geometry = NominalThreadGeometry {
            origin: stock.origin_m,
            axes: [
                self.radial_axis,
                cross(stock.axis, self.radial_axis),
                stock.axis,
            ],
            profile: self.profile,
        };
        geometry.validate()?;
        Ok(geometry)
    }
}

/// Immutable actual analytic screw envelope; no document or cached body ownership.
#[derive(Debug, Clone, Copy)]
pub struct NominalThreadGeometry {
    origin: [f64; 3],
    axes: [[f64; 3]; 3],
    profile: NominalMetricThread,
}

/// Deterministic bounded mesh preflight; interpolation error is in metres.
#[derive(Debug, Clone, Copy)]
pub struct NominalThreadMeshPlan {
    pub angular_segments: u32,
    pub interpolation_bound_m: f64,
    pub normal_bound_rad: f64,
}

impl NominalThreadGeometry {
    pub fn mesh_plan(self, quality: &TessellationSettings) -> Result<NominalThreadMeshPlan> {
        self.validate()?;
        let linear = quality.linear_deflection;
        let angular = quality.angular_deflection_deg.to_radians();
        if !linear.is_finite()
            || linear <= 0.
            || !angular.is_finite()
            || angular <= 0.
            || angular > PI
        {
            return Err(invalid());
        }
        let p = self.profile.pitch().meters();
        let major = self.profile.major_diameter().meters() / 2.;
        let root = self.profile.basic_minor_diameter().meters() / 2.;
        let normal_band = 3_f64.sqrt() * (p / TAU) * (1. / root - 1. / major) / 2.;
        if angular <= 2. * normal_band {
            return Err(OpenCadError::validation(
                "thread angular quality needs profile-band subdivision",
            ));
        }
        for n in (64_u32..=8192).step_by(16) {
            let nf = f64::from(n);
            let side =
                major * PI * PI / (2. * nf * nf) + TAU * (5. * 3_f64.sqrt() * p / 16.) / (4. * nf);
            let cap = (TAU * TAU * major + 2. * TAU * 3_f64.sqrt() * p) / (8. * nf * nf);
            let bound = side.max(cap);
            if bound <= linear / 2. && 2. * (TAU / nf + normal_band) <= angular {
                if u64::from(10 * self.profile.turns() + 32) * u64::from(n) * 3 > 100_000 {
                    return Err(OpenCadError::validation(
                        "thread mesh vertex budget exceeded",
                    ));
                }
                return Ok(NominalThreadMeshPlan {
                    angular_segments: n,
                    interpolation_bound_m: bound,
                    normal_bound_rad: 2. * (TAU / nf + normal_band),
                });
            }
        }
        Err(OpenCadError::validation(
            "thread angular sample budget exceeded",
        ))
    }

    pub fn profile(self) -> NominalMetricThread {
        self.profile
    }
    pub fn origin_m(self) -> [f64; 3] {
        self.origin
    }
    pub fn axes(self) -> [[f64; 3]; 3] {
        self.axes
    }
    pub(crate) fn validate(self) -> Result<()> {
        if self.axes.iter().any(|a| !unit(*a))
            || (0..3)
                .any(|i| (0..i).any(|j| dot(self.axes[i], self.axes[j]).abs() > ROTATION_TOLERANCE))
            || (dot(cross(self.axes[0], self.axes[1]), self.axes[2]) - 1.).abs()
                > ROTATION_TOLERANCE
        {
            return Err(invalid());
        }
        let r = self.profile.major_diameter().meters() / 2.;
        for i in 0..3 {
            let end = self.origin[i] + self.axes[2][i] * self.profile.length().meters();
            if !self.origin[i].is_finite()
                || !end.is_finite()
                || self.origin[i].min(end) - r < -1e6
                || self.origin[i].max(end) + r > 1e6
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
    pub(crate) fn map(self, local: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| {
            self.origin[i] + (0..3).map(|j| self.axes[j][i] * local[j]).sum::<f64>()
        })
    }
    pub fn point_at(self, z: Length, azimuth: Angle) -> Result<[f64; 3]> {
        let r = self.profile.radius_at(z, azimuth)?.meters();
        Ok(self.map([
            r * azimuth.radians().cos(),
            r * azimuth.radians().sin(),
            z.meters(),
        ]))
    }
    pub fn transformed(self, transform: RigidTransform) -> Result<Self> {
        if transform.rotation.iter().any(|a| !unit(*a))
            || (dot(
                cross(transform.rotation[0], transform.rotation[1]),
                transform.rotation[2],
            ) - 1.)
                .abs()
                > ROTATION_TOLERANCE
        {
            return Err(invalid());
        }
        let result = Self {
            origin: transform.transform_point(self.origin),
            axes: self.axes.map(|a| transform.rotation.map(|row| dot(row, a))),
            profile: self.profile,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn mass_properties(self, density_kg_per_m3: f64) -> Result<MassProperties> {
        self.validate()?;
        let volume = self.profile.ideal_volume_m3();
        if !density_kg_per_m3.is_finite()
            || density_kg_per_m3 <= 0.
            || !(volume * density_kg_per_m3).is_finite()
        {
            return Err(invalid());
        }
        Ok(MassProperties {
            volume_m3: volume,
            area_m2: self.profile.ideal_surface_area_m2(),
            mass_kg: volume * density_kg_per_m3,
            center_of_mass: self.map([0., 0., self.profile.length().meters() / 2.]),
        })
    }
    /// Ideal-envelope support extrema; cap/helix critical points, not a stock box.
    pub fn bounding_box(self) -> BoundingBox {
        BoundingBox {
            min: std::array::from_fn(|i| self.origin[i] - self.support(self.axes.map(|a| -a[i]))),
            max: std::array::from_fn(|i| self.origin[i] + self.support(self.axes.map(|a| a[i]))),
        }
    }
    fn support(self, d: [f64; 3]) -> f64 {
        let p = self.profile.pitch().meters();
        let n = f64::from(self.profile.turns());
        let s = if self.profile.handedness() == ThreadHandedness::Right {
            1.
        } else {
            -1.
        };
        let knots = [0., 1. / 16., 3. / 8., 5. / 8., 15. / 16., 1.];
        let mut best = f64::NEG_INFINITY;
        for q in knots {
            let r = self.profile.radius_for_phase(q);
            for k in -2..=self.profile.turns() as i32 + 2 {
                let k = f64::from(k);
                let (lo, hi) = if s > 0. {
                    (-TAU * (q + k), TAU * (n - q - k))
                } else {
                    (TAU * (q + k - n), TAU * (q + k))
                };
                let lo = lo.max(0.);
                let hi = hi.min(TAU);
                if lo > hi {
                    continue;
                }
                let eval = |t: f64| {
                    r * (d[0] * t.cos() + d[1] * t.sin()) + d[2] * p * (q + s * t / TAU + k)
                };
                best = best.max(eval(lo)).max(eval(hi));
                let a = d[0].hypot(d[1]);
                if a > 1e-15 {
                    let v = d[2] * s * p / (TAU * r * a);
                    if v.abs() <= 1. {
                        let phase = d[1].atan2(d[0]);
                        for t in [phase + v.asin(), phase + PI - v.asin()] {
                            for shift in [-TAU, 0., TAU] {
                                let t = t + shift;
                                if t >= lo && t <= hi {
                                    best = best.max(eval(t));
                                }
                            }
                        }
                    }
                }
            }
        }
        // At either whole-pitch cap q=-s*u modulo1. Each anchored band has
        // positive curvature and tangent variation <pi, hence at most one root.
        for range in knots.windows(2) {
            let (lo, hi) = if s > 0. {
                (TAU * (1. - range[1]), TAU * (1. - range[0]))
            } else {
                (TAU * range[0], TAU * range[1])
            };
            let rlo = self
                .profile
                .radius_for_phase((-s * lo / TAU).rem_euclid(1.));
            let rhi = self
                .profile
                .radius_for_phase((-s * hi / TAU).rem_euclid(1.));
            let slope = (rhi - rlo) / (hi - lo);
            let radius = |t: f64| rlo + slope * (t - lo);
            let eval = |t: f64| radius(t) * (d[0] * t.cos() + d[1] * t.sin());
            let derivative = |t: f64| {
                slope * (d[0] * t.cos() + d[1] * t.sin())
                    + radius(t) * (-d[0] * t.sin() + d[1] * t.cos())
            };
            let mut cap = eval(lo).max(eval(hi));
            let (mut a, mut b) = (lo, hi);
            let sign = derivative(a);
            if sign * derivative(b) < 0. {
                for _ in 0..64 {
                    let mid = (a + b) / 2.;
                    if sign * derivative(mid) > 0. {
                        a = mid;
                    } else {
                        b = mid;
                    }
                }
                cap = cap.max(eval((a + b) / 2.));
            }
            best = best.max(cap).max(cap + d[2] * n * p);
        }
        best
    }
}
