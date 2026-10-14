//! Runtime kernel example, not a Design Graph authoring shortcut.
use opencad_core::{Length, Result};
use opencad_geometry::*;
fn main() -> Result<()> {
    let kernel = MockGeometryKernel::with_analytic_primitives();
    let profile = SolvedSketch {
        profile_ref: "stock".into(),
        points: vec![[0., 0.], [0.04, 0.], [0.04, 0.03], [0., 0.03]],
        closed: true,
        circle: None,
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    };
    let stock = kernel.extrude(
        kernel.make_wire_from_sketch(&profile)?,
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.01),
        },
        ExtrudeOperation::NewBody,
        None,
        [0., 0., 1.],
    )?;
    let cut = CountersinkCut {
        entry: FacePick {
            point_m: [0.012, 0.009, 0.01],
            normal: [0., 0., 1.],
        },
        bore_radius_m: 0.0033,
        seat_radius_m: 0.006,
        bore_depth_m: 0.01,
        seat_depth_m: 0.0027,
    };
    let result = kernel.cut_countersunk_stock(stock, &cut)?;
    let mass = kernel.mass_properties(&result, 2700.)?;
    println!("volume_m3: {:.15}", mass.volume_m3);
    println!("area_m2: {:.15}", mass.area_m2);
    println!("center_m: {:?}", mass.center_of_mass);
    let mesh = kernel.tessellate(
        &result,
        &TessellationSettings {
            linear_deflection: 1e-6,
            angular_deflection_deg: 6.,
        },
    )?;
    println!("closed_mesh: {} triangles", mesh.triangle_count());
    Ok(())
}
