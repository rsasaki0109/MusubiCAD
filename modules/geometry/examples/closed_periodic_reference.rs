//! ADR-079 diagnostic fixtures, not product solids or backend mass queries.
use opencad_core::Length;
use opencad_geometry::CircularFilletReference;
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
        let volume = reference.volume_bounds_m3(1e-12, 262_144)?;
        for (level, requested_position_m) in [("coarse", 1e-4), ("fine", 2e-5)] {
            let band =
                reference.bezier_band(Length::from_meters(requested_position_m), 0.8, 4096)?;
            let integrals = band.closed_diagnostic_integrals()?;
            let area = band.curved_diagnostic_area_bounds(1e-7, 32768)?;
            let cells: Vec<_> = band.cells.iter().map(|c|json!({"chart":c.chart,"chart_domain":c.chart_domain,"control_points_m":c.patch.control_points_m})).collect();
            cases.push(json!({"name":format!("{law}-{level}"),"law":law,"requested_position_error_m":requested_position_m,
                "divisions":band.divisions_per_chart_axis,"position_error_upper_m":band.position_error_upper_m,
                "normal_error_upper":band.normal_error_upper,"cells":cells,
                "curved_area_bounds":{"lower_m2":area.curved_lower_m2,"upper_m2":area.curved_upper_m2,"leaves":area.leaves,"absolute_width_m2":1e-7,"max_leaves":32768},
                "boundary_integrals":{"volume_m3":integrals.volume_m3,"first_moments_m4":integrals.first_moments_m4},
                "ideal":{"minimum_radius_m":0.0005,"maximum_radius_m":maximum,"body_lower_m3":volume.body_lower_m3,
                    "body_upper_m3":volume.body_upper_m3,"cells":volume.cells,"absolute_width_m3":1e-12}}));
        }
    }
    println!(
        "{}",
        json!({"format_version":1,"radius_m":0.01,"height_m":0.02,"cases":cases})
    );
    Ok(())
}
