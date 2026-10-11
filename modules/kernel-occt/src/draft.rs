//! Convex half-space reconstruction; no cached B-Rep owns design semantics.
use crate::convert::map_occt_error;
use cadrum::{DVec3, Solid};
use opencad_core::{OpenCadError, Result};
use opencad_geometry::DraftInput;

const PLANE_TOLERANCE_M: f64 = 1e-7;
const VOLUME_RELATIVE_TOLERANCE: f64 = 1e-7;

fn clip(planes: &[(DVec3, DVec3)], bounds: [DVec3; 2]) -> Result<Solid> {
    let mut result = Solid::cube(bounds[0], bounds[1]);
    for (point, outward) in planes {
        // cadrum fills the side toward the normal, hence negate outward.
        result = (result * Solid::half_space(*point, -*outward))
            .build()
            .map_err(map_occt_error)?;
    }
    Ok(result)
}

pub(crate) fn draft_solid(source: &Solid, input: &DraftInput) -> Result<Solid> {
    // Deep copy separates face TShapes shared at different locations.
    let owned = source.clone();
    let source = &owned;
    let faces: Vec<_> = source.iter_face().collect();
    if faces.len() < 4 || faces.len() > 64 {
        return Err(OpenCadError::validation(
            "draft supports 4..64 planar faces on one convex solid",
        ));
    }
    let mut planes = planar_planes(source)?;
    let bounds = source.bounding_box();
    let span = (bounds[1] - bounds[0]).length();
    if !span.is_finite() || span <= PLANE_TOLERANCE_M {
        return Err(OpenCadError::validation(
            "draft body extent must be finite and greater than 1e-7 m",
        ));
    }
    let guard_bounds = [
        bounds[0] - DVec3::splat(span * 4.0),
        bounds[1] + DVec3::splat(span * 4.0),
    ];
    let original = clip(&planes, guard_bounds)?;
    let source_volume = source.volume();
    let tolerance = (source_volume.abs() * VOLUME_RELATIVE_TOLERANCE).max(1e-15);
    let common = (source.clone() * original.clone())
        .build()
        .map_err(map_occt_error)?
        .volume();
    // Test both sides of the symmetric difference, not just equal total volume.
    if !common.is_finite()
        || !original.volume().is_finite()
        || !source_volume.is_finite()
        || source_volume <= tolerance
        || (source_volume - common).abs() > tolerance
        || (original.volume() - common).abs() > tolerance
    {
        return Err(OpenCadError::validation(
            "draft requires a convex solid without holes or cavities",
        ));
    }
    let neutral_normal = DVec3::from_array(input.neutral_normal).normalize();
    let neutral_origin = DVec3::from_array(input.neutral_origin_m);
    let mut selected = Vec::new();
    for pick in &input.faces {
        let face = crate::backend::pick_face(source, pick)?;
        let index = faces
            .iter()
            .position(|f| f.id() == face.id())
            .ok_or_else(|| OpenCadError::validation("draft face is absent"))?;
        if selected.contains(&index) {
            return Err(OpenCadError::validation(
                "draft selects the same face twice",
            ));
        }
        let (point, normal) = planes[index];
        if normal.dot(neutral_normal).abs() > 1e-6 {
            return Err(OpenCadError::validation(
                "draft side face must be perpendicular to neutral plane",
            ));
        }
        // The projection of point onto neutral plane stays on the old side plane.
        let pivot = point - neutral_normal * (point - neutral_origin).dot(neutral_normal);
        let tilted = normal * input.angle_rad.cos() + neutral_normal * input.angle_rad.sin();
        planes[index] = (pivot, tilted.normalize());
        selected.push(index);
    }
    let result = clip(&planes, guard_bounds)?.clone();
    let result_bounds = result.bounding_box();
    if !result.volume().is_finite()
        || result.volume() <= tolerance
        || (0..3).any(|i| {
            result_bounds[0][i] <= guard_bounds[0][i] + PLANE_TOLERANCE_M
                || result_bounds[1][i] >= guard_bounds[1][i] - PLANE_TOLERANCE_M
        })
    {
        return Err(OpenCadError::validation(
            "draft result collapsed or exceeded reconstruction bounds",
        ));
    }
    // A requested side disappearing must not silently become another shape.
    let result_planes = planar_planes(&result)?;
    for index in selected {
        let (point, normal) = planes[index];
        if !result_planes.iter().any(|(p, n)| {
            (*p - point).dot(normal).abs() <= PLANE_TOLERANCE_M && n.dot(normal) > 1.0 - 1e-8
        }) {
            return Err(OpenCadError::validation("draft removed a selected face"));
        }
    }
    Ok(result)
}

fn planar_planes(solid: &Solid) -> Result<Vec<(DVec3, DVec3)>> {
    let sampling = cadrum::Tessellation {
        deflection_linear: 1e-6,
        deflection_angular: 0.05,
        relative_linear: false,
    };
    let mesh = Solid::mesh([solid], sampling).map_err(map_occt_error)?;
    solid
        .iter_face()
        .map(|face| {
            let mut plane: Option<(DVec3, DVec3)> = None;
            for (triangle, id) in mesh.indices.chunks_exact(3).zip(&mesh.face_ids) {
                if *id != face.id() {
                    continue;
                }
                let [a, b, c] = [
                    mesh.vertices[triangle[0]],
                    mesh.vertices[triangle[1]],
                    mesh.vertices[triangle[2]],
                ];
                let normal = (b - a).cross(c - a).normalize_or_zero();
                if normal.length() < 0.5 {
                    continue;
                }
                if let Some((point, n)) = plane {
                    if normal.dot(n) < 1.0 - 1e-8
                        || [a, b, c]
                            .iter()
                            .any(|p| (*p - point).dot(n).abs() > PLANE_TOLERANCE_M)
                    {
                        return Err(OpenCadError::validation(
                            "draft requires planar faces; curved faces are unsupported",
                        ));
                    }
                } else {
                    plane = Some((a, normal));
                }
            }
            let (point, normal) = plane.ok_or_else(|| {
                OpenCadError::validation("draft face has no nondegenerate triangles")
            })?;
            for edge in face.iter_edge() {
                let mut points = edge.approximation_segments(sampling);
                points.extend([edge.start_point(), edge.end_point()]);
                if points
                    .iter()
                    .any(|p| (*p - point).dot(normal).abs() > PLANE_TOLERANCE_M)
                {
                    return Err(OpenCadError::validation("draft boundary is not planar"));
                }
            }
            Ok((point, normal))
        })
        .collect()
}
