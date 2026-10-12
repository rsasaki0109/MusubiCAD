//! Exact polygonal section witnesses; no sampled curves (ADR-042).
use crate::BoundingBox;
use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};
const EPS: f64 = 1e-8;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionPlane {
    pub origin_m: [f64; 3],
    pub normal: [f64; 3],
}
impl SectionPlane {
    pub fn validate(&self) -> Result<()> {
        let norm = self.normal.iter().map(|v| v * v).sum::<f64>().sqrt();
        if !self
            .origin_m
            .iter()
            .chain(&self.normal)
            .all(|v| v.is_finite())
            || (norm - 1.).abs() > 1e-9
        {
            return Err(invalid(
                "section plane needs finite model metres and a unit normal",
            ));
        }
        Ok(())
    }
    pub fn validate_crossing(&self, bbox: &BoundingBox) -> Result<()> {
        self.validate()?;
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for mask in 0..8 {
            let p = std::array::from_fn(|i| {
                if mask & (1 << i) == 0 {
                    bbox.min[i]
                } else {
                    bbox.max[i]
                }
            });
            let d = dot(sub(p, self.origin_m), self.normal);
            low = low.min(d);
            high = high.max(d);
        }
        if !low.is_finite() || !high.is_finite() || low >= -EPS || high <= EPS {
            return Err(invalid("section plane must strictly cross the body bounds"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SectionGeometry {
    pub loops_m: Vec<Vec<[f64; 3]>>,
    pub area_m2: f64,
}
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(message)
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    dot(sub(a, b), sub(a, b)) <= EPS * EPS
}
fn cmp(a: &[f64; 3], b: &[f64; 3]) -> std::cmp::Ordering {
    a.iter()
        .zip(b)
        .map(|(a, b)| a.total_cmp(b))
        .find(|o| !o.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}
fn axes(normal: [f64; 3]) -> (usize, usize, usize) {
    if normal[0].abs() >= normal[1].abs() && normal[0].abs() >= normal[2].abs() {
        (1, 2, 0)
    } else if normal[1].abs() >= normal[2].abs() {
        (0, 2, 1)
    } else {
        (0, 1, 2)
    }
}
fn inside(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        if (a[1] > point[1]) != (b[1] > point[1])
            && point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}
/// Stitch exact line witnesses into canonical closed loops, refusing open/branched
/// graphs, touching/intersecting loops and unsupported small/degenerate regions.
pub fn polygonal_section(
    plane: &SectionPlane,
    mut segments: Vec<[[f64; 3]; 2]>,
) -> Result<SectionGeometry> {
    plane.validate()?;
    if segments.is_empty() || segments.len() > 4096 {
        return Err(invalid("section needs 1..4096 exact line edges"));
    }
    for edge in &mut segments {
        if edge.iter().flatten().any(|v| !v.is_finite())
            || near(edge[0], edge[1])
            || edge
                .iter()
                .any(|p| dot(sub(*p, plane.origin_m), plane.normal).abs() > EPS)
        {
            return Err(invalid("invalid or nonplanar section edge"));
        }
        edge.sort_by(cmp);
    }
    segments.sort_by(|a, b| cmp(&a[0], &b[0]).then_with(|| cmp(&a[1], &b[1])));
    for p in segments.iter().flatten() {
        if segments.iter().flatten().filter(|q| near(*p, **q)).count() != 2 {
            return Err(invalid("section contour is open, touching or branched"));
        }
    }
    let mut loops = Vec::new();
    while !segments.is_empty() {
        let first = segments.remove(0);
        let mut points = vec![first[0], first[1]];
        while !near(points[0], points[points.len() - 1]) {
            let last = points[points.len() - 1];
            let matches: Vec<_> = segments
                .iter()
                .enumerate()
                .filter(|(_, e)| near(last, e[0]) || near(last, e[1]))
                .map(|(i, _)| i)
                .collect();
            if matches.len() != 1 {
                return Err(invalid("section contour has no unique continuation"));
            }
            let next = segments.remove(matches[0]);
            points.push(if near(last, next[0]) {
                next[1]
            } else {
                next[0]
            });
        }
        points.pop();
        if points.len() < 3 {
            return Err(invalid("section contour has fewer than three vertices"));
        }
        // The smallest remaining edge starts at the lexicographically smallest vertex.
        if cmp(&points[1], &points[points.len() - 1]).is_gt() {
            points[1..].reverse();
        }
        loops.push(points);
    }
    let (x, y, normal_axis) = axes(plane.normal);
    let polygons: Vec<Vec<[f64; 2]>> = loops
        .iter()
        .map(|p| p.iter().map(|p| [p[x], p[y]]).collect())
        .collect();
    // Only simple, mutually disjoint boundaries are admissible for even-odd fill.
    let orient = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    for (i, a) in polygons.iter().enumerate() {
        for (j, b) in polygons.iter().enumerate().skip(i) {
            for k in 0..a.len() {
                for l in 0..b.len() {
                    if i == j && (k == l || (k + 1) % a.len() == l || (l + 1) % a.len() == k) {
                        continue;
                    }
                    let (a0, a1, b0, b1) = (a[k], a[(k + 1) % a.len()], b[l], b[(l + 1) % b.len()]);
                    let overlap = (0..2).all(|d| {
                        a0[d].min(a1[d]) <= b0[d].max(b1[d]) + EPS
                            && b0[d].min(b1[d]) <= a0[d].max(a1[d]) + EPS
                    });
                    if overlap
                        && orient(a0, a1, b0) * orient(a0, a1, b1) <= 0.
                        && orient(b0, b1, a0) * orient(b0, b1, a1) <= 0.
                    {
                        return Err(invalid("section loops intersect or touch"));
                    }
                }
            }
        }
    }
    let mut area = 0.;
    for (i, p) in polygons.iter().enumerate() {
        let signed: f64 = (0..p.len())
            // Translate to a local origin before cross products to avoid
            // cancellation when a small part is far from the model origin.
            .map(|j| {
                let a = [p[j][0] - p[0][0], p[j][1] - p[0][1]];
                let b = [
                    p[(j + 1) % p.len()][0] - p[0][0],
                    p[(j + 1) % p.len()][1] - p[0][1],
                ];
                a[0] * b[1] - b[0] * a[1]
            })
            .sum();
        let loop_area = signed.abs() / 2. / plane.normal[normal_axis].abs();
        if !loop_area.is_finite() || loop_area <= EPS * EPS {
            return Err(invalid("section contour has degenerate area"));
        }
        let depth = polygons
            .iter()
            .enumerate()
            .filter(|(j, other)| i != *j && inside(p[0], other))
            .count();
        area += if depth % 2 == 0 {
            loop_area
        } else {
            -loop_area
        };
    }
    if !area.is_finite() || area <= EPS * EPS {
        return Err(invalid("section has no positive finite material area"));
    }
    Ok(SectionGeometry {
        loops_m: loops,
        area_m2: area,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plane() -> SectionPlane {
        SectionPlane {
            origin_m: [0., 0., 0.02],
            normal: [0., 0., 1.],
        }
    }
    fn rect(a: [f64; 2], b: [f64; 2]) -> Vec<[[f64; 3]; 2]> {
        let points = [
            [a[0], a[1], 0.02],
            [b[0], a[1], 0.02],
            [b[0], b[1], 0.02],
            [a[0], b[1], 0.02],
        ];
        (0..4).map(|i| [points[i], points[(i + 1) % 4]]).collect()
    }
    #[test]
    fn analytic_nested_holes_and_islands_are_canonical() {
        let mut edges = rect([0., 0.], [0.08, 0.06]);
        edges.extend(rect([0.03, 0.02], [0.05, 0.04]));
        let geometry = polygonal_section(&plane(), edges.clone()).expect("ring");
        assert!((geometry.area_m2 - 0.0044).abs() < 1e-12);
        assert_eq!(geometry.loops_m.len(), 2);
        let mut shifted = edges.clone();
        for point in shifted.iter_mut().flatten() {
            point[0] += 1000.;
            point[1] += 2000.;
        }
        assert!(
            (polygonal_section(&plane(), shifted)
                .expect("translated ring")
                .area_m2
                - geometry.area_m2)
                .abs()
                < 1e-12
        );
        edges.reverse();
        for edge in &mut edges {
            edge.reverse();
        }
        assert_eq!(
            geometry,
            polygonal_section(&plane(), edges).expect("canonical")
        );
        let mut islands = rect([0., 0.], [0.03, 0.06]);
        islands.extend(rect([0.05, 0.], [0.08, 0.06]));
        assert!(
            (polygonal_section(&plane(), islands)
                .expect("islands")
                .area_m2
                - 0.0036)
                .abs()
                < 1e-12
        );
        let mut nested = rect([0., 0.], [0.08, 0.06]);
        nested.extend(rect([0.02, 0.01], [0.06, 0.05]));
        nested.extend(rect([0.03, 0.02], [0.05, 0.04]));
        assert!(
            (polygonal_section(&plane(), nested).expect("nested").area_m2 - 0.0036).abs() < 1e-12
        );
    }
    #[test]
    fn open_branched_crossing_and_invalid_planes_fail_closed() {
        let rectangle = rect([0., 0.], [0.08, 0.06]);
        assert!(polygonal_section(&plane(), rectangle[..3].to_vec()).is_err());
        let mut branched = rectangle.clone();
        branched.push(rectangle[0]);
        assert!(polygonal_section(&plane(), branched).is_err());
        let mut touching = rectangle.clone();
        touching.extend(rect([0.08, 0.06], [0.1, 0.08]));
        assert!(polygonal_section(&plane(), touching).is_err());
        let mut crossing = rectangle;
        crossing.extend(rect([0.04, 0.03], [0.1, 0.08]));
        assert!(polygonal_section(&plane(), crossing).is_err());
        let mut invalid = plane();
        invalid.normal = [0., 0., 2.];
        assert!(invalid.validate().is_err());
        invalid.normal = [0.; 3];
        assert!(invalid.validate().is_err());
        invalid.normal = [f64::NAN, 0., 1.];
        assert!(invalid.validate().is_err());
        assert!(polygonal_section(&plane(), vec![]).is_err());
    }
    #[test]
    fn mock_box_sections_match_analytic_dimensions_in_three_planes() {
        use crate::{GeometryKernel, KernelBody, MockGeometryKernel};
        let kernel = MockGeometryKernel::new();
        let body = KernelBody(80);
        for axis in 0..3 {
            let mut normal = [0.; 3];
            normal[axis] = 1.;
            let mut origin = [0.; 3];
            origin[axis] = 0.04;
            let plane = SectionPlane {
                origin_m: origin,
                normal,
            };
            let section = kernel.section_body(&body, &plane).expect("slice");
            assert!((section.area_m2 - 0.0064).abs() < 1e-12);
            assert!(
                (kernel
                    .mass_properties(&body, 1.)
                    .expect("box volume")
                    .volume_m3
                    - section.area_m2 * 0.08)
                    .abs()
                    < 1e-12
            );
            assert_eq!(section.loops_m[0].len(), 4);
            for p in &section.loops_m[0] {
                assert!((p[axis] - 0.04).abs() < 1e-12);
            }
        }
        for z in [0., -0.01, 0.08, 0.1] {
            let mut p = plane();
            p.origin_m[2] = z;
            assert!(kernel.section_body(&body, &p).is_err());
        }
    }
}
