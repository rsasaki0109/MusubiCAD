#[cfg(feature = "occt")]
use cadrum::{DVec3, Edge, Error as OcctError};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::{ProfilePlane, SketchPlacement, SolvedSketch};

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
    if sketch.points.len() < 3 {
        return Err(OpenCadError::validation(
            "profile needs at least three points",
        ));
    }
    if !sketch.closed {
        return Err(OpenCadError::validation(
            "only closed profiles can be extruded in MVP",
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
