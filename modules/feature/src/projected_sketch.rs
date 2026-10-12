//! ADR-054: graph-owned semantic cross-sketch geometry, never authored copies.
use crate::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use opencad_core::{EntityId, OpenCadError, Result, SketchId};
use opencad_geometry::SketchPlacement;
use opencad_sketch::{
    CircleEntity, Coord, EllipseEntity, EntityBase, LineEntity, PointEntity, Sketch, SketchEntity,
    Workplane,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const EPS: f64 = 1e-9;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveProjection {
    pub source_feature: String,
    pub source_entity: String,
    pub curve_id: String,
    pub point_ids: Vec<String>,
    #[serde(default)]
    pub construction: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedSketchFeature {
    pub sketch_id: String,
    pub workplane: Workplane,
    pub projections: Vec<CurveProjection>,
}
fn stable(value: &str, prefix: &str) -> Result<()> {
    let suffix = value
        .strip_prefix(prefix)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            OpenCadError::validation("projection IDs require explicit stable prefixes")
        })?;
    if !suffix
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
    {
        return Err(OpenCadError::validation("invalid projection stable ID"));
    }
    Ok(())
}
fn plane(workplane: &Workplane) -> Result<SketchPlacement> {
    match workplane {
        Workplane::Global { .. } => {}
        Workplane::Custom {
            origin,
            normal,
            x_axis,
        } => {
            let norm = |v: &[f64; 3]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
            let n = norm(normal);
            let x = norm(x_axis);
            if !origin
                .iter()
                .chain(normal)
                .chain(x_axis)
                .all(|v| v.is_finite())
                || n <= 1e-12
                || x <= 1e-12
                || !n.is_finite()
                || !x.is_finite()
                || normal
                    .iter()
                    .zip(x_axis)
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
                    .abs()
                    > n * x * 1e-9
            {
                return Err(OpenCadError::validation(
                    "projection plane must have finite orthogonal axes",
                ));
            }
        }
        _ => {
            return Err(OpenCadError::validation(
                "projection initially requires explicit Global/Custom source and target planes",
            ))
        }
    }
    let p = crate::sketch_bridge::placement_from_workplane(workplane)?;
    opencad_geometry::SolvedEllipse {
        center_m: [0.; 2],
        major_radius_m: 1.,
        minor_radius_m: 1.,
        axis_angle_rad: 0.,
    }
    .frame(p)?;
    Ok(p)
}
impl ProjectedSketchFeature {
    pub fn validate(&self) -> Result<()> {
        stable(&self.sketch_id, "sketch:")?;
        plane(&self.workplane)?;
        if self.projections.is_empty() || self.projections.len() > 256 {
            return Err(OpenCadError::validation(
                "projection requires 1..256 curve bindings",
            ));
        }
        let mut curves = BTreeSet::new();
        let mut sources = BTreeSet::new();
        let mut points = BTreeSet::new();
        for p in &self.projections {
            stable(&p.source_feature, "feature:")?;
            stable(&p.source_entity, "ent:")?;
            stable(&p.curve_id, "ent:")?;
            if !curves.insert(&p.curve_id) || !sources.insert((&p.source_feature, &p.source_entity))
            {
                return Err(OpenCadError::validation(
                    "duplicate projection curve/source binding",
                ));
            }
            if p.point_ids.is_empty() || p.point_ids.len() > 2 {
                return Err(OpenCadError::validation(
                    "projection needs explicit centre or endpoint IDs",
                ));
            }
            for point in &p.point_ids {
                stable(point, "ent:")?;
                points.insert(point);
            }
        }
        if curves.iter().any(|id| points.contains(id)) {
            return Err(OpenCadError::validation(
                "projection curve and point IDs must differ",
            ));
        }
        Ok(())
    }
}
fn unique<'a>(sketch: &'a Sketch, id: &str) -> Result<&'a SketchEntity> {
    let mut found = sketch.entities.iter().filter(|e| e.id().as_str() == id);
    let e = found
        .next()
        .ok_or_else(|| OpenCadError::not_found(format!("projection source entity '{id}'")))?;
    if found.next().is_some() {
        return Err(OpenCadError::validation(format!(
            "ambiguous projection source entity '{id}'"
        )));
    }
    Ok(e)
}
fn point(sketch: &Sketch, id: &str) -> Result<[f64; 2]> {
    let SketchEntity::Point(p) = unique(sketch, id)? else {
        return Err(OpenCadError::validation(
            "projection source endpoint/centre must be a point",
        ));
    };
    Ok([
        opencad_sketch::solve::coord_value(&p.x)?,
        opencad_sketch::solve::coord_value(&p.y)?,
    ])
}
fn into_plane(world: [f64; 3], p: SketchPlacement) -> [f64; 2] {
    let d = std::array::from_fn::<_, 3, _>(|i| world[i] - p.origin_m[i]);
    [
        d.iter().zip(p.x_axis_m).map(|(a, b)| a * b).sum(),
        d.iter().zip(p.y_axis_m).map(|(a, b)| a * b).sum(),
    ]
}
fn vector_into_plane(v: [f64; 3], p: SketchPlacement) -> [f64; 2] {
    [
        v.iter().zip(p.x_axis_m).map(|(a, b)| a * b).sum(),
        v.iter().zip(p.y_axis_m).map(|(a, b)| a * b).sum(),
    ]
}
fn base(id: &str, construction: bool) -> Result<EntityBase> {
    Ok(EntityBase {
        id: EntityId::new(id)?,
        construction,
    })
}
/// Generate geometry from unique semantic bindings; lookup supplies solved inputs.
pub fn project_sketch(
    def: &ProjectedSketchFeature,
    name: &str,
    mut lookup: impl FnMut(&str) -> Result<Sketch>,
) -> Result<Sketch> {
    def.validate()?;
    let target = plane(&def.workplane)?;
    let mut output = Sketch::new(SketchId::new(&def.sketch_id)?, name, def.workplane.clone());
    let mut vertices: BTreeMap<String, (String, String, [f64; 2], bool)> = BTreeMap::new();
    let mut curves = Vec::new();
    let mut sources = BTreeMap::new();
    for binding in &def.projections {
        if !sources.contains_key(&binding.source_feature) {
            sources.insert(
                binding.source_feature.clone(),
                lookup(&binding.source_feature)?,
            );
        }
        let source = &sources[&binding.source_feature];
        let placement = plane(&source.workplane)?;
        let entity = unique(source, &binding.source_entity)?;
        let mut add_point =
            |source_id: &str, target_id: &str, value: [f64; 2]| -> Result<EntityId> {
                if !value.iter().all(|v| v.is_finite() && v.abs() <= 1e6) {
                    return Err(OpenCadError::validation(
                        "projected coordinates must be finite within 1e6 m",
                    ));
                }
                if let Some((feature, vertex, previous, construction)) = vertices.get_mut(target_id)
                {
                    if feature != &binding.source_feature
                        || vertex != source_id
                        || (previous[0] - value[0]).hypot(previous[1] - value[1]) > EPS
                    {
                        return Err(OpenCadError::validation(
                        "shared projection point must retain one semantic source vertex identity",
                    ));
                    }
                    *construction &= binding.construction;
                } else {
                    vertices.insert(
                        target_id.into(),
                        (
                            binding.source_feature.clone(),
                            source_id.into(),
                            value,
                            binding.construction,
                        ),
                    );
                }
                EntityId::new(target_id)
            };
        match entity {
            SketchEntity::Line(line) => {
                if binding.point_ids.len() != 2 {
                    return Err(OpenCadError::validation(
                        "line projection needs two endpoint IDs",
                    ));
                }
                let a = point(source, line.start.as_str())?;
                let b = point(source, line.end.as_str())?;
                let a = into_plane(placement.map_point(a[0], a[1]), target);
                let b = into_plane(placement.map_point(b[0], b[1]), target);
                if (a[0] - b[0]).hypot(a[1] - b[1]) <= EPS {
                    return Err(OpenCadError::validation(
                        "projection collapses a source line",
                    ));
                }
                let start = add_point(line.start.as_str(), &binding.point_ids[0], a)?;
                let end = add_point(line.end.as_str(), &binding.point_ids[1], b)?;
                curves.push(SketchEntity::Line(LineEntity {
                    base: base(&binding.curve_id, binding.construction)?,
                    start,
                    end,
                }));
            }
            SketchEntity::Circle(_) | SketchEntity::Ellipse(_) => {
                if binding.point_ids.len() != 1 {
                    return Err(OpenCadError::validation(
                        "conic projection needs one centre ID",
                    ));
                }
                let (centre_id, a, b, angle, is_circle) = match entity {
                    SketchEntity::Circle(c) => {
                        let r = opencad_sketch::solve::coord_value(&c.radius)?;
                        (c.center.as_str(), r, r, 0., true)
                    }
                    SketchEntity::Ellipse(e) => {
                        let [a, b, angle] = opencad_sketch::solve::ellipse_parameters(e)?;
                        (e.center.as_str(), a, b, angle, false)
                    }
                    _ => return Err(OpenCadError::validation("unsupported projection conic")),
                };
                let c = point(source, centre_id)?;
                let frame = opencad_geometry::SolvedEllipse {
                    center_m: c,
                    major_radius_m: a,
                    minor_radius_m: b,
                    axis_angle_rad: angle,
                }
                .frame(placement)?;
                let centre = add_point(
                    centre_id,
                    &binding.point_ids[0],
                    into_plane(frame.center_m, target),
                )?;
                let u = vector_into_plane(frame.major_vector_m, target);
                let v = vector_into_plane(frame.minor_vector_m, target);
                let xx = u[0] * u[0] + v[0] * v[0];
                let yy = u[1] * u[1] + v[1] * v[1];
                let xy = u[0] * u[1] + v[0] * v[1];
                let major2 = (xx + yy + (xx - yy).hypot(2. * xy)) / 2.;
                let det = u[0] * v[1] - u[1] * v[0];
                let major = major2.sqrt();
                let minor = (det * det / major2).sqrt();
                let orientation = 0.5 * (2. * xy).atan2(xx - yy);
                if !major.is_finite() || !minor.is_finite() || minor <= EPS {
                    return Err(OpenCadError::validation("projection collapses a conic"));
                }
                if is_circle && (major - minor).abs() <= 1e-12 * a {
                    curves.push(SketchEntity::Circle(CircleEntity {
                        base: base(&binding.curve_id, binding.construction)?,
                        center: centre,
                        radius: Coord::Literal(a),
                    }));
                } else {
                    curves.push(SketchEntity::Ellipse(EllipseEntity {
                        base: base(&binding.curve_id, binding.construction)?,
                        center: centre,
                        major_radius: Coord::Literal(major),
                        minor_radius: Coord::Literal(minor),
                        axis_angle: Coord::Literal(orientation),
                        resolved_parameters: None,
                    }));
                }
            }
            _ => {
                return Err(OpenCadError::validation(
                    "projection source must be a line or full circle/ellipse",
                ))
            }
        }
    }
    for (id, (_, _, p, construction)) in vertices {
        output.entities.push(SketchEntity::Point(PointEntity {
            base: base(&id, construction)?,
            x: Coord::Literal(p[0]),
            y: Coord::Literal(p[1]),
        }));
    }
    output.entities.extend(curves);
    crate::prepare_sketch(&mut output)?;
    Ok(output)
}
/// Pure recursive resolution for structural validation; generated data is local.
pub fn resolve_design_sketch(
    nodes: &[FeatureNode],
    sketches: &[Sketch],
    parameters: &opencad_graph::ParamGraph,
    id: &str,
) -> Result<Sketch> {
    let values = if sketches.iter().any(|s| {
        s.entities
            .iter()
            .any(|e| matches!(e, SketchEntity::Ellipse(_)))
    }) {
        opencad_graph::evaluate_param_graph_strict(parameters)?
    } else {
        opencad_graph::evaluate_param_graph(parameters)?
    };
    fn resolve(
        nodes: &[FeatureNode],
        sketches: &[Sketch],
        values: &indexmap::IndexMap<String, f64>,
        id: &str,
        active: &mut BTreeSet<String>,
    ) -> Result<Sketch> {
        if active.len() >= 128 || !active.insert(id.into()) {
            return Err(OpenCadError::validation(
                "projection dependency cycle/depth",
            ));
        }
        let mut matching = nodes.iter().filter(|n| n.id == id);
        let node = matching
            .next()
            .ok_or_else(|| OpenCadError::not_found(format!("projection source feature '{id}'")))?;
        if matching.next().is_some() || node.suppressed {
            return Err(OpenCadError::validation(
                "projection source feature is ambiguous or suppressed",
            ));
        }
        let result = match &node.definition {
            FeatureDefinition::Sketch(def) => {
                let mut matching = sketches.iter().filter(|s| s.id.as_str() == def.sketch_id);
                let source = matching
                    .next()
                    .ok_or_else(|| OpenCadError::not_found("projection source sketch"))?;
                if matching.next().is_some() {
                    return Err(OpenCadError::validation("ambiguous source sketch ID"));
                }
                crate::param_apply::solved_sketch_copy(source, values)
            }
            FeatureDefinition::ProjectedSketch(def) => project_sketch(def, &node.name, |source| {
                resolve(nodes, sketches, values, source, active)
            }),
            _ => Err(OpenCadError::validation(
                "projection source must be a sketch feature",
            )),
        };
        active.remove(id);
        result
    }
    resolve(nodes, sketches, &values, id, &mut BTreeSet::new())
}
pub struct ProjectedSketchExecutor;
impl Feature for ProjectedSketchExecutor {
    fn feature_type(&self) -> &'static str {
        "projected_sketch"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::ProjectedSketch(def) = &node.definition else {
            return Err(OpenCadError::validation("expected projected_sketch"));
        };
        let sketch = project_sketch(def, &node.name, |source| {
            ctx.sketch_for_feature(source).cloned()
        })?;
        Ok(FeatureOutput {
            body: None,
            datum: None,
            sketch: Some(sketch),
        })
    }
}
