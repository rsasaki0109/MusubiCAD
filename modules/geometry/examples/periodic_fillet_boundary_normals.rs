//! ADR-091: bounded read-only paired-normal case proof, no native body/editor.
use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::{
    CircularFilletBoundaryNormalPrecision as Precision,
    CircularFilletCylindricalContactSnapshot as Cylinder,
    CircularFilletRationalSnapshot as Surface, CircularFilletReference,
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
    fn canonical(&self, origin: [f64; 2], direction: [f64; 2]) -> bool {
        // Fixed parameter REPRESENTATION identity; preserve zero sign in echo.
        let same = |a: f64, b: f64| {
            if b.to_bits() == 0 {
                a.to_bits() & 0x7fff_ffff_ffff_ffff == 0
            } else {
                a.to_bits() == b.to_bits()
            }
        };
        self.kind == "geom2d_line"
            && self.origin.into_iter().zip(origin).all(|(a, b)| same(a, b))
            && self
                .direction
                .into_iter()
                .zip(direction)
                .all(|(a, b)| same(a, b))
            && same(self.range[0], 0.)
            && same(self.range[1], 1.)
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Blend {
    chart: u8,
    poles_mm: [[[f64; 3]; 3]; 9],
    weights: [[f64; 3]; 9],
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cylindrical {
    chart: u8,
    origin_mm: [f64; 3],
    directions: [[f64; 3]; 3],
    radius_mm: f64,
    poles_rad_mm: Vec<[f64; 2]>,
    weights: Vec<f64>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Seam {
    left_chart: u8,
    right_chart: u8,
    left_line: Line,
    right_line: Line,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    cylinder_radius_m: f64,
    cylinder_height_m: f64,
    minimum_radius_m: f64,
    maximum_radius_m: f64,
    blends: [Blend; 4],
    cylinders: [Cylindrical; 4],
    plane_axes: [[f64; 3]; 2],
    side_lines: [Line; 4],
    end_lines: [Line; 4],
    seams: [Seam; 4],
    maximum_position_error_m: f64,
    maximum_surface_normal_error: f64,
    maximum_normal_gap: f64,
    max_leaves: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    format_version: u8,
    requests: [Request; 2],
}
fn invalid() -> OpenCadError {
    OpenCadError::validation("unsupported current boundary normal request")
}
fn main() -> Result<()> {
    let mut raw = Vec::new();
    std::io::stdin()
        .take(1_048_577)
        .read_to_end(&mut raw)
        .map_err(|e| OpenCadError::validation(e.to_string()))?;
    if raw.len() > 1_048_576 {
        return Err(invalid());
    }
    let input: Input =
        serde_json::from_slice(&raw).map_err(|e| OpenCadError::validation(e.to_string()))?;
    if input.format_version != 1 {
        return Err(invalid());
    }
    let mut certificates = Vec::new();
    for request in input.requests {
        for (c, (blend, cyl)) in request.blends.iter().zip(&request.cylinders).enumerate() {
            let seam = &request.seams[c];
            if blend.chart != c as u8
                || cyl.chart != c as u8
                || cyl.weights.len() != 769
                || cyl.weights.iter().any(|x| x.to_bits() != 1_f64.to_bits())
                || !request.side_lines[c].canonical([0., 0.], [1., 0.])
                || !request.end_lines[c].canonical([0., 1.], [1., 0.])
                || seam.left_chart != c as u8
                || seam.right_chart != ((c + 1) % 4) as u8
                || !seam.left_line.canonical([1., 0.], [0., 1.])
                || !seam.right_line.canonical([0., 0.], [0., 1.])
            {
                return Err(invalid());
            }
        }
        let reference = CircularFilletReference::new(
            Length::from_meters(request.cylinder_radius_m),
            Length::from_meters(request.cylinder_height_m),
            Length::from_meters(request.minimum_radius_m),
            Length::from_meters(request.maximum_radius_m),
        )?;
        let surfaces: [Surface; 4] = request
            .blends
            .iter()
            .map(|b| Surface::from_millimeter_controls(&b.poles_mm, &b.weights))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid())?;
        let cylinders: [Cylinder; 4] = request
            .cylinders
            .iter()
            .map(|c| {
                Cylinder::from_millimeter_controls(
                    c.origin_mm,
                    c.directions,
                    c.radius_mm,
                    &c.poles_rad_mm,
                )
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid())?;
        let b = reference.certify_boundary_normals(
            &surfaces,
            &cylinders,
            request.plane_axes,
            Precision {
                maximum_position_error: Length::from_meters(request.maximum_position_error_m),
                maximum_surface_normal_error: request.maximum_surface_normal_error,
                maximum_normal_gap: request.maximum_normal_gap,
                max_leaves: request.max_leaves,
            },
        )?;
        let surfaces=b.surfaces.map(|s| {let n=s.normal; json!({"position_error_upper_m":s.position_error_upper_m,
            "normal_bounds":{"jacobian_area_lower_m2":n.jacobian_area_lower_m2,"jacobian_area_upper_m2":n.jacobian_area_upper_m2,
            "ideal_normal_alignment_lower":n.ideal_normal_alignment_lower,"normal_error_upper":n.normal_error_upper,
            "du_error_upper_m":n.du_error_upper_m,"dv_error_upper_m":n.dv_error_upper_m,"leaves":n.leaves}})});
        let cylinders=b.cylindrical_lifts.map(|c|json!({"position_error_upper_m":c.position_error_upper_m,"angle_error_upper_rad":c.angle_error_upper_rad,
            "height_error_upper_m":c.height_error_upper_m,"support_frame_error_upper_m":c.support_frame_error_upper_m}));
        certificates.push(json!({"request":request,"surfaces":surfaces,"cylindrical_lifts":cylinders,
            "plane_frame_normal_error_upper":b.plane_frame_normal_error_upper,"cylinder_frame_normal_error_upper":b.cylinder_frame_normal_error_upper,
            "cylinder_normal_error_upper":b.cylinder_normal_error_upper,"side_normal_gap_upper":b.side_normal_gap_upper,
            "end_normal_gap_upper":b.end_normal_gap_upper,"seam_normal_gap_upper":b.seam_normal_gap_upper}));
    }
    // No partial stdout even if the LAST case refuses after the first succeeds.
    println!(
        "{}",
        json!({"format_version":1,"certificates":certificates,"native_floating_evaluation_certified":false,
        "exact_g1_certified":false,"whole_brep_certified":false})
    );
    Ok(())
}
