//! ADR-090: bounded read-only current native quarter-seam certificates.
use opencad_core::{Length, OpenCadError};
use opencad_geometry::{CircularFilletRationalSnapshot, CircularFilletSeamSnapshot};
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
    fn canonical(&self, end: bool) -> bool {
        // Exact representation identity; zero sign remains preserved in echo.
        let zero = |x: f64| x.to_bits() & 0x7fff_ffff_ffff_ffff == 0;
        let one = |x: f64| x.to_bits() == 1_f64.to_bits();
        self.kind == "geom2d_line"
            && (if end {
                one(self.origin[0])
            } else {
                zero(self.origin[0])
            })
            && zero(self.origin[1])
            && zero(self.direction[0])
            && one(self.direction[1])
            && zero(self.range[0])
            && one(self.range[1])
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    left_chart: u8,
    right_chart: u8,
    edge_poles_mm: [[f64; 3]; 3],
    edge_weights: [f64; 3],
    left_poles_mm: [[[f64; 3]; 3]; 9],
    left_weights: [[f64; 3]; 9],
    right_poles_mm: [[[f64; 3]; 3]; 9],
    right_weights: [[f64; 3]; 9],
    left_line: Line,
    right_line: Line,
    maximum_gap_m: f64,
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
            "seam boundary input exceeds 256 KiB",
        ));
    }
    let input: Input =
        serde_json::from_slice(&input).map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.format_version != 1 {
        return Err(OpenCadError::validation("unsupported seam boundary format"));
    }
    let mut certificates = Vec::new();
    for request in input.requests {
        if request.left_chart > 3
            || request.right_chart != (request.left_chart + 1) % 4
            || !request.left_line.canonical(true)
            || !request.right_line.canonical(false)
        {
            return Err(OpenCadError::validation(
                "unsupported attached seam charts/lines",
            ));
        }
        let edge = CircularFilletSeamSnapshot::from_millimeter_controls(
            &request.edge_poles_mm,
            &request.edge_weights,
        )?;
        let left = CircularFilletRationalSnapshot::from_millimeter_controls(
            &request.left_poles_mm,
            &request.left_weights,
        )?;
        let right = CircularFilletRationalSnapshot::from_millimeter_controls(
            &request.right_poles_mm,
            &request.right_weights,
        )?;
        let b = edge.certify_boundary(&left, &right, Length::from_meters(request.maximum_gap_m))?;
        certificates.push(json!({"request":request,
            "edge_to_left_gap_upper_m":b.edge_to_left_gap_upper_m,
            "edge_to_right_gap_upper_m":b.edge_to_right_gap_upper_m,
            "left_to_right_gap_upper_m":b.left_to_right_gap_upper_m}));
    }
    // Emit no certificate stdout unless ALL eight proofs and line gates succeed.
    println!(
        "{}",
        json!({"format_version":1,"certificates":certificates,"native_floating_evaluation_certified":false,
        "paired_normals_certified":false,"whole_brep_certified":false})
    );
    Ok(())
}
