//! Developer kernel example; does not author or mutate a Design Graph document.
use opencad_core::{Length, Result};
use opencad_geometry::{
    ExtrudeExtent, ExtrudeOperation, FacePick, GeometryKernel, MockGeometryKernel, SketchPlacement,
    SolvedCircle, SolvedSketch, TessellationSettings,
};

fn main() -> Result<()> {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let stock = SolvedSketch {
        profile_ref: "stock".into(),
        points: vec![[0., 0.], [0.040, 0.], [0.040, 0.030], [0., 0.030]],
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    };
    let wire = kernel.make_wire_from_sketch(&stock)?;
    let mut body = kernel.extrude(
        wire,
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.010),
        },
        ExtrudeOperation::NewBody,
        None,
        [0., 0., 1.],
    )?;
    println!(
        "entry_witness: {:?}",
        kernel.rectangular_stock_geometry(
            &body,
            FacePick {
                point_m: [0.012, 0.009, 0.010],
                normal: [0., 0., 1.]
            }
        )?
    );
    for (radius_m, depth_m) in [(0.0033, 0.010), (0.0055, 0.004)] {
        let profile = SolvedSketch {
            profile_ref: "cylindrical-tool".into(),
            points: Vec::new(),
            circle: Some(SolvedCircle {
                center_m: [0.012, 0.009],
                radius_m,
            }),
            placement: Some(SketchPlacement {
                origin_m: [0., 0., 0.010],
                ..SketchPlacement::global_xy()
            }),
            ..stock.clone()
        };
        body = kernel.extrude(
            kernel.make_wire_from_sketch(&profile)?,
            ExtrudeExtent::Distance {
                length: Length::from_meters(depth_m),
            },
            ExtrudeOperation::Cut,
            Some(body),
            [0., 0., -1.],
        )?;
    }
    let mass = kernel.mass_properties(&body, 2700.)?;
    println!("volume_m3: {:.15}", mass.volume_m3);
    println!("area_m2: {:.15}", mass.area_m2);
    println!("mass_kg: {:.15}", mass.mass_kg);
    println!("center_m: {:?}", mass.center_of_mass);
    println!("bounds_m: {:?}", kernel.bounding_box(&body)?);
    let mesh = kernel.tessellate(
        &body,
        &TessellationSettings {
            linear_deflection: 1e-5,
            angular_deflection_deg: 8.,
        },
    )?;
    println!(
        "closed_mesh: {} vertices, {} triangles",
        mesh.positions.len(),
        mesh.triangle_count()
    );
    println!(
        "owned_faces: {:?}",
        mesh.triangle_face_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
    );
    Ok(())
}
