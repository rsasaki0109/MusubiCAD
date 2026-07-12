//! Rotation-vector pose math for the assembly solver.

use opencad_geometry::RigidTransform;

/// Build a rotation matrix from an axis-angle vector (axis * angle).
pub fn rotation_matrix_from_vector(v: [f64; 3]) -> [[f64; 3]; 3] {
    let angle_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if angle_sq <= 1e-18 {
        return RigidTransform::identity_rotation();
    }
    let angle = angle_sq.sqrt();
    let axis = [v[0] / angle, v[1] / angle, v[2] / angle];
    rodrigues(axis, angle)
}

/// Extract a rotation vector from a proper rotation matrix.
pub fn rotation_vector_from_matrix(rotation: [[f64; 3]; 3]) -> [f64; 3] {
    let trace = rotation[0][0] + rotation[1][1] + rotation[2][2];
    let angle = ((trace - 1.0) * 0.5).clamp(-1.0, 1.0).acos();
    if angle <= 1e-12 {
        return [0.0, 0.0, 0.0];
    }
    let denom = 2.0 * angle.sin();
    if denom.abs() <= 1e-12 {
        return [0.0, 0.0, 0.0];
    }
    [
        (rotation[2][1] - rotation[1][2]) / denom * angle,
        (rotation[0][2] - rotation[2][0]) / denom * angle,
        (rotation[1][0] - rotation[0][1]) / denom * angle,
    ]
}

fn rodrigues(axis: [f64; 3], angle: f64) -> [[f64; 3]; 3] {
    let [x, y, z] = axis;
    let c = angle.cos();
    let s = angle.sin();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

pub fn transform_point(transform: RigidTransform, local: [f64; 3]) -> [f64; 3] {
    transform.transform_point(local)
}

pub fn transform_direction(transform: RigidTransform, local: [f64; 3]) -> [f64; 3] {
    apply_rotation(transform.rotation, local)
}

fn apply_rotation(rotation: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    [
        rotation[0][0] * vector[0] + rotation[0][1] * vector[1] + rotation[0][2] * vector[2],
        rotation[1][0] * vector[0] + rotation[1][1] * vector[1] + rotation[1][2] * vector[2],
        rotation[2][0] * vector[0] + rotation[2][1] * vector[1] + rotation[2][2] * vector[2],
    ]
}

pub fn normalize(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= 1e-12 {
        return [0.0, 0.0, 1.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn subtract(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = subtract(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_vector_round_trip() {
        let original = rotation_matrix_from_vector([0.0, 0.0, std::f64::consts::FRAC_PI_2]);
        let vector = rotation_vector_from_matrix(original);
        let restored = rotation_matrix_from_vector(vector);
        for row in 0..3 {
            for col in 0..3 {
                assert!((original[row][col] - restored[row][col]).abs() < 1e-6);
            }
        }
    }
}
