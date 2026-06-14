//! Object-space ambient occlusion baked per vertex.
//!
//! Flat shading and feature edges still cannot convey the soft contact shading
//! that makes raised CAD features (a ring of bosses, a counterbored hole) read
//! as solid. Baking a cheap hemisphere occlusion term per vertex darkens
//! concave junctions — boss bases, hole interiors — without any screen-space
//! pass, so it is deterministic and resolution independent.

/// Deterministic cosine-ish hemisphere directions in local space (z = up).
/// Uses a Fibonacci spiral so no RNG is required and bakes are repeatable.
fn hemisphere_dirs(count: usize) -> Vec<[f32; 3]> {
    const GOLDEN_ANGLE: f32 = 2.399_963_2; // pi * (3 - sqrt(5))
    let mut dirs = Vec::with_capacity(count);
    for k in 0..count {
        let i = k as f32 + 0.5;
        let z = 1.0 - i / count as f32; // (0, 1], biased toward the pole
        let r = (1.0 - z * z).max(0.0).sqrt();
        let phi = i * GOLDEN_ANGLE;
        dirs.push([r * phi.cos(), r * phi.sin(), z]);
    }
    dirs
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= f32::EPSILON {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Build an orthonormal basis whose third axis is `n`.
fn basis_from_normal(n: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let helper = if n[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let t = normalize(cross(helper, n));
    let b = cross(n, t);
    (t, b, n)
}

/// Möller–Trumbore ray/triangle intersection. Returns the hit distance along
/// `dir` (assumed normalized) within `(eps, max_dist)`, if any.
fn ray_triangle(
    origin: [f32; 3],
    dir: [f32; 3],
    v0: [f32; 3],
    v1: [f32; 3],
    v2: [f32; 3],
    eps: f32,
    max_dist: f32,
) -> Option<f32> {
    let edge1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let edge2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    let pvec = cross(dir, edge2);
    let det = edge1[0] * pvec[0] + edge1[1] * pvec[1] + edge1[2] * pvec[2];
    if det.abs() < 1.0e-12 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = [origin[0] - v0[0], origin[1] - v0[1], origin[2] - v0[2]];
    let u = (tvec[0] * pvec[0] + tvec[1] * pvec[1] + tvec[2] * pvec[2]) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = cross(tvec, edge1);
    let v = (dir[0] * qvec[0] + dir[1] * qvec[1] + dir[2] * qvec[2]) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = (edge2[0] * qvec[0] + edge2[1] * qvec[1] + edge2[2] * qvec[2]) * inv_det;
    if t > eps && t < max_dist {
        Some(t)
    } else {
        None
    }
}

/// Compute a per-vertex ambient-occlusion factor in `[0, 1]` (1 = fully open).
///
/// `radius` bounds the occlusion search distance; `strength` (0..1) scales how
/// dark a fully occluded vertex becomes. Triangles incident to a vertex are
/// skipped so a face never occludes itself; concave occluders (e.g. a boss wall
/// rising from a plate) belong to different vertices and are still found.
pub(crate) fn compute_vertex_ao(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    indices: &[u32],
    radius: f32,
    samples: usize,
    strength: f32,
) -> Vec<f32> {
    let mut ao = vec![1.0_f32; positions.len()];
    if positions.is_empty() || indices.len() < 3 || radius <= 0.0 || samples == 0 {
        return ao;
    }

    let triangles: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let dirs = hemisphere_dirs(samples);
    let eps = radius * 1.0e-3;
    let origin_offset = radius * 1.0e-2;

    for (vi, (&position, &raw_normal)) in positions.iter().zip(normals).enumerate() {
        let n = normalize(raw_normal);
        let (t, b, _) = basis_from_normal(n);
        let origin = [
            position[0] + n[0] * origin_offset,
            position[1] + n[1] * origin_offset,
            position[2] + n[2] * origin_offset,
        ];
        let vi = vi as u32;

        let mut occluded = 0.0_f32;
        for local in &dirs {
            // Local hemisphere direction -> world.
            let dir = [
                t[0] * local[0] + b[0] * local[1] + n[0] * local[2],
                t[1] * local[0] + b[1] * local[1] + n[1] * local[2],
                t[2] * local[0] + b[2] * local[1] + n[2] * local[2],
            ];
            let mut nearest: Option<f32> = None;
            for tri in &triangles {
                if tri[0] == vi || tri[1] == vi || tri[2] == vi {
                    continue;
                }
                if let Some(hit) = ray_triangle(
                    origin,
                    dir,
                    positions[tri[0] as usize],
                    positions[tri[1] as usize],
                    positions[tri[2] as usize],
                    eps,
                    radius,
                ) {
                    nearest = Some(nearest.map_or(hit, |n| n.min(hit)));
                }
            }
            if let Some(hit) = nearest {
                // Closer hits occlude more.
                occluded += 1.0 - (hit / radius);
            }
        }

        let occlusion = (occluded / samples as f32).clamp(0.0, 1.0);
        ao[vi as usize] = 1.0 - occlusion * strength;
    }

    ao
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_plane_is_fully_lit() {
        // A single flat triangle: nothing can occlude its vertices.
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let normals = [[0.0, 0.0, 1.0]; 3];
        let indices = [0, 1, 2];
        let ao = compute_vertex_ao(&positions, &normals, &indices, 0.5, 16, 1.0);
        assert!(ao.iter().all(|&a| (a - 1.0).abs() < 1.0e-6));
    }

    #[test]
    fn vertex_under_overhang_is_darkened() {
        // A floor spanning [0,2]^2 with a low ceiling hovering over the [0,1]^2
        // corner. Floor vertices beneath the ceiling are occluded; the far
        // corner stays open.
        let positions = [
            // floor (z=0), normal +z
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
            // ceiling over the near corner (z=0.3), normal -z — distinct verts
            [0.0, 0.0, 0.3],
            [1.0, 0.0, 0.3],
            [1.0, 1.0, 0.3],
            [0.0, 1.0, 0.3],
        ];
        let mut normals = [[0.0, 0.0, 1.0]; 8];
        for normal in normals.iter_mut().skip(4) {
            *normal = [0.0, 0.0, -1.0];
        }
        let indices = [
            0, 1, 2, 0, 2, 3, // floor
            4, 5, 6, 4, 6, 7, // ceiling
        ];
        let ao = compute_vertex_ao(&positions, &normals, &indices, 1.5, 48, 1.0);
        // Floor vertex 0 sits directly under the ceiling -> occluded.
        assert!(
            ao[0] < 0.9,
            "expected occlusion under overhang, got {}",
            ao[0]
        );
        // Floor vertex 2 is the far corner, open to the sky.
        assert!(ao[2] > ao[0]);
        assert!(ao[2] > 0.9, "far corner should stay open, got {}", ao[2]);
    }
}
