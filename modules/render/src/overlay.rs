//! Sketch entity overlays projected into 3D viewport space.

use indexmap::IndexMap;
use opencad_core::{EntityId, Expression, OpenCadError, Result};
use opencad_graph::{eval_angle_expr, eval_length_expr};
use opencad_sketch::{
    constraint::{Constraint, DistanceTarget, EntityRef, EqualTarget, LineEnd, RectangleEdge},
    entity::{Coord, LineEntity, SketchEntity},
    workplane::{GlobalPlane, Workplane},
    Sketch,
};

use crate::stroke_font::append_text_lines;

const CIRCLE_SEGMENTS: usize = 32;
const SYMBOL_SIZE_M: f64 = 0.003;
const SYMBOL_OFFSET_M: f64 = 0.0015;

/// A line segment drawn over the solid body.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayLine {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub construction: bool,
    pub sketch_id: Option<String>,
    pub entity_id: Option<String>,
    pub segment_index: Option<usize>,
}

/// Resolved sketch entity behind a pickable overlay line index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickableSketchLine {
    pub sketch_id: String,
    pub entity_id: String,
    pub entity_kind: &'static str,
    pub segment_index: Option<usize>,
    pub construction: bool,
}

/// Text label anchored on the sketch plane.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayLabel {
    pub position: [f32; 3],
    pub text: String,
    pub right: [f32; 3],
    pub up: [f32; 3],
}

/// Sketch edges projected to world-space line segments.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SketchOverlay {
    pub lines: Vec<OverlayLine>,
    pub dimension_lines: Vec<OverlayLine>,
    pub symbol_lines: Vec<OverlayLine>,
    pub labels: Vec<OverlayLabel>,
}

impl SketchOverlay {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.labels.is_empty()
    }

    pub fn model_line_vertices(&self) -> Vec<[f32; 3]> {
        line_vertices_for(self, false)
    }

    pub fn construction_line_vertices(&self) -> Vec<[f32; 3]> {
        line_vertices_for(self, true)
    }

    pub fn line_vertices(&self) -> Vec<[f32; 3]> {
        let mut vertices = self.model_line_vertices();
        vertices.extend(self.construction_line_vertices());
        vertices
    }

    pub fn label_line_vertices(&self, scale: f32) -> Vec<[f32; 3]> {
        let mut vertices = self.dimension_line_vertices();
        vertices.extend(self.symbol_line_vertices());
        for label in &self.labels {
            append_text_lines(
                &label.text,
                label.position,
                label.right,
                label.up,
                scale,
                &mut vertices,
            );
        }
        vertices
    }

    /// Build label strokes aligned to a camera-facing billboard basis.
    pub fn label_line_vertices_billboard(
        &self,
        scale: f32,
        text_right: [f32; 3],
        text_up: [f32; 3],
        depth_bias: Option<([f32; 3], f32)>,
    ) -> Vec<[f32; 3]> {
        let mut vertices = self.dimension_line_vertices();
        vertices.extend(self.symbol_line_vertices());
        for label in &self.labels {
            append_text_lines(
                &label.text,
                label.position,
                text_right,
                text_up,
                scale,
                &mut vertices,
            );
        }
        if let Some((eye, offset_m)) = depth_bias {
            bias_vertices_toward_camera(&mut vertices, eye, offset_m);
        }
        vertices
    }

    fn symbol_line_vertices(&self) -> Vec<[f32; 3]> {
        let mut vertices = Vec::new();
        for line in &self.symbol_lines {
            vertices.push(line.start);
            vertices.push(line.end);
        }
        vertices
    }

    fn dimension_line_vertices(&self) -> Vec<[f32; 3]> {
        let mut vertices = Vec::new();
        for line in &self.dimension_lines {
            vertices.push(line.start);
            vertices.push(line.end);
        }
        vertices
    }

    pub fn highlight_line_vertices(&self, line_index: usize) -> Vec<[f32; 3]> {
        self.lines
            .get(line_index)
            .map(|line| vec![line.start, line.end])
            .unwrap_or_default()
    }

    /// Map a GPU pick line index to the source sketch entity, when known.
    pub fn pickable_line_at(&self, line_index: usize) -> Option<PickableSketchLine> {
        let line = self.lines.get(line_index)?;
        let sketch_id = line.sketch_id.clone()?;
        let entity_id = line.entity_id.clone()?;
        let entity_kind = if line.segment_index.is_some() {
            "circle"
        } else {
            "line"
        };
        Some(PickableSketchLine {
            sketch_id,
            entity_id,
            entity_kind,
            segment_index: line.segment_index,
            construction: line.construction,
        })
    }
}

pub fn label_scale_for_bounds(diagonal: f32) -> f32 {
    (diagonal * 0.02).max(0.0008)
}

