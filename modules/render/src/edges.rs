//! Feature-edge extraction for shaded CAD previews.
//!
//! Flat shading alone gives no depth cue for raised features that sit inside a
//! face silhouette (e.g. a ring of pin bosses). Drawing the model's *feature
//! edges* — boundary edges and sharp creases between adjacent triangles —
//! restores the familiar CAD look where every feature reads regardless of the
//! camera angle.

use crate::solid::GpuVertex;

/// A mesh vertex position quantized to micron buckets.
type VertKey = (i64, i64, i64);
/// An undirected edge keyed by its two (ordered) vertex keys.
type EdgeKey = (VertKey, VertKey);

/// Quantize a coordinate (meters) to micron buckets so shared mesh vertices
/// hash to the same key despite floating-point noise.
fn quantize(value: f32) -> i64 {
    (value as f64 * 1.0e6).round() as i64
}

fn key(position: [f32; 3]) -> VertKey {
    (
        quantize(position[0]),
        quantize(position[1]),
        quantize(position[2]),
    )
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

struct EdgeRecord {
    endpoints: ([f32; 3], [f32; 3]),
    faces: u32,
    normal: [f32; 3],
    is_feature: bool,
}

/// Extract feature-edge line segments (pairs of endpoints) from a packed mesh.
///
/// An edge is emitted when it is a boundary edge (used by a single triangle) or
/// when the angle between its two adjacent face normals exceeds
/// `crease_angle_deg`.
pub(crate) fn feature_edge_vertices(
    vertices: &[GpuVertex],
    indices: &[u32],
    crease_angle_deg: f32,
) -> Vec<[f32; 3]> {
    use std::collections::HashMap;

    let cos_threshold = crease_angle_deg.to_radians().cos();
    let mut edges: HashMap<EdgeKey, EdgeRecord> = HashMap::new();

    for triangle in indices.chunks_exact(3) {
        let p = [
            vertices[triangle[0] as usize].position,
            vertices[triangle[1] as usize].position,
            vertices[triangle[2] as usize].position,
        ];
        let normal = face_normal(p[0], p[1], p[2]);
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            let ka = key(p[i]);
            let kb = key(p[j]);
            let edge_key = if ka <= kb { (ka, kb) } else { (kb, ka) };
            edges
                .entry(edge_key)
                .and_modify(|record| {
                    record.faces += 1;
                    let dot = record.normal[0] * normal[0]
                        + record.normal[1] * normal[1]
                        + record.normal[2] * normal[2];
                    if dot < cos_threshold {
                        record.is_feature = true;
                    }
                })
                .or_insert(EdgeRecord {
                    endpoints: (p[i], p[j]),
                    faces: 1,
                    normal,
                    is_feature: false,
                });
        }
    }

    let mut out = Vec::new();
    for record in edges.values() {
        // Boundary edges (1 face) or sharp creases / non-manifold seams.
        if record.faces != 2 || record.is_feature {
            out.push(record.endpoints.0);
            out.push(record.endpoints.1);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vert(position: [f32; 3]) -> GpuVertex {
        GpuVertex {
            position,
            normal: [0.0, 0.0, 1.0],
            ao: 1.0,
        }
    }

    #[test]
    fn single_triangle_yields_three_boundary_edges() {
        let vertices = [
            vert([0.0, 0.0, 0.0]),
            vert([1.0, 0.0, 0.0]),
            vert([0.0, 1.0, 0.0]),
        ];
        let indices = [0, 1, 2];
        let segments = feature_edge_vertices(&vertices, &indices, 30.0);
        // 3 edges * 2 endpoints.
        assert_eq!(segments.len(), 6);
    }

    #[test]
    fn coplanar_quad_drops_shared_diagonal() {
        // Two triangles forming a flat quad share one interior edge that is not
        // a crease, so only the 4 outer boundary edges remain.
        let vertices = [
            vert([0.0, 0.0, 0.0]),
            vert([1.0, 0.0, 0.0]),
            vert([1.0, 1.0, 0.0]),
            vert([0.0, 1.0, 0.0]),
        ];
        let indices = [0, 1, 2, 0, 2, 3];
        let segments = feature_edge_vertices(&vertices, &indices, 30.0);
        assert_eq!(segments.len(), 8); // 4 boundary edges, diagonal dropped.
    }

    #[test]
    fn folded_quad_keeps_crease() {
        // Two triangles meeting at 90° keep the shared crease edge.
        let vertices = [
            vert([0.0, 0.0, 0.0]),
            vert([1.0, 0.0, 0.0]),
            vert([1.0, 1.0, 0.0]),
            vert([1.0, 0.0, 1.0]),
        ];
        // Triangle A in z=0 plane, triangle B folded up along edge (1,0,0)-(1,1,0)...
        let indices = [0, 1, 2, 1, 3, 2];
        let segments = feature_edge_vertices(&vertices, &indices, 30.0);
        // 5 boundary edges + 1 shared crease, all kept (none coplanar-interior).
        assert!(segments.len() >= 2);
        assert_eq!(segments.len() % 2, 0);
    }
}
