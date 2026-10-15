//! ADR-087: bounded read-only proof I/O for sixteen current native 3D contacts.
use opencad_core::{Length, OpenCadError};
use opencad_geometry::{
    CircularFilletContactKind, CircularFilletContactSnapshot, CircularFilletReference,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Read;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Side,
    End,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    kind: Kind,
    chart: u8,
    cylinder_radius_m: f64,
    cylinder_height_m: f64,
    minimum_radius_m: f64,
    maximum_radius_m: f64,
    poles_mm: Vec<[f64; 3]>,
    weights: Vec<f64>,
    maximum_position_error_m: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    format_version: u8,
    requests: [Request; 16],
}
fn main() -> opencad_core::Result<()> {
    let mut input = Vec::new();
    std::io::stdin()
        .take(65_537)
        .read_to_end(&mut input)
        .map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.len() > 65_536 {
        return Err(OpenCadError::validation(
            "contact snapshot input exceeds 64 KiB",
        ));
    }
    let input: Input =
        serde_json::from_slice(&input).map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.format_version != 1 {
        return Err(OpenCadError::validation(
            "unsupported contact snapshot diagnostic format",
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
        let kind = match request.kind {
            Kind::Side => CircularFilletContactKind::Side,
            Kind::End => CircularFilletContactKind::End,
        };
        let snapshot = CircularFilletContactSnapshot::from_millimeter_controls(
            kind,
            &request.poles_mm,
            &request.weights,
        )?;
        let bounds = reference.certify_contact_snapshot(
            request.chart,
            &snapshot,
            Length::from_meters(request.maximum_position_error_m),
        )?;
        certificates.push(
            json!({"request":request,"position_error_upper_m":bounds.position_error_upper_m}),
        );
    }
    // No partial stdout if even the final request refuses.
    println!(
        "{}",
        json!({"format_version":1,"certificates":certificates,
            "native_floating_evaluation_certified":false,"attached_pcurve_lifts_certified":false,
            "whole_brep_certified":false})
    );
    Ok(())
}
