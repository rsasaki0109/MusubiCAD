//! Actual ADR-076 fixture consumed by the isolated pinned-native diagnostic.
use opencad_core::{Angle, Length};
use opencad_geometry::CircularFilletReference;
use serde_json::json;
use std::f64::consts::FRAC_PI_2;

fn main() -> opencad_core::Result<()> {
    let mut cases = Vec::new();
    for constant in [true, false] {
        let reference = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(if constant { 0.0005 } else { 0.0015 }),
        )?;
        for chart in 0..4 {
            for (region, v0, v1) in [
                ("side", 0., 0.005),
                ("interior", 0.3, 0.305),
                ("top", 0.995, 1.),
                ("seam-start", 0.3, 0.305),
                ("seam-end", 0.3, 0.305),
            ] {
                let (u0, u1) = match region {
                    "seam-start" => (0_f64, 0.005_f64),
                    "seam-end" => (0.995_f64, 1_f64),
                    _ => (0.25_f64, 0.255_f64),
                };
                let patch = reference.bezier_patch(chart, u0, u1, v0, v1)?;
                let normals = patch.normal_bounds(reference, chart, u0, u1, v0, v1)?;
                let mut samples = Vec::new();
                for u in [0_f64, 0.25, 0.5, 0.75, 1.] {
                    for v in [0., 0.25, 0.5, 0.75, 1.] {
                        let s = reference.sample(
                            Angle::from_radians(
                                f64::from(chart) * FRAC_PI_2 + 2. * (u0 + (u1 - u0) * u).atan(),
                            ),
                            Angle::from_radians(2. * (v0 + (v1 - v0) * v).atan()),
                        )?;
                        samples.push(json!({"u":u,"v":v,"reference_point_m":s.point_m,"reference_normal":s.normal}));
                    }
                }
                cases.push(json!({"name":format!("{}-{chart}-{region}",if constant {"constant"} else {"variable"}),
                    "control_points_m":patch.control_points_m,"position_error_upper_m":patch.position_error_upper_m,
                    "normal_lower":normals.normal_lower,"normal_upper":normals.normal_upper,
                    "jacobian_area_lower_m2":normals.jacobian_area_lower_m2,"jacobian_area_upper_m2":normals.jacobian_area_upper_m2,
                    "ideal_normal_alignment_lower":normals.ideal_normal_alignment_lower,"normal_error_upper":normals.normal_error_upper,
                    "reference_to_bilinear_error_upper_m":patch.reference_to_bilinear_error_upper_m,
                    "control_net_deviation_upper_m":patch.control_net_deviation_upper_m,"samples":samples}));
            }
        }
    }
    println!("{}", json!({"format_version":1,"cases":cases}));
    Ok(())
}
