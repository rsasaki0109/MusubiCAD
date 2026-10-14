//! Developer runtime example; no Design Graph/source mutation or MCP shortcut.
use opencad_core::{Length, Result};
use opencad_geometry::*;
fn main() -> Result<()> {
    let k = MockGeometryKernel::with_analytic_primitives();
    let profile = SolvedSketch {
        profile_ref: "round-stock".into(),
        points: Vec::new(),
        closed: true,
        circle: Some(SolvedCircle {
            center_m: [0.012, 0.009],
            radius_m: 0.02,
        }),
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: None,
    };
    let body = k.extrude(
        k.make_wire_from_sketch(&profile)?,
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.03),
        },
        ExtrudeOperation::NewBody,
        None,
        [0., 0., 1.],
    )?;
    let stock = k.cylindrical_stock_geometry(&body, [0., 0., 1.])?;
    println!("stock_witness: {stock:?}");
    let draft = CylindricalDraftInput {
        axis_hint: [0., 0., 1.],
        neutral_origin_m: [0.012, 0.009, 0.015],
        neutral_normal: [0., 0., 1.],
        angle_rad: 7_f64.to_radians(),
    };
    println!("endpoint_radii_m: {:?}", draft.endpoint_radii(&stock)?);
    let out = k.draft_cylindrical_stock(body, &draft)?;
    let mass = k.mass_properties(&out, 2700.)?;
    println!(
        "volume_m3: {:.15}\narea_m2: {:.15}\ncenter_m: {:?}",
        mass.volume_m3, mass.area_m2, mass.center_of_mass
    );
    let mesh = k.tessellate(
        &out,
        &TessellationSettings {
            linear_deflection: 1e-6,
            angular_deflection_deg: 6.,
        },
    )?;
    println!("closed_mesh: {} triangles", mesh.triangle_count());
    Ok(())
}
