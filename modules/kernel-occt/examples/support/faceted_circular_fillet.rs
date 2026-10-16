//! Numerical runtime demonstration, never an authored document feature.
use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::*;
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::{json, Value};
use std::collections::BTreeSet;
fn request(edge: u64, variable: bool, placement: RigidTransform) -> FacetedCircularFilletInput {
    FacetedCircularFilletInput {
        end_edge_id: edge,
        origin_m: placement.translation_m,
        phase_x: placement.rotation.map(|r| r[0]),
        axis: placement.rotation.map(|r| r[2]),
        cylinder_radius: Length::from_meters(0.01),
        cylinder_height: Length::from_meters(0.02),
        minimum_radius: Length::from_meters(0.0005),
        maximum_radius: Length::from_meters(if variable { 0.0015 } else { 0.0005 }),
        mesh_precision: CircularFilletMeshPrecision {
            maximum_position_error: Length::from_meters(4e-4),
            maximum_normal_error: 0.8,
            max_vertices: 33026,
            max_triangles: 66048,
        },
        grid_precision: CircularFilletGridPrecision {
            maximum_position_error: Length::from_meters(4e-4),
            maximum_normal_error: 0.8,
        },
        volume_precision: CircularFilletGridVolumePrecision {
            maximum_reference_interval_width_m3: 1e-11,
            maximum_absolute_difference_m3: 1e-7,
            max_cells: 262144,
        },
        area_precision: CircularFilletGridAreaPrecision {
            maximum_reference_interval_width_m2: 1e-7,
            maximum_absolute_difference_m2: 1e-5,
            max_cells: 262144,
        },
        moment_precision: CircularFilletGridMomentPrecision {
            ideal: CircularFilletMomentPrecision {
                maximum_volume_interval_width_m3: 1e-11,
                maximum_first_moment_interval_width_m4: 1e-12,
                maximum_centroid_interval_width: Length::from_meters(1e-6),
                max_cells: 262144,
            },
            maximum_absolute_first_moment_difference_m4: 1e-9,
            maximum_absolute_centroid_difference: Length::from_meters(2e-6),
        },
    }
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}
pub fn report() -> Result<Value> {
    let mut cases = Vec::new();
    for (law, variable, placement) in [
        ("constant", false, RigidTransform::identity()),
        (
            "variable",
            true,
            RigidTransform {
                rotation: [[0., -1., 0.], [0.6, 0., -0.8], [0.8, 0., 0.6]],
                translation_m: [0.05, -0.03, 0.08],
            },
        ),
    ] {
        let mock = MockGeometryKernel::with_analytic_primitives();
        let native = OcctGeometryKernel::new();
        let mut bodies = Vec::new();
        let mut canonical = None;
        for (name, k) in [
            ("analytic_mock", &mock as &dyn GeometryKernel),
            ("occt", &native),
        ] {
            let profile = SolvedSketch {
                profile_ref: "stock".into(),
                points: vec![],
                closed: true,
                circle: Some(SolvedCircle {
                    center_m: [0.; 2],
                    radius_m: 0.01,
                }),
                ellipse: None,
                cubic_bezier: None,
                segments: vec![],
                placement: None,
            };
            let base = k.extrude(
                k.make_wire_from_sketch(&profile)?,
                ExtrudeExtent::Distance {
                    length: Length::from_meters(0.02),
                },
                ExtrudeOperation::NewBody,
                None,
                [0., 0., 1.],
            )?;
            let stock = k.transform_body(base, placement)?;
            let before = k.mass_properties(&stock, 1000.)?;
            let rims = k.cylindrical_stock_rims(&stock, placement.rotation.map(|r| r[2]))?;
            let input = request(rims.end_edge_id, variable, placement);
            let plan = input.prepare(&rims)?;
            let volume = plan.volume_comparison();
            let area = plan.area_comparison();
            let moments = plan.moment_comparison();
            canonical = Some(json!({
                "position_error_upper_m":plan.grid().position_error_upper_m(),"normal_error_upper":plan.grid().normal_error_upper(),
                "volume_m3":volume.grid_signed_volume_flux_m3(),"area_m2":area.grid_unsigned_area_m2(),
                "first_moments_m4":moments.grid_signed_first_moments_m4(),"centroid_m":moments.grid_centroid_quotient_m(),
                "ideal_volume_m3":moments.ideal().volume_m3,"ideal_first_moments_m4":moments.ideal().first_moments_m4,"ideal_centroid_m":moments.ideal().centroid_m,
                "volume_error_upper_m3":volume.absolute_difference_upper_m3(),"area_error_upper_m2":area.absolute_difference_upper_m2(),
                "first_moment_error_upper_m4":moments.absolute_first_moment_difference_upper_m4(),"centroid_error_upper_m":moments.absolute_centroid_difference_upper_m(),
                "volume_cells":moments.ideal().volume_cells,"moment_cells":moments.ideal().moment_cells,
                "native_conversion_certified":false,"placed_conversion_certified":false,"f32_conversion_certified":false,
            }));
            let out = k.grid_faceted_periodic_cosine(stock.clone(), &input)?;
            let mass = k.mass_properties(&out, 1000.)?;
            let mesh = k.tessellate(&out, &TessellationSettings::default())?;
            let max = if variable { 0.0015 } else { 0.0005 };
            let expected = [
                [0.01 - max, 0., 0.02],
                [-0.0095, 0., 0.02],
                [0.01, 0., 0.02 - max],
                [-0.01, 0., 0.0195],
            ]
            .map(|p| placement.transform_point(p));
            let mut measured = Vec::new();
            let mut maximum_error = 0_f64;
            for point in expected {
                let nearest = mesh
                    .positions
                    .iter()
                    .map(|p| p.map(f64::from))
                    .min_by(|a, b| distance(*a, point).total_cmp(&distance(*b, point)))
                    .ok_or_else(|| OpenCadError::validation("empty faceted contact mesh"))?;
                maximum_error = maximum_error.max(distance(nearest, point));
                measured.push(nearest);
            }
            let after = k.mass_properties(&stock, 1000.)?;
            if maximum_error > 1e-8
                || (after.volume_m3 - before.volume_m3).abs() > 1e-18
                || k.cylindrical_stock_rims(&stock, input.axis)? != rims
            {
                return Err(OpenCadError::validation(
                    "physical faceted contact or stock preservation check failed",
                ));
            }
            bodies.push(json!({"kernel":name,"stock_volume_m3":before.volume_m3,"volume_m3":mass.volume_m3,"area_m2":mass.area_m2,"center_of_mass_m":mass.center_of_mass,"mass_kg":mass.mass_kg,"bounds_m":k.bounding_box(&out)?,"triangles":mesh.triangle_count(),"owned_facet_count":mesh.triangle_face_ids.iter().collect::<BTreeSet<_>>().len(),"contacts_expected_world_m":expected,"contacts_measured_world_m":measured,"maximum_contact_vertex_error_m":maximum_error,"stock_preserved":true}));
        }
        cases.push(json!({"law":law,"origin_m":placement.translation_m,"phase_x":placement.rotation.map(|r|r[0]),"axis":placement.rotation.map(|r|r[2]),"rotation":placement.rotation,"cylinder_radius_m":0.01,"cylinder_height_m":0.02,"minimum_radius_m":0.0005,"maximum_radius_m":if variable {0.0015} else {0.0005},"canonical_input_grid":canonical,"bodies":bodies}));
    }
    Ok(
        json!({"representation":"grid_faceted_periodic_cosine","runtime_operation_executed":true,"source_feature_created":false,"native_conversion_certified":false,"exact_global_embedding_certified":false,"maximum_input_position_error_m":4e-4,"maximum_input_normal_error":0.8,"maximum_volume_error_m3":1e-7,"maximum_area_error_m2":1e-5,"maximum_first_moment_error_m4":1e-9,"maximum_centroid_error_m":2e-6,"physical_contact_vertex_numerical_allowance_m":1e-8,"cases":cases}),
    )
}
