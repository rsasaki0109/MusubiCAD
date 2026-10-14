//! Runtime SDK example; no document feature, MCP tool, or model calls.
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
    let input = TaperedRibInput {
        entry: FacePick {
            point_m: [0.020, 0.015, 0.010],
            normal: [0., 0., 1.],
        },
        length_axis_hint: [1., 0., 0.],
        length_m: 0.030,
        base_thickness_m: 0.004,
        height_m: 0.020,
        angle_rad: 0.05_f64.atan(),
    };
    let witness = kernel.rectangular_stock_geometry(&stock, input.entry)?;
    let geometry = input.admit(&witness)?;
    let result = kernel.add_tapered_rib_stock(stock, &input)?;
    let mass = kernel.mass_properties(&result, 1.)?;
    let mesh = kernel.tessellate(&result, &TessellationSettings::default())?;
    println!(
        "base/top thickness_m: {:.12}/{:.12}",
        geometry.base_thickness_m, geometry.top_thickness_m
    );
    println!(
        "volume_m3: {:.12}; area_m2: {:.12}",
        mass.volume_m3, mass.area_m2
    );
    println!(
        "center_of_mass_m: {:?}; closed triangles: {}",
        mass.center_of_mass,
        mesh.triangle_count()
    );
    Ok(())
}
