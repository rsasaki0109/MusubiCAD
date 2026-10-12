//! Exact closed non-rational cubic span and analytic prism metadata (ADR-057).
use crate::{BoundingBox, MassProperties, RigidTransform, SketchPlacement};
use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};
fn invalid() -> OpenCadError {
    OpenCadError::validation("closed cubic requires finite distinct CCW controls, bounded conditioning and a proper plane/normal extrusion")
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
/// ADR-057: proper finite rotation/translation, with 1e-9 matrix tolerances.
pub fn validate_rigid_transform(transform: RigidTransform) -> Result<()> {
    transform.inverse()?;
    let [a, b, c] = transform.rotation;
    if (dot(a, cross(b, c)) - 1.).abs() > 1e-9
        || [a, b, c].iter().any(|row| (norm(*row) - 1.).abs() > 1e-9)
        || dot(a, b).abs() > 1e-9
        || dot(a, c).abs() > 1e-9
        || dot(b, c).abs() > 1e-9
        || transform
            .translation_m
            .iter()
            .chain(transform.rotation.iter().flatten())
            .any(|v| !v.is_finite())
    {
        return Err(invalid());
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolvedCubicBezier {
    /// Three distinct controls; fourth is implicitly the first by closed subtype.
    pub controls_m: [[f64; 2]; 3],
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezierFrame {
    pub controls_m: [[f64; 3]; 3],
}
impl SolvedCubicBezier {
    pub fn validate(self) -> Result<()> {
        if self
            .controls_m
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 1e6)
        {
            return Err(invalid());
        }
        let [p, a, b] = self.controls_m;
        let signed = (a[0] - p[0]) * (b[1] - p[1]) - (a[1] - p[1]) * (b[0] - p[0]);
        if signed <= 1e-18 {
            return Err(invalid());
        }
        CubicBezierFrame {
            controls_m: self.controls_m.map(|c| [c[0], c[1], 0.]),
        }
        .validate()
    }
    pub fn frame(self, plane: SketchPlacement) -> Result<CubicBezierFrame> {
        self.validate()?;
        if plane
            .origin_m
            .iter()
            .chain(plane.x_axis_m.iter())
            .chain(plane.y_axis_m.iter())
            .any(|v| !v.is_finite())
            || (norm(plane.x_axis_m) - 1.).abs() > 1e-9
            || (norm(plane.y_axis_m) - 1.).abs() > 1e-9
            || dot(plane.x_axis_m, plane.y_axis_m).abs() > 1e-9
        {
            return Err(invalid());
        }
        Ok(CubicBezierFrame {
            controls_m: self.controls_m.map(|p| plane.map_point(p[0], p[1])),
        })
    }
}
impl CubicBezierFrame {
    fn validate(self) -> Result<()> {
        if self.controls_m.iter().flatten().any(|v| !v.is_finite()) {
            return Err(invalid());
        }
        let [p, a, b] = self.controls_m;
        let lengths = [norm(sub(a, p)), norm(sub(b, p)), norm(sub(a, b))];
        let minimum = lengths.iter().copied().fold(f64::INFINITY, f64::min);
        let maximum = lengths.iter().copied().fold(0., f64::max);
        let product = norm(cross(sub(a, p), sub(b, p)));
        if minimum <= 1e-9
            || minimum / maximum < 1e-3
            || product <= 1e-18
            || product / (lengths[0] * lengths[1]) < 1e-3
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn native_data(self) -> [f64; 12] {
        let mut data = [0.; 12];
        for (i, p) in self
            .controls_m
            .into_iter()
            .chain(std::iter::once(self.controls_m[0]))
            .enumerate()
        {
            data[i * 3..i * 3 + 3].copy_from_slice(&p);
        }
        data
    }
    pub fn normal(self) -> [f64; 3] {
        let [p, a, b] = self.controls_m;
        let n = cross(sub(a, p), sub(b, p));
        let size = norm(n);
        n.map(|v| v / size)
    }
    pub fn area_m2(self) -> f64 {
        let [p, a, b] = self.controls_m;
        0.15 * norm(cross(sub(a, p), sub(b, p)))
    }
    pub fn point(self, t: f64) -> [f64; 3] {
        let [p, a, b] = self.controls_m;
        let u = 1. - t;
        std::array::from_fn(|i| {
            p[i] * (u * u * u + t * t * t) + 3. * a[i] * t * u * u + 3. * b[i] * t * t * u
        })
    }
    fn speed(self, t: f64) -> f64 {
        let [p, a, b] = self.controls_m;
        let u = 1. - t;
        norm(std::array::from_fn(|i| {
            3. * (u * u * (a[i] - p[i]) + 2. * u * t * (b[i] - a[i]) + t * t * (p[i] - b[i]))
        }))
    }
    /// Deterministic capped quadrature; convergence is not a certified error bound.
    pub fn perimeter_m(self) -> Result<f64> {
        self.validate()?;
        const NODES: [f64; 4] = [
            0.1834346424956498,
            0.525532409916329,
            0.7966664774136267,
            0.9602898564975363,
        ];
        const WEIGHTS: [f64; 4] = [
            0.362683783378362,
            0.3137066458778873,
            0.2223810344533745,
            0.1012285362903763,
        ];
        let mut previous: Option<f64> = None;
        for level in 0..=12 {
            let panels = 1_usize << level;
            let half = 0.5 / panels as f64;
            let mut sum = 0.;
            for panel in 0..panels {
                let middle = (panel as f64 + 0.5) / panels as f64;
                for (x, w) in NODES.into_iter().zip(WEIGHTS) {
                    sum +=
                        w * half * (self.speed(middle - half * x) + self.speed(middle + half * x));
                }
            }
            if let Some(old) = previous {
                if (sum - old).abs() <= 1e-10_f64.max(sum.abs() * 1e-10) {
                    return Ok(sum);
                }
            }
            previous = Some(sum);
        }
        Err(OpenCadError::validation(
            "cubic perimeter metric did not converge within 4096 panels",
        ))
    }
    pub fn prism(self, direction: [f64; 3], length_m: f64) -> Result<CubicBezierPrism> {
        self.validate()?;
        let size = norm(direction);
        if !length_m.is_finite()
            || length_m <= 1e-9
            || direction.iter().any(|v| !v.is_finite())
            || size <= 1e-12
            || !size.is_finite()
            || (dot(direction.map(|v| v / size), self.normal()).abs() - 1.).abs() > 1e-9
        {
            return Err(invalid());
        }
        Ok(CubicBezierPrism {
            frame: self,
            delta_m: direction.map(|v| (v / size) * length_m),
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct CubicBezierPrism {
    pub frame: CubicBezierFrame,
    pub delta_m: [f64; 3],
}
impl CubicBezierPrism {
    pub fn bounding_box(self) -> BoundingBox {
        let [p, a, b] = self.frame.controls_m;
        let mut min = [0.; 3];
        let mut max = [0.; 3];
        for i in 0..3 {
            let (c1, c2, c3) = (
                3. * (a[i] - p[i]),
                3. * (p[i] - 2. * a[i] + b[i]),
                3. * (a[i] - b[i]),
            );
            let mut values = vec![p[i]];
            for t in quadratic_roots(3. * c3, 2. * c2, c1) {
                if (0.0..=1.0).contains(&t) {
                    values.push(self.frame.point(t)[i]);
                }
            }
            min[i] = values.iter().copied().fold(f64::INFINITY, f64::min) + self.delta_m[i].min(0.);
            max[i] =
                values.iter().copied().fold(f64::NEG_INFINITY, f64::max) + self.delta_m[i].max(0.);
        }
        BoundingBox { min, max }
    }
    pub fn mass_properties(self, density: f64) -> Result<MassProperties> {
        if !density.is_finite() || density <= 0. {
            return Err(OpenCadError::validation("density must be positive kg/m3"));
        }
        let [p, a, b] = self.frame.controls_m;
        let height = norm(self.delta_m);
        let area = self.frame.area_m2();
        let volume = area * height;
        let surface_area = 2. * area + self.frame.perimeter_m()? * height;
        if !volume.is_finite() || !surface_area.is_finite() || !(volume * density).is_finite() {
            return Err(OpenCadError::validation(
                "cubic physical metrics overflow SI values",
            ));
        }
        Ok(MassProperties {
            volume_m3: volume,
            area_m2: surface_area,
            mass_kg: volume * density,
            center_of_mass: std::array::from_fn(|i| {
                p[i] + 3. * (a[i] + b[i] - 2. * p[i]) / 14. + self.delta_m[i] / 2.
            }),
        })
    }
    pub fn transformed(self, transform: RigidTransform) -> Result<Self> {
        validate_rigid_transform(transform)?;
        let result = Self {
            frame: CubicBezierFrame {
                controls_m: self.frame.controls_m.map(|p| transform.transform_point(p)),
            },
            delta_m: std::array::from_fn(|i| dot(transform.rotation[i], self.delta_m)),
        };
        result.frame.validate()?;
        Ok(result)
    }
}
fn quadratic_roots(a: f64, b: f64, c: f64) -> Vec<f64> {
    let scale = a.abs().max(b.abs()).max(c.abs());
    if scale <= 1e-18 {
        return vec![];
    }
    let (a, b, c) = (a / scale, b / scale, c / scale);
    if a.abs() < 1e-14 {
        return if b.abs() < 1e-14 {
            vec![]
        } else {
            vec![-c / b]
        };
    }
    let discriminant = b * b - 4. * a * c;
    if discriminant < 0. {
        return vec![];
    }
    let q = -0.5 * (b + discriminant.sqrt().copysign(b));
    if q.abs() < 1e-14 {
        vec![-b / (2. * a)]
    } else {
        vec![q / a, c / q]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> CubicBezierFrame {
        SolvedCubicBezier {
            controls_m: [[0., 0.], [0.04, 0.], [0., 0.04]],
        }
        .frame(SketchPlacement::global_xy())
        .unwrap()
    }
    fn close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn closed_cubic_prism_matches_independent_area_moments_and_extrema() {
        let prism = frame().prism([0., 0., 1.], 0.006).unwrap();
        let mass = prism.mass_properties(2700.).unwrap();
        close(mass.volume_m3, 3. * 0.04 * 0.04 * 0.006 / 20., 1e-18);
        close(mass.mass_kg, 0.003888, 1e-14);
        let bounds = prism.bounding_box();
        for i in 0..2 {
            close(bounds.min[i], 0., 1e-15);
            close(bounds.max[i], 4. * 0.04 / 9., 1e-15);
            close(mass.center_of_mass[i], 3. * 0.04 / 14., 1e-15);
        }
        close(bounds.max[2], 0.006, 1e-15);
        close(mass.center_of_mass[2], 0.003, 1e-15);
    }

    #[test]
    fn cubic_perimeter_agrees_with_independent_composite_simpson_reference() {
        // Independent formula for the derivative of (3a*t*(1-t)^2,
        // 3b*t^2*(1-t)); Simpson rather than the implementation's GL8.
        let n = 65536;
        let speed =
            |t: f64| (0.12 * (1. - 4. * t + 3. * t * t)).hypot(0.12 * (2. * t - 3. * t * t));
        let mut sum = speed(0.) + speed(1.);
        for i in 1..n {
            sum += if i % 2 == 0 { 2. } else { 4. } * speed(i as f64 / n as f64);
        }
        close(frame().perimeter_m().unwrap(), sum / (3. * n as f64), 1e-12);
    }

    #[test]
    fn cubic_rigid_transform_and_reverse_extrusion_preserve_physics() {
        let prism = frame().prism([0., 0., -1.], 0.006).unwrap();
        let transform = RigidTransform {
            rotation: [[1., 0., 0.], [0., 0., -1.], [0., 1., 0.]],
            translation_m: [0.1, 0.2, 0.3],
        };
        let moved = prism.transformed(transform).unwrap();
        let original = prism.mass_properties(1000.).unwrap();
        let actual = moved.mass_properties(1000.).unwrap();
        close(actual.volume_m3, original.volume_m3, 1e-18);
        for (a, b) in actual
            .center_of_mass
            .into_iter()
            .zip(transform.transform_point(original.center_of_mass))
        {
            close(a, b, 1e-14);
        }
        let bounds = moved.bounding_box();
        close(bounds.min[1], 0.2, 1e-14);
        close(bounds.max[1], 0.206, 1e-14);
        close(bounds.min[2], 0.3, 1e-14);
        close(bounds.max[2], 0.3 + 4. * 0.04 / 9., 1e-14);
    }

    #[test]
    fn cubic_admission_refuses_degenerate_clockwise_unbounded_and_skew_prisms() {
        for controls_m in [
            [[0., 0.], [0., 0.04], [0.04, 0.]],
            [[0., 0.], [0.04, 0.], [0.08, 0.]],
            [[0., 0.], [0.04, 0.], [0., 1e-10]],
            [[f64::NAN, 0.], [0.04, 0.], [0., 0.04]],
            [[0., 0.], [1e7, 0.], [0., 0.04]],
        ] {
            assert!(SolvedCubicBezier { controls_m }.validate().is_err());
        }
        assert!(frame().prism([1., 0., 1.], 0.006).is_err());
        assert!(frame().prism([f64::MAX, 0., f64::MAX], 0.006).is_err());
        assert!(frame().prism([0., 0., 1.], f64::NAN).is_err());
        assert!(frame()
            .prism([0., 0., 1.], f64::MAX)
            .unwrap()
            .mass_properties(1000.)
            .is_err());
        let reflection = RigidTransform {
            rotation: [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            translation_m: [0.; 3],
        };
        assert!(frame()
            .prism([0., 0., 1.], 0.006)
            .unwrap()
            .transformed(reflection)
            .is_err());
    }
}
