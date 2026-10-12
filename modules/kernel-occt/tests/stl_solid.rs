//! Native integration evidence for ADR-049, against independently known measures.
use opencad_core::LengthUnit;
use opencad_geometry::{
    read_stl_solid, GeometryKernel, MockGeometryKernel, RigidTransform, TessellationSettings,
};
use opencad_kernel_occt::OcctGeometryKernel;

#[test]
fn both_kernels_preserve_stl_dimensions_volume_area_and_rigid_placement() {
    let bytes = include_bytes!("../../../examples/import/closed_plate_mm.stl");
    let mesh = read_stl_solid(bytes, LengthUnit::Millimeter).expect("parse");
    let native = OcctGeometryKernel::new();
    let mock = MockGeometryKernel::new();
    for kernel in [&native as &dyn GeometryKernel, &mock as &dyn GeometryKernel] {
        let body = kernel.import_triangle_solid(&mesh).expect("solid");
        let mass = kernel.mass_properties(&body, 1000.).expect("mass");
        assert!((mass.volume_m3 - 2.88e-5).abs() < 1e-12);
        assert!((mass.area_m2 - 0.01128).abs() < 1e-12);
        let placement = RigidTransform {
            translation_m: [1., 2., 3.],
            rotation: [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
        };
        let placed = kernel
            .transform_body(body.clone(), placement)
            .expect("place");
        let bounds = kernel.bounding_box(&placed).expect("bbox");
        for (actual, expected) in bounds.min.into_iter().zip([0.94, 2., 3.]) {
            assert!((actual - expected).abs() < 2e-7);
        }
        for (actual, expected) in bounds.max.into_iter().zip([1., 2.08, 3.006]) {
            assert!((actual - expected).abs() < 2e-7);
        }
        assert!(
            (kernel
                .mass_properties(&placed, 1.)
                .expect("placed")
                .volume_m3
                - 2.88e-5)
                .abs()
                < 1e-12
        );
        assert!(
            (kernel
                .mass_properties(&body, 1.)
                .expect("unchanged")
                .volume_m3
                - 2.88e-5)
                .abs()
                < 1e-12
        );
        assert_eq!(
            kernel
                .tessellate(&body, &TessellationSettings::default())
                .expect("tessellate")
                .triangle_count(),
            12
        );
    }
}