/// Small world-space pull toward the camera to keep labels above coplanar solids.
pub fn label_depth_offset_for_bounds(diagonal: f32) -> f32 {
    (diagonal * 0.002).max(0.0001)
}

/// Nudge overlay vertices slightly toward `eye` to reduce depth fighting.
pub fn bias_vertices_toward_camera(vertices: &mut [[f32; 3]], eye: [f32; 3], offset_m: f32) {
    if offset_m <= 0.0 {
        return;
    }
    for vertex in vertices {
        let dx = eye[0] - vertex[0];
        let dy = eye[1] - vertex[1];
        let dz = eye[2] - vertex[2];
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len <= f32::EPSILON {
            continue;
        }
        let scale = offset_m / len;
        vertex[0] += dx * scale;
        vertex[1] += dy * scale;
        vertex[2] += dz * scale;
    }
}

fn line_vertices_for(overlay: &SketchOverlay, construction: bool) -> Vec<[f32; 3]> {
    let mut vertices = Vec::new();
    for line in overlay
        .lines
        .iter()
        .filter(|line| line.construction == construction)
    {
        vertices.push(line.start);
        vertices.push(line.end);
    }
    vertices
}

fn overlay_line(start: [f32; 3], end: [f32; 3], construction: bool) -> OverlayLine {
    OverlayLine {
        start,
        end,
        construction,
        sketch_id: None,
        entity_id: None,
        segment_index: None,
    }
}

fn pickable_overlay_line(
    start: [f32; 3],
    end: [f32; 3],
    construction: bool,
    sketch_id: &str,
    entity_id: &str,
    segment_index: Option<usize>,
) -> OverlayLine {
    OverlayLine {
        start,
        end,
        construction,
        sketch_id: Some(sketch_id.to_string()),
        entity_id: Some(entity_id.to_string()),
        segment_index,
    }
}

/// Build overlay lines and constraint labels from solved sketches.
pub fn build_sketch_overlay(
    sketches: &[Sketch],
    values: &IndexMap<String, f64>,
) -> Result<SketchOverlay> {
    let mut overlay = SketchOverlay::default();
    for sketch in sketches {
        append_sketch_lines(sketch, values, &mut overlay.lines)?;
        append_constraint_labels(sketch, values, &mut overlay)?;
    }
    Ok(overlay)
}

fn append_sketch_lines(
    sketch: &Sketch,
    values: &IndexMap<String, f64>,
    lines: &mut Vec<OverlayLine>,
) -> Result<()> {
    let points = point_map(sketch, values)?;
    let sketch_id = sketch.id.as_str();
    for entity in &sketch.entities {
        match entity {
            SketchEntity::Line(line) => {
                let start = points.get(line.start.as_str()).ok_or_else(|| {
                    OpenCadError::not_found(format!("point '{}'", line.start.as_str()))
                })?;
                let end = points.get(line.end.as_str()).ok_or_else(|| {
                    OpenCadError::not_found(format!("point '{}'", line.end.as_str()))
                })?;
                lines.push(pickable_overlay_line(
                    plane_to_world(&sketch.workplane, start),
                    plane_to_world(&sketch.workplane, end),
                    line.base.construction,
                    sketch_id,
                    line.base.id.as_str(),
                    None,
                ));
            }
            SketchEntity::Circle(circle) => {
                let center = points.get(circle.center.as_str()).ok_or_else(|| {
                    OpenCadError::not_found(format!("point '{}'", circle.center.as_str()))
                })?;
                let radius = eval_coord(&circle.radius, values)?;
                append_circle_lines(
                    &sketch.workplane,
                    center,
                    radius,
                    circle.base.construction,
                    sketch_id,
                    circle.base.id.as_str(),
                    lines,
                );
            }
            SketchEntity::Point(_) | SketchEntity::Arc(_) | SketchEntity::Rectangle(_) => {}
        }
    }
    Ok(())
}

