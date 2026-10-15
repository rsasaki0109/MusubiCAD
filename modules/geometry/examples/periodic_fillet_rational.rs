//! ADR-084: transient rational rolling-sphere charts, never a source feature.
use opencad_core::Length;
use opencad_geometry::{CircularFilletContactKind, CircularFilletReference};
use serde_json::json;
fn main() -> opencad_core::Result<()> {
    let mut cases = Vec::new();
    for (law, maximum) in [("constant", 0.0005), ("variable", 0.0015)] {
        let reference = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(maximum),
        )?;
        let mut patches = Vec::new();
        let mut contacts = Vec::new();
        let mut side_pcurves = Vec::new();
        for chart in 0..4 {
            let patch = reference.rational_patch(chart)?;
            let normal = patch.normal_bounds(1e-9, 4096)?;
            patches.push(json!({"chart":chart,"u_degree":patch.u_degree(),"v_degree":patch.v_degree(),"control_points_m":patch.control_points_m(),"weights":patch.weights(),"position_error_upper_m":patch.position_error_upper_m(),"normal_bounds":{
                "jacobian_area_lower_m2":normal.jacobian_area_lower_m2,"jacobian_area_upper_m2":normal.jacobian_area_upper_m2,
                "ideal_normal_alignment_lower":normal.ideal_normal_alignment_lower,"normal_error_upper":normal.normal_error_upper,
                "du_error_upper_m":normal.du_error_upper_m,"dv_error_upper_m":normal.dv_error_upper_m,"leaves":normal.leaves
            }}));
            let pc = reference.cylindrical_contact_pcurve(chart)?;
            side_pcurves.push(json!({"chart":chart,"divisions":pc.divisions(),"control_points_rad_m":pc.control_points_rad_m(),"angle_error_upper_rad":pc.angle_error_upper_rad(),"height_error_upper_m":pc.height_error_upper_m(),"position_error_upper_m":pc.position_error_upper_m()}));
        }
        for kind in [
            CircularFilletContactKind::Side,
            CircularFilletContactKind::End,
        ] {
            for chart in 0..4 {
                let curve = reference.rational_contact_curve(kind, chart)?;
                contacts.push(json!({"chart":chart,"kind":if kind==CircularFilletContactKind::Side {"side"} else {"end"},"degree":curve.degree(),"control_points_m":curve.control_points_m(),"weights":curve.weights(),"position_error_upper_m":curve.position_error_upper_m()}));
            }
        }
        let bounds = reference.volume_bounds_m3(1e-12, 262_144)?;
        cases.push(json!({"law":law,"minimum_radius_m":0.0005,"maximum_radius_m":maximum,"patches":patches,"contacts":contacts,"side_pcurves":side_pcurves,"volume_lower_m3":bounds.body_lower_m3,"volume_upper_m3":bounds.body_upper_m3,"constant_limit_volume_m3":reference.constant_limit_volume_m3().ok()}));
    }
    println!(
        "{}",
        json!({"format_version":1,"radius_m":0.01,"height_m":0.02,"normal_error_request":1e-9,"normal_max_leaves":4096,"cases":cases})
    );
    Ok(())
}
