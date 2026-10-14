//! ADR-063 bounded ray-paired caps; no general polygon triangulation.
use super::*;
use crate::{FacePick, MeshSet, RectangularStockGeometry, TessellationSettings};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FaceKey {
    Outer(usize, bool),
    Wall(u64, u64, u64),
    Cone(u64, u64, u64, u64),
    Cap(u64, u64, u64),
    Cylinder(usize),
    Frustum(usize),
}
fn wall(a: f64, b: f64, r: f64, end_radius: f64) -> FaceKey {
    if (r - end_radius).abs() <= roundoff(r.max(end_radius)) {
        FaceKey::Wall(a.to_bits(), b.to_bits(), r.to_bits())
    } else {
        FaceKey::Cone(a.to_bits(), b.to_bits(), r.to_bits(), end_radius.to_bits())
    }
}
fn cap(z: f64, a: f64, b: f64) -> FaceKey {
    FaceKey::Cap(z.to_bits(), a.min(b).to_bits(), a.max(b).to_bits())
}
fn validate_frame(frame: Frame) -> Result<()> {
    if frame.axes.iter().flatten().any(|v| !v.is_finite())
        || (0..3).any(|i| (dot(frame.axes[i], frame.axes[i]) - 1.).abs() > ROTATION_TOLERANCE)
        || (0..3)
            .any(|i| (0..i).any(|j| dot(frame.axes[i], frame.axes[j]).abs() > ROTATION_TOLERANCE))
    {
        return Err(invalid("accumulated frame drift"));
    }
    Ok(())
}
pub(super) fn keys(body: &Body) -> Result<Vec<FaceKey>> {
    match body {
        Body::Cylinder { .. } => Ok((0..3).map(FaceKey::Cylinder).collect()),
        Body::Frustum { .. } => Ok((0..3).map(FaceKey::Frustum).collect()),
        Body::Stock {
            dimensions, cuts, ..
        } => {
            let mut result: Vec<_> = (0..3)
                .flat_map(|i| [FaceKey::Outer(i, false), FaceKey::Outer(i, true)])
                .collect();
            let rows = intervals(cuts, dimensions[2])?;
            for &[a, b, r, end_radius] in &rows {
                if r > 0. {
                    result.push(wall(a, b, r, end_radius));
                }
            }
            for pair in rows.windows(2) {
                if (pair[0][3] - pair[1][2]).abs() > roundoff(dimensions[2]) {
                    result.push(cap(pair[0][1], pair[0][3], pair[1][2]));
                }
            }
            Ok(result)
        }
    }
}
impl AnalyticMock {
    pub(crate) fn validate_face(&self, body: &KernelBody, id: u64) -> Result<()> {
        self.body(body)?;
        if self
            .faces
            .get(&body.0)
            .is_some_and(|f| f.values().any(|v| *v == id))
        {
            Ok(())
        } else {
            Err(invalid("face is not owned by this body"))
        }
    }
    pub(crate) fn history(&self, body: &KernelBody) -> Vec<(u64, u64)> {
        self.histories.get(&body.0).cloned().unwrap_or_default()
    }
    pub(crate) fn stock_geometry(
        &self,
        body: &KernelBody,
        pick: FacePick,
    ) -> Result<RectangularStockGeometry> {
        let Body::Stock {
            frame,
            dimensions,
            cuts,
        } = self.body(body)?
        else {
            return Err(unsupported());
        };
        if !cuts.is_empty() {
            return Err(invalid("stock witness requires uncut cuboid"));
        }
        validate_frame(*frame)?;
        let mut candidates = Vec::new();
        for axis in 0..3 {
            for high in [false, true] {
                let u = (axis + 1) % 3;
                let v = (axis + 2) % 3;
                let mut local = [0.; 3];
                if high {
                    local[axis] = dimensions[axis];
                }
                let witness = RectangularStockGeometry {
                    entry_face_id: self.faces[&body.0][&FaceKey::Outer(axis, high)],
                    origin_m: frame.map(local),
                    u_axis: frame.axes[u],
                    v_axis: frame.axes[v],
                    outward_normal: scale(frame.axes[axis], if high { 1. } else { -1. }),
                    face_spans_m: [dimensions[u], dimensions[v]],
                    inward_thickness_m: dimensions[axis],
                };
                if witness.matches_pick(pick)? {
                    candidates.push(witness);
                }
            }
        }
        if candidates.len() != 1 {
            return Err(invalid("stock pick has no unique physical face"));
        }
        Ok(candidates[0])
    }
    pub(crate) fn tessellate(
        &self,
        body: &KernelBody,
        quality: &TessellationSettings,
    ) -> Result<MeshSet> {
        let linear = quality.linear_deflection;
        let angular = quality.angular_deflection_deg.to_radians();
        if !linear.is_finite()
            || linear <= 0.
            || !angular.is_finite()
            || angular <= 0.
            || angular > PI
        {
            return Err(invalid("invalid mesh quality"));
        }
        let body_record = self.body(body)?;
        let face_ids = &self.faces[&body.0];
        let radius = match body_record {
            Body::Stock { cuts, .. } => cuts
                .iter()
                .map(|c| c.radius.max(c.end_radius))
                .fold(0., f64::max),
            Body::Cylinder { radius, .. } => *radius,
            Body::Frustum {
                radius, end_radius, ..
            } => radius.max(*end_radius),
        };
        let bound = if radius > 0. {
            angular.min(2. * (1. - (linear / radius).min(2.)).acos())
        } else {
            angular
        };
        let count = (2. * PI / (bound * 0.5)).ceil().max(8.);
        if !count.is_finite() || count > 8192. {
            return Err(invalid("8192 angular sample budget exceeded"));
        }
        let mut angles: Vec<_> = (0..count as usize)
            .map(|i| 2. * PI * i as f64 / count)
            .collect();
        let mut builder = Builder::new();
        match body_record {
            Body::Stock {
                frame,
                dimensions,
                cuts,
            } => {
                validate_frame(*frame)?;
                let [x, y, h] = *dimensions;
                let center = cuts.first().map(|c| c.center).unwrap_or([x / 2., y / 2.]);
                if cuts.is_empty() {
                    for axis in 0..3 {
                        for high in [false, true] {
                            let u = (axis + 1) % 3;
                            let v = (axis + 2) % 3;
                            let points = [
                                [0., 0.],
                                [dimensions[u], 0.],
                                [dimensions[u], dimensions[v]],
                                [0., dimensions[v]],
                            ]
                            .map(|p| {
                                let mut q = [0.; 3];
                                q[axis] = if high { dimensions[axis] } else { 0. };
                                q[u] = p[0];
                                q[v] = p[1];
                                frame.map(q)
                            });
                            builder.quad(
                                points,
                                scale(frame.axes[axis], if high { 1. } else { -1. }),
                                face_ids[&FaceKey::Outer(axis, high)],
                            )?;
                        }
                    }
                } else {
                    let anchors = [[0., 0.], [x, 0.], [x, y], [0., y]].map(|p| {
                        (p[1] - center[1])
                            .atan2(p[0] - center[0])
                            .rem_euclid(2. * PI)
                    });
                    // Remove only arithmetic coincidences, retaining the exact corner ray.
                    angles.retain(|a| {
                        !anchors.iter().any(|b| {
                            let d = (a - b).abs();
                            d.min(2. * PI - d) <= 64. * f64::EPSILON
                        })
                    });
                    angles.extend(anchors);
                    angles.sort_by(f64::total_cmp);
                    if angles.len() > 8192 {
                        return Err(invalid("corner anchors exceed angular budget"));
                    }
                    let rings = intervals(cuts, h)?;
                    let cap_triangles = |r: f64| if r > 0. { 2 } else { 1 };
                    let triangles_per_ray = 2
                        + cap_triangles(rings[0][2])
                        + cap_triangles(rings[rings.len() - 1][3])
                        + 2 * rings.iter().filter(|r| r[2] > 0.).count()
                        + rings
                            .windows(2)
                            .filter(|p| (p[0][3] - p[1][2]).abs() > roundoff(h))
                            .map(|p| cap_triangles(p[0][3].min(p[1][2])))
                            .sum::<usize>();
                    if angles.len() * triangles_per_ray > 100_000 / 3 {
                        return Err(invalid("mesh vertex budget exceeded"));
                    }
                    let outer: Vec<_> = angles
                        .iter()
                        .map(|&a| {
                            if let Some(i) =
                                anchors.iter().position(|b| (*b - a).abs() <= f64::EPSILON)
                            {
                                return [[0., 0.], [x, 0.], [x, y], [0., y]][i];
                            }
                            let c = a.cos();
                            let s = a.sin();
                            let tx = if c > 0. {
                                (x - center[0]) / c
                            } else if c < 0. {
                                -center[0] / c
                            } else {
                                f64::INFINITY
                            };
                            let ty = if s > 0. {
                                (y - center[1]) / s
                            } else if s < 0. {
                                -center[1] / s
                            } else {
                                f64::INFINITY
                            };
                            let t = tx.min(ty);
                            [
                                if tx <= ty {
                                    if c > 0. {
                                        x
                                    } else {
                                        0.
                                    }
                                } else {
                                    center[0] + t * c
                                },
                                if ty <= tx {
                                    if s > 0. {
                                        y
                                    } else {
                                        0.
                                    }
                                } else {
                                    center[1] + t * s
                                },
                            ]
                        })
                        .collect();
                    let at = |p: [f64; 2], z: f64| frame.map([p[0], p[1], z]);
                    for i in 0..angles.len() {
                        let j = (i + 1) % angles.len();
                        let p = outer[i];
                        let q = outer[j];
                        let (axis, high) = if (p[0] + q[0]).abs() <= roundoff(x) {
                            (0, false)
                        } else if (p[0] + q[0] - 2. * x).abs() <= roundoff(x) {
                            (0, true)
                        } else if (p[1] + q[1]).abs() <= roundoff(y) {
                            (1, false)
                        } else {
                            (1, true)
                        };
                        builder.quad(
                            [at(p, 0.), at(q, 0.), at(q, h), at(p, h)],
                            scale(frame.axes[axis], if high { 1. } else { -1. }),
                            face_ids[&FaceKey::Outer(axis, high)],
                        )?;
                    }
                    for (z, r, high) in [
                        (0., rings[0][2], false),
                        (h, rings[rings.len() - 1][3], true),
                    ] {
                        let inner = ring(&angles, center, r);
                        builder.annulus(
                            &outer,
                            &inner,
                            z,
                            *frame,
                            scale(frame.axes[2], if high { 1. } else { -1. }),
                            face_ids[&FaceKey::Outer(2, high)],
                        )?;
                    }
                    for &[a, b, r, end_radius] in &rings {
                        if r > 0. {
                            let points = ring(&angles, center, r);
                            let end_points = ring(&angles, center, end_radius);
                            for i in 0..points.len() {
                                let j = (i + 1) % points.len();
                                let radial = add(
                                    scale(frame.axes[0], (angles[i] + gap(&angles, i) / 2.).cos()),
                                    scale(frame.axes[1], (angles[i] + gap(&angles, i) / 2.).sin()),
                                );
                                builder.quad(
                                    [
                                        at(points[i], a),
                                        at(points[j], a),
                                        at(end_points[j], b),
                                        at(end_points[i], b),
                                    ],
                                    add(
                                        scale(radial, -1.),
                                        scale(frame.axes[2], (end_radius - r) / (b - a)),
                                    ),
                                    face_ids[&wall(a, b, r, end_radius)],
                                )?;
                            }
                        }
                    }
                    for pair in rings.windows(2) {
                        let a = pair[0][3];
                        let b = pair[1][2];
                        if (a - b).abs() > roundoff(h) {
                            builder.annulus(
                                &ring(&angles, center, a.max(b)),
                                &ring(&angles, center, a.min(b)),
                                pair[0][1],
                                *frame,
                                scale(frame.axes[2], if b > a { 1. } else { -1. }),
                                face_ids[&cap(pair[0][1], a, b)],
                            )?;
                        }
                    }
                }
            }
            Body::Cylinder {
                origin,
                axis,
                radius,
                length,
            }
            | Body::Frustum {
                origin,
                axis,
                radius,
                length,
                ..
            } => {
                let (end_radius, frustum) = match body_record {
                    Body::Frustum { end_radius, .. } => (*end_radius, true),
                    _ => (*radius, false),
                };
                let key = |i| {
                    if frustum {
                        FaceKey::Frustum(i)
                    } else {
                        FaceKey::Cylinder(i)
                    }
                };
                let original = *axis;
                let axis = unit(original)?;
                if (dot(original, original) - 1.).abs() > ROTATION_TOLERANCE {
                    return Err(invalid("cylinder frame drift"));
                }
                let seed = if axis[0].abs() < 0.8 {
                    [1., 0., 0.]
                } else {
                    [0., 1., 0.]
                };
                let u = unit(cross(axis, seed))?;
                let v = cross(axis, u);
                let frame = Frame {
                    origin: *origin,
                    axes: [u, v, axis],
                };
                let points = ring(&angles, [0., 0.], *radius);
                let end_points = ring(&angles, [0., 0.], end_radius);
                let center = vec![[0., 0.]; angles.len()];
                builder.annulus(
                    &points,
                    &center,
                    0.,
                    frame,
                    scale(axis, -1.),
                    face_ids[&key(0)],
                )?;
                builder.annulus(
                    &end_points,
                    &center,
                    *length,
                    frame,
                    axis,
                    face_ids[&key(1)],
                )?;
                for i in 0..angles.len() {
                    let j = (i + 1) % angles.len();
                    let at = |p: [f64; 2], z| frame.map([p[0], p[1], z]);
                    builder.quad(
                        [
                            at(points[i], 0.),
                            at(points[j], 0.),
                            at(end_points[j], *length),
                            at(end_points[i], *length),
                        ],
                        add(
                            add(
                                scale(u, (angles[i] + gap(&angles, i) / 2.).cos()),
                                scale(v, (angles[i] + gap(&angles, i) / 2.).sin()),
                            ),
                            scale(axis, -(end_radius - radius) / length),
                        ),
                        face_ids[&key(2)],
                    )?;
                }
            }
        }
        let max_gap = (0..angles.len())
            .map(|i| gap(&angles, i))
            .fold(0., f64::max);
        if builder.error > linear
            || radius > 0.
                && (max_gap > angular + 64. * f64::EPSILON
                    || radius * (1. - (max_gap / 2.).cos())
                        + builder.error
                        + 64. * f64::EPSILON * radius
                        > linear)
        {
            return Err(invalid(
                "requested deflection is below representable coordinate error",
            ));
        }
        builder.finish()
    }
}
fn gap(angles: &[f64], i: usize) -> f64 {
    if i + 1 < angles.len() {
        angles[i + 1] - angles[i]
    } else {
        2. * PI + angles[0] - angles[i]
    }
}
fn ring(angles: &[f64], center: [f64; 2], r: f64) -> Vec<[f64; 2]> {
    angles
        .iter()
        .map(|a| [center[0] + r * a.cos(), center[1] + r * a.sin()])
        .collect()
}
struct Builder {
    mesh: MeshSet,
    error: f64,
}
impl Builder {
    fn new() -> Self {
        Self {
            mesh: MeshSet {
                positions: Vec::new(),
                normals: Vec::new(),
                indices: Vec::new(),
                triangle_face_ids: Vec::new(),
            },
            error: 0.,
        }
    }
    fn triangle(&mut self, mut p: [[f64; 3]; 3], normal: [f64; 3], face: u64) -> Result<()> {
        if self.mesh.positions.len() + 3 > 100_000 || self.mesh.triangle_count() + 1 > 200_000 {
            return Err(invalid("mesh output budget exceeded"));
        }
        if dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), normal) < 0. {
            p.swap(1, 2);
        }
        let converted = p.map(|v| v.map(|c| if c == 0. { 0. } else { c as f32 }));
        for (a, b) in p.into_iter().zip(converted) {
            self.error = self
                .error
                .max(dot(sub(a, b.map(f64::from)), sub(a, b.map(f64::from))).sqrt());
        }
        let world = converted.map(|v| v.map(f64::from));
        let n = cross(sub(world[1], world[0]), sub(world[2], world[0]));
        if !dot(n, n).is_finite() || dot(n, n) <= 0. || dot(n, normal) <= 0. {
            return Err(invalid("f32 degenerate or inverted triangle"));
        }
        let normal = scale(n, 1. / dot(n, n).sqrt()).map(|v| v as f32);
        for p in converted {
            self.mesh.indices.push(self.mesh.positions.len() as u32);
            self.mesh.positions.push(p);
            self.mesh.normals.push(normal);
        }
        self.mesh.triangle_face_ids.push(face);
        Ok(())
    }
    fn quad(&mut self, p: [[f64; 3]; 4], normal: [f64; 3], face: u64) -> Result<()> {
        self.triangle([p[0], p[1], p[2]], normal, face)?;
        self.triangle([p[0], p[2], p[3]], normal, face)
    }
    fn annulus(
        &mut self,
        outer: &[[f64; 2]],
        inner: &[[f64; 2]],
        z: f64,
        frame: Frame,
        normal: [f64; 3],
        face: u64,
    ) -> Result<()> {
        for i in 0..outer.len() {
            let j = (i + 1) % outer.len();
            let at = |p: [f64; 2]| frame.map([p[0], p[1], z]);
            // Exact equality here identifies the deliberately repeated disk centre;
            // it is not used to coalesce independently supplied geometry.
            if inner[i] == inner[j] {
                self.triangle([at(outer[i]), at(outer[j]), at(inner[i])], normal, face)?;
            } else {
                self.quad(
                    [at(outer[i]), at(outer[j]), at(inner[j]), at(inner[i])],
                    normal,
                    face,
                )?;
            }
        }
        Ok(())
    }
    fn finish(self) -> Result<MeshSet> {
        let mut edges: BTreeMap<([u32; 3], [u32; 3]), (usize, i32)> = BTreeMap::new();
        let mut faces = BTreeSet::new();
        let origin = self.mesh.positions[0].map(f64::from);
        let mut volume = 0.;
        for (triangle, face) in self
            .mesh
            .indices
            .chunks_exact(3)
            .zip(&self.mesh.triangle_face_ids)
        {
            let p = triangle.map_array(&self.mesh.positions);
            volume += dot(
                sub(p[0].map(f64::from), origin),
                cross(
                    sub(p[1].map(f64::from), origin),
                    sub(p[2].map(f64::from), origin),
                ),
            ) / 6.;
            faces.insert(*face);
            for i in 0..3 {
                let a = p[i].map(f32::to_bits);
                let b = p[(i + 1) % 3].map(f32::to_bits);
                let (key, sign) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
                let entry = edges.entry(key).or_default();
                entry.0 += 1;
                entry.1 += sign;
            }
        }
        if edges
            .values()
            .any(|&(count, balance)| count != 2 || balance != 0)
            || !volume.is_finite()
            || volume <= 0.
            || faces.is_empty()
        {
            return Err(invalid("mesh is not an outward closed manifold"));
        }
        Ok(self.mesh)
    }
}
trait TrianglePositions {
    fn map_array(&self, positions: &[[f32; 3]]) -> [[f32; 3]; 3];
}
impl TrianglePositions for [u32] {
    fn map_array(&self, positions: &[[f32; 3]]) -> [[f32; 3]; 3] {
        std::array::from_fn(|i| positions[self[i] as usize])
    }
}