fn append_constraint_labels(
    sketch: &Sketch,
    values: &IndexMap<String, f64>,
    overlay: &mut SketchOverlay,
) -> Result<()> {
    if sketch.constraints.is_empty() {
        return Ok(());
    }

    let points = point_map(sketch, values)?;
    let centroid = sketch_centroid(&points);
    let (right, up) = workplane_basis(&sketch.workplane);

    for constraint in &sketch.constraints {
        match constraint {
            Constraint::Distance { target, expr, .. } => {
                let text = format_dimension_expr(expr, values)?;
                let (anchor, outward) = distance_anchor(sketch, &points, target, centroid)?;
                let offset = label_offset_distance(sketch, &points, target);
                let position = offset_point(&sketch.workplane, anchor, outward, offset);
                overlay.labels.push(OverlayLabel {
                    position,
                    text,
                    right,
                    up,
                });
                if let Some(dimension_line) = dimension_witness_line(
                    &sketch.workplane,
                    sketch,
                    &points,
                    target,
                    outward,
                    offset,
                ) {
                    overlay.dimension_lines.push(dimension_line);
                }
            }
            Constraint::Horizontal { line, .. } => {
                let Some((anchor, outward)) = line_anchor(sketch, &points, line, centroid) else {
                    continue;
                };
                overlay.labels.push(OverlayLabel {
                    position: offset_point(&sketch.workplane, anchor, outward, 0.002),
                    text: "H".to_string(),
                    right,
                    up,
                });
            }
            Constraint::Vertical { line, .. } => {
                let Some((anchor, outward)) = line_anchor(sketch, &points, line, centroid) else {
                    continue;
                };
                overlay.labels.push(OverlayLabel {
                    position: offset_point(&sketch.workplane, anchor, outward, 0.002),
                    text: "V".to_string(),
                    right,
                    up,
                });
            }
            Constraint::Radius { target, expr, .. } => {
                let text = format!("R{}", format_dimension_expr(expr, values)?);
                if let Some(position) = circle_label_position(sketch, &points, target, values)? {
                    overlay.labels.push(OverlayLabel {
                        position,
                        text,
                        right,
                        up,
                    });
                }
            }
            Constraint::Diameter { target, expr, .. } => {
                let text = format!("D{}", format_dimension_expr(expr, values)?);
                if let Some(position) = circle_label_position(sketch, &points, target, values)? {
                    overlay.labels.push(OverlayLabel {
                        position,
                        text,
                        right,
                        up,
                    });
                }
            }
            Constraint::Coincident { a, b, .. } => {
                let Some(point) = coincident_anchor(sketch, &points, a, b) else {
                    continue;
                };
                append_coincident_symbol(&sketch.workplane, point, &mut overlay.symbol_lines);
            }
            Constraint::Parallel { line_a, line_b, .. } => {
                let Some(anchor) = pair_line_anchor(sketch, &points, line_a, line_b, centroid)
                else {
                    continue;
                };
                overlay.labels.push(OverlayLabel {
                    position: plane_to_world(&sketch.workplane, &anchor),
                    text: "//".to_string(),
                    right,
                    up,
                });
            }
            Constraint::Perpendicular { line_a, line_b, .. } => {
                let (Some(anchor), Some(dir)) =
                    perpendicular_symbol_anchor(sketch, &points, line_a, line_b, centroid)
                else {
                    continue;
                };
                append_perpendicular_symbol(
                    &sketch.workplane,
                    anchor,
                    dir,
                    &mut overlay.symbol_lines,
                );
            }
            Constraint::Equal { a, b, .. } => {
                let Some(anchor) = equal_anchor(sketch, &points, values, a, b, centroid) else {
                    continue;
                };
                overlay.labels.push(OverlayLabel {
                    position: offset_point(&sketch.workplane, anchor, [0.0, 0.0], SYMBOL_OFFSET_M),
                    text: "=".to_string(),
                    right,
                    up,
                });
            }
            Constraint::Angle {
                line_a,
                line_b,
                expr,
                ..
            } => {
                let Some(anchor) = pair_line_anchor(sketch, &points, line_a, line_b, centroid)
                else {
                    continue;
                };
                let degrees = eval_angle_expr(expr.as_str(), values)?.to_degrees();
                overlay.labels.push(OverlayLabel {
                    position: plane_to_world(&sketch.workplane, &anchor),
                    text: format!("<{}", format_angle_degrees(degrees)),
                    right,
                    up,
                });
            }
            Constraint::Midpoint { line, .. }
            | Constraint::Symmetric { line, .. }
            | Constraint::Tangent { line, .. } => {
                let Some((anchor, outward)) = line_anchor(sketch, &points, line, centroid) else {
                    continue;
                };
                let text = match constraint {
                    Constraint::Midpoint { .. } => "M",
                    Constraint::Symmetric { .. } => "S",
                    _ => "T",
                };
                overlay.labels.push(OverlayLabel {
                    position: offset_point(&sketch.workplane, anchor, outward, 0.002),
                    text: text.to_string(),
                    right,
                    up,
                });
            }
        }
    }

    Ok(())
}

