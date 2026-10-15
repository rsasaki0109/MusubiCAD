//! ADR-082: fixed support contacts, not a source feature or editable NURBS.
use opencad_core::Length;
use opencad_geometry::{CircularFilletContactKind, CircularFilletReference};
use serde_json::json;
fn main() -> opencad_core::Result<()> {
    let mut curves = Vec::new();
    for (law, maximum) in [("constant", 0.0005), ("variable", 0.0015)] {
        let reference = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(maximum),
        )?;
        for kind in [
            CircularFilletContactKind::Side,
            CircularFilletContactKind::End,
        ] {
            for chart in 0..4 {
                let curve = reference.rational_contact_curve(kind, chart)?;
                curves.push(json!({"law":law,"kind":if kind==CircularFilletContactKind::Side {"side"} else {"end"},"chart":chart,"degree":curve.degree(),"control_points_m":curve.control_points_m(),"weights":curve.weights(),"position_error_upper_m":curve.position_error_upper_m(),"minimum_radius_m":0.0005,"maximum_radius_m":maximum}));
            }
        }
    }
    println!(
        "{}",
        json!({"format_version":1,"radius_m":0.01,"height_m":0.02,"curves":curves})
    );
    Ok(())
}
