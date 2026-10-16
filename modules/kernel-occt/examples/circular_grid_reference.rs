//! ADR-094: real faceted reference bodies through the ordinary kernel API.
use opencad_core::Length;
use opencad_geometry::{
    CircularFilletGridPrecision, CircularFilletGridVolumePrecision, CircularFilletMeshPrecision,
    CircularFilletReference, GeometryKernel, MockGeometryKernel, TessellationSettings,
};
use opencad_kernel_occt::OcctGeometryKernel;
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
        let g = r
            .closed_reference_mesh(CircularFilletMeshPrecision {
                maximum_position_error: Length::from_meters(4e-4),
                maximum_normal_error: 0.8,
                max_vertices: 33026,
                max_triangles: 66048,
            })?
            .grid_solid(CircularFilletGridPrecision {
                maximum_position_error: Length::from_meters(4e-4),
                maximum_normal_error: 0.8,
            })?;
        let comparison = g.volume_comparison(CircularFilletGridVolumePrecision {
            maximum_reference_interval_width_m3: 1e-12,
            maximum_absolute_difference_m3: 1e-7,
            max_cells: 262144,
        })?;
        let mock = MockGeometryKernel::with_analytic_primitives();
        let native = OcctGeometryKernel::new();
        let mut bodies = Vec::new();
        for (name, kernel) in [
            ("analytic_mock", &mock as &dyn GeometryKernel),
            ("occt", &native),
        ] {
            let body = kernel.import_circular_fillet_grid(&g)?;
            let mass = kernel.mass_properties(&body, 1000.)?;
            let mesh = kernel.tessellate(&body, &TessellationSettings::default())?;
            bodies.push(json!({"kernel":name,"volume_m3":mass.volume_m3,"area_m2":mass.area_m2,"mass_kg":mass.mass_kg,"center_of_mass_m":mass.center_of_mass,"bounds_m":kernel.bounding_box(&body)?,"triangles":mesh.triangle_count(),"owned_facet_count":mesh.triangle_face_ids.iter().collect::<std::collections::BTreeSet<_>>().len()}));
        }
        cases.push(json!({"law":law,"input_position_error_upper_m":g.position_error_upper_m(),"input_normal_error_upper":g.normal_error_upper(),"bodies":bodies,"volume_comparison":{"grid_signed_volume_flux_m3":comparison.grid_signed_volume_flux_m3(),"ideal_volume_m3":comparison.ideal_volume_m3(),"signed_difference_m3":comparison.signed_difference_m3(),"absolute_difference_upper_m3":comparison.absolute_difference_upper_m3(),"relative_difference_upper":comparison.relative_difference_upper(),"reference_cells":comparison.reference_cells(),"native_conversion_certified":false,"exact_global_embedding_certified":false}}));
    }
    println!(
        "{}",
        json!({"format_version":1,"physical_reference_bodies_created":true,"authored_fillet_created":false,"native_conversion_certified":false,"cases":cases})
    );
    Ok(())
}