/// Angle label text in degrees, without trailing zeros (`30`, `22.5`).
fn format_angle_degrees(degrees: f64) -> String {
    let text = format!("{degrees:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn format_dimension_expr(expr: &Expression, values: &IndexMap<String, f64>) -> Result<String> {
    let meters = eval_length_expr(expr.as_str(), values)?;
    Ok(format_length_label(meters))
}

fn format_length_label(meters: f64) -> String {
    let mm = meters * 1000.0;
    if mm.abs() >= 100.0 {
        format!("{:.0} mm", mm)
    } else if mm.abs() >= 10.0 {
        format!("{:.1} mm", mm)
    } else {
        format!("{:.2} mm", mm)
    }
}

fn distance_anchor(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    target: &DistanceTarget,
    centroid: [f64; 2],
) -> Result<([f64; 2], [f64; 2])> {
    match target {
        DistanceTarget::LineLength { line } => line_anchor(sketch, points, line, centroid)
            .ok_or_else(|| OpenCadError::not_found(format!("line '{}'", line.as_str()))),
        DistanceTarget::PointToPoint { a, b } => {
            let start = *points
                .get(a.as_str())
                .ok_or_else(|| OpenCadError::not_found(format!("point '{}'", a.as_str())))?;
            let end = *points
                .get(b.as_str())
                .ok_or_else(|| OpenCadError::not_found(format!("point '{}'", b.as_str())))?;
            Ok((
                midpoint(start, end),
                outward_from_segment(start, end, centroid),
            ))
        }
        DistanceTarget::RectangleDimension { rectangle, edge } => {
            rectangle_edge_anchor(sketch, points, rectangle, *edge, centroid)
        }
    }
}

fn label_offset_distance(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    target: &DistanceTarget,
) -> f64 {
    let length = match target {
        DistanceTarget::LineLength { line } => line_length(sketch, points, line),
        DistanceTarget::PointToPoint { a, b } => {
            let start = points.get(a.as_str()).copied().unwrap_or([0.0, 0.0]);
            let end = points.get(b.as_str()).copied().unwrap_or([0.0, 0.0]);
            segment_length(start, end)
        }
        DistanceTarget::RectangleDimension { rectangle, edge } => {
            rectangle_edge_length(sketch, points, rectangle, *edge)
        }
    };
    length * 0.12 + 0.002
}

fn dimension_witness_line(
    workplane: &Workplane,
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    target: &DistanceTarget,
    outward: [f64; 2],
    offset: f64,
) -> Option<OverlayLine> {
    let (start, end) = segment_endpoints(sketch, points, target)?;
    let witness_start = offset_point(workplane, start, outward, offset * 0.75);
    let witness_end = offset_point(workplane, end, outward, offset * 0.75);
    Some(overlay_line(witness_start, witness_end, true))
}

fn segment_endpoints(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    target: &DistanceTarget,
) -> Option<([f64; 2], [f64; 2])> {
    match target {
        DistanceTarget::LineLength { line } => line_endpoints(sketch, points, line),
        DistanceTarget::PointToPoint { a, b } => {
            let start = *points.get(a.as_str())?;
            let end = *points.get(b.as_str())?;
            Some((start, end))
        }
        DistanceTarget::RectangleDimension { rectangle, edge } => {
            rectangle_edge_endpoints(sketch, points, rectangle, *edge)
        }
    }
}

fn line_anchor(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    line_id: &EntityId,
    centroid: [f64; 2],
) -> Option<([f64; 2], [f64; 2])> {
    let (start, end) = line_endpoints(sketch, points, line_id)?;
    let anchor = midpoint(start, end);
    let outward = outward_from_segment(start, end, centroid);
    Some((anchor, outward))
}

fn circle_label_position(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    circle_id: &EntityId,
    values: &IndexMap<String, f64>,
) -> Result<Option<[f32; 3]>> {
    let Some(SketchEntity::Circle(circle)) = sketch
        .entities
        .iter()
        .find(|entity| entity.id().as_str() == circle_id.as_str())
    else {
        return Ok(None);
    };
    let center = points
        .get(circle.center.as_str())
        .ok_or_else(|| OpenCadError::not_found(format!("point '{}'", circle.center.as_str())))?;
    let radius = eval_coord(&circle.radius, values)?;
    let anchor = [center[0] + radius * 0.35, center[1] + radius * 0.35];
    Ok(Some(plane_to_world(&sketch.workplane, &anchor)))
}

fn rectangle_edge_anchor(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    rectangle_id: &EntityId,
    edge: RectangleEdge,
    centroid: [f64; 2],
) -> Result<([f64; 2], [f64; 2])> {
    let (start, end) = rectangle_edge_endpoints(sketch, points, rectangle_id, edge)
        .ok_or_else(|| OpenCadError::not_found(format!("rectangle '{}'", rectangle_id.as_str())))?;
    Ok((
        midpoint(start, end),
        outward_from_segment(start, end, centroid),
    ))
}

fn rectangle_edge_endpoints(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    rectangle_id: &EntityId,
    edge: RectangleEdge,
) -> Option<([f64; 2], [f64; 2])> {
    let rectangle = sketch.entities.iter().find_map(|entity| {
        let SketchEntity::Rectangle(rect) = entity else {
            return None;
        };
        (rect.base.id.as_str() == rectangle_id.as_str()).then_some(rect)
    })?;
    let edge_index = match edge {
        RectangleEdge::Width => 0,
        RectangleEdge::Height => 1,
    };
    let line_id = rectangle.edge_ids.get(edge_index)?;
    line_endpoints(sketch, points, line_id)
}

fn rectangle_edge_length(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    rectangle_id: &EntityId,
    edge: RectangleEdge,
) -> f64 {
    rectangle_edge_endpoints(sketch, points, rectangle_id, edge)
        .map(|(start, end)| segment_length(start, end))
        .unwrap_or(0.01)
}

fn line_length(sketch: &Sketch, points: &IndexMap<String, [f64; 2]>, line_id: &EntityId) -> f64 {
    line_endpoints(sketch, points, line_id)
        .map(|(start, end)| segment_length(start, end))
        .unwrap_or(0.01)
}

fn line_endpoints(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    line_id: &EntityId,
) -> Option<([f64; 2], [f64; 2])> {
    let line = find_line(sketch, line_id)?;
    let start = *points.get(line.start.as_str())?;
    let end = *points.get(line.end.as_str())?;
    Some((start, end))
}

fn find_line<'a>(sketch: &'a Sketch, line_id: &EntityId) -> Option<&'a LineEntity> {
    sketch.entities.iter().find_map(|entity| {
        let SketchEntity::Line(line) = entity else {
            return None;
        };
        (line.base.id.as_str() == line_id.as_str()).then_some(line)
    })
}

