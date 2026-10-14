//! ADR-069 guarded nominal basic flat-root external thread source.
use crate::sketch_bridge::{
    placement_from_workplane, profile_to_solved_with_context, resolve_workplane,
};
use crate::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use opencad_core::{Angle, Length, OpenCadError, Result};
use opencad_geometry::{
    CylindricalStockGeometry, ExtrudeExtent, ExtrudeOperation, FaceRefDiscovery, KernelBody,
    MetricThreadSize, NominalMetricThread, NominalThreadGeometry, NominalThreadInput,
    SketchPlacement, TessellationSettings, ThreadHandedness, TopoRefKind,
};
use opencad_sketch::{Coord, Sketch, SketchEntity, Workplane};
use serde::{Deserialize, Serialize};
const EPS: f64 = 1e-9;
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("nominal metric thread: {message}"))
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
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
fn frame(p: SketchPlacement) -> Result<[f64; 3]> {
    if p.origin_m
        .iter()
        .chain(&p.x_axis_m)
        .chain(&p.y_axis_m)
        .any(|v| !v.is_finite())
        || (norm(p.x_axis_m) - 1.).abs() > EPS
        || (norm(p.y_axis_m) - 1.).abs() > EPS
        || dot(p.x_axis_m, p.y_axis_m).abs() > EPS
    {
        return Err(invalid("nonorthonormal source phase frame"));
    }
    Ok(cross(p.x_axis_m, p.y_axis_m))
}
fn checked_placement(plane: &Workplane) -> Result<SketchPlacement> {
    if let Workplane::Custom { normal, x_axis, .. } = plane {
        if normal.iter().chain(x_axis).any(|v| !v.is_finite())
            || (norm(*normal) - 1.).abs() > EPS
            || (norm(*x_axis) - 1.).abs() > EPS
            || dot(*normal, *x_axis).abs() > EPS
        {
            return Err(invalid(
                "custom plane must carry finite unit orthogonal axes",
            ));
        }
    }
    let placement = placement_from_workplane(plane)?;
    frame(placement)?;
    Ok(placement)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NominalThreadRepresentation {
    NominalMetricBasicFlatRoot,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NominalThreadEnds {
    Square,
}
fn turns<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<u32, D::Error> {
    let n = u32::deserialize(d)?;
    if !(1..=32).contains(&n) {
        return Err(serde::de::Error::custom(
            "thread turns must be an integer in 1..=32",
        ));
    }
    Ok(n)
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NominalMetricThreadFeature {
    pub target_feature: String,
    pub face_ref: String,
    pub representation: NominalThreadRepresentation,
    pub size: MetricThreadSize,
    pub handedness: ThreadHandedness,
    #[serde(deserialize_with = "turns")]
    pub turns: u32,
    pub ends: NominalThreadEnds,
}
impl NominalMetricThreadFeature {
    pub fn profile(&self) -> Result<NominalMetricThread> {
        NominalMetricThread::new(self.size, self.handedness, self.turns)
    }
    pub fn validate(&self) -> Result<()> {
        if !self.target_feature.starts_with("feature:")
            || self.target_feature.len() <= 8
            || !self.face_ref.starts_with("ref:face:")
            || self.face_ref.len() <= 9
        {
            return Err(invalid(
                "target and owned cylindrical face reference required",
            ));
        }
        self.profile()?;
        Ok(())
    }
    /// Validate original fallbacks before parameter application can overwrite them.
    pub fn validate_source(
        &self,
        nodes: &[&FeatureNode],
        sketches: &[&Sketch],
        scope: &indexmap::IndexMap<String, f64>,
    ) -> Result<()> {
        self.validate()?;
        let target = nodes
            .iter()
            .find(|n| n.id == self.target_feature && !n.suppressed)
            .ok_or_else(|| invalid("missing target"))?;
        let FeatureDefinition::Extrude(e) = &target.definition else {
            return Err(invalid("original extrusion required"));
        };
        let ExtrudeExtent::Distance { length } = e.extent else {
            return Err(invalid("distance extent required"));
        };
        if !length.meters().is_finite()
            || length.meters() <= EPS
            || e.operation != ExtrudeOperation::NewBody
            || e.target_feature.is_some()
        {
            return Err(invalid("invalid original cylinder fallback/operation"));
        }
        if let Some(expr) = &e.length_expr {
            strict_length(expr, scope)?;
        }
        let source = nodes
            .iter()
            .find(|n| n.id == e.sketch_feature && !n.suppressed)
            .ok_or_else(|| invalid("missing authored sketch"))?;
        let FeatureDefinition::Sketch(s) = &source.definition else {
            return Err(invalid("authored sketch required"));
        };
        let sketch = sketches
            .iter()
            .find(|s0| s0.id.as_str() == s.sketch_id)
            .ok_or_else(|| invalid("missing authored sketch data"))?;
        checked_placement_without_resolution(&sketch.workplane)?;
        for constraint in &sketch.constraints {
            match constraint {
                opencad_sketch::Constraint::Distance { expr, .. } => {
                    if strict_length(expr.as_str(), scope)? < 0. {
                        return Err(invalid("negative source distance constraint"));
                    }
                }
                opencad_sketch::Constraint::Radius { expr, .. }
                | opencad_sketch::Constraint::Diameter { expr, .. }
                    if strict_length(expr.as_str(), scope)? <= EPS =>
                {
                    return Err(invalid("nonpositive source circle constraint"));
                }
                _ => {}
            }
        }
        for entity in &sketch.entities {
            if let SketchEntity::Point(point) = entity {
                for coordinate in [&point.x, &point.y] {
                    let value = match coordinate {
                        Coord::Literal(v) => *v,
                        Coord::Expr(e) => strict_length(e.as_str(), scope)?,
                    };
                    if !value.is_finite() {
                        return Err(invalid("nonfinite source point"));
                    }
                }
            }
            if let SketchEntity::Circle(c) = entity {
                let radius = match &c.radius {
                    Coord::Literal(r) => *r,
                    Coord::Expr(e) => strict_length(e.as_str(), scope)?,
                };
                if !radius.is_finite() || radius <= EPS {
                    return Err(invalid("invalid circle radius fallback/expression"));
                }
            }
        }
        Ok(())
    }
}
fn strict_length(expr: &str, scope: &indexmap::IndexMap<String, f64>) -> Result<f64> {
    if opencad_graph::parameter_names_in_expr(expr)
        .iter()
        .any(|n| opencad_graph::param_eval::is_angle_parameter(n))
    {
        return Err(invalid("length expression references an angular parameter"));
    }
    opencad_graph::eval_length_expr_strict(expr, scope)
}
fn checked_placement_without_resolution(plane: &Workplane) -> Result<()> {
    if matches!(plane, Workplane::Custom { .. }) {
        checked_placement(plane)?;
    }
    Ok(())
}
struct Admission {
    target: KernelBody,
    stock: CylindricalStockGeometry,
    input: NominalThreadInput,
    geometry: NominalThreadGeometry,
}
fn admit(node: &FeatureNode, ctx: &dyn RegenContext) -> Result<Admission> {
    let FeatureDefinition::NominalMetricThread(def) = &node.definition else {
        return Err(invalid("wrong definition"));
    };
    def.validate()?;
    let nodes = ctx.source_feature_nodes()?;
    if nodes.iter().any(|n| {
        n.id != node.id
            && !n.suppressed
            && n.definition.feature_inputs().iter().any(|(field, id)| {
                ["target_feature", "source_feature"].contains(field) && *id == def.target_feature
            })
    }) {
        return Err(invalid(
            "another active solid consumer uses the target stock",
        ));
    }
    let targets: Vec<_> = nodes
        .iter()
        .filter(|n| n.id == def.target_feature && !n.suppressed)
        .collect();
    if targets.len() != 1 {
        return Err(invalid("missing/ambiguous target source"));
    }
    if nodes.iter().any(|n| {
        n.id != node.id
            && !n.suppressed
            && n.definition
                .consumed_face_refs()
                .contains(&def.face_ref.as_str())
    }) {
        return Err(invalid("side reference consumed by another feature"));
    }
    let FeatureDefinition::Extrude(extrude) = &targets[0].definition else {
        return Err(invalid("target must be original exact-circle extrusion"));
    };
    let ExtrudeExtent::Distance { length } = extrude.extent else {
        return Err(invalid("target must use distance extrusion"));
    };
    if extrude.operation != ExtrudeOperation::NewBody || extrude.target_feature.is_some() {
        return Err(invalid("target must be original NewBody cylinder"));
    }
    let sketch_sources: Vec<_> = nodes
        .iter()
        .filter(|n| n.id == extrude.sketch_feature && !n.suppressed)
        .collect();
    if sketch_sources.len() != 1
        || !matches!(sketch_sources[0].definition, FeatureDefinition::Sketch(_))
    {
        return Err(invalid("target needs an original authored circle sketch"));
    }
    let sketch = ctx.sketch_for_feature(&extrude.sketch_feature)?;
    checked_placement(&resolve_workplane(&sketch.workplane, ctx)?)?;
    let profile = profile_to_solved_with_context(sketch, &extrude.profile_ref, ctx)?;
    let circle = profile
        .circle
        .ok_or_else(|| invalid("target profile must be an exact circle"))?;
    if !profile.segments.is_empty() || profile.ellipse.is_some() || profile.cubic_bezier.is_some() {
        return Err(invalid("mixed or approximated circle unsupported"));
    }
    let placement = profile
        .placement
        .ok_or_else(|| invalid("missing source placement"))?;
    let axis = frame(placement)?;
    let origin = placement.map_point(circle.center_m[0], circle.center_m[1]);
    if origin.iter().any(|v| !v.is_finite())
        || !circle.radius_m.is_finite()
        || !length.meters().is_finite()
    {
        return Err(invalid("nonfinite cylinder source"));
    }
    let target = ctx.body_for_feature(&def.target_feature)?;
    let stock = ctx.kernel().cylindrical_stock_geometry(&target, axis)?;
    stock.validate()?;
    if norm(sub(stock.origin_m, origin)) > EPS
        || norm(sub(stock.axis, axis)) > EPS
        || (stock.radius_m - circle.radius_m).abs() > EPS
        || (stock.height_m - length.meters()).abs() > EPS
    {
        return Err(invalid("actual cylinder disagrees with source"));
    }
    let refs: Vec<_> = ctx
        .semantic_refs()
        .iter()
        .filter(|r| r.ref_id.as_str() == def.face_ref)
        .collect();
    if refs.len() != 1
        || refs[0].kind != TopoRefKind::Face
        || refs[0].semantic.created_by != def.target_feature
        || refs[0].semantic.role.as_deref() != Some("cylindrical")
    {
        return Err(invalid(
            "missing, ambiguous or foreign full cylindrical side reference",
        ));
    }
    let reference = refs[0];
    if reference.semantic.normal_hint.is_some_and(|n| {
        !norm(n).is_finite() || (norm(n) - 1.).abs() > 1e-6 || dot(n, axis).abs() > 1e-6
    }) {
        return Err(invalid("side normal annotation must be radial"));
    }
    if let Some(fp) = &reference.geometric_fingerprint {
        if fp.surface_type != "cylinder" || fp.kernel_edge_id.is_some() {
            return Err(invalid("side fingerprint must identify a cylinder"));
        }
        if let Some([lo, hi]) = fp.area_range {
            let area = 2. * std::f64::consts::PI * stock.radius_m * stock.height_m;
            if !lo.is_finite()
                || !hi.is_finite()
                || lo < 0.
                || hi < lo
                || area < lo - 64. * f64::EPSILON * area
                || area > hi + 64. * f64::EPSILON * area
            {
                return Err(invalid("stale side area fingerprint"));
            }
        }
        // The legacy hint bounds a face centroid, not the whole face. Its
        // midpoint must lie within the exact source cylinder's occupied box.
        if let Some(bounds) = fp.bbox_hint {
            for i in 0..3 {
                let [lo, hi] = [bounds[0][i], bounds[1][i]];
                let end = stock.origin_m[i] + axis[i] * stock.height_m;
                let radial = stock.radius_m * (1. - axis[i] * axis[i]).max(0.).sqrt();
                let error = EPS
                    + 4. * f32::EPSILON as f64
                        * lo.abs().max(hi.abs()).max(stock.origin_m[i].abs());
                if !lo.is_finite()
                    || !hi.is_finite()
                    || hi < lo
                    || (lo + hi) / 2. < stock.origin_m[i].min(end) - radial - error
                    || (lo + hi) / 2. > stock.origin_m[i].max(end) + radial + error
                {
                    return Err(invalid("stale side centroid fingerprint"));
                }
            }
        }
    }
    if let Some(id) = reference.kernel_face_id() {
        if [stock.start_face_id, stock.end_face_id].contains(&id) {
            return Err(invalid("side reference bound to a present cap"));
        }
        // IDs absent on this body can only rebind through the unique, typed,
        // owned actual cylindrical witness above; no nearest-face search.
    }

    let input = NominalThreadInput {
        profile: def.profile()?,
        axis_hint: axis,
        radial_axis: placement.x_axis_m,
    };
    let geometry = input.admit(&stock)?;
    ctx.kernel()
        .assign_face_ref(&target, stock.lateral_face_id, reference.ref_id.clone())?;
    Ok(Admission {
        target,
        stock,
        input,
        geometry,
    })
}
fn require_output(
    node: &FeatureNode,
    ctx: &dyn RegenContext,
    body: &KernelBody,
    a: &Admission,
) -> Result<Vec<FaceRefDiscovery>> {
    let profile = a.input.profile;
    let p = profile.pitch().meters();
    let length = profile.length().meters();
    let major = profile.major_diameter().meters() / 2.;
    let root = major - 5. * 3_f64.sqrt() * p / 16.;
    let area_section =
        std::f64::consts::PI * (11. * root * root + 5. * root * major + 8. * major * major) / 24.;
    let volume = area_section * length;
    let expected = a.geometry.mass_properties(1.)?;
    let m = ctx.kernel().mass_properties(body, 1.)?;
    if !m.volume_m3.is_finite()
        || !m.area_m2.is_finite()
        || (m.volume_m3 - volume).abs() > volume * 2e-5
        || (m.area_m2 - expected.area_m2).abs() > expected.area_m2 * 2e-5
    {
        return Err(invalid("output volume/area disagrees with nominal profile"));
    }
    for i in 0..3 {
        if !m.center_of_mass[i].is_finite()
            || (m.center_of_mass[i] - expected.center_of_mass[i]).abs()
                > 2e-8 + 64. * f64::EPSILON * expected.center_of_mass[i].abs()
        {
            return Err(invalid("output first moments disagree"));
        }
    }
    let mesh = ctx
        .kernel()
        .tessellate(body, &TessellationSettings::default())?;
    if mesh.indices.len() % 3 != 0
        || mesh.triangle_count() == 0
        || mesh.triangle_face_ids.len() != mesh.triangle_count()
        || mesh.positions.iter().flatten().any(|v| !v.is_finite())
        || mesh
            .indices
            .iter()
            .any(|i| *i as usize >= mesh.positions.len())
    {
        return Err(invalid("physical finite face-bearing mesh required"));
    }
    crate::seated_standard_hole::validate_closed_mesh(&mesh)?;
    let axes = a.geometry.axes();
    let origin = a.stock.origin_m;
    let coordinate = mesh
        .positions
        .iter()
        .flatten()
        .map(|v| f64::from(v.abs()))
        .fold(major.max(length), f64::max);
    let error = 1e-9 + 8. * f32::EPSILON as f64 * coordinate;
    // Radial law slope sqrt(3), phase propagation P/r for azimuth roundoff.
    let radial_error = error * (1. + 3_f64.sqrt() * (1. + p / (std::f64::consts::TAU * root)));
    if radial_error > (major - root) / 32. {
        return Err(invalid("coordinate precision cannot resolve thread phase"));
    }
    let mut groups = std::collections::BTreeMap::<u64, (usize, [f64; 3], usize)>::new();
    let mut seen = [false; 3];
    for (t, id) in mesh.indices.chunks_exact(3).zip(&mesh.triangle_face_ids) {
        if *id == 0 {
            return Err(invalid("missing physical face ID"));
        }
        let world = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize].map(f64::from));
        let local = world.map(|v| {
            let q = sub(v, origin);
            axes.map(|axis| dot(q, axis))
        });
        let normal = cross(sub(world[1], world[0]), sub(world[2], world[0]));
        let start = local.iter().all(|v| v[2].abs() <= error) && dot(normal, axes[2]) < 0.;
        let end = local.iter().all(|v| (v[2] - length).abs() <= error) && dot(normal, axes[2]) > 0.;
        let class = if start {
            0
        } else if end {
            1
        } else {
            2
        };
        for [x, y, z] in local {
            if z < -error || z > length + error {
                return Err(invalid("output outside square ends"));
            }
            let r = x.hypot(y);
            if class < 2 && r < root - radial_error {
                continue;
            }
            let expected = profile
                .radius_at(
                    Length::from_meters(z.clamp(0., length)),
                    Angle::from_radians(y.atan2(x)),
                )?
                .meters();
            if (class == 2 && (r - expected).abs() > radial_error)
                || (class < 2 && r > expected + radial_error)
            {
                return Err(invalid("output boundary violates requested screw phase"));
            }
        }
        let c = std::array::from_fn(|i| (world[0][i] + world[1][i] + world[2][i]) / 3.);
        if class == 2 {
            let q = sub(c, origin);
            let z = dot(q, axes[2]);
            let radial = sub(q, axes[2].map(|v| v * z));
            if dot(normal, radial) <= 0. {
                return Err(invalid("thread side winding not outward"));
            }
        }
        let entry = groups.entry(*id).or_insert((class, [0.; 3], 0));
        if entry.0 != class {
            return Err(invalid("physical face mixes end and lateral surfaces"));
        }
        for (sum, v) in entry.1.iter_mut().zip(c) {
            *sum += v;
        }
        entry.2 += 1;
        seen[class] = true;
    }
    if seen != [true; 3] {
        return Err(invalid("missing physical end or lateral surface"));
    }
    Ok(groups
        .into_iter()
        .map(|(id, (class, sum, count))| FaceRefDiscovery {
            kernel_face_id: id,
            role: ["thread_start", "thread_end", "thread_surface"][class].into(),
            normal_m: if class == 2 {
                [0.; 3]
            } else {
                axes[2].map(|v| (v * if class == 0 { -1. } else { 1. }) as f32)
            },
            centroid_m: sum.map(|v| (v / count as f64) as f32),
            feature_id: Some(node.id.clone()),
        })
        .collect())
}
#[derive(Debug, Default)]
pub struct NominalMetricThreadFeatureExecutor;
impl Feature for NominalMetricThreadFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "nominal_metric_thread"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let a = admit(node, ctx)?;
        let body = ctx
            .kernel()
            .cut_nominal_metric_thread(a.target.clone(), &a.input)?;
        require_output(node, ctx, &body, &a)?;
        Ok(FeatureOutput {
            body: Some(body),
            ..FeatureOutput::default()
        })
    }
    fn discover_output_faces(
        &self,
        node: &FeatureNode,
        ctx: &dyn RegenContext,
        output: &FeatureOutput,
    ) -> Result<Option<Vec<FaceRefDiscovery>>> {
        let a = admit(node, ctx)?;
        let body = output
            .body
            .as_ref()
            .ok_or_else(|| invalid("missing cached output body"))?;
        Ok(Some(require_output(node, ctx, body, &a)?))
    }
}
