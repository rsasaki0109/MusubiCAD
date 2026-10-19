//! ADR-112: exact bounded right-rectangle revolves in SI world coordinates.
use crate::{
    BoundingBox, MassProperties, ProfileSegment, RevolveInput, RevolveOperation, RigidTransform,
    ROTATION_TOLERANCE,
};
use opencad_core::{OpenCadError, Result};
use serde::Serialize;
use std::f64::consts::TAU;

fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("rectangular revolve: {message}"))
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    a.map(|v| v * s)
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
fn roundoff(value: f64) -> f64 {
    (64. * f64::EPSILON * value.abs().max(1.)).min(crate::POSITION_TOLERANCE_M)
}
fn position(p: [f64; 3]) -> Result<()> {
    if p.into_iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
        return Err(invalid("position must be finite within 1e6 m"));
    }
    Ok(())
}
fn dimension(v: f64) -> Result<()> {
    if !v.is_finite() || v <= 1e-9 || v > 1000. {
        return Err(invalid("dimension must be above 1 nm and at most 1000 m"));
    }
    Ok(())
}
fn unit(v: [f64; 3]) -> Result<[f64; 3]> {
    let n = dot(v, v).sqrt();
    if !n.is_finite() || n <= 1e-12 {
        return Err(invalid("axis must be finite and nonzero"));
    }
    Ok(scale(v, 1. / n))
}

