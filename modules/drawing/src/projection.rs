//! Orthographic projection kinds for drawing views.

use serde::{Deserialize, Serialize};

/// Standard orthographic projections for MVP wireframe export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionKind {
    #[default]
    Front,
    Top,
    Right,
    Isometric,
}

impl ProjectionKind {
    pub fn project_point(self, point_m: [f64; 3]) -> [f64; 2] {
        match self {
            Self::Front => [point_m[0], point_m[1]],
            Self::Top => [point_m[0], point_m[2]],
            Self::Right => [point_m[1], point_m[2]],
            Self::Isometric => [
                point_m[0] - 0.5 * point_m[2],
                point_m[1] + 0.35 * point_m[2],
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_projection_drops_depth_axis() {
        let projected = ProjectionKind::Front.project_point([1.0, 2.0, 3.0]);
        assert!((projected[0] - 1.0).abs() < 1e-9);
        assert!((projected[1] - 2.0).abs() < 1e-9);
    }
}