fn pair_line_anchor(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    line_a: &EntityId,
    line_b: &EntityId,
    centroid: [f64; 2],
) -> Option<[f64; 2]> {
    let (anchor_a, _) = line_anchor(sketch, points, line_a, centroid)?;
    let (anchor_b, _) = line_anchor(sketch, points, line_b, centroid)?;
    Some(midpoint(anchor_a, anchor_b))
}

fn perpendicular_symbol_anchor(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    line_a: &EntityId,
    line_b: &EntityId,
    centroid: [f64; 2],
) -> (Option<[f64; 2]>, Option<[f64; 2]>) {
    let Some((start_a, end_a)) = line_endpoints(sketch, points, line_a) else {
        return (None, None);
    };
    let anchor = pair_line_anchor(sketch, points, line_a, line_b, centroid);
    let dir = normalize_2d([end_a[0] - start_a[0], end_a[1] - start_a[1]]);
    (anchor, Some(dir))
}

fn coincident_anchor(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    a: &EntityRef,
    b: &EntityRef,
) -> Option<[f64; 2]> {
    let point_a = entity_ref_point(sketch, points, a)?;
    let point_b = entity_ref_point(sketch, points, b)?;
    Some(midpoint(point_a, point_b))
}

fn equal_anchor(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    values: &IndexMap<String, f64>,
    a: &EqualTarget,
    b: &EqualTarget,
    centroid: [f64; 2],
) -> Option<[f64; 2]> {
    let anchor_a = equal_target_anchor(sketch, points, values, a, centroid)?;
    let anchor_b = equal_target_anchor(sketch, points, values, b, centroid)?;
    Some(midpoint(anchor_a, anchor_b))
}

fn equal_target_anchor(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    values: &IndexMap<String, f64>,
    target: &EqualTarget,
    centroid: [f64; 2],
) -> Option<[f64; 2]> {
    match target {
        EqualTarget::LineLength(line) => {
            let (anchor, _) = line_anchor(sketch, points, line, centroid)?;
            Some(anchor)
        }
        EqualTarget::Radius(circle) => {
            let center = circle_center_point(sketch, points, circle)?;
            let radius = circle_radius(sketch, circle, values).ok()?;
            Some([center[0] + radius * 0.25, center[1] + radius * 0.25])
        }
    }
}

fn entity_ref_point(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    entity_ref: &EntityRef,
) -> Option<[f64; 2]> {
    match entity_ref {
        EntityRef::Entity(entity_id) => entity_point(sketch, points, entity_id),
        EntityRef::PointOnLine { line, end } => {
            let line = find_line(sketch, line)?;
            let point_id = match end {
                LineEnd::Start => line.start.as_str(),
                LineEnd::End => line.end.as_str(),
            };
            points.get(point_id).copied()
        }
    }
}

fn entity_point(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    entity_id: &EntityId,
) -> Option<[f64; 2]> {
    if let Some(point) = points.get(entity_id.as_str()).copied() {
        return Some(point);
    }
    let (start, end) = line_endpoints(sketch, points, entity_id)?;
    Some(midpoint(start, end))
}

fn circle_center_point(
    sketch: &Sketch,
    points: &IndexMap<String, [f64; 2]>,
    circle_id: &EntityId,
) -> Option<[f64; 2]> {
    let circle = sketch.entities.iter().find_map(|entity| {
        let SketchEntity::Circle(circle) = entity else {
            return None;
        };
        (circle.base.id.as_str() == circle_id.as_str()).then_some(circle)
    })?;
    points.get(circle.center.as_str()).copied()
}

