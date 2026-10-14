//! ADR-066 guarded authored tapered-rib source and physical face ownership.
use crate::sketch_bridge::{
    placement_from_workplane, profile_to_solved_with_context, resolve_workplane,
};
use crate::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use opencad_core::{Angle, Length, OpenCadError, Result};
use opencad_geometry::{
    ExtrudeExtent, ExtrudeOperation, FacePick, FaceRefDiscovery, KernelBody,
    RectangularStockGeometry, SketchPlacement, TaperedRibGeometry, TaperedRibInput,
    TessellationSettings, TopoRefKind,
};
use opencad_sketch::{Coord, SketchEntity, Workplane};
use serde::{Deserialize, Serialize};
const EPS: f64 = 1e-9;
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("tapered rib: {message}"))
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
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
        return Err(invalid("nonorthonormal source frame"));
    }
    Ok(cross(p.x_axis_m, p.y_axis_m))
}
fn checked_plane(p: &Workplane, ctx: &dyn RegenContext) -> Result<SketchPlacement> {
    let p = resolve_workplane(p, ctx)?;
    if let Workplane::Custom { normal, x_axis, .. } = &p {
        if normal.iter().chain(x_axis).any(|v| !v.is_finite())
            || (norm(*normal) - 1.).abs() > EPS
            || (norm(*x_axis) - 1.).abs() > EPS
            || dot(*normal, *x_axis).abs() > EPS
        {
            return Err(invalid("custom plane needs finite unit orthogonal axes"));
        }
    }
    let p = placement_from_workplane(&p)?;
    frame(p)?;
    Ok(p)
}
fn strict_length<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Length, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        value_si: f64,
    }
    Ok(Length::from_meters(Wire::deserialize(d)?.value_si))
}
fn strict_angle<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Angle, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        value_si: f64,
    }
    Ok(Angle::from_radians(Wire::deserialize(d)?.value_si))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaperedRibEnds {
    Square,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaperedRibFeature {
    pub target_feature: String,
    pub sketch_feature: String,
    pub centerline_ref: String,
    pub face_ref: String,
    #[serde(deserialize_with = "strict_length")]
    pub base_thickness: Length,
    #[serde(deserialize_with = "strict_length")]
    pub height: Length,
    #[serde(deserialize_with = "strict_angle")]
    pub angle: Angle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_thickness_expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_expr: Option<String>,
    pub ends: TaperedRibEnds,
}
impl TaperedRibFeature {
    pub fn validate(&self) -> Result<()> {
        if self.target_feature.trim().is_empty()
            || self.sketch_feature.trim().is_empty()
            || !self.centerline_ref.starts_with("ent:")
            || self.centerline_ref.len() <= 4
            || !self.face_ref.starts_with("ref:face:")
            || self.face_ref.len() <= 9
        {
            return Err(invalid(
                "stock/sketch/line/end reference identities required",
            ));
        }
        for v in [self.base_thickness.meters(), self.height.meters()] {
            if !v.is_finite() || v <= EPS || v > 1000. {
                return Err(invalid("length must be finite in (1nm,1000m]"));
            }
        }
        crate::draft::validate_angle(self.angle.radians())
    }
    fn validate_resolved(&self) -> Result<()> {
        self.validate()?;
        let top =
            self.base_thickness.meters() - 2. * self.height.meters() * self.angle.radians().tan();
        if !top.is_finite()
            || top <= EPS
            || top > 1000.
            || (top - self.base_thickness.meters()).abs() <= EPS
        {
            return Err(invalid("collapsed or unresolvable resolved top thickness"));
        }
        Ok(())
    }
    pub(crate) fn resolve_expressions(
        &mut self,
        scope: &indexmap::IndexMap<String, f64>,
    ) -> Result<()> {
        // New wire static ranges are valid independently of expression overrides.
        self.validate()?;
        for (field, expr) in [
            (&mut self.base_thickness, &self.base_thickness_expr),
            (&mut self.height, &self.height_expr),
        ] {
            if let Some(expr) = expr {
                if opencad_graph::parameter_names_in_expr(expr)
                    .iter()
                    .any(|n| opencad_graph::param_eval::is_angle_parameter(n))
                {
                    return Err(invalid(
                        "length expression cannot reference angle parameters",
                    ));
                }
                *field = Length::from_meters(opencad_graph::eval_length_expr_strict(expr, scope)?);
            }
        }
        if let Some(expr) = &self.angle_expr {
            if opencad_graph::parameter_names_in_expr(expr)
                .iter()
                .any(|n| !opencad_graph::param_eval::is_angle_parameter(n))
            {
                return Err(invalid(
                    "angle expression cannot reference length parameters",
                ));
            }
            self.angle = Angle::from_radians(opencad_graph::eval_angle_expr_strict(expr, scope)?);
        }
        self.validate_resolved()
    }
}
struct Admission {
    target: KernelBody,
    stock: RectangularStockGeometry,
    input: TaperedRibInput,
    geometry: TaperedRibGeometry,
    end_role: &'static str,
}
fn point(sketch: &opencad_sketch::Sketch, id: &str) -> Result<[f64; 2]> {
    let Some(SketchEntity::Point(p)) = sketch.find_entity(id) else {
        return Err(invalid("line endpoint must be an authored point"));
    };
    let (Coord::Literal(x), Coord::Literal(y)) = (&p.x, &p.y) else {
        return Err(invalid("line endpoints require solved SI coordinates"));
    };
    if !x.is_finite() || !y.is_finite() {
        return Err(invalid("nonfinite line endpoint"));
    }
    Ok([*x, *y])
}
/// Only an explicitly consumed complete construction centreline may have no
/// material profile; legacy sketch/profile validation remains unchanged.
pub(crate) fn admits_profile_free_sketch(
    sketch: &opencad_sketch::Sketch,
    nodes: &indexmap::IndexMap<String, FeatureNode>,
) -> bool {
    sketch.profiles.is_empty()
        && nodes.values().any(|node| {
            let FeatureDefinition::TaperedRib(def) = &node.definition else {
                return false;
            };
            let Some(source) = nodes.get(&def.sketch_feature) else {
                return false;
            };
            let FeatureDefinition::Sketch(source_def) = &source.definition else {
                return false;
            };
            if source_def.sketch_id != sketch.id.as_str() {
                return false;
            }
            let Some(SketchEntity::Line(line)) = sketch.find_entity(&def.centerline_ref) else {
                return false;
            };
            if !line.base.construction {
                return false;
            }
            match (
                point(sketch, line.start.as_str()),
                point(sketch, line.end.as_str()),
            ) {
                (Ok(a), Ok(b)) => (a[0] - b[0]).hypot(a[1] - b[1]) > EPS,
                _ => false,
            }
        })
}
fn admit(node: &FeatureNode, ctx: &dyn RegenContext) -> Result<Admission> {
    let FeatureDefinition::TaperedRib(def) = &node.definition else {
        return Err(invalid("wrong definition"));
    };
    def.validate_resolved()?;
    let nodes = ctx.source_feature_nodes()?;
    let source: Vec<_> = nodes
        .iter()
        .filter(|n| n.id == def.target_feature && !n.suppressed)
        .collect();
    if source.len() != 1 {
        return Err(invalid("missing/ambiguous target source"));
    }
    let FeatureDefinition::Extrude(stock_def) = &source[0].definition else {
        return Err(invalid("target must be original rectangle extrusion"));
    };
    let ExtrudeExtent::Distance { length } = stock_def.extent else {
        return Err(invalid("target must use distance extent"));
    };
    if stock_def.operation != ExtrudeOperation::NewBody || stock_def.target_feature.is_some() {
        return Err(invalid("target must be original NewBody stock"));
    }
    for id in [&stock_def.sketch_feature, &def.sketch_feature] {
        let candidates: Vec<_> = nodes
            .iter()
            .filter(|n| n.id == *id && !n.suppressed)
            .collect();
        if candidates.len() != 1
            || !matches!(candidates[0].definition, FeatureDefinition::Sketch(_))
        {
            return Err(invalid(
                "stock and centreline require original authored sketches",
            ));
        }
    }
    if nodes.iter().any(|n| {
        !n.suppressed
            && n.definition
                .consumed_face_refs()
                .contains(&def.face_ref.as_str())
    }) {
        return Err(invalid("consumed stock end reference"));
    }
    let source_sketch = ctx.sketch_for_feature(&stock_def.sketch_feature)?;
    checked_plane(&source_sketch.workplane, ctx)?;
    let original = profile_to_solved_with_context(source_sketch, &stock_def.profile_ref, ctx)?;
    if original.points.len() != 4
        || original.circle.is_some()
        || original.ellipse.is_some()
        || original.cubic_bezier.is_some()
        || original
            .segments
            .iter()
            .any(|s| !matches!(s, opencad_geometry::ProfileSegment::Line { .. }))
    {
        return Err(invalid("stock source must be a right rectangle"));
    }
    let plane = original
        .placement
        .ok_or_else(|| invalid("missing stock frame"))?;
    let axis = frame(plane)?;
    let corners: Vec<_> = original
        .points
        .iter()
        .map(|p| plane.map_point(p[0], p[1]))
        .collect();
    if corners.iter().flatten().any(|v| !v.is_finite())
        || !length.meters().is_finite()
        || length.meters() <= EPS
    {
        return Err(invalid("nonfinite or collapsed stock source"));
    }
    let u = sub(corners[1], corners[0]);
    let v = sub(corners[3], corners[0]);
    if norm(u) <= EPS
        || norm(v) <= EPS
        || dot(u, v).abs() > EPS * norm(u) * norm(v)
        || norm(sub(sub(corners[2], corners[1]), v)) > EPS
    {
        return Err(invalid("stock source is not a right rectangle"));
    }
    let sketch = ctx.sketch_for_feature(&def.sketch_feature)?;
    let Some(SketchEntity::Line(line)) = sketch.find_entity(&def.centerline_ref) else {
        return Err(invalid(
            "centreline must be a complete straight line entity",
        ));
    };
    let line_plane = checked_plane(&sketch.workplane, ctx)?;
    let normal = frame(line_plane)?;
    let start = point(sketch, line.start.as_str())?;
    let end = point(sketch, line.end.as_str())?;
    let start = line_plane.map_point(start[0], start[1]);
    let end = line_plane.map_point(end[0], end[1]);
    let delta = sub(end, start);
    let length_m = norm(delta);
    if !length_m.is_finite() || length_m <= EPS {
        return Err(invalid("collapsed line"));
    }
    let center = std::array::from_fn(|i| (start[i] + end[i]) / 2.);
    let sign = dot(normal, axis);
    if norm(cross(normal, axis)) > EPS || sign.abs() < 0.999999 {
        return Err(invalid("line plane must match an original extrusion end"));
    }
    let end_role = if sign > 0. {
        "extrusion_end"
    } else {
        "extrusion_start"
    };
    let target = ctx.body_for_feature(&def.target_feature)?;
    let stock = ctx.kernel().rectangular_stock_geometry(
        &target,
        FacePick {
            point_m: center,
            normal,
        },
    )?;
    if (stock.inward_thickness_m - length.meters()).abs() > EPS {
        return Err(invalid("source thickness disagrees with actual stock"));
    }
    let mut spans = [norm(u), norm(v)];
    let mut actual = stock.face_spans_m;
    spans.sort_by(f64::total_cmp);
    actual.sort_by(f64::total_cmp);
    if spans
        .into_iter()
        .zip(actual)
        .any(|(a, b)| (a - b).abs() > EPS)
    {
        return Err(invalid("source spans disagree with actual stock"));
    }
    for corner in corners {
        let entry = std::array::from_fn(|i| {
            corner[i]
                + if sign > 0. {
                    axis[i] * length.meters()
                } else {
                    0.
                }
        });
        let p = sub(entry, stock.origin_m);
        if dot(p, normal).abs() > EPS
            || ![dot(p, stock.u_axis), dot(p, stock.v_axis)]
                .into_iter()
                .zip(stock.face_spans_m)
                .all(|(c, s)| c.abs() <= EPS || (c - s).abs() <= EPS)
        {
            return Err(invalid("actual stock end corners disagree with source"));
        }
    }
    let refs: Vec<_> = ctx
        .semantic_refs()
        .iter()
        .filter(|r| r.ref_id.as_str() == def.face_ref)
        .collect();
    if refs.len() != 1
        || refs[0].kind != TopoRefKind::Face
        || refs[0].semantic.created_by != def.target_feature
        || refs[0].semantic.role.as_deref() != Some(end_role)
    {
        return Err(invalid(
            "missing, ambiguous, foreign or wrong named stock end",
        ));
    }
    let reference = refs[0];
    if reference.semantic.normal_hint.is_some_and(|n| {
        !norm(n).is_finite()
            || (norm(n) - 1.).abs() > 1e-6
            || norm(cross(n, normal)) > 1e-6
            || dot(n, normal) <= 0.
    }) {
        return Err(invalid("end normal annotation disagrees"));
    }
    if let Some(fp) = &reference.geometric_fingerprint {
        if !matches!(fp.surface_type.as_str(), "plane" | "brep_face") || fp.kernel_edge_id.is_some()
        {
            return Err(invalid("end fingerprint must be planar"));
        }
        if let Some([lo, hi]) = fp.area_range {
            let area = stock.face_spans_m[0] * stock.face_spans_m[1];
            if !lo.is_finite()
                || !hi.is_finite()
                || lo < 0.
                || hi < lo
                || area < lo - EPS
                || area > hi + EPS
            {
                return Err(invalid("stale end area fingerprint"));
            }
        }
        if let Some(bounds) = fp.bbox_hint {
            if bounds.iter().flatten().any(|v| !v.is_finite())
                || (0..3).any(|i| bounds[1][i] < bounds[0][i])
            {
                return Err(invalid("invalid end centroid hint"));
            }
            let centroid: [f64; 3] = std::array::from_fn(|i| {
                stock.origin_m[i]
                    + stock.u_axis[i] * stock.face_spans_m[0] / 2.
                    + stock.v_axis[i] * stock.face_spans_m[1] / 2.
            });
            for i in 0..3 {
                let err = EPS
                    + 4. * f32::EPSILON as f64
                        * bounds[0][i]
                            .abs()
                            .max(bounds[1][i].abs())
                            .max(centroid[i].abs());
                if centroid[i] < bounds[0][i] - err || centroid[i] > bounds[1][i] + err {
                    return Err(invalid("stale end centroid bounds"));
                }
            }
            let mid = std::array::from_fn(|i| (bounds[0][i] + bounds[1][i]) / 2.);
            let err = (0..3)
                .map(|i| {
                    normal[i].abs()
                        * 4.
                        * f32::EPSILON as f64
                        * bounds[0][i]
                            .abs()
                            .max(bounds[1][i].abs())
                            .max(stock.origin_m[i].abs())
                })
                .sum::<f64>();
            if dot(sub(mid, stock.origin_m), normal).abs() > EPS + err {
                return Err(invalid("stale end plane fingerprint"));
            }
        }
    }
    if let Some(id) = reference.kernel_face_id() {
        if ctx
            .face_discoveries_on(&target)?
            .iter()
            .any(|f| f.kernel_face_id == id)
            && id != stock.entry_face_id
        {
            return Err(invalid("reference bound to another present stock face"));
        }
    }
    let input = TaperedRibInput {
        entry: FacePick {
            point_m: center,
            normal,
        },
        length_axis_hint: delta.map(|v| v / length_m),
        length_m,
        base_thickness_m: def.base_thickness.meters(),
        height_m: def.height.meters(),
        angle_rad: def.angle.radians(),
    };
    let geometry = input.admit(&stock)?;
    ctx.kernel()
        .assign_face_ref(&target, stock.entry_face_id, reference.ref_id.clone())?;
    Ok(Admission {
        target,
        stock,
        input,
        geometry,
        end_role,
    })
}
struct Plane {
    origin: [f64; 3],
    normal: [f64; 3],
    offset: f64,
    role: String,
    source: bool,
}
fn output_faces(
    node: &FeatureNode,
    ctx: &dyn RegenContext,
    body: &KernelBody,
    a: &Admission,
) -> Result<Vec<FaceRefDiscovery>> {
    let g = a.geometry;
    let before = ctx.kernel().mass_properties(&a.target, 1.)?;
    let after = ctx.kernel().mass_properties(body, 1.)?;
    let l = g.length_m;
    let h = g.height_m;
    let t = g.base_thickness_m;
    let k = a.input.angle_rad.tan();
    let added = l * (t * h - k * h * h);
    let first = l * (t * h * h / 2. - 2. * k * h * h * h / 3.);
    let expected = before.volume_m3 + added;
    let area = before.area_m2 - 2. * l * k * h + 2. * t * h - 2. * k * h * h
        + 2. * l * h / a.input.angle_rad.cos();
    if !after.volume_m3.is_finite()
        || (after.volume_m3 - expected).abs() > expected * 1e-8
        || !after.area_m2.is_finite()
        || (after.area_m2 - area).abs() > area * 1e-8
    {
        return Err(invalid(
            "physical union volume/area disagrees with cross-section integral",
        ));
    }
    for i in 0..3 {
        let center = (before.volume_m3 * before.center_of_mass[i]
            + added * g.base_center_m[i]
            + first * g.outward_normal[i])
            / expected;
        if !after.center_of_mass[i].is_finite() || (after.center_of_mass[i] - center).abs() > EPS {
            return Err(invalid("physical union first moments disagree"));
        }
    }
    let mesh = ctx
        .kernel()
        .tessellate(body, &TessellationSettings::default())?;
    if mesh.triangle_count() == 0 || mesh.triangle_face_ids.len() != mesh.triangle_count() {
        return Err(invalid("physical face-bearing mesh required"));
    }
    crate::seated_standard_hole::validate_closed_mesh(&mesh)?;
    let mut planes = Vec::new();
    for (axis, span) in [
        (a.stock.u_axis, a.stock.face_spans_m[0]),
        (a.stock.v_axis, a.stock.face_spans_m[1]),
    ] {
        for high in [false, true] {
            let normal = axis.map(|v| v * if high { 1. } else { -1. });
            planes.push(Plane {
                origin: a.stock.origin_m,
                normal,
                offset: if high { span } else { 0. },
                role: crate::face_discover::role_from_normal(normal.map(|v| v as f32)).into(),
                source: true,
            });
        }
    }
    planes.push(Plane {
        origin: a.stock.origin_m,
        normal: g.outward_normal.map(|v| -v),
        offset: a.stock.inward_thickness_m,
        role: if a.end_role == "extrusion_end" {
            "extrusion_start".into()
        } else {
            "extrusion_end".into()
        },
        source: true,
    });
    planes.push(Plane {
        origin: a.stock.origin_m,
        normal: g.outward_normal,
        offset: 0.,
        role: a.end_role.into(),
        source: true,
    });
    for (normal, offset, role) in [
        (g.length_axis.map(|v| -v), l / 2., "rib_start"),
        (g.length_axis, l / 2., "rib_end"),
        (g.outward_normal, h, "rib_top"),
    ] {
        planes.push(Plane {
            origin: g.base_center_m,
            normal,
            offset,
            role: role.into(),
            source: false,
        });
    }
    for (sign, role) in [(-1., "rib_side_negative"), (1., "rib_side_positive")] {
        let n = std::array::from_fn(|i| g.width_axis[i] * sign + g.outward_normal[i] * k);
        let size = norm(n);
        planes.push(Plane {
            origin: g.base_center_m,
            normal: n.map(|v| v / size),
            offset: t / 2. / size,
            role: role.into(),
            source: false,
        });
    }
    let mut groups = std::collections::BTreeMap::<u64, Vec<[usize; 3]>>::new();
    for (tri, id) in mesh.indices.chunks_exact(3).zip(&mesh.triangle_face_ids) {
        if *id == 0 {
            return Err(invalid("missing face identity"));
        }
        groups
            .entry(*id)
            .or_default()
            .push([tri[0] as usize, tri[1] as usize, tri[2] as usize]);
    }
    if groups.len() != 11 {
        return Err(invalid("union must have eleven physical faces"));
    }
    let FeatureDefinition::TaperedRib(def) = &node.definition else {
        return Err(invalid("wrong output producer"));
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut discoveries = Vec::new();
    for (id, triangles) in groups {
        let matches: Vec<_> = planes
            .iter()
            .enumerate()
            .filter(|(_, plane)| {
                triangles.iter().all(|tri| {
                    let p = tri.map(|i| mesh.positions[i].map(f64::from));
                    let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                    dot(n, plane.normal) > 0.
                        && p.iter().all(|p| {
                            let err = (0..3)
                                .map(|i| {
                                    plane.normal[i].abs()
                                        * 8.
                                        * f32::EPSILON as f64
                                        * p[i].abs().max(plane.origin[i].abs())
                                })
                                .sum::<f64>();
                            (dot(sub(*p, plane.origin), plane.normal) - plane.offset).abs()
                                <= EPS + err
                        })
                })
            })
            .collect();
        if matches.len() != 1 || !seen.insert(matches[0].0) {
            return Err(invalid("ambiguous or stale physical output surface"));
        }
        let plane = matches[0].1;
        let mut centroid = [0.; 3];
        for tri in &triangles {
            for (i, value) in centroid.iter_mut().enumerate() {
                *value += tri
                    .iter()
                    .map(|v| mesh.positions[*v][i] as f64)
                    .sum::<f64>()
                    / 3.;
            }
        }
        discoveries.push(FaceRefDiscovery {
            kernel_face_id: id,
            role: plane.role.clone(),
            normal_m: plane.normal.map(|v| v as f32),
            centroid_m: centroid.map(|v| (v / triangles.len() as f64) as f32),
            feature_id: Some(if plane.source {
                def.target_feature.clone()
            } else {
                node.id.clone()
            }),
        });
    }
    Ok(discoveries)
}
#[derive(Debug, Default)]
pub struct TaperedRibFeatureExecutor;
impl Feature for TaperedRibFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "tapered_rib"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let a = admit(node, ctx)?;
        let body = ctx
            .kernel()
            .add_tapered_rib_stock(a.target.clone(), &a.input)?;
        output_faces(node, ctx, &body, &a)?;
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
            .ok_or_else(|| invalid("missing physical output"))?;
        Ok(Some(output_faces(node, ctx, body, &a)?))
    }
}
