//! ADR-062: bounded physical Mock records, never synthetic ID geometry.
use std::{collections::BTreeMap, f64::consts::PI};

use opencad_core::{OpenCadError, Result};

use crate::{
    BooleanOp, BoundingBox, ExtrudeExtent, ExtrudeOperation, KernelBody, KernelWire,
    MassProperties, ProfileSegment, RigidTransform, SketchPlacement, SolvedSketch,
    DEFAULT_VECTOR_NORM_EPSILON, POSITION_TOLERANCE_M, ROTATION_TOLERANCE,
};

mod mesh;
mod rib;
mod thread;
use rib::Rib;

const RECORD_BUDGET: usize = 100_000;
const CUT_BUDGET: usize = 64;
const MAX_LENGTH_M: f64 = 1_000.;
const MAX_POSITION_M: f64 = 1_000_000.;

pub(crate) fn unsupported() -> OpenCadError {
    invalid("operation/query is outside analytic Mock admission")
}
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("analytic Mock: {message}"))
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn scale(a: [f64; 3], factor: f64) -> [f64; 3] {
    a.map(|v| v * factor)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(v: [f64; 3]) -> Result<[f64; 3]> {
    let length = dot(v, v).sqrt();
    if !length.is_finite() || length <= DEFAULT_VECTOR_NORM_EPSILON {
        return Err(invalid("direction must be finite and nonzero"));
    }
    Ok(scale(v, 1. / length))
}
fn dimension(value: f64) -> Result<()> {
    if !value.is_finite() || value <= POSITION_TOLERANCE_M || value > MAX_LENGTH_M {
        return Err(invalid(
            "length must be finite, above 1e-9 m and at most 1000 m",
        ));
    }
    Ok(())
}
fn position(point: [f64; 3]) -> Result<()> {
    if point
        .iter()
        .any(|v| !v.is_finite() || v.abs() > MAX_POSITION_M)
    {
        return Err(invalid("position must be finite and within 1e6 m"));
    }
    Ok(())
}
// Only arithmetic roundoff is coalesced. Near-but-distinct thin material
// intervals below the admission tolerance are refused, never erased.
fn roundoff(length: f64) -> f64 {
    64. * f64::EPSILON * length.max(1.)
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    origin: [f64; 3],
    axes: [[f64; 3]; 3],
}
impl Frame {
    fn map(self, local: [f64; 3]) -> [f64; 3] {
        (0..3).fold(self.origin, |point, i| {
            add(point, scale(self.axes[i], local[i]))
        })
    }
    fn local(self, world: [f64; 3]) -> [f64; 3] {
        self.axes.map(|axis| dot(sub(world, self.origin), axis))
    }
    fn transformed(self, transform: RigidTransform) -> Self {
        Self {
            origin: transform.transform_point(self.origin),
            axes: self
                .axes
                .map(|axis| transform.rotation.map(|row| dot(row, axis))),
        }
    }
}

#[derive(Clone, Debug)]
enum Profile {
    Rectangle {
        frame: Frame,
        sides: [f64; 2],
    },
    Circle {
        center: [f64; 3],
        normal: [f64; 3],
        radius: f64,
    },
}
impl Profile {
    fn from_sketch(sketch: &SolvedSketch) -> Result<Self> {
        if !sketch.closed || sketch.ellipse.is_some() || sketch.cubic_bezier.is_some() {
            return Err(invalid(
                "only exact closed rectangle/circle profiles are admitted",
            ));
        }
        let plane = sketch.placement.unwrap_or(SketchPlacement::global_xy());
        position(plane.origin_m)?;
        for axis in [plane.x_axis_m, plane.y_axis_m] {
            if (dot(axis, axis) - 1.).abs() > ROTATION_TOLERANCE
                || axis.iter().any(|v| !v.is_finite())
            {
                return Err(invalid("sketch frame must be orthonormal"));
            }
        }
        if dot(plane.x_axis_m, plane.y_axis_m).abs() > ROTATION_TOLERANCE {
            return Err(invalid("sketch frame must be orthonormal"));
        }
        if let Some(circle) = sketch.circle {
            dimension(circle.radius_m)?;
            if !sketch.segments.is_empty() || circle.center_m.iter().any(|v| !v.is_finite()) {
                return Err(invalid("circle profile cannot contain mixed segments"));
            }
            let center = plane.map_point(circle.center_m[0], circle.center_m[1]);
            position(center)?;
            return Ok(Self::Circle {
                center,
                normal: unit(cross(plane.x_axis_m, plane.y_axis_m))?,
                radius: circle.radius_m,
            });
        }
        if sketch.points.len() != 4 || sketch.points.iter().flatten().any(|v| !v.is_finite()) {
            return Err(invalid("rectangle requires four finite corners"));
        }
        if !sketch.segments.is_empty() {
            if sketch.segments.len() != 4 {
                return Err(invalid("rectangle segments do not match its corners"));
            }
            for (i, segment) in sketch.segments.iter().enumerate() {
                let ProfileSegment::Line { start_m, end_m } = segment else {
                    return Err(invalid("curved rectangle segments are unsupported"));
                };
                for (actual, expected) in [
                    (*start_m, sketch.points[i]),
                    (*end_m, sketch.points[(i + 1) % 4]),
                ] {
                    if actual
                        .into_iter()
                        .zip(expected)
                        .any(|(a, b)| !a.is_finite() || (a - b).abs() > POSITION_TOLERANCE_M)
                    {
                        return Err(invalid("rectangle segments do not match its corners"));
                    }
                }
            }
        }
        let points: Vec<_> = sketch
            .points
            .iter()
            .map(|p| plane.map_point(p[0], p[1]))
            .collect();
        for point in &points {
            position(*point)?;
        }
        let u = sub(points[1], points[0]);
        let v = sub(points[3], points[0]);
        let sides = [dot(u, u).sqrt(), dot(v, v).sqrt()];
        for side in sides {
            dimension(side)?;
        }
        let axes = [unit(u)?, unit(v)?];
        if dot(axes[0], axes[1]).abs() > ROTATION_TOLERANCE
            || sub(points[2], add(points[0], add(u, v)))
                .iter()
                .any(|v| v.abs() > POSITION_TOLERANCE_M)
        {
            return Err(invalid("profile is not a right rectangle"));
        }
        Ok(Self::Rectangle {
            frame: Frame {
                origin: points[0],
                axes: [axes[0], axes[1], unit(cross(axes[0], axes[1]))?],
            },
            sides,
        })
    }
    fn extruded(&self, extent: &ExtrudeExtent, direction: [f64; 3]) -> Result<Body> {
        let length = match extent {
            ExtrudeExtent::Distance { length } => length.meters(),
            ExtrudeExtent::Symmetric { length } => length.meters() * 2.,
            ExtrudeExtent::ThroughAll => {
                return Err(invalid(
                    "through-all needs a future explicit bounded contract",
                ))
            }
        };
        dimension(length)?;
        let direction = unit(direction)?;
        let normal = match self {
            Self::Rectangle { frame, .. } => frame.axes[2],
            Self::Circle { normal, .. } => *normal,
        };
        let lateral = cross(normal, direction);
        if dot(lateral, lateral).sqrt() > ROTATION_TOLERANCE {
            return Err(invalid("oblique extrusion is unsupported"));
        }
        let shift = if matches!(extent, ExtrudeExtent::Symmetric { .. }) {
            scale(direction, -length / 2.)
        } else {
            [0.; 3]
        };
        Ok(match self {
            Self::Rectangle { frame, sides } => Body::Stock {
                frame: Frame {
                    origin: add(frame.origin, shift),
                    axes: [frame.axes[0], frame.axes[1], direction],
                },
                dimensions: [sides[0], sides[1], length],
                cuts: Vec::new(),
            },
            Self::Circle { center, radius, .. } => Body::Cylinder {
                origin: add(*center, shift),
                axis: direction,
                radius: *radius,
                length,
            },
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Cut {
    center: [f64; 2],
    start: f64,
    end: f64,
    radius: f64,
    end_radius: f64,
}
impl Cut {
    fn radius_at(&self, z: f64) -> f64 {
        if (z - self.start).abs() <= roundoff(self.end) {
            return self.radius;
        }
        if (z - self.end).abs() <= roundoff(self.end) {
            return self.end_radius;
        }
        self.radius + (self.end_radius - self.radius) * ((z - self.start) / (self.end - self.start))
    }
}
#[derive(Clone, Debug)]
enum Body {
    Thread(crate::NominalThreadGeometry),
    RibbedStock {
        frame: Frame,
        dimensions: [f64; 3],
        rib: Rib,
    },
    Stock {
        frame: Frame,
        dimensions: [f64; 3],
        cuts: Vec<Cut>,
    },
    Cylinder {
        origin: [f64; 3],
        axis: [f64; 3],
        radius: f64,
        length: f64,
    },
    Frustum {
        origin: [f64; 3],
        axis: [f64; 3],
        radius: f64,
        end_radius: f64,
        length: f64,
    },
}
// Exact axial decomposition, not geometric sampling.
fn intervals(cuts: &[Cut], height: f64) -> Result<Vec<[f64; 4]>> {
    let epsilon = roundoff(height);
    let mut endpoints = vec![0., height];
    endpoints.extend(cuts.iter().flat_map(|cut| [cut.start, cut.end]));
    endpoints.sort_by(f64::total_cmp);
    let mut distinct: Vec<f64> = Vec::new();
    for point in endpoints {
        if let Some(last) = distinct.last() {
            let gap = point - last;
            if gap <= epsilon {
                continue;
            }
            if gap <= POSITION_TOLERANCE_M {
                return Err(invalid("ambiguous thin axial interval"));
            }
        }
        distinct.push(point);
    }
    Ok(distinct
        .windows(2)
        .map(|pair| {
            let middle = (pair[0] + pair[1]) / 2.;
            // Cylindrical chains have constant envelopes. ADR-064 admits only
            // one taper with its matching bore, whose join is an exact endpoint.
            let active = cuts
                .iter()
                .filter(|cut| middle >= cut.start - epsilon && middle <= cut.end + epsilon)
                .max_by(|a, b| a.radius_at(middle).total_cmp(&b.radius_at(middle)));
            let (r0, r1) = active
                .map(|cut| (cut.radius_at(pair[0]), cut.radius_at(pair[1])))
                .unwrap_or((0., 0.));
            [pair[0], pair[1], r0, r1]
        })
        .collect())
}
impl Body {
    fn subtract(&self, tool: &Self) -> Result<Self> {
        let (
            Self::Stock {
                frame,
                dimensions,
                cuts,
            },
            Self::Cylinder {
                origin,
                axis,
                radius,
                length,
            },
        ) = (self, tool)
        else {
            return Err(invalid(
                "subtraction requires rectangular stock and a cylinder",
            ));
        };
        if cuts
            .iter()
            .any(|c| (c.radius - c.end_radius).abs() > roundoff(c.radius.max(c.end_radius)))
        {
            return Err(invalid("generic cuts on tapered stock are unsupported"));
        }
        if cuts.len() >= CUT_BUDGET {
            return Err(invalid("64-cut budget exceeded"));
        }
        if dot(*axis, frame.axes[0]).abs() > ROTATION_TOLERANCE
            || dot(*axis, frame.axes[1]).abs() > ROTATION_TOLERANCE
            || (dot(*axis, frame.axes[2]).abs() - 1.).abs() > ROTATION_TOLERANCE
        {
            return Err(invalid("oblique cylindrical cut is unsupported"));
        }
        let local = frame.local(*origin);
        for i in 0..2 {
            if local[i] - radius <= POSITION_TOLERANCE_M
                || dimensions[i] - local[i] - radius <= POSITION_TOLERANCE_M
            {
                return Err(invalid("cut must be strictly radially contained"));
            }
        }
        let epsilon = roundoff(dimensions[2]);
        let axial_end = local[2] + dot(*axis, frame.axes[2]) * length;
        let mut ends = [local[2].min(axial_end), local[2].max(axial_end)];
        for end in &mut ends {
            if end.abs() <= epsilon {
                *end = 0.;
            }
            if (*end - dimensions[2]).abs() <= epsilon {
                *end = dimensions[2];
            }
            if *end < 0.
                || *end > dimensions[2]
                || (*end > epsilon && *end <= POSITION_TOLERANCE_M)
                || (dimensions[2] - *end > epsilon && dimensions[2] - *end <= POSITION_TOLERANCE_M)
            {
                return Err(invalid("cut must have unambiguous bounded axial endpoints"));
            }
        }
        let center = [local[0], local[1]];
        for previous in cuts {
            if center
                .into_iter()
                .zip(previous.center)
                .any(|(a, b)| (a - b).abs() > epsilon)
            {
                return Err(invalid("only one concentric cut chain is admitted"));
            }
            let radius_gap = (radius - previous.radius).abs();
            if radius_gap > epsilon && radius_gap <= POSITION_TOLERANCE_M {
                return Err(invalid("ambiguous thin radial step"));
            }
        }
        let mut cuts = cuts.clone();
        cuts.push(Cut {
            center,
            start: ends[0],
            end: ends[1],
            radius: *radius,
            end_radius: *radius,
        });
        intervals(&cuts, dimensions[2])?;
        Ok(Self::Stock {
            frame: *frame,
            dimensions: *dimensions,
            cuts,
        })
    }
    pub(crate) fn mass(&self, density: f64) -> Result<MassProperties> {
        if !density.is_finite() || density <= 0. {
            return Err(invalid("density must be finite and positive in kg/m^3"));
        }
        let (volume, area, center) = match self {
            Self::Thread(g) => {
                let m = g.mass_properties(density)?;
                (m.volume_m3, m.area_m2, m.center_of_mass)
            }
            Self::RibbedStock {
                frame,
                dimensions,
                rib,
            } => rib.mass(*frame, *dimensions)?,
            Self::Cylinder {
                origin,
                axis,
                radius,
                length,
            } => (
                PI * radius * radius * length,
                2. * PI * radius * (length + radius),
                add(*origin, scale(*axis, length / 2.)),
            ),
            Self::Frustum {
                origin,
                axis,
                radius: a,
                end_radius: b,
                length: h,
            } => {
                if axis.iter().any(|v| !v.is_finite())
                    || (dot(*axis, *axis) - 1.).abs() > ROTATION_TOLERANCE
                {
                    return Err(invalid("accumulated frustum axis drift"));
                }
                let sum = a * a + a * b + b * b;
                let v = PI * h * sum / 3.;
                let z = h * (a * a + 2. * a * b + 3. * b * b) / (4. * sum);
                (
                    v,
                    PI * (a * a + b * b) + PI * (a + b) * h.hypot(b - a),
                    add(*origin, scale(*axis, z)),
                )
            }
            Self::Stock {
                frame,
                dimensions: [x, y, z],
                cuts,
            } => {
                let base_volume = x * y * z;
                let mut volume = base_volume;
                let mut first = [
                    base_volume * x / 2.,
                    base_volume * y / 2.,
                    base_volume * z / 2.,
                ];
                let mut area = 2. * (x * y + x * z + y * z);
                let mut previous_radius_squared = 0.;
                let epsilon = roundoff(*z);
                for [start, end, r0, r1] in intervals(cuts, *z)? {
                    let length = end - start;
                    let squared = r0 * r0;
                    let removed = PI * length * (r0 * r0 + r0 * r1 + r1 * r1) / 3.;
                    volume -= removed;
                    if let Some(cut) = cuts.first() {
                        first[0] -= removed * cut.center[0];
                        first[1] -= removed * cut.center[1];
                    }
                    first[2] -= removed * start
                        + PI * length * length * (r0 * r0 + 2. * r0 * r1 + 3. * r1 * r1) / 12.;
                    area += PI * (r0 + r1) * length.hypot(r1 - r0);
                    if start <= epsilon {
                        area -= PI * squared;
                    } else {
                        area += PI * (previous_radius_squared - squared).abs();
                    }
                    if (*z - end).abs() <= epsilon {
                        area -= PI * r1 * r1;
                    }
                    previous_radius_squared = r1 * r1;
                }
                if !volume.is_finite() || volume <= 0. {
                    return Err(invalid("invalid remaining volume"));
                }
                (volume, area, frame.map(first.map(|value| value / volume)))
            }
        };
        if !volume.is_finite()
            || volume <= 0.
            || !area.is_finite()
            || area <= 0.
            || !(density * volume).is_finite()
            || center.iter().any(|v| !v.is_finite())
        {
            return Err(invalid("non-finite or empty physical measurement"));
        }
        Ok(MassProperties {
            volume_m3: volume,
            area_m2: area,
            mass_kg: density * volume,
            center_of_mass: center,
        })
    }
    pub(crate) fn bounds(&self) -> BoundingBox {
        match self {
            Self::Thread(g) => g.bounding_box(),
            Self::RibbedStock {
                frame,
                dimensions,
                rib,
            } => {
                let mut bounds = Self::Stock {
                    frame: *frame,
                    dimensions: *dimensions,
                    cuts: Vec::new(),
                }
                .bounds();
                for p in rib.vertices(dimensions[2]).map(|p| frame.map(p)) {
                    for (i, v) in p.into_iter().enumerate() {
                        bounds.min[i] = bounds.min[i].min(v);
                        bounds.max[i] = bounds.max[i].max(v);
                    }
                }
                bounds
            }
            Self::Cylinder {
                origin,
                axis,
                radius,
                length,
            } => {
                let end = add(*origin, scale(*axis, *length));
                let radial = axis.map(|v| radius * (1. - v * v).max(0.).sqrt());
                BoundingBox {
                    min: std::array::from_fn(|i| origin[i].min(end[i]) - radial[i]),
                    max: std::array::from_fn(|i| origin[i].max(end[i]) + radial[i]),
                }
            }
            Self::Frustum {
                origin,
                axis,
                radius,
                end_radius,
                length,
            } => {
                let end = add(*origin, scale(*axis, *length));
                let radial = axis.map(|v| (1. - v * v).max(0.).sqrt());
                BoundingBox {
                    min: std::array::from_fn(|i| {
                        (origin[i] - radius * radial[i]).min(end[i] - end_radius * radial[i])
                    }),
                    max: std::array::from_fn(|i| {
                        (origin[i] + radius * radial[i]).max(end[i] + end_radius * radial[i])
                    }),
                }
            }
            Self::Stock {
                frame, dimensions, ..
            } => {
                let corners: Vec<_> = (0..8)
                    .map(|mask| {
                        frame.map(std::array::from_fn(|i| {
                            if mask & (1 << i) != 0 {
                                dimensions[i]
                            } else {
                                0.
                            }
                        }))
                    })
                    .collect();
                BoundingBox {
                    min: std::array::from_fn(|i| {
                        corners.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min)
                    }),
                    max: std::array::from_fn(|i| {
                        corners
                            .iter()
                            .map(|p| p[i])
                            .fold(f64::NEG_INFINITY, f64::max)
                    }),
                }
            }
        }
    }
    fn transformed(&self, transform: RigidTransform) -> Result<Self> {
        position(transform.translation_m)?;
        let rows = transform.rotation;
        if rows.iter().flatten().any(|v| !v.is_finite())
            || (0..3).any(|i| (dot(rows[i], rows[i]) - 1.).abs() > ROTATION_TOLERANCE)
            || (0..3).any(|i| (0..i).any(|j| dot(rows[i], rows[j]).abs() > ROTATION_TOLERANCE))
            || (dot(cross(rows[0], rows[1]), rows[2]) - 1.).abs() > ROTATION_TOLERANCE
        {
            return Err(invalid("transform must be a finite proper rigid rotation"));
        }
        Ok(match self {
            Self::Thread(g) => Self::Thread(g.transformed(transform)?),
            Self::RibbedStock {
                frame,
                dimensions,
                rib,
            } => Self::RibbedStock {
                frame: frame.transformed(transform),
                dimensions: *dimensions,
                rib: rib.clone(),
            },
            Self::Stock {
                frame,
                dimensions,
                cuts,
            } => Self::Stock {
                frame: frame.transformed(transform),
                dimensions: *dimensions,
                cuts: cuts.clone(),
            },
            Self::Frustum {
                origin,
                axis,
                radius,
                end_radius,
                length,
            } => Self::Frustum {
                origin: transform.transform_point(*origin),
                axis: rows.map(|row| dot(row, *axis)),
                radius: *radius,
                end_radius: *end_radius,
                length: *length,
            },
            Self::Cylinder {
                origin,
                axis,
                radius,
                length,
            } => Self::Cylinder {
                origin: transform.transform_point(*origin),
                axis: rows.map(|row| dot(row, *axis)),
                radius: *radius,
                length: *length,
            },
        })
    }
}

#[derive(Default, Debug)]
pub(crate) struct AnalyticMock {
    wires: BTreeMap<u64, Profile>,
    bodies: BTreeMap<u64, Body>,
    faces: BTreeMap<u64, BTreeMap<mesh::FaceKey, u64>>,
    rims: BTreeMap<u64, [u64; 2]>,
    histories: BTreeMap<u64, Vec<(u64, u64)>>,
    next_face: u64,
}
impl AnalyticMock {
    fn next_id(count: usize) -> Result<u64> {
        if count >= RECORD_BUDGET {
            return Err(invalid("100000-record budget exceeded"));
        }
        // Distinct from mesh MAX, ellipse MAX/2, cubic MAX/3 and compound MAX/4.
        Ok(u64::MAX / 5 - count as u64)
    }
    pub(crate) fn contains(&self, body: &KernelBody) -> bool {
        self.bodies.contains_key(&body.0)
    }
    fn body(&self, body: &KernelBody) -> Result<&Body> {
        self.bodies
            .get(&body.0)
            .ok_or_else(|| invalid("unknown or untracked physical body"))
    }
    pub(crate) fn mass(&self, body: &KernelBody, density: f64) -> Result<MassProperties> {
        self.body(body)?.mass(density)
    }
    pub(crate) fn bounds(&self, body: &KernelBody) -> Result<BoundingBox> {
        Ok(self.body(body)?.bounds())
    }
    fn store(
        &mut self,
        body: Body,
        source: Option<KernelBody>,
        preserve: bool,
    ) -> Result<KernelBody> {
        let id = Self::next_id(self.bodies.len())?;
        body.mass(1.)?;
        let bounds = body.bounds();
        position(bounds.min)?;
        position(bounds.max)?;
        let keys = mesh::keys(&body)?;
        let previous = source.as_ref().and_then(|b| self.faces.get(&b.0));
        let mut faces = BTreeMap::new();
        let mut history = Vec::new();
        for key in keys {
            let old = previous.and_then(|p| p.get(&key)).copied();
            let face = if preserve && old.is_some() {
                old.ok_or_else(|| invalid("missing inherited face"))?
            } else {
                self.next_face += 1;
                u64::MAX / 6 - self.next_face
            };
            if let Some(old) = old {
                history.push((face, old));
            } else if let mesh::FaceKey::Wall(a, b, r) = &key {
                // A later cut may split an existing cylindrical wall. Retain
                // its actual same-radius overlapping ancestry, not a nearby face.
                if let Some(previous) = previous {
                    for (parent, parent_id) in previous {
                        if let mesh::FaceKey::Wall(c, d, s) = parent {
                            if r == s
                                && f64::from_bits(*a).max(f64::from_bits(*c))
                                    < f64::from_bits(*b).min(f64::from_bits(*d))
                            {
                                history.push((face, *parent_id));
                            }
                        }
                    }
                }
            }
            faces.insert(key, face);
        }
        self.faces.insert(id, faces);
        self.histories.insert(id, history);
        if matches!(&body, Body::Cylinder { .. }) {
            // The 100000-record budget bounds this arithmetic; MAX/7 is
            // separate from physical body/face and legacy synthetic namespaces.
            let first = u64::MAX / 7 - 2 * self.bodies.len() as u64;
            self.rims.insert(id, [first, first - 1]);
        }
        self.bodies.insert(id, body);
        Ok(KernelBody::new(id))
    }
    pub(crate) fn wire(&mut self, sketch: &SolvedSketch) -> Result<KernelWire> {
        let profile = Profile::from_sketch(sketch)?;
        let id = Self::next_id(self.wires.len())?;
        self.wires.insert(id, profile);
        Ok(KernelWire::new(id))
    }
    pub(crate) fn extrude(
        &mut self,
        wire: KernelWire,
        extent: ExtrudeExtent,
        operation: ExtrudeOperation,
        target: Option<KernelBody>,
        direction: [f64; 3],
    ) -> Result<KernelBody> {
        let profile = self
            .wires
            .get(&wire.0)
            .ok_or_else(|| invalid("unknown physical wire"))?;
        let tool = profile.extruded(&extent, direction)?;
        let source = target.clone();
        let output = match (operation, target) {
            (ExtrudeOperation::NewBody, None) => tool,
            (ExtrudeOperation::Cut, Some(target)) => self.body(&target)?.subtract(&tool)?,
            _ => {
                return Err(invalid(
                    "requires NewBody without target or cylinder Cut with stock target",
                ))
            }
        };
        self.store(output, source, true)
    }
    pub(crate) fn boolean(
        &mut self,
        lhs: KernelBody,
        rhs: KernelBody,
        op: BooleanOp,
    ) -> Result<KernelBody> {
        if op != BooleanOp::Subtract {
            return Err(unsupported());
        }
        let output = self.body(&lhs)?.subtract(self.body(&rhs)?)?;
        self.store(output, Some(lhs), true)
    }
    pub(crate) fn cylinder_geometry(
        &self,
        body: &KernelBody,
        axis_hint: [f64; 3],
    ) -> Result<crate::CylindricalStockGeometry> {
        let Body::Cylinder {
            origin,
            axis,
            radius,
            length,
        } = self.body(body)?
        else {
            return Err(unsupported());
        };
        let ids = &self.faces[&body.0];
        crate::CylindricalStockGeometry {
            origin_m: *origin,
            axis: *axis,
            radius_m: *radius,
            height_m: *length,
            start_face_id: ids[&mesh::FaceKey::Cylinder(0)],
            end_face_id: ids[&mesh::FaceKey::Cylinder(1)],
            lateral_face_id: ids[&mesh::FaceKey::Cylinder(2)],
        }
        .oriented(axis_hint)
    }
    pub(crate) fn cylinder_rims(
        &self,
        body: &KernelBody,
        axis_hint: [f64; 3],
    ) -> Result<crate::CylindricalRimsGeometry> {
        let stock = self.cylinder_geometry(body, axis_hint)?;
        let ids = self.rims.get(&body.0).ok_or_else(unsupported)?;
        let Body::Cylinder { axis, .. } = self.body(body)? else {
            return Err(unsupported());
        };
        let [start_edge_id, end_edge_id] = if dot(stock.axis, *axis) > 0. {
            *ids
        } else {
            [ids[1], ids[0]]
        };
        let witness = crate::CylindricalRimsGeometry {
            stock,
            start_edge_id,
            end_edge_id,
        };
        witness.validate_circles(
            &self.rim_circle(body, start_edge_id)?,
            &self.rim_circle(body, end_edge_id)?,
        )?;
        Ok(witness)
    }

    pub(crate) fn rim_circle(
        &self,
        body: &KernelBody,
        edge_id: u64,
    ) -> Result<crate::CircularEdgeGeometry> {
        let Body::Cylinder {
            origin,
            axis,
            radius,
            length,
        } = self.body(body)?
        else {
            return Err(unsupported());
        };
        let ids = self.rims.get(&body.0).ok_or_else(unsupported)?;
        let z = if edge_id == ids[0] {
            0.
        } else if edge_id == ids[1] {
            *length
        } else {
            return Err(invalid("edge is not a current body-owned cylinder rim"));
        };
        Ok(crate::CircularEdgeGeometry {
            center_m: add(*origin, scale(*axis, z)),
            normal: *axis,
            radius_m: *radius,
        })
    }

    pub(crate) fn nominal_thread(
        &mut self,
        body: KernelBody,
        input: &crate::NominalThreadInput,
    ) -> Result<KernelBody> {
        let witness = self.cylinder_geometry(&body, input.axis_hint)?;
        let geometry = input.admit(&witness)?;
        self.store(Body::Thread(geometry), None, false)
    }
    pub(crate) fn cylindrical_draft(
        &mut self,
        body: KernelBody,
        input: &crate::CylindricalDraftInput,
    ) -> Result<KernelBody> {
        let stock = self.cylinder_geometry(&body, input.axis_hint)?;
        let [radius, end_radius] = input.endpoint_radii(&stock)?;
        self.store(
            Body::Frustum {
                origin: stock.origin_m,
                axis: stock.axis,
                radius,
                end_radius,
                length: stock.height_m,
            },
            None,
            false,
        )
    }

    pub(crate) fn tapered_rib(
        &mut self,
        body: KernelBody,
        input: &crate::TaperedRibInput,
    ) -> Result<KernelBody> {
        let witness = self.stock_geometry(&body, input.entry)?;
        let geometry = input.admit(&witness)?;
        let swap = dot(
            cross(witness.u_axis, witness.v_axis),
            witness.outward_normal,
        ) < 0.;
        let [u, v] = if swap {
            [witness.v_axis, witness.u_axis]
        } else {
            [witness.u_axis, witness.v_axis]
        };
        let frame = Frame {
            origin: sub(
                witness.origin_m,
                scale(witness.outward_normal, witness.inward_thickness_m),
            ),
            axes: [u, v, witness.outward_normal],
        };
        let spans = if swap {
            [witness.face_spans_m[1], witness.face_spans_m[0]]
        } else {
            witness.face_spans_m
        };
        let dimensions = [spans[0], spans[1], witness.inward_thickness_m];
        let local = frame.local(geometry.base_center_m);
        let rib = Rib {
            center: [local[0], local[1]],
            along_u: dot(u, geometry.length_axis).abs() > 0.999999,
            length: geometry.length_m,
            base_thickness: geometry.base_thickness_m,
            top_thickness: geometry.top_thickness_m,
            height: geometry.height_m,
        };
        self.store(
            Body::RibbedStock {
                frame,
                dimensions,
                rib,
            },
            None,
            false,
        )
    }

    pub(crate) fn countersink(
        &mut self,
        body: KernelBody,
        input: &crate::CountersinkCut,
    ) -> Result<KernelBody> {
        let witness = self.stock_geometry(&body, input.entry)?;
        input.validate(&witness)?;
        let Body::Stock {
            frame, dimensions, ..
        } = self.body(&body)?
        else {
            return Err(unsupported());
        };
        let frame = *frame;
        let dimensions = *dimensions;
        let local = frame.local(input.entry.point_m);
        let sign = -dot(input.entry.normal, frame.axes[2]);
        // This analytic representation admits either extrusion end only.
        if (sign.abs() - 1.).abs() > ROTATION_TOLERANCE
            || dot(input.entry.normal, frame.axes[0]).abs() > ROTATION_TOLERANCE
            || dot(input.entry.normal, frame.axes[1]).abs() > ROTATION_TOLERANCE
        {
            return Err(invalid("analytic countersink requires stock extrusion end"));
        }
        let endpoint = |depth: f64| {
            let z = local[2] + sign * depth;
            if z.abs() <= roundoff(dimensions[2]) {
                0.
            } else if (z - dimensions[2]).abs() <= roundoff(dimensions[2]) {
                dimensions[2]
            } else {
                z
            }
        };
        let bore_end = endpoint(input.bore_depth_m);
        let seat_end = endpoint(input.seat_depth_m);
        let center = [local[0], local[1]];
        let bore = Cut {
            center,
            start: local[2].min(bore_end),
            end: local[2].max(bore_end),
            radius: input.bore_radius_m,
            end_radius: input.bore_radius_m,
        };
        let taper = Cut {
            center,
            start: local[2].min(seat_end),
            end: local[2].max(seat_end),
            radius: if sign > 0. {
                input.seat_radius_m
            } else {
                input.bore_radius_m
            },
            end_radius: if sign > 0. {
                input.bore_radius_m
            } else {
                input.seat_radius_m
            },
        };
        self.store(
            Body::Stock {
                frame,
                dimensions,
                cuts: vec![bore, taper],
            },
            Some(body),
            true,
        )
    }

    pub(crate) fn transformed(
        &mut self,
        body: KernelBody,
        transform: RigidTransform,
    ) -> Result<KernelBody> {
        let output = self.body(&body)?.transformed(transform)?;
        self.store(output, Some(body), false)
    }
}