fn circle_radius(
    sketch: &Sketch,
    circle_id: &EntityId,
    values: &IndexMap<String, f64>,
) -> Result<f64> {
    let circle = sketch
        .entities
        .iter()
        .find_map(|entity| {
            let SketchEntity::Circle(circle) = entity else {
                return None;
            };
            (circle.base.id.as_str() == circle_id.as_str()).then_some(circle)
        })
        .ok_or_else(|| OpenCadError::not_found(format!("circle '{}'", circle_id.as_str())))?;
    eval_coord(&circle.radius, values)
}

fn append_coincident_symbol(workplane: &Workplane, center: [f64; 2], lines: &mut Vec<OverlayLine>) {
    let half = SYMBOL_SIZE_M * 0.5;
    let segments = [([-half, 0.0], [half, 0.0]), ([0.0, -half], [0.0, half])];
    for (start, end) in segments {
        lines.push(overlay_line(
            plane_to_world(workplane, &[center[0] + start[0], center[1] + start[1]]),
            plane_to_world(workplane, &[center[0] + end[0], center[1] + end[1]]),
            true,
        ));
    }
}

fn append_perpendicular_symbol(
    workplane: &Workplane,
    anchor: [f64; 2],
    line_dir: [f64; 2],
    lines: &mut Vec<OverlayLine>,
) {
    let perp = [-line_dir[1], line_dir[0]];
    let size = SYMBOL_SIZE_M;
    let corner = [
        anchor[0] + line_dir[0] * size,
        anchor[1] + line_dir[1] * size,
    ];
    let end = [corner[0] + perp[0] * size, corner[1] + perp[1] * size];
    lines.push(overlay_line(
        plane_to_world(workplane, &anchor),
        plane_to_world(workplane, &corner),
        true,
    ));
    lines.push(overlay_line(
        plane_to_world(workplane, &corner),
        plane_to_world(workplane, &end),
        true,
    ));
}

fn sketch_centroid(points: &IndexMap<String, [f64; 2]>) -> [f64; 2] {
    if points.is_empty() {
        return [0.0, 0.0];
    }
    let mut sum = [0.0, 0.0];
    for point in points.values() {
        sum[0] += point[0];
        sum[1] += point[1];
    }
    let count = points.len() as f64;
    [sum[0] / count, sum[1] / count]
}

