//! Reviewable analytic sketch curve edits; no filesystem or kernel mutation.
use crate::{DesignPatch, DesignState, ExpectedEffect, PatchOperation, PatchPrecondition};
use opencad_core::{EntityId, OpenCadError, Result};
use opencad_sketch::curve_edit::{
    curve_edit_intersections, CurveEditEnd, CurveEditIntersection, CurveEditMode, EditCurve,
};
use opencad_sketch::{Coord, EntityBase, PointEntity, Sketch, SketchEntity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveEditRequest {
    pub sketch_id: String,
    pub target_id: String,
    pub cutter_id: String,
    pub mode: CurveEditMode,
    pub end: CurveEditEnd,
    pub new_point_id: String,
    #[serde(default)]
    pub intersection_index: Option<usize>,
    #[serde(default)]
    pub remove_constraints: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct CurveEditPlan {
    pub patch: DesignPatch,
    pub intersections: Vec<CurveEditIntersection>,
    pub selected_index: usize,
}

pub fn plan_curve_edit(state: &DesignState, request: &CurveEditRequest) -> Result<CurveEditPlan> {
    if request.target_id == request.cutter_id {
        return Err(OpenCadError::validation("target and cutter must differ"));
    }
    crate::patch::validate_stable_id(&request.new_point_id, "ent")?;
    let source = state
        .sketches
        .iter()
        .find(|s| s.id.as_str() == request.sketch_id)
        .ok_or_else(|| OpenCadError::not_found("sketch edit source"))?;
    if source
        .entities
        .iter()
        .flat_map(SketchEntity::defined_ids)
        .any(|id| id.as_str() == request.new_point_id)
    {
        return Err(OpenCadError::validation("new endpoint ID already exists"));
    }
    let target = source.find_entity(&request.target_id).ok_or_else(|| {
        OpenCadError::not_found("stored target curve; rectangle-derived edges require conversion")
    })?;
    let mut prepared = source.clone();
    opencad_feature::prepare_sketch(&mut prepared)?;
    let mut removals = std::collections::BTreeSet::new();
    for id in &request.remove_constraints {
        if !removals.insert(id) {
            return Err(OpenCadError::validation("duplicate constraint removal"));
        }
        let position = prepared
            .constraints
            .iter()
            .position(|c| c.id().as_str() == id)
            .ok_or_else(|| OpenCadError::not_found(format!("constraint '{id}'")))?;
        prepared.constraints.remove(position);
    }
    let values = opencad_graph::evaluate_param_graph(&state.parameters)?;
    // Geometry for the cut comes from the original valid solution. Removal is
    // checked separately; it must not change the selected original intersection.
    let solved = opencad_feature::param_apply::solved_sketch_copy(source, &values)?;
    let target_curve = curve(&solved, &request.target_id)?;
    let cutter_curve = curve(&solved, &request.cutter_id)?;
    let intersections =
        curve_edit_intersections(target_curve, cutter_curve, request.mode, request.end)?;
    let selected_index = match (request.intersection_index, intersections.len()) {
        (Some(i), _) if i < intersections.len() => i,
        (None, 1) => 0,
        _ => {
            return Err(OpenCadError::validation(format!(
                "choose intersection_index among {} eligible intersections",
                intersections.len()
            )))
        }
    };
    let selected = intersections[selected_index];
    let point_id = EntityId::new(&request.new_point_id)?;
    let mut edited = target.clone();
    match &mut edited {
        SketchEntity::Line(line) => match request.end {
            CurveEditEnd::Start => line.start = point_id.clone(),
            CurveEditEnd::End => line.end = point_id.clone(),
        },
        SketchEntity::Arc(arc) => {
            let EditCurve::Arc { start_rad, .. } = target_curve else {
                return Err(OpenCadError::validation("resolved target is not an arc"));
            };
            let angle = Coord::Literal(start_rad + selected.target_parameter);
            match request.end {
                CurveEditEnd::Start => {
                    arc.start_point = Some(point_id.clone());
                    arc.start_angle = angle;
                }
                CurveEditEnd::End => {
                    arc.end_point = Some(point_id.clone());
                    arc.end_angle = angle;
                }
            }
            arc.resolved_angles_rad = None;
        }
        _ => {
            return Err(OpenCadError::validation(
                "target must be a stored line or arc",
            ))
        }
    }
    let mut operations: Vec<_> = request
        .remove_constraints
        .iter()
        .map(|id| PatchOperation::RemoveSketchConstraint {
            sketch_id: request.sketch_id.clone(),
            constraint_id: id.clone(),
        })
        .collect();
    operations.push(PatchOperation::AddSketchEntity {
        sketch_id: request.sketch_id.clone(),
        entity: SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: point_id,
                construction: target.is_construction(),
            },
            x: Coord::Literal(selected.position_m[0]),
            y: Coord::Literal(selected.position_m[1]),
        }),
    });
    operations.push(PatchOperation::ReplaceSketchEntity {
        sketch_id: request.sketch_id.clone(),
        entity: edited,
    });
    let mut patch = DesignPatch::new(operations);
    patch.intent = Some(format!(
        "{:?} {:?} of {} at an explicit analytic intersection",
        request.mode, request.end, request.target_id
    ));
    patch
        .preconditions
        .push(PatchPrecondition::revision_equals(state)?);
    patch
        .expected_effects
        .push(ExpectedEffect::SketchPointPosition {
            sketch_id: request.sketch_id.clone(),
            point_id: request.new_point_id.clone(),
            position_m: selected.position_m,
            tolerance_m: 1e-9,
        });
    Ok(CurveEditPlan {
        patch,
        intersections,
        selected_index,
    })
}

fn point(sketch: &Sketch, id: &str) -> Result<[f64; 2]> {
    let Some(SketchEntity::Point(point)) = sketch.find_entity(id) else {
        return Err(OpenCadError::not_found(format!("point '{id}'")));
    };
    let (Coord::Literal(x), Coord::Literal(y)) = (&point.x, &point.y) else {
        return Err(OpenCadError::validation("curve coordinates unresolved"));
    };
    Ok([*x, *y])
}
fn literal(coord: &Coord) -> Result<f64> {
    match coord {
        Coord::Literal(v) => Ok(*v),
        _ => Err(OpenCadError::validation("curve radius unresolved")),
    }
}
pub(crate) fn curve(sketch: &Sketch, id: &str) -> Result<EditCurve> {
    match sketch
        .find_entity(id)
        .ok_or_else(|| OpenCadError::not_found(format!("curve '{id}'")))?
    {
        SketchEntity::Line(line) => Ok(EditCurve::Line {
            start_m: point(sketch, line.start.as_str())?,
            end_m: point(sketch, line.end.as_str())?,
        }),
        SketchEntity::Circle(circle) => Ok(EditCurve::Circle {
            center_m: point(sketch, circle.center.as_str())?,
            radius_m: literal(&circle.radius)?,
        }),
        SketchEntity::Arc(arc) => {
            let (start, end) = opencad_sketch::solve::arc_angles(arc)?;
            Ok(EditCurve::Arc {
                center_m: point(sketch, arc.center.as_str())?,
                radius_m: literal(&arc.radius)?,
                start_rad: start,
                sweep_rad: (end - start).rem_euclid(std::f64::consts::TAU),
            })
        }
        _ => Err(OpenCadError::validation(
            "cutter must be a line, circle or arc",
        )),
    }
}
