//! ADR-089: bounded read-only current native side-boundary certificates.
use opencad_core::{Length, OpenCadError};
use opencad_geometry::{
    CircularFilletContactKind, CircularFilletContactSnapshot,
    CircularFilletCylindricalContactSnapshot, CircularFilletRationalSnapshot,
    CircularFilletReference, CircularFilletSideBoundaryPrecision,
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
            && zero(self.origin[1])
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
    edge_poles_mm: [[f64; 3]; 3],
    edge_weights: [f64; 3],
    cylinder_origin_mm: [f64; 3],
    cylinder_directions: [[f64; 3]; 3],
    cylinder_radius_mm: f64,
    pcurve_poles_rad_mm: Vec<[f64; 2]>,
    pcurve_weights: Vec<f64>,
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
        .take(1_048_577)
        .read_to_end(&mut input)
        .map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.len() > 1_048_576 {
        return Err(OpenCadError::validation(
            "side boundary input exceeds 1 MiB",
        ));
    }
    let input: Input =
        serde_json::from_slice(&input).map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.format_version != 1 {
        return Err(OpenCadError::validation("unsupported side boundary format"));
    }
    let mut certificates = Vec::new();
    for request in input.requests {
        if !request.blend_line.canonical() {
            return Err(OpenCadError::validation("unsupported attached side line"));
        }
        let reference = CircularFilletReference::new(
            Length::from_meters(request.cylinder_radius_m),
            Length::from_meters(request.cylinder_height_m),
            Length::from_meters(request.minimum_radius_m),
            Length::from_meters(request.maximum_radius_m),
        )?;
        let edge = CircularFilletContactSnapshot::from_millimeter_controls(
            CircularFilletContactKind::Side,
            &request.edge_poles_mm,
            &request.edge_weights,
        )?;
        // Exact unit-weight REPRESENTATION gate, not geometric proximity.
        if request.pcurve_weights.len() != 769
            || request
                .pcurve_weights
                .iter()
                .any(|w| w.to_bits() != 1_f64.to_bits())
        {
            return Err(OpenCadError::validation(
                "unsupported cylindrical pcurve weights",
            ));
        }
        let cylinder = CircularFilletCylindricalContactSnapshot::from_millimeter_controls(
            request.cylinder_origin_mm,
            request.cylinder_directions,
            request.cylinder_radius_mm,
            &request.pcurve_poles_rad_mm,
        )?;
        let blend = CircularFilletRationalSnapshot::from_millimeter_controls(
            &request.blend_poles_mm,
            &request.blend_weights,
        )?;
        let b = reference.certify_side_boundary(
            request.chart,
            &edge,
            &cylinder,
            &blend,
            CircularFilletSideBoundaryPrecision {
                maximum_gap: Length::from_meters(request.maximum_gap_m),
                maximum_blend_normal_error: request.maximum_blend_normal_error,
                max_leaves: request.max_leaves,
            },
        )?;
        let n = b.blend_normal;
        certificates.push(json!({"request":request,
            "edge_position_error_upper_m":b.edge_position_error_upper_m,"cylindrical_lift_position_error_upper_m":b.cylindrical_lift.position_error_upper_m,
            "angle_error_upper_rad":b.cylindrical_lift.angle_error_upper_rad,"height_error_upper_m":b.cylindrical_lift.height_error_upper_m,"support_frame_error_upper_m":b.cylindrical_lift.support_frame_error_upper_m,
            "blend_position_error_upper_m":b.blend_position_error_upper_m,"edge_to_cylinder_gap_upper_m":b.edge_to_cylinder_gap_upper_m,
            "edge_to_blend_gap_upper_m":b.edge_to_blend_gap_upper_m,"cylinder_to_blend_gap_upper_m":b.cylinder_to_blend_gap_upper_m,
            "blend_normal_bounds":{"jacobian_area_lower_m2":n.jacobian_area_lower_m2,"jacobian_area_upper_m2":n.jacobian_area_upper_m2,
            "ideal_normal_alignment_lower":n.ideal_normal_alignment_lower,"normal_error_upper":n.normal_error_upper,
            "du_error_upper_m":n.du_error_upper_m,"dv_error_upper_m":n.dv_error_upper_m,"leaves":n.leaves}}));
    }
    // Emit no certificate stdout unless ALL eight proofs and line gates succeed.
    println!(
        "{}",
        json!({"format_version":1,"certificates":certificates,"native_floating_evaluation_certified":false,
        "end_contact_boundaries_certified":false,"whole_brep_certified":false})
    );
    Ok(())
}
