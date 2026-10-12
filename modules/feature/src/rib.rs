//! Parametric rib feature from an open sketch centreline (MCAD-P11-003).

use serde::{Deserialize, Serialize};

use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::{BooleanOp, ExtrudeExtent, ExtrudeOperation, SolvedSketch};
use opencad_sketch::entity::{Coord, SketchEntity};

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::sketch_bridge::{
    extrude_direction_for_sketch, placement_from_workplane, resolve_workplane,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RibFeature {
    pub sketch_feature: String,
    pub centerline_ref: String,
    pub target_feature: String,
    pub thickness: Length,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thickness_expr: Option<String>,
    pub extent: ExtrudeExtent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_expr: Option<String>,
}

pub fn validate_thickness(thickness_m: f64) -> Result<()> {
    if !thickness_m.is_finite() || thickness_m <= 0.0 {
        return Err(OpenCadError::validation(
            "rib thickness must be positive and finite",
        ));
    }
    Ok(())
}

impl RibFeature {
    pub fn validate(&self) -> Result<()> {
        if self.sketch_feature.is_empty()
            || self.centerline_ref.is_empty()
            || self.target_feature.is_empty()
        {
            return Err(OpenCadError::validation(
                "rib needs sketch_feature, centerline_ref and target_feature",
            ));
        }
        validate_thickness(self.thickness.meters())
    }
}

pub struct RibFeatureExecutor;

impl Feature for RibFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "rib"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Rib(def) = &node.definition else {
            return Err(OpenCadError::validation("expected rib"));
        };
        def.validate()?;

        let sketch = ctx.sketch_for_feature(&def.sketch_feature)?;
        let profile = rib_profile_from_centerline(sketch, &def.centerline_ref, def.thickness, ctx)?;
        let wire = ctx.kernel().make_wire_from_sketch(&profile)?;
        let direction = extrude_direction_for_sketch(sketch, ctx)?;
        let rib_body = ctx.kernel().extrude(
            wire,
            def.extent.clone(),
            ExtrudeOperation::NewBody,
            None,
            direction,
        )?;
        let target = ctx.body_for_feature(&def.target_feature)?;
        let body = ctx.kernel().boolean(target, rib_body, BooleanOp::Union)?;
        Ok(FeatureOutput {
            sketch: None,
            body: Some(body),
            datum: None,
        })
    }
}

fn rib_profile_from_centerline(
    sketch: &opencad_sketch::Sketch,
    centerline_ref: &str,
    thickness: Length,
    ctx: &dyn RegenContext,
) -> Result<SolvedSketch> {
    let line = sketch
        .entities
        .iter()
        .find_map(|entity| match entity {
            SketchEntity::Line(line) if line.base.id.as_str() == centerline_ref => Some(line),
            _ => None,
        })
        .ok_or_else(|| OpenCadError::not_found(format!("rib centerline '{centerline_ref}'")))?;
    let start = point_coord(sketch, line.start.as_str())?;
    let end = point_coord(sketch, line.end.as_str())?;
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite() || len <= 1e-9 {
        return Err(OpenCadError::validation(
            "rib centerline must have non-zero length",
        ));
    }
    let half = thickness.meters() * 0.5;
    validate_thickness(thickness.meters())?;
    let nx = -dy / len * half;
    let ny = dx / len * half;
    let points = vec![
        [start[0] + nx, start[1] + ny],
        [end[0] + nx, end[1] + ny],
        [end[0] - nx, end[1] - ny],
        [start[0] - nx, start[1] - ny],
    ];
    let workplane = resolve_workplane(&sketch.workplane, ctx)?;
    Ok(SolvedSketch {
        profile_ref: centerline_ref.to_string(),
        points,
        closed: true,
        ellipse: None,
        cubic_bezier: None,
        circle: None,
        segments: Vec::new(),
        placement: Some(placement_from_workplane(&workplane)?),
    })
}

fn point_coord(sketch: &opencad_sketch::Sketch, id: &str) -> Result<[f64; 2]> {
    let entity = sketch
        .find_entity(id)
        .ok_or_else(|| OpenCadError::not_found(format!("point '{id}'")))?;
    let SketchEntity::Point(point) = entity else {
        return Err(OpenCadError::validation(format!(
            "entity '{id}' is not a point"
        )));
    };
    match (&point.x, &point.y) {
        (Coord::Literal(x), Coord::Literal(y)) => Ok([*x, *y]),
        _ => Err(OpenCadError::validation(format!(
            "point '{id}' must have literal coordinates; solve the sketch first"
        ))),
    }
}
