//! Deterministic mesh-based hidden-line classification (Task-177).

use std::collections::BTreeMap;

use opencad_geometry::MeshSet;

use crate::projection::ProjectionKind;

/// Depth tolerance used when comparing projected triangles, in meters.
pub const HIDDEN_LINE_DEPTH_TOLERANCE_M: f64 = 1.0e-7;

/// Visibility of a projected drawing edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineVisibility {
    /// The edge is not occluded from the selected projection.
    Visible,
    /// The edge is behind another mesh triangle.
    Hidden,
}

/// A model-space edge classified for a drawing projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassifiedEdge {
    /// Projected edge start in model-space meters.
    pub start_m: [f64; 2],
    /// Projected edge end in model-space meters.
    pub end_m: [f64; 2],
    /// Visibility determined by the mesh depth test.
    pub visibility: LineVisibility,
}

#[derive(Debug, Clone, Copy)]
struct ViewPoint {
    uv: [f64; 2],
    depth_m: f64,
}

/// Project mesh edges and classify edges occluded by a triangle at their midpoint.
///
/// Tessellation diagonals belonging to the same B-Rep face are omitted. The
/// midpoint test is intentionally deterministic and is an approximation: an edge
/// that crosses an occluder boundary is not split into visible and hidden pieces.
pub fn classify_hidden_lines(mesh: &MeshSet, projection: ProjectionKind) -> Vec<ClassifiedEdge> {
    let triangles: Vec<[ViewPoint; 3]> = mesh
        .indices
        .chunks_exact(3)
        .filter_map(|indices| {
            Some([
                view_point(*mesh.positions.get(indices[0] as usize)?, projection),
                view_point(*mesh.positions.get(indices[1] as usize)?, projection),
                view_point(*mesh.positions.get(indices[2] as usize)?, projection),
            ])
        })
        .collect();

    let mut edges: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for (triangle_index, triangle) in mesh.indices.chunks_exact(3).enumerate() {
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            edges
                .entry(if a <= b { (a, b) } else { (b, a) })
                .or_default()
                .push(triangle_index);
        }
    }

    edges
        .into_iter()
        .filter_map(|((a, b), adjacent)| {
            if is_tessellation_diagonal(mesh, &adjacent) {
                return None;
            }
            let start = view_point(*mesh.positions.get(a as usize)?, projection);
            let end = view_point(*mesh.positions.get(b as usize)?, projection);
            if squared_distance(start.uv, end.uv) <= f64::EPSILON {
                return None;
            }
            let midpoint = [
                (start.uv[0] + end.uv[0]) * 0.5,
                (start.uv[1] + end.uv[1]) * 0.5,
            ];
            let edge_depth = (start.depth_m + end.depth_m) * 0.5;
            let hidden = triangles.iter().enumerate().any(|(index, triangle)| {
                !adjacent.contains(&index)
                    && triangle_depth_at(triangle, midpoint)
                        .is_some_and(|depth| depth > edge_depth + HIDDEN_LINE_DEPTH_TOLERANCE_M)
            });
            Some(ClassifiedEdge {
                start_m: start.uv,
                end_m: end.uv,
                visibility: if hidden {
                    LineVisibility::Hidden
                } else {
                    LineVisibility::Visible
                },
            })
        })
        .collect()
}

fn is_tessellation_diagonal(mesh: &MeshSet, adjacent: &[usize]) -> bool {
    adjacent.len() == 2
        && mesh.has_triangle_face_ids()
        && mesh.triangle_face_ids[adjacent[0]] == mesh.triangle_face_ids[adjacent[1]]
}

fn view_point(point: [f32; 3], projection: ProjectionKind) -> ViewPoint {
    let point = [point[0] as f64, point[1] as f64, point[2] as f64];
    let depth_m = match projection {
        ProjectionKind::Front => point[2],
        ProjectionKind::Top => point[1],
        ProjectionKind::Right => point[0],
        ProjectionKind::Isometric => 0.5 * point[0] - 0.35 * point[1] + point[2],
    };
    ViewPoint {
        uv: projection.project_point(point),
        depth_m,
    }
}

fn triangle_depth_at(triangle: &[ViewPoint; 3], point: [f64; 2]) -> Option<f64> {
    let [a, b, c] = *triangle;
    let denominator =
        (b.uv[1] - c.uv[1]) * (a.uv[0] - c.uv[0]) + (c.uv[0] - b.uv[0]) * (a.uv[1] - c.uv[1]);
    if denominator.abs() <= f64::EPSILON {
        return None;
    }
    let wa = ((b.uv[1] - c.uv[1]) * (point[0] - c.uv[0])
        + (c.uv[0] - b.uv[0]) * (point[1] - c.uv[1]))
        / denominator;
    let wb = ((c.uv[1] - a.uv[1]) * (point[0] - c.uv[0])
        + (a.uv[0] - c.uv[0]) * (point[1] - c.uv[1]))
        / denominator;
    let wc = 1.0 - wa - wb;
    let inside_tolerance = 1.0e-9;
    (wa >= -inside_tolerance && wb >= -inside_tolerance && wc >= -inside_tolerance)
        .then_some(wa * a.depth_m + wb * b.depth_m + wc * c.depth_m)
}

fn squared_distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_triangle_hides_edge_behind_it() {
        let mesh = MeshSet {
            positions: vec![
                [-1.0, -1.0, 1.0],
                [1.0, -1.0, 1.0],
                [0.0, 1.0, 1.0],
                [-0.25, 0.0, 0.0],
                [0.25, 0.0, 0.0],
                [0.0, -0.25, 0.0],
            ],
            normals: Vec::new(),
            indices: vec![0, 1, 2, 3, 4, 5],
            triangle_face_ids: vec![1, 2],
        };
        let lines = classify_hidden_lines(&mesh, ProjectionKind::Front);
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.visibility == LineVisibility::Hidden)
                .count(),
            3
        );
    }

    #[test]
    fn same_face_tessellation_diagonal_is_omitted() {
        let mesh = MeshSet {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            normals: Vec::new(),
            indices: vec![0, 1, 2, 0, 2, 3],
            triangle_face_ids: vec![7, 7],
        };
        assert_eq!(classify_hidden_lines(&mesh, ProjectionKind::Front).len(), 4);
    }
}
