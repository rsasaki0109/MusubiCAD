//! ADR-101: explicit numerical approximation with guarded authored phase.
use crate::sketch_bridge::{profile_to_solved_with_context, resolve_workplane};
use crate::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::{
    CircularFilletGridAreaPrecision, CircularFilletGridMomentPrecision,
    CircularFilletGridPrecision, CircularFilletGridVolumePrecision, CircularFilletMeshPrecision,
    CircularFilletMomentPrecision, FaceRefDiscovery, FacetedCircularFilletInput,
    FacetedCircularFilletPlan, KernelBody, TessellationSettings, TopoRefKind,
};
use opencad_sketch::Sketch;
use serde::{Deserialize, Serialize};
fn si_length<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Length, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SiLength {
        value_si: f64,
    }
    Ok(Length::from_meters(SiLength::deserialize(d)?.value_si))
}
fn invalid(s: &str) -> OpenCadError {
    OpenCadError::validation(format!("faceted circular fillet: {s}"))
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
fn norm(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}
// Apply the stricter phase contract before workplane placement normalizes axes,
// including direct execution and resolved datum/face workplanes.
fn phase_placement(plane: &opencad_sketch::Workplane) -> Result<opencad_geometry::SketchPlacement> {
    if let opencad_sketch::Workplane::Custom { normal, x_axis, .. } = plane {
        if normal.iter().chain(x_axis).any(|v| !v.is_finite())
            || (norm(*normal) - 1.).abs() > 1e-12
            || (norm(*x_axis) - 1.).abs() > 1e-12
            || dot(*normal, *x_axis).abs() > 1e-12
        {
            return Err(invalid("raw authored axes must be unit orthogonal"));
        }
    }
    crate::nominal_metric_thread::checked_placement(plane)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacetedFilletRepresentation {
    GridFacetedPeriodicCosine,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacetedFilletVerification {
    CanonicalGridWithNumericalBackendChecks,
}
/// SI absolute budgets. Canonical input-grid bounds do not certify conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FacetedFilletQuality {
    #[serde(deserialize_with = "si_length")]
    pub maximum_position_error: Length,
    pub maximum_normal_chord: f64,
    pub ideal_volume_width_m3: f64,
    pub maximum_volume_difference_m3: f64,
    pub ideal_area_width_m2: f64,
    pub maximum_area_difference_m2: f64,
    pub ideal_first_moment_width_m4: f64,
    pub maximum_first_moment_difference_m4: f64,
    #[serde(deserialize_with = "si_length")]
    pub ideal_centroid_width: Length,
    #[serde(deserialize_with = "si_length")]
    pub maximum_centroid_difference: Length,
    pub max_integration_cells: u32,
    pub max_vertices: u32,
    pub max_triangles: u32,
}
impl FacetedFilletQuality {
    fn validate(&self) -> Result<()> {
        if [
            self.maximum_position_error.meters(),
            self.maximum_normal_chord,
            self.ideal_volume_width_m3,
            self.maximum_volume_difference_m3,
            self.ideal_area_width_m2,
            self.maximum_area_difference_m2,
            self.ideal_first_moment_width_m4,
            self.maximum_first_moment_difference_m4,
            self.ideal_centroid_width.meters(),
            self.maximum_centroid_difference.meters(),
        ]
        .into_iter()
        .any(|v| !v.is_finite() || v <= 0.)
            || self.maximum_normal_chord >= 1.
            || !(1..=262_144).contains(&self.max_integration_cells)
            || !(4..=10_000).contains(&self.max_vertices)
            || !(4..=10_000).contains(&self.max_triangles)
        {
            return Err(invalid("invalid explicit quality or bounded work"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FacetedCircularFilletFeature {
    pub target_feature: String,
    pub edge_ref: String,
    pub representation: FacetedFilletRepresentation,
    pub verification: FacetedFilletVerification,
    #[serde(deserialize_with = "si_length")]
    pub minimum_radius: Length,
    #[serde(deserialize_with = "si_length")]
    pub maximum_radius: Length,
    pub quality: FacetedFilletQuality,
}
impl FacetedCircularFilletFeature {
    pub fn validate(&self) -> Result<()> {
        if !self.target_feature.starts_with("feature:")
            || self.target_feature.len() <= 8
            || !self.edge_ref.starts_with("ref:edge:")
            || self.edge_ref.len() <= 9
            || !self.minimum_radius.meters().is_finite()
            || !self.maximum_radius.meters().is_finite()
            || self.minimum_radius.meters() <= 0.
            || self.maximum_radius.meters() < self.minimum_radius.meters()
        {
            return Err(invalid(
                "target, owned end rim and ordered SI radii required",
            ));
        }
        self.quality.validate()
    }
    pub fn validate_source(
        &self,
        nodes: &[&FeatureNode],
        sketches: &[&Sketch],
        scope: &indexmap::IndexMap<String, f64>,
    ) -> Result<()> {
        self.validate()?;
        let source_sketch = nodes
            .iter()
            .find(|n| n.id == self.target_feature)
            .and_then(|n| match &n.definition {
                FeatureDefinition::Extrude(e) => Some(e.sketch_feature.as_str()),
                _ => None,
            })
            .and_then(|id| nodes.iter().find(|n| n.id == id))
            .and_then(|n| match &n.definition {
                FeatureDefinition::Sketch(s) => Some(s.sketch_id.as_str()),
                _ => None,
            })
            .and_then(|id| sketches.iter().find(|s| s.id.as_str() == id));
        if let Some(sketch) = source_sketch {
            if matches!(sketch.workplane, opencad_sketch::Workplane::Custom { .. }) {
                phase_placement(&sketch.workplane)?;
            }
        }
        crate::nominal_metric_thread::validate_original_circle_source(
            &self.target_feature,
            nodes,
            sketches,
            scope,
        )
    }
    fn input(
        &self,
        edge: u64,
        origin: [f64; 3],
        x: [f64; 3],
        axis: [f64; 3],
        radius: f64,
        height: Length,
    ) -> FacetedCircularFilletInput {
        let q = &self.quality;
        FacetedCircularFilletInput {
            end_edge_id: edge,
            origin_m: origin,
            phase_x: x,
            axis,
            cylinder_radius: Length::from_meters(radius),
            cylinder_height: height,
            minimum_radius: self.minimum_radius,
            maximum_radius: self.maximum_radius,
            mesh_precision: CircularFilletMeshPrecision {
                maximum_position_error: q.maximum_position_error,
                maximum_normal_error: q.maximum_normal_chord,
                max_vertices: q.max_vertices as usize,
                max_triangles: q.max_triangles as usize,
            },
            grid_precision: CircularFilletGridPrecision {
                maximum_position_error: q.maximum_position_error,
                maximum_normal_error: q.maximum_normal_chord,
            },
            volume_precision: CircularFilletGridVolumePrecision {
                maximum_reference_interval_width_m3: q.ideal_volume_width_m3,
                maximum_absolute_difference_m3: q.maximum_volume_difference_m3,
                max_cells: q.max_integration_cells as usize,
            },
            area_precision: CircularFilletGridAreaPrecision {
                maximum_reference_interval_width_m2: q.ideal_area_width_m2,
                maximum_absolute_difference_m2: q.maximum_area_difference_m2,
                max_cells: q.max_integration_cells as usize,
            },
            moment_precision: CircularFilletGridMomentPrecision {
                ideal: CircularFilletMomentPrecision {
                    maximum_volume_interval_width_m3: q.ideal_volume_width_m3,
                    maximum_first_moment_interval_width_m4: q.ideal_first_moment_width_m4,
                    maximum_centroid_interval_width: q.ideal_centroid_width,
                    max_cells: q.max_integration_cells as usize,
                },
                maximum_absolute_first_moment_difference_m4: q.maximum_first_moment_difference_m4,
                maximum_absolute_centroid_difference: q.maximum_centroid_difference,
            },
        }
    }
}
struct Admission {
    target: KernelBody,
    input: FacetedCircularFilletInput,
    plan: FacetedCircularFilletPlan,
}
fn admit(node: &FeatureNode, ctx: &dyn RegenContext) -> Result<Admission> {
    let FeatureDefinition::FacetedCircularFillet(def) = &node.definition else {
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
        return Err(invalid("another active solid consumer uses stock"));
    }
    let targets: Vec<_> = nodes
        .iter()
        .filter(|n| n.id == def.target_feature && !n.suppressed)
        .collect();
    if targets.len() != 1 {
        return Err(invalid("unique original extrusion required"));
    }
    let FeatureDefinition::Extrude(e) = &targets[0].definition else {
        return Err(invalid("original cylinder extrusion required"));
    };
    let opencad_geometry::ExtrudeExtent::Distance { length } = e.extent else {
        return Err(invalid("distance extrusion required"));
    };
    if e.operation != opencad_geometry::ExtrudeOperation::NewBody || e.target_feature.is_some() {
        return Err(invalid("original NewBody required"));
    }
    let sketches: Vec<_> = nodes
        .iter()
        .filter(|n| n.id == e.sketch_feature && !n.suppressed)
        .collect();
    if sketches.len() != 1 || !matches!(sketches[0].definition, FeatureDefinition::Sketch(_)) {
        return Err(invalid("original authored sketch required"));
    }
    let sketch = ctx.sketch_for_feature(&e.sketch_feature)?;
    let raw = phase_placement(&resolve_workplane(&sketch.workplane, ctx)?)?;
    if (norm(raw.x_axis_m) - 1.).abs() > 1e-12
        || (norm(raw.y_axis_m) - 1.).abs() > 1e-12
        || dot(raw.x_axis_m, raw.y_axis_m).abs() > 1e-12
    {
        return Err(invalid("proper authored phase frame required"));
    }
    let profile = profile_to_solved_with_context(sketch, &e.profile_ref, ctx)?;
    let circle = profile
        .circle
        .ok_or_else(|| invalid("exact circle required"))?;
    if !profile.segments.is_empty() || profile.ellipse.is_some() || profile.cubic_bezier.is_some() {
        return Err(invalid("mixed circle unsupported"));
    }
    let placement = profile
        .placement
        .ok_or_else(|| invalid("missing placement"))?;
    let axis = cross(raw.x_axis_m, raw.y_axis_m);
    let origin = placement.map_point(circle.center_m[0], circle.center_m[1]);
    let target = ctx.body_for_feature(&def.target_feature)?;
    let rims = ctx.kernel().cylindrical_stock_rims(&target, axis)?;
    let input = def.input(
        rims.end_edge_id,
        origin,
        raw.x_axis_m,
        axis,
        circle.radius_m,
        length,
    );
    // Privately rebuild all canonical bounds; neither cache nor caller owns a certificate.
    let plan = input.prepare(&rims)?;
    let refs: Vec<_> = ctx
        .semantic_refs()
        .iter()
        .filter(|r| r.ref_id.as_str() == def.edge_ref)
        .collect();
    if refs.len() != 1
        || refs[0].kind != TopoRefKind::Edge
        || refs[0].semantic.created_by != def.target_feature
        || refs[0].semantic.role.as_deref() != Some("extrusion_end")
    {
        return Err(invalid("unique owned positive end Edge required"));
    }
    let r = refs[0];
    if ctx.semantic_refs().iter().any(|other| {
        other.ref_id != r.ref_id
            && other.kind == TopoRefKind::Edge
            && other.semantic.created_by == def.target_feature
            && other.semantic.role == r.semantic.role
    }) {
        return Err(invalid("ambiguous end-rim aliases"));
    }
    if r.fallback_query.is_some()
        || r.semantic
            .normal_hint
            .is_some_and(|n| norm(sub(n, axis)) > 1e-9 || n.iter().any(|v| !v.is_finite()))
    {
        return Err(invalid("stale or unsupported end annotation"));
    }
    if let Some(fp) = &r.geometric_fingerprint {
        if fp.surface_type != "circle"
            || fp.kernel_face_id.is_some()
            || fp.area_range.is_some()
            || fp.bbox_hint.is_some()
            || !fp.adjacent_feature_ids.is_empty()
        {
            return Err(invalid("unsupported circle fingerprint"));
        }
        if let Some(id) = fp.kernel_edge_id {
            // Generic discovery only lists top edges in the native backend.
            // The typed rim witness proves the current start independently;
            // a direct endpoint query detects the present seam/other edges.
            if id == 0
                || (id != rims.end_edge_id
                    && (id == rims.start_edge_id
                        || ctx.kernel().edge_endpoints(&target, id).is_ok()))
            {
                return Err(invalid("rim bound to another current edge"));
            }
        }
    }
    let edge = ctx
        .kernel()
        .circular_edge_geometry(&target, rims.end_edge_id)?;
    let expected = std::array::from_fn(|i| origin[i] + axis[i] * length.meters());
    if edge
        .center_m
        .iter()
        .chain(&edge.normal)
        .chain(std::iter::once(&edge.radius_m))
        .any(|v| !v.is_finite())
        || norm(sub(edge.center_m, expected)) > 1e-9
        || (edge.radius_m - circle.radius_m).abs() > 1e-9
        || (norm(edge.normal) - 1.).abs() > 1e-12
        || (dot(edge.normal, axis).abs() - 1.).abs() > 1e-12
    {
        return Err(invalid("current complete end circle disagrees"));
    }
    Ok(Admission {
        target,
        input,
        plan,
    })
}
fn require_output(
    node: &FeatureNode,
    ctx: &dyn RegenContext,
    body: &KernelBody,
    a: &Admission,
) -> Result<Vec<FaceRefDiscovery>> {
    let expected = a.plan.grid().solid().transformed(a.plan.placement())?;
    let mass = ctx.kernel().mass_properties(body, 1.)?;
    let reference = expected.mass_properties(1.)?;
    if !mass.volume_m3.is_finite()
        || !mass.area_m2.is_finite()
        || (mass.volume_m3 - reference.volume_m3).abs() > reference.volume_m3 * 1e-8
        || (mass.area_m2 - reference.area_m2).abs() > reference.area_m2 * 1e-8
        || mass
            .center_of_mass
            .into_iter()
            .zip(reference.center_of_mass)
            .any(|(v, e)| !v.is_finite() || (v - e).abs() > 2e-8 + 64. * f64::EPSILON * e.abs())
    {
        return Err(invalid("actual numerical mass differs from placed grid"));
    }
    let mesh = ctx
        .kernel()
        .tessellate(body, &TessellationSettings::default())?;
    if mesh.triangle_count() != expected.triangles().len()
        || mesh.indices.len() % 3 != 0
        || mesh.triangle_face_ids.len() != mesh.triangle_count()
        || mesh.positions.iter().flatten().any(|v| !v.is_finite())
        || mesh
            .indices
            .iter()
            .any(|i| *i as usize >= mesh.positions.len())
    {
        return Err(invalid("finite all-facet physical mesh required"));
    }
    crate::seated_standard_hole::validate_closed_mesh(&mesh)?;
    let coordinate = expected
        .vertices_m()
        .iter()
        .flatten()
        .map(|v| v.abs())
        .fold(0., f64::max);
    let error = 1e-9 + 8. * f32::EPSILON as f64 * coordinate;
    let amplitude = (a.input.maximum_radius.meters() - a.input.minimum_radius.meters()) / 2.;
    if error > a.input.minimum_radius.meters() / 32. || (amplitude > 0. && error > amplitude / 32.)
    {
        return Err(invalid("coordinate precision cannot resolve radius law"));
    }
    // Distinct corner identification must be unique at the numerical tolerance.
    for (i, p) in expected.vertices_m().iter().enumerate() {
        if expected.vertices_m()[..i]
            .iter()
            .any(|q| norm(sub(*p, *q)) <= 2. * error)
        {
            return Err(invalid("facet corners numerically ambiguous"));
        }
    }
    let mut lookup = std::collections::BTreeMap::new();
    for (i, t) in expected.triangles().iter().enumerate() {
        let min = (0..3)
            .min_by_key(|j| t[*j])
            .ok_or_else(|| invalid("missing facet"))?;
        lookup.insert([t[min], t[(min + 1) % 3], t[(min + 2) % 3]], i);
    }
    let mut matched = std::collections::BTreeSet::new();
    let mut ids = std::collections::BTreeSet::new();
    let mut discoveries = Vec::new();
    for (tri, id) in mesh.indices.chunks_exact(3).zip(&mesh.triangle_face_ids) {
        if *id == 0 || !ids.insert(*id) {
            return Err(invalid("missing or repeated physical facet identity"));
        }
        let mut corners = [0; 3];
        let world =
            std::array::from_fn::<_, 3, _>(|j| mesh.positions[tri[j] as usize].map(f64::from));
        for j in 0..3 {
            let candidates: Vec<_> = expected
                .vertices_m()
                .iter()
                .enumerate()
                .filter(|(_, p)| norm(sub(world[j], **p)) <= error)
                .collect();
            if candidates.len() != 1 {
                return Err(invalid("actual facet violates authored phase/grid"));
            }
            corners[j] = candidates[0].0 as u32;
        }
        let min = (0..3)
            .min_by_key(|j| corners[*j])
            .ok_or_else(|| invalid("missing corner"))?;
        let key = [corners[min], corners[(min + 1) % 3], corners[(min + 2) % 3]];
        if lookup.get(&key).map_or(true, |i| !matched.insert(*i)) {
            return Err(invalid("wrong facet connectivity/winding/duplication"));
        }
        let normal = cross(sub(world[1], world[0]), sub(world[2], world[0]));
        let length = norm(normal);
        discoveries.push(FaceRefDiscovery {
            kernel_face_id: *id,
            role: "faceted_surface".into(),
            normal_m: normal.map(|v| (v / length) as f32),
            centroid_m: std::array::from_fn(|i| {
                ((world[0][i] + world[1][i] + world[2][i]) / 3.) as f32
            }),
            feature_id: Some(node.id.clone()),
        });
    }
    if matched.len() != expected.triangles().len() {
        return Err(invalid("missing expected facet"));
    }
    Ok(discoveries)
}
#[derive(Debug, Default)]
pub struct FacetedCircularFilletFeatureExecutor;
impl Feature for FacetedCircularFilletFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "faceted_circular_fillet"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let a = admit(node, ctx)?;
        let body = ctx
            .kernel()
            .grid_faceted_periodic_cosine(a.target.clone(), &a.input)?;
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
        Ok(Some(require_output(
            node,
            ctx,
            output
                .body
                .as_ref()
                .ok_or_else(|| invalid("missing cached physical body"))?,
            &a,
        )?))
    }
}
