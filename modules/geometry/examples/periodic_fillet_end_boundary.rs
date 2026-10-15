//! ADR-088: bounded read-only current native end-boundary certificates.
use opencad_core::{Length, OpenCadError};
use opencad_geometry::{
    CircularFilletContactKind, CircularFilletContactSnapshot, CircularFilletEndBoundaryPrecision,
    CircularFilletPlanarContactSnapshot, CircularFilletRationalSnapshot, CircularFilletReference,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Read;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Line {
    kind: String,
    origin: [f64; 2],
    direction: [f64; 2],
    range: [f64; 2],
}
impl Line {
    fn canonical(&self) -> bool {
        // Fixed parameter REPRESENTATION identity, not geometric proximity.
        // Signed zeros represent the same canonical line; echo retains the sign.
        let zero = |x: f64| x.to_bits() & 0x7fff_ffff_ffff_ffff == 0;
        let one = |x: f64| x.to_bits() == 1_f64.to_bits();
        self.kind == "geom2d_line"
            && zero(self.origin[0])
            && one(self.origin[1])
            && one(self.direction[0])
            && zero(self.direction[1])
            && zero(self.range[0])
            && one(self.range[1])
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    chart: u8,
    cylinder_radius_m: f64,
    cylinder_height_m: f64,
    minimum_radius_m: f64,
    maximum_radius_m: f64,
    edge_poles_mm: [[f64; 3]; 5],
    edge_weights: [f64; 5],
    plane_origin_mm: [f64; 3],
    plane_x_direction: [f64; 3],
    plane_y_direction: [f64; 3],
    pcurve_poles_mm: [[f64; 2]; 5],
    pcurve_weights: [f64; 5],
    blend_poles_mm: [[[f64; 3]; 3]; 9],
    blend_weights: [[f64; 3]; 9],
    blend_line: Line,
    maximum_gap_m: f64,
    maximum_blend_normal_error: f64,
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
            "end boundary input exceeds 256 KiB",
        ));
    }
    let input: Input =
        serde_json::from_slice(&input).map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.format_version != 1 {
        return Err(OpenCadError::validation("unsupported end boundary format"));
    }
    let mut certificates = Vec::new();
    for request in input.requests {
        if !request.blend_line.canonical() {
            return Err(OpenCadError::validation("unsupported attached end line"));
        }
        let reference = CircularFilletReference::new(
            Length::from_meters(request.cylinder_radius_m),
            Length::from_meters(request.cylinder_height_m),
            Length::from_meters(request.minimum_radius_m),
            Length::from_meters(request.maximum_radius_m),
        )?;
        let edge = CircularFilletContactSnapshot::from_millimeter_controls(
            CircularFilletContactKind::End,
            &request.edge_poles_mm,
            &request.edge_weights,
        )?;
        let plane = CircularFilletPlanarContactSnapshot::from_millimeter_controls(
            request.plane_origin_mm,
            request.plane_x_direction,
            request.plane_y_direction,
            &request.pcurve_poles_mm,
            &request.pcurve_weights,
        )?;
        let blend = CircularFilletRationalSnapshot::from_millimeter_controls(
            &request.blend_poles_mm,
            &request.blend_weights,
        )?;
        let b = reference.certify_end_boundary(
            request.chart,
            &edge,
            &plane,
            &blend,
            CircularFilletEndBoundaryPrecision {
                maximum_gap: Length::from_meters(request.maximum_gap_m),
                maximum_blend_normal_error: request.maximum_blend_normal_error,
                max_leaves: request.max_leaves,
            },
        )?;
        let n = b.blend_normal;
        certificates.push(json!({"request":request,
            "edge_position_error_upper_m":b.edge_position_error_upper_m,"plane_lift_position_error_upper_m":b.plane_lift_position_error_upper_m,
            "blend_position_error_upper_m":b.blend_position_error_upper_m,"edge_to_plane_gap_upper_m":b.edge_to_plane_gap_upper_m,
            "edge_to_blend_gap_upper_m":b.edge_to_blend_gap_upper_m,"plane_to_blend_gap_upper_m":b.plane_to_blend_gap_upper_m,
            "blend_normal_bounds":{"jacobian_area_lower_m2":n.jacobian_area_lower_m2,"jacobian_area_upper_m2":n.jacobian_area_upper_m2,
            "ideal_normal_alignment_lower":n.ideal_normal_alignment_lower,"normal_error_upper":n.normal_error_upper,
            "du_error_upper_m":n.du_error_upper_m,"dv_error_upper_m":n.dv_error_upper_m,"leaves":n.leaves}}));
    }
    // Emit no certificate stdout unless ALL eight proofs and line gates succeed.
    println!(
        "{}",
        json!({"format_version":1,"certificates":certificates,"native_floating_evaluation_certified":false,
        "side_contact_boundaries_certified":false,"whole_brep_certified":false})
    );
    Ok(())
}
