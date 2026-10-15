//! ADR-086: explicit bounded I/O for read-only native definition certificates.
use opencad_core::{Length, OpenCadError};
use opencad_geometry::{CircularFilletRationalSnapshot, CircularFilletReference};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Read;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    chart: u8,
    cylinder_radius_m: f64,
    cylinder_height_m: f64,
    minimum_radius_m: f64,
    maximum_radius_m: f64,
    poles_mm: [[[f64; 3]; 3]; 9],
    weights: [[f64; 3]; 9],
    maximum_position_error_m: f64,
    maximum_normal_error: f64,
    max_leaves: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    format_version: u8,
    requests: [Request; 8],
}
fn main() -> opencad_core::Result<()> {
    let mut input = Vec::new();
    std::io::stdin()
        .take(262_145)
        .read_to_end(&mut input)
        .map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.len() > 262_144 {
        return Err(OpenCadError::validation(
            "snapshot diagnostic input exceeds 256 KiB",
        ));
    }
    let input: Input =
        serde_json::from_slice(&input).map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.format_version != 1 {
        return Err(OpenCadError::validation(
            "unsupported snapshot diagnostic format",
        ));
    }
    let mut certificates = Vec::new();
    for request in input.requests {
        let reference = CircularFilletReference::new(
            Length::from_meters(request.cylinder_radius_m),
            Length::from_meters(request.cylinder_height_m),
            Length::from_meters(request.minimum_radius_m),
            Length::from_meters(request.maximum_radius_m),
        )?;
        let snapshot = CircularFilletRationalSnapshot::from_millimeter_controls(
            &request.poles_mm,
            &request.weights,
        )?;
        let bounds = reference.certify_rational_snapshot(
            request.chart,
            &snapshot,
            Length::from_meters(request.maximum_position_error_m),
            request.maximum_normal_error,
            request.max_leaves,
        )?;
        let b = bounds.normal;
        certificates.push(json!({"request":request,"position_error_upper_m":bounds.position_error_upper_m,"normal_bounds":{
            "jacobian_area_lower_m2":b.jacobian_area_lower_m2,"jacobian_area_upper_m2":b.jacobian_area_upper_m2,
            "ideal_normal_alignment_lower":b.ideal_normal_alignment_lower,"normal_error_upper":b.normal_error_upper,
            "du_error_upper_m":b.du_error_upper_m,"dv_error_upper_m":b.dv_error_upper_m,"leaves":b.leaves
        }}));
    }
    // Emit only after ALL eight complete proofs succeeded.
    println!(
        "{}",
        json!({"format_version":1,"certificates":certificates,"native_floating_evaluation_certified":false,"whole_brep_certified":false})
    );
    Ok(())
}
