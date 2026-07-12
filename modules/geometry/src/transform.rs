//! Rigid body transforms (translation + 3×3 rotation matrix).

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};

/// Position comparison tolerance in meters.
pub const POSITION_TOLERANCE_M: f64 = 1e-9;

/// Position comparison tolerance for rotation matrix entries.
pub const ROTATION_TOLERANCE: f64 = 1e-9;

/// Rigid transform: `p' = rotation * p + translation_m`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidTransform {
    pub translation_m: [f64; 3],
    pub rotation: [[f64; 3]; 3],
}

impl RigidTransform {
    pub fn identity() -> Self {
        Self {
            translation_m: [0.0, 0.0, 0.0],
            rotation: Self::identity_rotation(),
        }
    }

    pub fn identity_rotation() -> [[f64; 3]; 3] {
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    }

    pub fn from_translation(translation_m: [f64; 3]) -> Self {
        Self {
            translation_m,
            rotation: Self::identity_rotation(),
        }
    }

    pub fn is_identity(&self) -> bool {
        self.is_identity_translation() && self.is_identity_rotation()
    }

    pub fn is_identity_translation(&self) -> bool {
        self.translation_m
            .iter()
            .all(|v| v.abs() <= POSITION_TOLERANCE_M)
    }

    pub fn is_identity_rotation(&self) -> bool {
        Self::matrices_near(
            &self.rotation,
            &Self::identity_rotation(),
            ROTATION_TOLERANCE,
        )
    }

    pub fn compose(self, other: Self) -> Self {
        Self {
            rotation: Self::multiply_matrices(self.rotation, other.rotation),
            translation_m: Self::apply_rotation(self.rotation, other.translation_m)
                .into_iter()
                .zip(self.translation_m)
                .map(|(a, b)| a + b)
                .collect::<Vec<_>>()
                .try_into()
                .expect("translation vector"),
        }
    }

    pub fn inverse(self) -> Result<Self> {
        let transposed = Self::transpose_matrix(self.rotation);
        if !Self::matrices_near(
            &Self::multiply_matrices(self.rotation, transposed),
            &Self::identity_rotation(),
            1e-6,
        ) {
            return Err(OpenCadError::validation(
                "rotation matrix is not orthonormal; cannot invert",
            ));
        }
        let neg_t = [
            -self.translation_m[0],
            -self.translation_m[1],
            -self.translation_m[2],
        ];
        Ok(Self {
            rotation: transposed,
            translation_m: Self::apply_rotation(transposed, neg_t),
        })
    }

    pub fn transform_point(self, point: [f64; 3]) -> [f64; 3] {
        let rotated = Self::apply_rotation(self.rotation, point);
        [
            rotated[0] + self.translation_m[0],
            rotated[1] + self.translation_m[1],
            rotated[2] + self.translation_m[2],
        ]
    }

    pub fn points_near(a: [f64; 3], b: [f64; 3], tolerance: f64) -> bool {
        a.iter()
            .zip(b)
            .all(|(left, right)| (left - right).abs() <= tolerance)
    }

    pub fn matrices_near(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3], tolerance: f64) -> bool {
        a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .all(|(left, right)| (left - right).abs() <= tolerance)
    }

    fn apply_rotation(rotation: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
        [
            rotation[0][0] * vector[0] + rotation[0][1] * vector[1] + rotation[0][2] * vector[2],
            rotation[1][0] * vector[0] + rotation[1][1] * vector[1] + rotation[1][2] * vector[2],
            rotation[2][0] * vector[0] + rotation[2][1] * vector[1] + rotation[2][2] * vector[2],
        ]
    }

    fn multiply_matrices(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
        let mut out = [[0.0; 3]; 3];
        for row in 0..3 {
            for col in 0..3 {
                out[row][col] =
                    a[row][0] * b[0][col] + a[row][1] * b[1][col] + a[row][2] * b[2][col];
            }
        }
        out
    }

    fn transpose_matrix(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
        [
            [m[0][0], m[1][0], m[2][0]],
            [m[0][1], m[1][1], m[2][1]],
            [m[0][2], m[1][2], m[2][2]],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_transform_point() {
        let xf = RigidTransform::identity();
        let point = [1.0, 2.0, 3.0];
        assert!(RigidTransform::points_near(
            xf.transform_point(point),
            point,
            POSITION_TOLERANCE_M
        ));
    }

    #[test]
    fn compose_associative() {
        let a = RigidTransform::from_translation([0.1, 0.0, 0.0]);
        let b = RigidTransform {
            translation_m: [0.0, 0.2, 0.0],
            rotation: [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        };
        let c = RigidTransform::from_translation([0.0, 0.0, 0.3]);
        let left = a.compose(b).compose(c);
        let right = a.compose(b.compose(c));
        assert!(left
            .translation_m
            .iter()
            .zip(right.translation_m)
            .all(|(l, r)| (l - r).abs() <= 1e-9));
        assert!(RigidTransform::matrices_near(
            &left.rotation,
            &right.rotation,
            1e-9
        ));
    }

    #[test]
    fn inverse_round_trip() {
        let xf = RigidTransform {
            translation_m: [0.05, -0.02, 0.01],
            rotation: [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        };
        let point = [0.12, -0.04, 0.08];
        let round_trip = xf
            .compose(xf.inverse().expect("inverse"))
            .transform_point(point);
        assert!(RigidTransform::points_near(round_trip, point, 1e-9));
    }

    #[test]
    fn translation_only_is_identity_rotation() {
        let xf = RigidTransform::from_translation([1.0, 2.0, 3.0]);
        assert!(RigidTransform::matrices_near(
            &xf.rotation,
            &RigidTransform::identity_rotation(),
            ROTATION_TOLERANCE
        ));
    }
}
