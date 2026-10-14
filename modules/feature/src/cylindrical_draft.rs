//! ADR-065: guarded full-cylinder source, owned side and physical draft output.
use crate::sketch_bridge::{
    placement_from_workplane, profile_to_solved_with_context, resolve_workplane,
};
use crate::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use opencad_core::{Angle, OpenCadError, Result};
use opencad_geometry::{
    CylindricalDraftInput, CylindricalStockGeometry, ExtrudeExtent, ExtrudeOperation,
    FaceRefDiscovery, KernelBody, MeshSet, SketchPlacement, TessellationSettings, TopoRefKind,
};
use opencad_sketch::Workplane;
use serde::{Deserialize, Serialize};
const EPS: f64 = 1e-9;
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("cylindrical draft: {message}"))
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
        return Err(invalid("nonorthonormal source/neutral frame"));
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
fn strict_angle<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Angle, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        value_si: f64,
    }
    Ok(Angle::from_radians(Wire::deserialize(d)?.value_si))
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CylindricalDraftFeature {
    pub target_feature: String,
    pub face_ref: String,
    #[serde(deserialize_with = "strict_angle")]
    pub angle: Angle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle_expr: Option<String>,
    pub neutral_plane: Workplane,
}
impl CylindricalDraftFeature {
    pub fn validate(&self) -> Result<()> {
        if self.target_feature.trim().is_empty()
            || !self.face_ref.starts_with("ref:face:")
            || self.face_ref.len() <= 9
        {
            return Err(invalid(
                "target and lateral semantic face reference required",
            ));
        }
        crate::draft::validate_angle(self.angle.radians())
    }
    pub(crate) fn resolve_expressions(
        &mut self,
        scope: &indexmap::IndexMap<String, f64>,
    ) -> Result<()> {
        if let Some(expr) = &self.angle_expr {
            if opencad_graph::parameter_names_in_expr(expr)
                .iter()
                .any(|n| !opencad_graph::param_eval::is_angle_parameter(n))
            {
                return Err(invalid(
                    "angle expression cannot reference a length parameter",
                ));
            }
            self.angle = Angle::from_radians(opencad_graph::eval_angle_expr_strict(expr, scope)?);
        }
        self.validate()
    }
}
struct Admission {
    target: KernelBody,
    stock: CylindricalStockGeometry,
    input: CylindricalDraftInput,
    radii: [f64; 2],
}
fn admit(node: &FeatureNode, ctx: &dyn RegenContext) -> Result<Admission> {
    let FeatureDefinition::CylindricalDraft(def) = &node.definition else {
        return Err(invalid("wrong definition"));
    };
    def.validate()?;
    let nodes = ctx.source_feature_nodes()?;
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
                || area < lo - EPS
                || area > hi + EPS
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
    let plane = checked_placement(&resolve_workplane(&def.neutral_plane, ctx)?)?;
    let input = CylindricalDraftInput {
        axis_hint: axis,
        neutral_origin_m: plane.origin_m,
        neutral_normal: frame(plane)?,
        angle_rad: def.angle.radians(),
    };
    let radii = input.endpoint_radii(&stock)?;
    ctx.kernel()
        .assign_face_ref(&target, stock.lateral_face_id, reference.ref_id.clone())?;
    Ok(Admission {
        target,
        stock,
        input,
        radii,
    })
}
/// Independently integrate r(z)=a+s*z rather than call kernel mass formulae.
fn require_output(
    ctx: &dyn RegenContext,
    body: &KernelBody,
    a: &Admission,
) -> Result<Vec<FaceRefDiscovery>> {
    let [r0, r1] = a.radii;
    let h = a.stock.height_m;
    let slope = (r1 - r0) / h;
    let pi = std::f64::consts::PI;
    let volume = pi * (r0 * r0 * h + r0 * slope * h * h + slope * slope * h * h * h / 3.);
    let first = pi
        * (r0 * r0 * h * h / 2.
            + 2. * r0 * slope * h * h * h / 3.
            + slope * slope * h.powi(4) / 4.);
    let area = pi * (r0 * r0 + r1 * r1) + pi * (r0 + r1) * h.hypot(r1 - r0);
    let mass = ctx.kernel().mass_properties(body, 1.)?;
    if !mass.volume_m3.is_finite()
        || (mass.volume_m3 - volume).abs() > volume * 1e-8
        || !mass.area_m2.is_finite()
        || (mass.area_m2 - area).abs() > area * 1e-8
    {
        return Err(invalid(
            "physical volume/area disagrees with radial integral",
        ));
    }
    for i in 0..3 {
        let expected = a.stock.origin_m[i] + a.stock.axis[i] * first / volume;
        if !mass.center_of_mass[i].is_finite() || (mass.center_of_mass[i] - expected).abs() > EPS {
            return Err(invalid("physical first moments disagree"));
        }
    }
    let mesh = ctx
        .kernel()
        .tessellate(body, &TessellationSettings::default())?;
    if mesh.triangle_count() == 0 || mesh.triangle_face_ids.len() != mesh.triangle_count() {
        return Err(invalid("physical face-bearing mesh required"));
    }
    crate::seated_standard_hole::validate_closed_mesh(&mesh)?;
    classify(&mesh, a)
}
fn classify(mesh: &MeshSet, a: &Admission) -> Result<Vec<FaceRefDiscovery>> {
    let mut groups = std::collections::BTreeMap::<u64, Vec<[usize; 3]>>::new();
    for (t, id) in mesh.indices.chunks_exact(3).zip(&mesh.triangle_face_ids) {
        if *id == 0 {
            return Err(invalid("missing output face identity"));
        }
        groups
            .entry(*id)
            .or_default()
            .push([t[0] as usize, t[1] as usize, t[2] as usize]);
    }
    if groups.len() != 3 {
        return Err(invalid("output must have exactly three physical faces"));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for (id, triangles) in groups {
        let mut class = None;
        let mut centroid = [0.; 3];
        let mut count = 0.;
        for t in triangles {
            let p = t.map(|i| mesh.positions[i].map(f64::from));
            let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            let c = std::array::from_fn(|i| (p[0][i] + p[1][i] + p[2][i]) / 3.);
            let rel = p.map(|p| sub(p, a.stock.origin_m));
            let zs = rel.map(|p| dot(p, a.stock.axis));
            let tolerance = EPS
                + 8. * f32::EPSILON as f64
                    * p.iter()
                        .flatten()
                        .map(|v| v.abs())
                        .chain(a.stock.origin_m.map(f64::abs))
                        .fold(a.stock.radius_m.max(a.stock.height_m), f64::max);
            let start = zs.iter().all(|z| z.abs() <= tolerance) && dot(n, a.stock.axis) < 0.;
            let end = zs.iter().all(|z| (z - a.stock.height_m).abs() <= tolerance)
                && dot(n, a.stock.axis) > 0.;
            let kind = if start {
                0
            } else if end {
                1
            } else {
                2
            };
            if kind < 2 {
                let radius = a.radii[kind];
                if rel
                    .iter()
                    .zip(zs)
                    .any(|(p, z)| norm(sub(*p, a.stock.axis.map(|v| v * z))) > radius + tolerance)
                {
                    return Err(invalid("output cap exceeds physical circle"));
                }
            } else {
                for (p, z) in rel.iter().zip(zs) {
                    let radial = norm(sub(*p, a.stock.axis.map(|v| v * z)));
                    let expected = a.radii[0] + (a.radii[1] - a.radii[0]) * z / a.stock.height_m;
                    if z < -tolerance
                        || z > a.stock.height_m + tolerance
                        || (radial - expected).abs() > tolerance
                    {
                        return Err(invalid("output side violates conical radial law"));
                    }
                }
                let q = sub(c, a.stock.origin_m);
                let z = dot(q, a.stock.axis);
                let radial = sub(q, a.stock.axis.map(|v| v * z));
                if dot(n, radial) <= 0. {
                    return Err(invalid("output side is not outward"));
                }
            }
            if class.is_some_and(|k| k != kind) {
                return Err(invalid("one output face mixes physical surface classes"));
            }
            class = Some(kind);
            for i in 0..3 {
                centroid[i] += c[i];
            }
            count += 1.;
        }
        let kind = class.ok_or_else(|| invalid("empty output face"))?;
        if !seen.insert(kind) {
            return Err(invalid("ambiguous output cap/side identity"));
        }
        let normal = if kind == 2 {
            [0.; 3]
        } else {
            a.stock.axis.map(|v| v * if kind == 0 { -1. } else { 1. })
        };
        result.push(FaceRefDiscovery {
            kernel_face_id: id,
            role: if kind == 2 {
                "conical".into()
            } else {
                crate::face_discover::role_from_normal(normal.map(|v| v as f32)).into()
            },
            normal_m: normal.map(|v| v as f32),
            centroid_m: centroid.map(|v| (v / count) as f32),
            feature_id: None,
        });
    }
    Ok(result)
}
#[derive(Debug, Default)]
pub struct CylindricalDraftFeatureExecutor;
impl Feature for CylindricalDraftFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "cylindrical_draft"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let a = admit(node, ctx)?;
        let body = ctx
            .kernel()
            .draft_cylindrical_stock(a.target.clone(), &a.input)?;
        require_output(ctx, &body, &a)?;
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
            .ok_or_else(|| invalid("missing output body"))?;
        let mut faces = require_output(ctx, body, &a)?;
        for f in &mut faces {
            let unchanged = f.role != "conical"
                && [0, 1].into_iter().any(|i| {
                    let normal = a.stock.axis.map(|v| v * if i == 0 { -1. } else { 1. });
                    dot(f.normal_m.map(f64::from), normal) > 0.999999
                        && (a.radii[i] - a.stock.radius_m).abs()
                            <= 64. * f64::EPSILON * a.stock.radius_m.max(1.)
                });
            f.feature_id = Some(if unchanged {
                let FeatureDefinition::CylindricalDraft(def) = &node.definition else {
                    return Err(invalid("wrong output owner"));
                };
                def.target_feature.clone()
            } else {
                node.id.clone()
            });
        }
        Ok(Some(faces))
    }
}