/// Derived physical evidence, never a persisted source or a replacement profile.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RectangularRevolveGeometry {
    pub origin_m: [f64; 3],
    pub radial_direction: [f64; 3],
    pub tangent_direction: [f64; 3],
    pub axis_direction: [f64; 3],
    pub inner_radius_m: f64,
    pub outer_radius_m: f64,
    pub height_m: f64,
    pub sweep_rad: f64,
    pub full_turn: bool,
}
impl RectangularRevolveGeometry {
    /// Admit only the portable subset; never repair or snap authored geometry.
    pub fn from_input(input: &RevolveInput) -> Result<Self> {
        let s = &input.sketch;
        if input.operation != RevolveOperation::NewBody || input.target.is_some() {
            return Err(invalid("requires NewBody without a target"));
        }
        let t = input.angle_rad;
        if !t.is_finite() || t <= 1e-9 || t > TAU {
            return Err(invalid(
                "angle must be finite, positive and at most 2*pi rad",
            ));
        }
        let full_turn = (t - TAU).abs() <= roundoff(TAU);
        if !full_turn && TAU - t <= 1e-6 {
            return Err(invalid(
                "near-full partial turn is ambiguous under the native circle threshold",
            ));
        }
        if !s.closed
            || s.points.len() != 4
            || s.circle.is_some()
            || s.ellipse.is_some()
            || s.cubic_bezier.is_some()
        {
            return Err(invalid(
                "requires one exact closed four-line right rectangle",
            ));
        }
        if !s.segments.is_empty() {
            if s.segments.len() != 4 {
                return Err(invalid("rectangle needs exactly four ordered lines"));
            }
            for (i, segment) in s.segments.iter().enumerate() {
                let ProfileSegment::Line { start_m, end_m } = segment else {
                    return Err(invalid("curved profiles are unsupported"));
                };
                for (actual, expected) in [(*start_m, s.points[i]), (*end_m, s.points[(i + 1) % 4])]
                {
                    if actual.into_iter().zip(expected).any(|(a, b)| {
                        !a.is_finite()
                            || !b.is_finite()
                            || (a - b).abs() > roundoff(a.abs().max(b.abs()))
                    }) {
                        return Err(invalid(
                            "line endpoints must match the ordered rectangle without gap repair",
                        ));
                    }
                }
            }
        }
        let map = |p: [f64; 2]| input.profile_plane.map_point(p[0], p[1]);
        let x = map([1., 0.]);
        let y = map([0., 1.]);
        let normal = cross(x, y);
        if let Some(p) = s.placement {
            if p.origin_m
                .into_iter()
                .chain(p.x_axis_m)
                .chain(p.y_axis_m)
                .zip([0.; 3].into_iter().chain(x).chain(y))
                .any(|(a, b)| !a.is_finite() || (a - b).abs() > roundoff(a.abs().max(b.abs())))
            {
                return Err(invalid(
                    "solved placement must equal the declared global profile plane",
                ));
            }
        }
        let p: Vec<_> = s.points.iter().copied().map(map).collect();
        for q in &p {
            position(*q)?;
        }
        let u = sub(p[1], p[0]);
        let v = sub(p[3], p[0]);
        dimension(dot(u, u).sqrt())?;
        dimension(dot(v, v).sqrt())?;
        let eu = unit(u)?;
        let ev = unit(v)?;
        if dot(eu, ev).abs() > roundoff(1.)
            || sub(p[2], add(p[0], add(u, v)))
                .into_iter()
                .any(|a| a.abs() > roundoff(dot(p[2], p[2]).sqrt()))
        {
            return Err(invalid("profile must be a right rectangle without repair"));
        }
        position(input.axis_origin_m)?;
        let axis = unit(input.axis_direction_m)?;
        if dot(axis, normal).abs() > roundoff(1.)
            || dot(sub(input.axis_origin_m, p[0]), normal).abs()
                > roundoff(dot(input.axis_origin_m, input.axis_origin_m).sqrt())
        {
            return Err(invalid("axis must lie in the profile plane"));
        }
        if dot(cross(axis, eu), cross(axis, eu))
            .sqrt()
            .min(dot(cross(axis, ev), cross(axis, ev)).sqrt())
            > roundoff(1.)
        {
            return Err(invalid("axis must be parallel to a rectangle edge"));
        }
        let axial: Vec<_> = p
            .iter()
            .map(|p| dot(sub(*p, input.axis_origin_m), axis))
            .collect();
        let lo = axial.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = axial.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let height_m = hi - lo;
        dimension(height_m)?;
        let origin_m = add(input.axis_origin_m, scale(axis, lo));
        position(origin_m)?;
        let radial: Vec<_> = p
            .iter()
            .zip(&axial)
            .map(|(p, z)| sub(sub(*p, input.axis_origin_m), scale(axis, *z)))
            .collect();
        let far = radial
            .iter()
            .max_by(|a, b| dot(**a, **a).total_cmp(&dot(**b, **b)))
            .ok_or_else(|| invalid("missing radial corner"))?;
        let radial_direction = unit(*far)?;
        let radii: Vec<_> = radial.iter().map(|r| dot(*r, radial_direction)).collect();
        let mut inner_radius_m = radii.iter().copied().fold(f64::INFINITY, f64::min);
        let outer_radius_m = radii.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if inner_radius_m.abs() <= roundoff(outer_radius_m) {
            inner_radius_m = 0.;
        }
        if inner_radius_m < 0. {
            return Err(invalid("profile crosses the rotation axis"));
        }
        dimension(inner_radius_m)?;
        dimension(outer_radius_m)?;
        dimension(outer_radius_m - inner_radius_m)?;
        if outer_radius_m * t <= 1e-9 {
            return Err(invalid(
                "angular span must be above 1 nm at the outer radius",
            ));
        }
        let result = Self {
            origin_m,
            radial_direction,
            tangent_direction: cross(axis, radial_direction),
            axis_direction: axis,
            inner_radius_m,
            outer_radius_m,
            height_m,
            sweep_rad: if full_turn { TAU } else { t },
            full_turn,
        };
        let bounds = result.bounding_box();
        position(bounds.min)?;
        position(bounds.max)?;
        Ok(result)
    }
    pub fn point(&self, r: f64, t: f64, z: f64) -> [f64; 3] {
        add(
            self.origin_m,
            add(
                scale(self.axis_direction, z),
                add(
                    scale(self.radial_direction, r * t.cos()),
                    scale(self.tangent_direction, r * t.sin()),
                ),
            ),
        )
    }
    pub fn mass_properties(&self, density: f64) -> Result<MassProperties> {
        if !density.is_finite() || density <= 0. {
            return Err(invalid("density must be positive and finite in kg/m^3"));
        }
        let axes = [
            self.radial_direction,
            self.tangent_direction,
            self.axis_direction,
        ];
        if axes.iter().flatten().any(|v| !v.is_finite())
            || (0..3).any(|i| {
                (dot(axes[i], axes[i]) - 1.).abs() > ROTATION_TOLERANCE
                    || (0..i).any(|j| dot(axes[i], axes[j]).abs() > ROTATION_TOLERANCE)
            })
            || (dot(cross(axes[0], axes[1]), axes[2]) - 1.).abs() > ROTATION_TOLERANCE
        {
            return Err(invalid("accumulated revolve frame drift"));
        }
        position(self.origin_m)?;
        dimension(self.inner_radius_m)?;
        dimension(self.outer_radius_m - self.inner_radius_m)?;
        dimension(self.outer_radius_m)?;
        dimension(self.height_m)?;
        let (a, b, h, t) = (
            self.inner_radius_m,
            self.outer_radius_m,
            self.height_m,
            self.sweep_rad,
        );
        let volume = (b * b - a * a) * t * h / 2.;
        let k = 2. * (b * b * b - a * a * a) / (3. * (b * b - a * a) * t);
        let center = if self.full_turn {
            add(self.origin_m, scale(self.axis_direction, h / 2.))
        } else {
            add(
                add(self.origin_m, scale(self.axis_direction, h / 2.)),
                add(
                    scale(self.radial_direction, k * t.sin()),
                    scale(self.tangent_direction, k * (1. - t.cos())),
                ),
            )
        };
        let area = (b * b - a * a) * t
            + (a + b) * t * h
            + if self.full_turn { 0. } else { 2. * (b - a) * h };
        if !volume.is_finite()
            || volume <= 0.
            || !area.is_finite()
            || area <= 0.
            || !(density * volume).is_finite()
            || center.iter().any(|v| !v.is_finite())
        {
            return Err(invalid("nonfinite physical measurements"));
        }
        Ok(MassProperties {
            volume_m3: volume,
            area_m2: area,
            mass_kg: density * volume,
            center_of_mass: center,
        })
    }
    pub fn bounding_box(&self) -> BoundingBox {
        let mut angles = vec![0., self.sweep_rad];
        for i in 0..3 {
            let a = self.tangent_direction[i]
                .atan2(self.radial_direction[i])
                .rem_euclid(TAU);
            for t in [a, (a + std::f64::consts::PI).rem_euclid(TAU)] {
                if t <= self.sweep_rad {
                    angles.push(t);
                }
            }
        }
        let mut out = BoundingBox {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        };
        for z in [0., self.height_m] {
            for r in [self.inner_radius_m, self.outer_radius_m] {
                for t in &angles {
                    for (i, v) in self.point(r, *t, z).into_iter().enumerate() {
                        out.min[i] = out.min[i].min(v);
                        out.max[i] = out.max[i].max(v);
                    }
                }
            }
        }
        out
    }
    pub(crate) fn transformed(&self, t: RigidTransform) -> Result<Self> {
        let r = t.rotation;
        if r.iter().flatten().any(|v| !v.is_finite())
            || (0..3).any(|i| {
                (dot(r[i], r[i]) - 1.).abs() > ROTATION_TOLERANCE
                    || (0..i).any(|j| dot(r[i], r[j]).abs() > ROTATION_TOLERANCE)
            })
            || (dot(cross(r[0], r[1]), r[2]) - 1.).abs() > ROTATION_TOLERANCE
        {
            return Err(invalid("transform requires a proper orthonormal rotation"));
        }
        let out = Self {
            origin_m: t.transform_point(self.origin_m),
            radial_direction: r.map(|row| dot(row, self.radial_direction)),
            tangent_direction: r.map(|row| dot(row, self.tangent_direction)),
            axis_direction: r.map(|row| dot(row, self.axis_direction)),
            ..*self
        };
        let b = out.bounding_box();
        position(b.min)?;
        position(b.max)?;
        Ok(out)
    }
}