fn midpoint(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn segment_length(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}

fn outward_from_segment(start: [f64; 2], end: [f64; 2], centroid: [f64; 2]) -> [f64; 2] {
    let mid = midpoint(start, end);
    let dir = normalize_2d([end[0] - start[0], end[1] - start[1]]);
    let perp_a = [-dir[1], dir[0]];
    let perp_b = [dir[1], -dir[0]];
    let to_mid = [mid[0] - centroid[0], mid[1] - centroid[1]];
    if dot_2d(perp_a, to_mid) >= dot_2d(perp_b, to_mid) {
        perp_a
    } else {
        perp_b
    }
}

fn normalize_2d(v: [f64; 2]) -> [f64; 2] {
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if len <= f64::EPSILON {
        return [0.0, 1.0];
    }
    [v[0] / len, v[1] / len]
}

fn dot_2d(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn offset_point(
    workplane: &Workplane,
    anchor: [f64; 2],
    outward: [f64; 2],
    distance: f64,
) -> [f32; 3] {
    let point = [
        anchor[0] + outward[0] * distance,
        anchor[1] + outward[1] * distance,
    ];
    plane_to_world(workplane, &point)
}

fn workplane_basis(workplane: &Workplane) -> ([f32; 3], [f32; 3]) {
    match workplane {
        Workplane::Global { plane } => match plane {
            GlobalPlane::XY => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            GlobalPlane::YZ => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            GlobalPlane::XZ => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        },
        Workplane::FaceRef { .. } => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        Workplane::Custom { x_axis, .. } => {
            let right = normalize3_f64(x_axis);
            let up = normalize_cross(x_axis, &[0.0, 1.0, 0.0]);
            (to_f32(right), to_f32(up))
        }
    }
}

fn normalize3_f64(v: &[f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= f64::EPSILON {
        return [1.0, 0.0, 0.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn to_f32(v: [f64; 3]) -> [f32; 3] {
    [v[0] as f32, v[1] as f32, v[2] as f32]
}

fn point_map(
    sketch: &Sketch,
    values: &IndexMap<String, f64>,
) -> Result<IndexMap<String, [f64; 2]>> {
    let mut points = IndexMap::new();
    for entity in &sketch.entities {
        let SketchEntity::Point(point) = entity else {
            continue;
        };
        let x = eval_coord(&point.x, values)?;
        let y = eval_coord(&point.y, values)?;
        points.insert(point.base.id.as_str().to_string(), [x, y]);
    }
    Ok(points)
}

fn eval_coord(coord: &Coord, values: &IndexMap<String, f64>) -> Result<f64> {
    match coord {
        Coord::Literal(value) => Ok(*value),
        Coord::Expr(expr) => eval_length_expr(expr.as_str(), values),
    }
}

fn append_circle_lines(
    workplane: &Workplane,
    center: &[f64; 2],
    radius: f64,
    construction: bool,
    sketch_id: &str,
    entity_id: &str,
    lines: &mut Vec<OverlayLine>,
) {
    let mut previous = None;
    for segment in 0..=CIRCLE_SEGMENTS {
        let angle = (segment as f64 / CIRCLE_SEGMENTS as f64) * std::f64::consts::TAU;
        let point = [
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ];
        if let Some(prev) = previous {
            lines.push(pickable_overlay_line(
                plane_to_world(workplane, &prev),
                plane_to_world(workplane, &point),
                construction,
                sketch_id,
                entity_id,
                Some(segment - 1),
            ));
        }
        previous = Some(point);
    }
}

fn plane_to_world(workplane: &Workplane, point: &[f64; 2]) -> [f32; 3] {
    let [u, v] = *point;
    let world = match workplane {
        Workplane::Global { plane } => match plane {
            GlobalPlane::XY => [u, v, 0.0],
            GlobalPlane::YZ => [0.0, u, v],
            GlobalPlane::XZ => [u, 0.0, v],
        },
        Workplane::FaceRef { .. } => [u, v, 0.0],
        Workplane::Custom {
            origin,
            normal: _,
            x_axis,
        } => {
            let y_axis = normalize_cross(x_axis, &[0.0, 1.0, 0.0]);
            [
                origin[0] + x_axis[0] * u + y_axis[0] * v,
                origin[1] + x_axis[1] * u + y_axis[1] * v,
                origin[2] + x_axis[2] * u + y_axis[2] * v,
            ]
        }
    };
    [world[0] as f32, world[1] as f32, world[2] as f32]
}

fn normalize_cross(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    if len <= f64::EPSILON {
        return [0.0, 1.0, 0.0];
    }
    [cross[0] / len, cross[1] / len, cross[2] / len]
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_feature::{apply_parameters, bracket_with_hole};
    use opencad_graph::bracket_parameters;

    #[test]
    fn bracket_overlay_contains_sketch_lines() {
        let mut model = bracket_with_hole().expect("model");
        let params = bracket_parameters();
        apply_parameters(&mut model, &params).expect("apply");
        let values = opencad_graph::evaluate_param_graph(&params).expect("eval");
        let sketches: Vec<Sketch> = model.sketches.values().cloned().collect();
        let overlay = build_sketch_overlay(&sketches, &values).expect("overlay");
        assert!(!overlay.lines.is_empty());
        assert!(!overlay.model_line_vertices().is_empty());
        assert!(overlay.lines.iter().all(|line| line.sketch_id.is_some()));
        assert!(overlay.lines.iter().all(|line| line.entity_id.is_some()));
        assert!(overlay.lines.iter().any(|line| {
            line.sketch_id.as_deref() == Some("sketch:base")
                && line.entity_id.as_deref() == Some("ent:e0")
        }));
        let line_index = overlay
            .lines
            .iter()
            .position(|line| line.entity_id.as_deref() == Some("ent:e0"))
            .expect("ent:e0");
        let mapped = overlay.pickable_line_at(line_index).expect("mapped");
        assert_eq!(mapped.sketch_id, "sketch:base");
        assert_eq!(mapped.entity_id, "ent:e0");
        assert_eq!(mapped.entity_kind, "line");
    }

    #[test]
    fn hole_circle_overlay_segments_share_entity_id() {
        let mut model = bracket_with_hole().expect("model");
        let params = bracket_parameters();
        apply_parameters(&mut model, &params).expect("apply");
        let values = opencad_graph::evaluate_param_graph(&params).expect("eval");
        let sketches: Vec<Sketch> = model.sketches.values().cloned().collect();
        let overlay = build_sketch_overlay(&sketches, &values).expect("overlay");
        let circle_segments: Vec<_> = overlay
            .lines
            .iter()
            .filter(|line| line.entity_id.as_deref() == Some("ent:hole_circle"))
            .collect();
        assert_eq!(circle_segments.len(), 32);
        assert!(circle_segments
            .iter()
            .all(|line| line.sketch_id.as_deref() == Some("sketch:hole")));
        assert!(circle_segments[0].segment_index == Some(0));
        let mapped = overlay
            .pickable_line_at(
                overlay
                    .lines
                    .iter()
                    .position(|line| line.entity_id.as_deref() == Some("ent:hole_circle"))
                    .expect("circle segment"),
            )
            .expect("mapped circle");
        assert_eq!(mapped.entity_kind, "circle");
        assert_eq!(mapped.segment_index, Some(0));
    }

    #[test]
    fn bracket_overlay_contains_constraint_labels() {
        let mut model = bracket_with_hole().expect("model");
        let params = bracket_parameters();
        apply_parameters(&mut model, &params).expect("apply");
        let values = opencad_graph::evaluate_param_graph(&params).expect("eval");
        let sketches: Vec<Sketch> = model.sketches.values().cloned().collect();
        let overlay = build_sketch_overlay(&sketches, &values).expect("overlay");
        assert!(!overlay.labels.is_empty());
        assert!(overlay.labels.iter().any(|label| label.text.contains("mm")));
        let scale = label_scale_for_bounds(0.1);
        assert!(!overlay.label_line_vertices(scale).is_empty());
    }

    #[test]
    fn billboard_labels_follow_camera_basis() {
        use crate::camera::OrbitCamera;

        let label = OverlayLabel {
            position: [0.04, 0.03, 0.0],
            text: "80 mm".to_string(),
            right: [1.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
        };
        let overlay = SketchOverlay {
            labels: vec![label],
            ..Default::default()
        };
        let scale = 0.001;
        let plane_fixed = overlay.label_line_vertices(scale);
        let camera_a = OrbitCamera {
            target: [0.04, 0.03, 0.0],
            distance: 0.2,
            yaw_rad: 0.2,
            pitch_rad: 0.3,
            fov_y_deg: 45.0,
            aspect: 1.0,
        };
        let camera_b = OrbitCamera {
            yaw_rad: 1.4,
            pitch_rad: -0.6,
            ..camera_a
        };
        let (right_a, up_a) = camera_a.billboard_basis();
        let (right_b, up_b) = camera_b.billboard_basis();
        let billboard_a = overlay.label_line_vertices_billboard(scale, right_a, up_a, None);
        let billboard_b = overlay.label_line_vertices_billboard(scale, right_b, up_b, None);
        assert_ne!(plane_fixed, billboard_a);
        assert_ne!(billboard_a, billboard_b);
    }

    #[test]
    fn label_depth_bias_moves_vertices_toward_camera() {
        let mut vertices = vec![[0.0, 0.0, 0.0], [0.01, 0.0, 0.0]];
        bias_vertices_toward_camera(&mut vertices, [0.0, 0.0, 1.0], 0.001);
        assert!((vertices[0][2] - 0.001).abs() < 1e-6);
        assert!(vertices[1][2] > 0.0);
    }

    #[test]
    fn geometric_constraint_symbols_are_generated() {
        use opencad_core::{ConstraintId, EntityId, SketchId};
        use opencad_sketch::constraint::{Constraint, EntityRef, LineEnd};
        use opencad_sketch::entity::{Coord, EntityBase, LineEntity, PointEntity, SketchEntity};

        let mut sketch = Sketch::new(
            SketchId::new("sketch:symbols").expect("id"),
            "Symbols",
            Workplane::xy(),
        );
        for (id, x, y) in [
            ("ent:p0", 0.0, 0.0),
            ("ent:p1", 0.04, 0.0),
            ("ent:p2", 0.0, 0.03),
        ] {
            sketch
                .add_entity(SketchEntity::Point(PointEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    x: Coord::literal(x),
                    y: Coord::literal(y),
                }))
                .expect("point");
        }
        for (id, start, end) in [
            ("ent:l0", "ent:p0", "ent:p1"),
            ("ent:l1", "ent:p0", "ent:p2"),
        ] {
            sketch
                .add_entity(SketchEntity::Line(LineEntity {
                    base: EntityBase {
                        id: EntityId::new(id).expect("id"),
                        construction: false,
                    },
                    start: EntityId::new(start).expect("id"),
                    end: EntityId::new(end).expect("id"),
                }))
                .expect("line");
        }
        sketch
            .add_constraint(Constraint::Parallel {
                id: ConstraintId::new("con:parallel").expect("id"),
                line_a: EntityId::new("ent:l0").expect("id"),
                line_b: EntityId::new("ent:l1").expect("id"),
            })
            .expect("parallel");
        sketch
            .add_constraint(Constraint::Coincident {
                id: ConstraintId::new("con:coincident").expect("id"),
                a: EntityRef::Entity(EntityId::new("ent:p0").expect("id")),
                b: EntityRef::PointOnLine {
                    line: EntityId::new("ent:l0").expect("id"),
                    end: LineEnd::Start,
                },
            })
            .expect("coincident");
        sketch
            .add_constraint(Constraint::Perpendicular {
                id: ConstraintId::new("con:perpendicular").expect("id"),
                line_a: EntityId::new("ent:l0").expect("id"),
                line_b: EntityId::new("ent:l1").expect("id"),
            })
            .expect("perpendicular");

        let values = IndexMap::new();
        let overlay = build_sketch_overlay(&[sketch], &values).expect("overlay");
        assert!(overlay.labels.iter().any(|label| label.text == "//"));
        assert!(!overlay.symbol_lines.is_empty());
        let scale = label_scale_for_bounds(0.1);
        assert!(!overlay.label_line_vertices(scale).is_empty());
    }
}
