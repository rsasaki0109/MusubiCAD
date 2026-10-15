//! ADR-092: generate closed reference shells and their actual flux intervals.
use opencad_core::Length;
use opencad_geometry::{CircularFilletMeshPrecision as Precision, CircularFilletReference};
use serde_json::json;
fn main() -> opencad_core::Result<()> {
    let mut cases = Vec::new();
    for (law, maximum) in [("constant", 0.0005), ("variable", 0.0015)] {
        let r = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(0.0005),
            Length::from_meters(maximum),
        )?;
        let volume = r.volume_bounds_m3(1e-12, 262144)?;
        let mut meshes = Vec::new();
        for (quality, position, normal) in [("coarse", 4e-4, 0.8), ("fine", 1e-4, 0.5)] {
            let m = r.closed_reference_mesh(Precision {
                maximum_position_error: Length::from_meters(position),
                maximum_normal_error: normal,
                max_vertices: 33026,
                max_triangles: 66048,
            })?;
            let integrals = m.boundary_integrals();
            meshes.push(json!({"quality":quality,"requested_position_error_m":position,"requested_normal_error":normal,"max_vertices":33026,"max_triangles":66048,
                "divisions":m.divisions_per_chart_axis(),"vertices_m":m.vertices_m(),"triangles":m.triangles(),"face_groups":m.triangle_face_groups(),
                "position_error_upper_m":m.position_error_upper_m(),"normal_error_upper":m.normal_error_upper(),
                "boundary_integrals":{"volume_m3":integrals.volume_m3,"area_m2":integrals.area_m2,"first_moments_m4":integrals.first_moments_m4}}));
        }
        cases.push(json!({"law":law,"minimum_radius_m":0.0005,"maximum_radius_m":maximum,"ideal_volume_m3":[volume.body_lower_m3,volume.body_upper_m3],"meshes":meshes}));
    }
    println!(
        "{}",
        json!({"format_version":1,"radius_m":0.01,"height_m":0.02,"cases":cases,"global_embedding_certified":false,"native_floating_evaluation_certified":false,"product_body_created":false})
    );
    Ok(())
}
