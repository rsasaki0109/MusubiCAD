#[cfg(feature = "occt")]
use cadrum::{DVec3, Edge, Error as OcctError};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::{ProfilePlane, SketchPlacement, SolvedSketch};

/// Edges of a sweep path: its line and arc segments, open or closed,
/// placed by the path sketch placement (ADR-023).
#[cfg(feature = "occt")]
pub fn path_to_edges(path: &SolvedSketch) -> Result<Vec<Edge>> {
    if path.segments.is_empty() {
        return Err(OpenCadError::validation("sweep path has no segments"));
    }
    let placement = path.placement.unwrap_or(SketchPlacement::global_xy());
    segments_to_edges(&path.segments, placement)
}

#[cfg(feature = "occt")]
fn segments_to_edges(
    segments: &[opencad_geometry::ProfileSegment],
    placement: SketchPlacement,
) -> Result<Vec<Edge>> {
    let to_world = |p: [f64; 2]| DVec3::from_array(placement.map_point(p[0], p[1]));
    segments
        .iter()
        .map(|segment| match *segment {
            opencad_geometry::ProfileSegment::Line { start_m, end_m } => {
                Edge::line(to_world(start_m), to_world(end_m))
            }
            opencad_geometry::ProfileSegment::Arc {
                start_m,
                mid_m,
                end_m,
            } => Edge::arc_3pts(to_world(start_m), to_world(mid_m), to_world(end_m)),
        })
        .collect::<std::result::Result<Vec<Edge>, _>>()
        .map_err(map_occt_error)
}

#[cfg(feature = "occt")]
pub fn sketch_to_edges(sketch: &SolvedSketch) -> Result<Vec<Edge>> {
    let placement = sketch.placement.unwrap_or(SketchPlacement::global_xy());
    sketch_to_edges_placed(sketch, placement)
}

#[cfg(feature = "occt")]
pub fn sketch_to_edges_placed(
    sketch: &SolvedSketch,
    placement: SketchPlacement,
) -> Result<Vec<Edge>> {
    if !sketch.closed {
        return Err(OpenCadError::validation(
            "only closed profiles can be extruded in MVP",
        ));
    }

    // Exact circle profiles become one true circular edge (ADR-020).
    if let Some(circle) = sketch.circle {
        if !circle.radius_m.is_finite() || circle.radius_m <= 0.0 {
            return Err(OpenCadError::validation(
                "circle profile radius must be positive",
            ));
        }
        let x = DVec3::from_array(placement.x_axis_m);
        let y = DVec3::from_array(placement.y_axis_m);
        let normal = x.cross(y).normalize_or_zero();
        if normal == DVec3::ZERO {
            return Err(OpenCadError::validation(
                "sketch placement axes must not be parallel",
            ));
        }
        let center = placement.map_point(circle.center_m[0], circle.center_m[1]);
        let edge = Edge::circle(circle.radius_m, normal)
            .map_err(map_occt_error)?
            .translate(DVec3::from_array(center));
        return Ok(vec![edge]);
    }
    // Loops with arcs become exact line and arc edges (ADR-021).
    if !sketch.segments.is_empty() {
        return segments_to_edges(&sketch.segments, placement);
    }
    if sketch.points.len() < 3 {
        return Err(OpenCadError::validation(
            "profile needs at least three points",
        ));
    }

    let points: Vec<DVec3> = sketch
        .points
        .iter()
        .map(|p| {
            let world = placement.map_point(p[0], p[1]);
            DVec3::new(world[0], world[1], world[2])
        })
        .collect();

    Edge::polygon(&points).map_err(map_occt_error)
}

#[cfg(feature = "occt")]
pub fn sketch_to_edges_on_plane(sketch: &SolvedSketch, plane: ProfilePlane) -> Result<Vec<Edge>> {
    let placement = match plane {
        ProfilePlane::Xy => SketchPlacement::global_xy(),
        ProfilePlane::Yz => SketchPlacement {
            origin_m: [0.0, 0.0, 0.0],
            x_axis_m: [0.0, 1.0, 0.0],
            y_axis_m: [0.0, 0.0, 1.0],
        },
        ProfilePlane::Xz => SketchPlacement {
            origin_m: [0.0, 0.0, 0.0],
            x_axis_m: [1.0, 0.0, 0.0],
            y_axis_m: [0.0, 0.0, 1.0],
        },
    };
    sketch_to_edges_placed(sketch, placement)
}

#[cfg(feature = "occt")]
pub fn sketch_to_edge(sketch: &SolvedSketch) -> Result<Edge> {
    let edges = sketch_to_edges(sketch)?;
    edges
        .into_iter()
        .next()
        .ok_or_else(|| OpenCadError::validation("polygon produced no edges"))
}

#[cfg(feature = "occt")]
pub fn map_occt_error(err: OcctError) -> OpenCadError {
    OpenCadError::Other(format!("OCCT error: {err}"))
}

#[cfg(not(feature = "occt"))]
pub fn sketch_to_edge(_sketch: &SolvedSketch) -> Result<()> {
    Err(OpenCadError::Other(
        "OCCT backend not enabled; rebuild with --features occt".into(),
    ))
}
