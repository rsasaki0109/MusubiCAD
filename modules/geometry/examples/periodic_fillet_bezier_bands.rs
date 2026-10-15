//! Actual full-period reference covers consumed by the native sewing diagnostic.
use opencad_core::Length;
use opencad_geometry::CircularFilletReference;
use serde_json::json;
fn main() -> opencad_core::Result<()> {
    let mut cases = Vec::new();
    for constant in [true, false] {
        let reference = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(if constant { 0.0005 } else { 0.0015 }),
        )?;
        let band = reference.bezier_band(Length::from_meters(1e-4), 0.8, 4096)?;
        let cells:Vec<_>=band.cells.iter().map(|c|json!({"chart":c.chart,"chart_domain":c.chart_domain,"control_points_m":c.patch.control_points_m,
            "position_error_upper_m":c.patch.position_error_upper_m,"normal_error_upper":c.normals.normal_error_upper})).collect();
        cases.push(json!({"name":if constant {"constant"} else {"variable"},"divisions":band.divisions_per_chart_axis,
            "position_error_upper_m":band.position_error_upper_m,"normal_error_upper":band.normal_error_upper,"cells":cells}));
    }
    println!(
        "{}",
        json!({"format_version":1,"requested_position_error_m":1e-4,"requested_normal_error":0.8,"max_patches":4096,"cases":cases})
    );
    Ok(())
}
