//! Read-only independent offset copies; writes use DesignPatch transactions.
use crate::{DesignPatch, DesignState, PatchOperation, PatchPrecondition};
use opencad_core::{EntityId, OpenCadError, Result};
use opencad_sketch::{
    curve_edit::EditCurve, offset::offset_loop, ArcEntity, CircleEntity, Coord, EntityBase,
    LineEntity, PointEntity, SketchEntity,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OffsetAssociation {
    IndependentCopy,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OffsetJoin {
    MitreTangent,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OffsetRequest {
    pub source_sketch_id: String,
    pub curve_ids: Vec<String>,
    pub distance_m: f64,
    pub association: OffsetAssociation,
    pub join: OffsetJoin,
    pub destination_sketch_id: String,
    pub name: String,
    pub destination_curve_ids: Vec<String>,
    pub destination_point_ids: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct OffsetPlan {
    pub patch: DesignPatch,
    pub curves: Vec<EditCurve>,
    pub association: OffsetAssociation,
}
fn base(id: &str) -> Result<EntityBase> {
    Ok(EntityBase {
        id: EntityId::new(id)?,
        construction: false,
    })
}
fn point(id: &str, p: [f64; 2]) -> Result<SketchEntity> {
    Ok(SketchEntity::Point(PointEntity {
        base: base(id)?,
        x: Coord::Literal(p[0]),
        y: Coord::Literal(p[1]),
    }))
}
pub fn plan_offset(state: &DesignState, request: &OffsetRequest) -> Result<OffsetPlan> {
    crate::patch::validate_stable_id(&request.destination_sketch_id, "sketch")?;
    if request.name.trim().is_empty()
        || state
            .sketches
            .iter()
            .any(|s| s.id.as_str() == request.destination_sketch_id)
    {
        return Err(OpenCadError::validation(
            "offset destination must be a new named sketch",
        ));
    }
    let source = state
        .sketches
        .iter()
        .find(|s| s.id.as_str() == request.source_sketch_id)
        .ok_or_else(|| OpenCadError::not_found("offset source sketch"))?;
    if request.curve_ids.is_empty() || request.curve_ids.len() > 256 {
        return Err(OpenCadError::validation(
            "offset requires 1..256 explicit curve IDs",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for id in &request.curve_ids {
        if !seen.insert(id) {
            return Err(OpenCadError::validation("duplicate offset source curve"));
        }
    }
    let values = opencad_graph::evaluate_param_graph_strict(&state.parameters)?;
    let solved = opencad_feature::param_apply::solved_sketch_copy(source, &values)?;
    let input: Vec<_> = request
        .curve_ids
        .iter()
        .map(|id| crate::curve_edit::curve(&solved, id))
        .collect::<Result<_>>()?;
    if !matches!(input.as_slice(), [EditCurve::Circle { .. }]) {
        let endpoints = |id: &str| -> Result<(&EntityId, &EntityId)> {
            match solved.find_entity(id) {
                Some(SketchEntity::Line(line)) => Ok((&line.start, &line.end)),
                Some(SketchEntity::Arc(arc)) => match (&arc.start_point, &arc.end_point) {
                    (Some(start), Some(end)) => Ok((start, end)),
                    _ => Err(OpenCadError::validation(
                        "offset arcs need explicit endpoint IDs",
                    )),
                },
                _ => Err(OpenCadError::validation(
                    "offset contour requires line/arc source IDs",
                )),
            }
        };
        for (i, id) in request.curve_ids.iter().enumerate() {
            let (_, end) = endpoints(id)?;
            let (start, _) = endpoints(&request.curve_ids[(i + 1) % request.curve_ids.len()])?;
            if end != start {
                return Err(OpenCadError::validation("offset source curves must share ordered endpoint IDs; coincident coordinates do not repair topology"));
            }
        }
    }
    let curves = offset_loop(&input, request.distance_m)?;
    let circle = matches!(curves.as_slice(), [EditCurve::Circle { .. }]);
    let points = if circle {
        1
    } else {
        curves.len()
            + curves
                .iter()
                .filter(|c| matches!(c, EditCurve::Arc { .. }))
                .count()
    };
    if request.destination_curve_ids.len() != curves.len()
        || request.destination_point_ids.len() != points
    {
        return Err(OpenCadError::validation(format!(
            "offset requires {} curve IDs and {points} point IDs",
            curves.len()
        )));
    }
    let existing: std::collections::BTreeSet<_> = state
        .sketches
        .iter()
        .flat_map(|s| s.entities.iter())
        .flat_map(SketchEntity::defined_ids)
        .map(|id| id.as_str().to_owned())
        .collect();
    let mut fresh = std::collections::BTreeSet::new();
    for id in request
        .destination_curve_ids
        .iter()
        .chain(&request.destination_point_ids)
    {
        crate::patch::validate_stable_id(id, "ent")?;
        if existing.contains(id) || !fresh.insert(id) {
            return Err(OpenCadError::validation(
                "offset destination entity IDs must be fresh and unique",
            ));
        }
    }
    let mut entities = Vec::new();
    if let [EditCurve::Circle { center_m, radius_m }] = curves.as_slice() {
        entities.push(point(&request.destination_point_ids[0], *center_m)?);
        entities.push(SketchEntity::Circle(CircleEntity {
            base: base(&request.destination_curve_ids[0])?,
            center: EntityId::new(&request.destination_point_ids[0])?,
            radius: Coord::Literal(*radius_m),
        }));
    } else {
        let mut centre_index = curves.len();
        for (i, c) in curves.iter().enumerate() {
            let start = match *c {
                EditCurve::Line { start_m, .. } => start_m,
                EditCurve::Arc {
                    center_m,
                    radius_m,
                    start_rad,
                    ..
                } => [
                    center_m[0] + radius_m * start_rad.cos(),
                    center_m[1] + radius_m * start_rad.sin(),
                ],
                _ => return Err(OpenCadError::validation("mixed circle offset")),
            };
            entities.push(point(&request.destination_point_ids[i], start)?);
            let start_id = EntityId::new(&request.destination_point_ids[i])?;
            let end_id = EntityId::new(&request.destination_point_ids[(i + 1) % curves.len()])?;
            let entity = match *c {
                EditCurve::Line { .. } => SketchEntity::Line(LineEntity {
                    base: base(&request.destination_curve_ids[i])?,
                    start: start_id,
                    end: end_id,
                }),
                EditCurve::Arc {
                    center_m,
                    radius_m,
                    start_rad,
                    sweep_rad,
                } => {
                    let centre = &request.destination_point_ids[centre_index];
                    centre_index += 1;
                    entities.push(point(centre, center_m)?);
                    SketchEntity::Arc(ArcEntity {
                        base: base(&request.destination_curve_ids[i])?,
                        center: EntityId::new(centre)?,
                        radius: Coord::Literal(radius_m),
                        start_angle: Coord::Literal(start_rad),
                        end_angle: Coord::Literal(start_rad + sweep_rad),
                        start_point: Some(start_id),
                        end_point: Some(end_id),
                        resolved_angles_rad: None,
                    })
                }
                _ => return Err(OpenCadError::validation("mixed circle offset")),
            };
            entities.push(entity);
        }
    }
    let mut ops = vec![PatchOperation::AddSketch {
        id: request.destination_sketch_id.clone(),
        name: request.name.clone(),
        workplane: source.workplane.clone(),
    }];
    ops.extend(
        entities
            .into_iter()
            .map(|entity| PatchOperation::AddSketchEntity {
                sketch_id: request.destination_sketch_id.clone(),
                entity,
            }),
    );
    let mut patch = DesignPatch::new(ops);
    patch.intent = Some(format!(
        "Create independent analytic offset copy at {} m",
        request.distance_m
    ));
    patch
        .preconditions
        .push(PatchPrecondition::revision_equals(state)?);
    Ok(OffsetPlan {
        patch,
        curves,
        association: request.association,
    })
}
