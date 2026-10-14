//! Temporary kernel geometry only; no document mutation or invented MCP feature.
use opencad_core::{Length, Result};
use opencad_geometry::{
    ExtrudeExtent, ExtrudeOperation, GeometryKernel, MetricThreadSize, NominalMetricThread,
    NominalThreadInput, SketchPlacement, SolvedCircle, SolvedSketch, TessellationSettings,
    ThreadHandedness,
};

pub fn run(kernel: &dyn GeometryKernel) -> Result<()> {
    let sketch = SolvedSketch {
        profile_ref: "nominal-thread-stock".into(),
        points: Vec::new(),
        closed: true,
        circle: Some(SolvedCircle {
            center_m: [0., 0.],
            radius_m: 0.003,
        }),
        ellipse: None,
        cubic_bezier: None,
        segments: Vec::new(),
        placement: Some(SketchPlacement::global_xy()),
    };
    let stock = kernel.extrude(
        kernel.make_wire_from_sketch(&sketch)?,
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.008),
        },
        ExtrudeOperation::NewBody,
        None,
        [0., 0., 1.],
    )?;
    let input = NominalThreadInput {
        profile: NominalMetricThread::new(MetricThreadSize::M6, ThreadHandedness::Right, 8)?,
        axis_hint: [0., 0., 1.],
        radial_axis: [1., 0., 0.],
    };
    let thread = kernel.cut_nominal_metric_thread(stock.clone(), &input)?;
    let properties = kernel.mass_properties(&thread, 2700.)?;
    let mesh = kernel.tessellate(&thread, &TessellationSettings::default())?;
    println!("Physical nominal M6x1 basic flat-root thread, right hand, 8 pitches, square ends.");
    println!(
        "volume {:.9} mm^3; area {:.9} mm^2; mass {:.9} kg; {} triangles",
        properties.volume_m3 * 1e9,
        properties.area_m2 * 1e6,
        properties.mass_kg,
        mesh.triangle_count()
    );
    println!(
        "protected stock volume {:.9} mm^3",
        kernel.mass_properties(&stock, 1.)?.volume_m3 * 1e9
    );
    println!("SDK runtime only; no source feature, manufactured fit or rounded-root claim.");
    Ok(())
}
