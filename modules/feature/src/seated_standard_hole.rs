//! ADR-063/064: explicit contained seats with source and physical ownership.
use crate::sketch_bridge::profile_to_solved_with_context;
use crate::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, HoleStandard, RegenContext};
use opencad_core::{Angle, Length, OpenCadError, Result};
use opencad_geometry::{
    ExtrudeExtent, ExtrudeOperation, FacePick, GeometryKernel, KernelBody, MeshSet,
    SketchPlacement, TessellationSettings, TopoRefKind,
};
use serde::{Deserialize, Serialize};
const EPS: f64 = 1e-9;
pub(crate) fn evaluate_length(expr: &str, scope: &indexmap::IndexMap<String, f64>) -> Result<f64> {
    if opencad_graph::parameter_names_in_expr(expr)
        .iter()
        .any(|name| opencad_graph::param_eval::is_angle_parameter(name))
    {
        return Err(invalid(
            "length expression cannot reference an angle parameter",
        ));
    }
    opencad_graph::eval_length_expr_strict(expr, scope)
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
pub(crate) fn evaluate_angle(expr: &str, scope: &indexmap::IndexMap<String, f64>) -> Result<f64> {
    if opencad_graph::parameter_names_in_expr(expr)
        .iter()
        .any(|name| !opencad_graph::param_eval::is_angle_parameter(name))
    {
        return Err(invalid(
            "angle expression cannot reference a length parameter",
        ));
    }
    opencad_graph::eval_angle_expr_strict(expr, scope)
}

fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(format!("seated standard hole: {message}"))
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
fn aligned(a: [f64; 3], b: [f64; 3]) -> bool {
    norm(cross(a, b)) <= EPS && dot(a, b) > 0.
}
fn frame(p: SketchPlacement) -> Result<[f64; 3]> {
    if p.origin_m
        .iter()
        .chain(p.x_axis_m.iter())
        .chain(p.y_axis_m.iter())
        .any(|v| !v.is_finite())
        || (norm(p.x_axis_m) - 1.).abs() > EPS
        || (norm(p.y_axis_m) - 1.).abs() > EPS
        || dot(p.x_axis_m, p.y_axis_m).abs() > EPS
    {
        return Err(invalid("nonorthonormal sketch frame"));
    }
    Ok(cross(p.x_axis_m, p.y_axis_m))
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StandardHoleSeat {
    Counterbore {
        #[serde(deserialize_with = "strict_length")]
        diameter: Length,
        #[serde(deserialize_with = "strict_length")]
        depth: Length,
        #[serde(skip_serializing_if = "Option::is_none")]
        diameter_expr: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        depth_expr: Option<String>,
    },
    Countersink {
        #[serde(deserialize_with = "strict_length")]
        diameter: Length,
        #[serde(deserialize_with = "strict_angle")]
        included_angle: Angle,
        #[serde(skip_serializing_if = "Option::is_none")]
        diameter_expr: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        included_angle_expr: Option<String>,
    },
}
impl StandardHoleSeat {
    /// Seat diameter and physical depth; countersink depth depends on the bore.
    pub fn dimensions(&self, bore_diameter: Length) -> Result<(Length, Length)> {
        match self {
            Self::Counterbore {
                diameter, depth, ..
            } => Ok((*diameter, *depth)),
            Self::Countersink {
                diameter,
                included_angle,
                ..
            } => {
                let angle = included_angle.radians();
                if !angle.is_finite() || angle <= 0. || angle >= std::f64::consts::PI {
                    return Err(invalid("included angle must be finite in (0,pi) radians"));
                }
                let depth = (diameter.meters() - bore_diameter.meters()) / 2. / (angle / 2.).tan();
                Ok((*diameter, Length::from_meters(depth)))
            }
        }
    }
    pub(crate) fn resolve_expressions(
        &mut self,
        scope: &indexmap::IndexMap<String, f64>,
    ) -> Result<()> {
        match self {
            Self::Counterbore {
                diameter,
                depth,
                diameter_expr,
                depth_expr,
            } => {
                if let Some(expr) = diameter_expr {
                    *diameter = Length::from_meters(evaluate_length(expr, scope)?);
                }
                if let Some(expr) = depth_expr {
                    *depth = Length::from_meters(evaluate_length(expr, scope)?);
                }
            }
            Self::Countersink {
                diameter,
                included_angle,
                diameter_expr,
                included_angle_expr,
            } => {
                if let Some(expr) = diameter_expr {
                    *diameter = Length::from_meters(evaluate_length(expr, scope)?);
                }
                if let Some(expr) = included_angle_expr {
                    *included_angle = Angle::from_radians(evaluate_angle(expr, scope)?);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatedStandardHoleFeature {
    pub sketch_feature: String,
    pub profile_ref: String,
    pub target_feature: String,
    pub face_ref: String,
    #[serde(deserialize_with = "strict_length")]
    pub bore_depth: Length,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bore_depth_expr: Option<String>,
    pub standard: HoleStandard,
    pub seat: StandardHoleSeat,
}
impl SeatedStandardHoleFeature {
    pub fn validate_dimensions(&self) -> Result<()> {
        let (diameter, depth) = self
            .seat
            .dimensions(self.standard.dimensions().cut_diameter)?;
        let bore = self.standard.dimensions().cut_diameter.meters();
        for v in [self.bore_depth.meters(), diameter.meters(), depth.meters()] {
            if !v.is_finite() || v <= EPS || v > 1000. {
                return Err(invalid("length must be finite in (1e-9,1000] m"));
            }
        }
        if diameter.meters() - bore <= EPS || self.bore_depth.meters() - depth.meters() <= EPS {
            return Err(invalid(
                "seat diameter must exceed bore and seat depth must be smaller than bore depth",
            ));
        }
        if matches!(self.seat, StandardHoleSeat::Countersink { .. })
            && diameter.meters() - bore <= 2. * EPS
        {
            return Err(invalid("countersink radial margin must exceed 1e-9 m"));
        }
        if [
            &self.sketch_feature,
            &self.profile_ref,
            &self.target_feature,
            &self.face_ref,
        ]
        .iter()
        .any(|v| v.trim().is_empty())
        {
            return Err(invalid(
                "source/profile/target/face identities are required",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Default)]
pub struct SeatedStandardHoleFeatureExecutor;
impl Feature for SeatedStandardHoleFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "seated_standard_hole"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::SeatedStandardHole(def) = &node.definition else {
            return Err(invalid("wrong executor definition"));
        };
        let OwnedHoleInputs {
            target,
            mut profile,
            inward_direction: direction,
            ..
        } = admit_inputs(def, ctx)?;
        let kernel = ctx.kernel();
        let circle = profile
            .circle
            .ok_or_else(|| invalid("missing admitted circle"))?;
        let (seat_diameter, seat_depth) = def
            .seat
            .dimensions(def.standard.dimensions().cut_diameter)?;
        let radius = seat_diameter.meters() / 2.;
        let body = match def.seat {
            StandardHoleSeat::Countersink { .. } => kernel.cut_countersunk_stock(
                target.clone(),
                &opencad_geometry::CountersinkCut {
                    entry: FacePick {
                        point_m: profile
                            .placement
                            .ok_or_else(|| invalid("missing admitted frame"))?
                            .map_point(circle.center_m[0], circle.center_m[1]),
                        normal: direction.map(|v| -v),
                    },
                    bore_radius_m: circle.radius_m,
                    seat_radius_m: radius,
                    bore_depth_m: def.bore_depth.meters(),
                    seat_depth_m: seat_depth.meters(),
                },
            )?,
            StandardHoleSeat::Counterbore { .. } => {
                let mut body = target.clone();
                for (r, depth) in [
                    (circle.radius_m, def.bore_depth.meters()),
                    (radius, seat_depth.meters()),
                ] {
                    if let Some(c) = &mut profile.circle {
                        c.radius_m = r;
                    }
                    body = kernel.extrude(
                        kernel.make_wire_from_sketch(&profile)?,
                        ExtrudeExtent::Distance {
                            length: Length::from_meters(depth),
                        },
                        ExtrudeOperation::Cut,
                        Some(body),
                        direction,
                    )?;
                }
                body
            }
        };
        require_output(kernel, &target, &body, &profile, def, direction)?;
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
    ) -> Result<Option<Vec<opencad_geometry::FaceRefDiscovery>>> {
        let FeatureDefinition::SeatedStandardHole(def) = &node.definition else {
            return Err(invalid("wrong face owner definition"));
        };
        let inputs = admit_inputs(def, ctx)?;
        discover_owned_hole_faces(node, &def.target_feature, ctx, output, &inputs)
    }
}
pub(crate) fn discover_owned_hole_faces(
    node: &FeatureNode,
    target_feature: &str,
    ctx: &dyn RegenContext,
    output: &FeatureOutput,
    inputs: &OwnedHoleInputs,
) -> Result<Option<Vec<opencad_geometry::FaceRefDiscovery>>> {
    let witness = inputs.stock;
    let outward = inputs.inward_direction.map(|v| -v);
    let body = output
        .body
        .as_ref()
        .ok_or_else(|| invalid("no physical output"))?;
    let mesh = ctx
        .kernel()
        .tessellate(body, &TessellationSettings::default())?;
    validate_owned_hole_mesh(&mesh)?;
    let nodes: Vec<_> = ctx.source_feature_nodes()?.into_iter().cloned().collect();
    let mut discoveries = crate::face_discover::discover_face_refs_from_mesh(&mesh, &nodes);
    let axes = [witness.u_axis, witness.v_axis, outward.map(|v| -v)];
    let spans = [
        witness.face_spans_m[0],
        witness.face_spans_m[1],
        witness.inward_thickness_m,
    ];
    for d in &mut discoveries {
        let triangles: Vec<_> = mesh
            .indices
            .chunks_exact(3)
            .zip(&mesh.triangle_face_ids)
            .filter(|(_, id)| **id == d.kernel_face_id)
            .map(|(t, _)| t)
            .collect();
        let mut matches = Vec::new();
        for axis in 0..3 {
            for high in [false, true] {
                let normal = axes[axis].map(|v| v * if high { 1. } else { -1. });
                let offset = if high { spans[axis] } else { 0. };
                let is_outer = triangles.iter().all(|t| {
                    let p: [[f64; 3]; 3] =
                        std::array::from_fn(|i| mesh.positions[t[i] as usize].map(f64::from));
                    // Componentwise f32 rounding bound, projected onto the exact
                    // admitted source plane. No box or nearest-face inference.
                    p.iter().all(|p| {
                        let error: f64 = (0..3)
                            .map(|i| {
                                axes[axis][i].abs()
                                    * 4.
                                    * f32::EPSILON as f64
                                    * p[i].abs().max(witness.origin_m[i].abs())
                            })
                            .sum();
                        (dot(sub(*p, witness.origin_m), axes[axis]) - offset).abs() <= EPS + error
                    }) && dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), normal) > 0.
                });
                if is_outer {
                    matches.push(normal);
                }
            }
        }
        if matches.len() > 1 {
            return Err(invalid(
                "output face has ambiguous physical source ownership",
            ));
        }
        if let Some(normal) = matches.first() {
            d.feature_id = Some(target_feature.into());
            d.normal_m = normal.map(|v| v as f32);
            d.role = crate::face_discover::role_from_normal(d.normal_m).into();
        } else {
            d.feature_id = Some(node.id.clone());
        }
    }
    if !discoveries.iter().any(|d| {
        d.feature_id.as_deref() == Some(target_feature)
            && dot(d.normal_m.map(f64::from), outward) > 0.999999
    }) {
        return Err(invalid("output lost physical entry face"));
    }
    Ok(Some(discoveries))
}
pub(crate) fn validate_cached_seated_hole(
    node: &FeatureNode,
    ctx: &dyn RegenContext,
    body: &KernelBody,
) -> Result<()> {
    let FeatureDefinition::SeatedStandardHole(def) = &node.definition else {
        return Err(invalid("wrong cached definition"));
    };
    let inputs = admit_inputs(def, ctx)?;
    require_output(
        ctx.kernel(),
        &inputs.target,
        body,
        &inputs.profile,
        def,
        inputs.inward_direction,
    )
}
fn admit_inputs(
    def: &SeatedStandardHoleFeature,
    ctx: &dyn RegenContext,
) -> Result<OwnedHoleInputs> {
    def.validate_dimensions()?;
    admit_owned_hole_inputs(
        &crate::StandardHoleFeature {
            hole: crate::HoleFeature {
                sketch_feature: def.sketch_feature.clone(),
                profile_ref: def.profile_ref.clone(),
                target_feature: def.target_feature.clone(),
                face_ref: Some(def.face_ref.clone()),
                depth: ExtrudeExtent::Distance {
                    length: def.bore_depth,
                },
                depth_expr: def.bore_depth_expr.clone(),
            },
            standard: def.standard.clone(),
        },
        def.seat
            .dimensions(def.standard.dimensions().cut_diameter)?
            .0,
        ctx,
    )
}

/// Transient existing source/semantic/physical admission; no persisted handles.
pub struct OwnedHoleInputs {
    pub target: KernelBody,
    pub profile: opencad_geometry::SolvedSketch,
    pub inward_direction: [f64; 3],
    pub stock: opencad_geometry::RectangularStockGeometry,
}

/// Same original-stock/end-face guard used by seated features and native forms.
/// The profile normal is outward; callers must verify the actual inward cut.
pub fn admit_owned_hole_inputs(
    def: &crate::StandardHoleFeature,
    outer_diameter: Length,
    ctx: &dyn RegenContext,
) -> Result<OwnedHoleInputs> {
    let ExtrudeExtent::Distance { length: depth } = def.hole.depth else {
        return Err(invalid("owned hole requires explicit distance depth"));
    };
    if [depth.meters(), outer_diameter.meters()]
        .iter()
        .any(|v| !v.is_finite() || *v <= EPS || *v > 1000.)
        || outer_diameter.meters() + EPS < def.standard.dimensions().cut_diameter.meters()
    {
        return Err(invalid("invalid owned hole depth or outer diameter"));
    }
    let face_ref = def
        .hole
        .face_ref
        .as_ref()
        .ok_or_else(|| invalid("owned entry reference required"))?;
    let sources = ctx.source_feature_nodes()?;
    let source: Vec<_> = sources
        .iter()
        .filter(|n| n.id == def.hole.target_feature && !n.suppressed)
        .collect();
    if source.len() != 1 {
        return Err(invalid("missing/ambiguous target source"));
    }
    if sources.iter().any(|n| {
        n.definition
            .consumed_face_refs()
            .contains(&face_ref.as_str())
    }) {
        return Err(invalid("consumed entry reference"));
    }
    let FeatureDefinition::Extrude(stock) = &source[0].definition else {
        return Err(invalid("target must be original rectangle extrusion"));
    };
    let ExtrudeExtent::Distance { length: height } = stock.extent else {
        return Err(invalid("target must use distance extrusion"));
    };
    if stock.operation != ExtrudeOperation::NewBody || stock.target_feature.is_some() {
        return Err(invalid("target must be original NewBody stock"));
    }
    let original = profile_to_solved_with_context(
        ctx.sketch_for_feature(&stock.sketch_feature)?,
        &stock.profile_ref,
        ctx,
    )?;
    if original.points.len() != 4
        || original.circle.is_some()
        || original.ellipse.is_some()
        || original.cubic_bezier.is_some()
        || original
            .segments
            .iter()
            .any(|s| !matches!(s, opencad_geometry::ProfileSegment::Line { .. }))
    {
        return Err(invalid("target source profile must be a right rectangle"));
    }
    let placement = original
        .placement
        .ok_or_else(|| invalid("missing stock frame"))?;
    let axis = frame(placement)?;
    let corners: Vec<_> = original
        .points
        .iter()
        .map(|p| placement.map_point(p[0], p[1]))
        .collect();
    if corners.iter().flatten().any(|v| !v.is_finite()) {
        return Err(invalid("nonfinite stock source"));
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
    let sketch = ctx.sketch_for_feature(&def.hole.sketch_feature)?;
    let profile = profile_to_solved_with_context(sketch, &def.hole.profile_ref, ctx)?;
    let circle = profile
        .circle
        .ok_or_else(|| invalid("requires an exact source circle"))?;
    if !circle.radius_m.is_finite()
        || circle.radius_m <= EPS
        || !profile.segments.is_empty()
        || profile.ellipse.is_some()
        || profile.cubic_bezier.is_some()
        || (2. * circle.radius_m - def.standard.dimensions().cut_diameter.meters()).abs() > EPS
    {
        return Err(invalid("circle does not match standard cut diameter"));
    }
    let plane = profile
        .placement
        .ok_or_else(|| invalid("missing circle frame"))?;
    let normal = frame(plane)?;
    let center = plane.map_point(circle.center_m[0], circle.center_m[1]);
    let target = ctx.body_for_feature(&def.hole.target_feature)?;
    let kernel = ctx.kernel();
    let witness = kernel.rectangular_stock_geometry(
        &target,
        FacePick {
            point_m: center,
            normal,
        },
    )?;
    let top = aligned(normal, axis);
    let bottom = aligned(normal, axis.map(|v| -v));
    if !top && !bottom {
        return Err(invalid("side/oblique stock entry unsupported"));
    }
    if (witness.inward_thickness_m - height.meters()).abs() > EPS
        || depth.meters() > height.meters() + 64. * f64::EPSILON * height.meters().max(1.)
    {
        return Err(invalid("source thickness mismatch or out-of-stock bore"));
    }
    let mut source_spans = [norm(u), norm(v)];
    let mut actual_spans = witness.face_spans_m;
    source_spans.sort_by(f64::total_cmp);
    actual_spans.sort_by(f64::total_cmp);
    if source_spans
        .into_iter()
        .zip(actual_spans)
        .any(|(a, b)| (a - b).abs() > EPS)
    {
        return Err(invalid("source dimensions disagree with physical stock"));
    }
    for corner in &corners {
        let entry =
            std::array::from_fn(|i| corner[i] + if top { axis[i] * height.meters() } else { 0. });
        let p = sub(entry, witness.origin_m);
        let a = dot(p, witness.u_axis);
        let b = dot(p, witness.v_axis);
        if dot(p, normal).abs() > EPS
            || ![a, b]
                .into_iter()
                .zip(witness.face_spans_m)
                .all(|(a, size)| a.abs() <= EPS || (a - size).abs() <= EPS)
        {
            return Err(invalid("actual stock corners disagree with source"));
        }
    }
    let radius = outer_diameter.meters() / 2.;
    let local = sub(center, witness.origin_m);
    for (c, size) in [dot(local, witness.u_axis), dot(local, witness.v_axis)]
        .into_iter()
        .zip(witness.face_spans_m)
    {
        if c - radius <= EPS || size - c - radius <= EPS {
            return Err(invalid("full seat disk must be strictly contained"));
        }
    }
    let refs: Vec<_> = ctx
        .semantic_refs()
        .iter()
        .filter(|r| r.ref_id.as_str() == face_ref.as_str())
        .collect();
    if refs.len() != 1
        || refs[0].kind != TopoRefKind::Face
        || refs[0].semantic.created_by != def.hole.target_feature
    {
        return Err(invalid("entry reference missing, ambiguous or foreign"));
    }
    let reference = refs[0];
    if reference
        .geometric_fingerprint
        .as_ref()
        .is_some_and(|fp| !matches!(fp.surface_type.as_str(), "plane" | "brep_face"))
    {
        return Err(invalid("entry fingerprint is not planar"));
    }
    // Semantic hints may originate in the f32 picking API. They are checked
    // as annotations; the actual sketch/source/kernel normal stays at 1e-9.
    if reference.semantic.normal_hint.is_some_and(|n| {
        !norm(n).is_finite()
            || (norm(n) - 1.).abs() > 1e-6
            || norm(cross(n, normal)) > 1e-6
            || dot(n, normal) <= 0.
    }) {
        return Err(invalid("entry reference normal disagrees"));
    }
    if let Some(hints) = reference
        .geometric_fingerprint
        .as_ref()
        .and_then(|fp| fp.bbox_hint)
    {
        if hints.iter().flatten().any(|v| !v.is_finite()) {
            return Err(invalid("nonfinite entry fingerprint"));
        }
        // A rectangle's axis-aligned bounds have their midpoint in the plane
        // even when their min/max corners are not themselves on the face.
        let midpoint = std::array::from_fn(|i| (hints[0][i] + hints[1][i]) / 2.);
        let error: f64 = (0..3)
            .map(|i| {
                normal[i].abs()
                    * 4.
                    * f32::EPSILON as f64
                    * hints[0][i]
                        .abs()
                        .max(hints[1][i].abs())
                        .max(witness.origin_m[i].abs())
            })
            .sum();
        if dot(sub(midpoint, witness.origin_m), normal).abs() > EPS + error {
            return Err(invalid("stale entry plane fingerprint"));
        }
    }
    let discoveries = ctx.face_discoveries_on(&target)?;
    let candidates: Vec<_> = discoveries
        .iter()
        .filter(|d| Some(d.role.as_str()) == reference.semantic.role.as_deref())
        .collect();
    if candidates.len() != 1 || candidates[0].kernel_face_id != witness.entry_face_id {
        return Err(invalid(
            "semantic entry does not uniquely identify physical stock face",
        ));
    }
    if let Some(id) = reference.kernel_face_id() {
        if discoveries.iter().any(|d| d.kernel_face_id == id) && id != witness.entry_face_id {
            return Err(invalid("stale entry face binding"));
        }
    }
    kernel.assign_face_ref(&target, witness.entry_face_id, reference.ref_id.clone())?;
    Ok(OwnedHoleInputs {
        target,
        profile,
        inward_direction: normal.map(|v| -v),
        stock: witness,
    })
}
fn require_output(
    kernel: &dyn GeometryKernel,
    target: &KernelBody,
    body: &KernelBody,
    profile: &opencad_geometry::SolvedSketch,
    def: &SeatedStandardHoleFeature,
    direction: [f64; 3],
) -> Result<()> {
    let before = kernel.mass_properties(target, 1.)?;
    let after = kernel.mass_properties(body, 1.)?;
    let bore_radius = def.standard.dimensions().cut_diameter.meters() / 2.;
    let (diameter, depth) = def
        .seat
        .dimensions(def.standard.dimensions().cut_diameter)?;
    let bore = std::f64::consts::PI * bore_radius.powi(2) * def.bore_depth.meters();
    let seat_radius = diameter.meters() / 2.;
    let d = depth.meters();
    let (annulus, seat_first) = match def.seat {
        StandardHoleSeat::Counterbore { .. } => {
            let volume = std::f64::consts::PI * (seat_radius.powi(2) - bore_radius.powi(2)) * d;
            (volume, volume * d / 2.)
        }
        StandardHoleSeat::Countersink { .. } => (
            std::f64::consts::PI
                * d
                * (seat_radius.powi(2) + seat_radius * bore_radius - 2. * bore_radius.powi(2))
                / 3.,
            std::f64::consts::PI
                * d
                * d
                * (seat_radius.powi(2) + 2. * seat_radius * bore_radius - 3. * bore_radius.powi(2))
                / 12.,
        ),
    };
    let expected = before.volume_m3 - bore - annulus;
    if !after.volume_m3.is_finite()
        || !expected.is_finite()
        || expected <= 0.
        || bore + annulus <= 64. * f64::EPSILON * before.volume_m3
        || (after.volume_m3 - expected).abs() > expected * 1e-8
    {
        return Err(invalid(
            "physical removed volume disagrees with contained seat",
        ));
    }
    let plane = profile
        .placement
        .ok_or_else(|| invalid("missing admitted frame"))?;
    let circle = profile
        .circle
        .ok_or_else(|| invalid("missing admitted circle"))?;
    let center = plane.map_point(circle.center_m[0], circle.center_m[1]);
    for i in 0..3 {
        let first = before.volume_m3 * before.center_of_mass[i]
            - bore * (center[i] + direction[i] * def.bore_depth.meters() / 2.)
            - annulus * center[i]
            - direction[i] * seat_first;
        if !after.center_of_mass[i].is_finite()
            || (after.center_of_mass[i] - first / expected).abs() > 1e-9
        {
            return Err(invalid("physical first moments disagree with seat"));
        }
    }
    require_physical_output(kernel, body)
}
fn require_physical_output(kernel: &dyn GeometryKernel, body: &KernelBody) -> Result<()> {
    let mesh = kernel.tessellate(body, &TessellationSettings::default())?;
    if mesh.triangle_count() == 0 || mesh.triangle_face_ids.len() != mesh.triangle_count() {
        return Err(invalid("physical mesh/face metadata required"));
    }
    validate_closed_mesh(&mesh)?;
    Ok(())
}
pub(crate) fn validate_closed_mesh(mesh: &MeshSet) -> Result<()> {
    let mut edges = std::collections::BTreeMap::new();
    let mut volume = 0.;
    let origin = mesh
        .positions
        .first()
        .ok_or_else(|| invalid("empty mesh"))?
        .map(f64::from);
    for t in mesh.indices.chunks_exact(3) {
        let mut p = [[0.; 3]; 3];
        for i in 0..3 {
            p[i] = mesh
                .positions
                .get(t[i] as usize)
                .ok_or_else(|| invalid("invalid mesh index"))?
                .map(f64::from);
        }
        if p.iter().flatten().any(|v| !v.is_finite())
            || norm(cross(sub(p[1], p[0]), sub(p[2], p[0]))) <= 0.
        {
            return Err(invalid("degenerate physical mesh"));
        }
        volume += dot(
            sub(p[0], origin),
            cross(sub(p[1], origin), sub(p[2], origin)),
        ) / 6.;
        for i in 0..3 {
            let key = |p: [f64; 3]| p.map(|v| (if v == 0. { 0. } else { v as f32 }).to_bits());
            let a = key(p[i]);
            let b = key(p[(i + 1) % 3]);
            let (k, s) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
            let entry = edges.entry(k).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += s;
        }
    }
    if !volume.is_finite() || volume <= 0. || edges.values().any(|v| *v != (2, 0)) {
        return Err(invalid("mesh must be outward and closed"));
    }
    Ok(())
}

/// Existing closed physical hole mesh gate, shared with reviewed native forms.
pub fn validate_owned_hole_mesh(mesh: &MeshSet) -> Result<()> {
    if mesh.indices.len() % 3 != 0
        || mesh.triangle_count() == 0
        || mesh.triangle_face_ids.len() != mesh.triangle_count()
        || mesh.triangle_face_ids.contains(&0)
    {
        return Err(invalid("physical mesh/face metadata required"));
    }
    validate_closed_mesh(mesh)
}
