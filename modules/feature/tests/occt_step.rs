//! OCCT STEP export/import round trip (integration test: requires OCCT).

use opencad_feature::{bracket_with_hole, FeatureRegistry};
use opencad_geometry::{GeometryKernel, KernelBody, MockGeometryKernel};
use opencad_graph::bracket_parameters;
use opencad_kernel_occt::step::STEP_TIMESTAMP;
use opencad_kernel_occt::OcctGeometryKernel;

/// Volume agreement after a STEP round trip, in cubic metres (0.001 mm^3).
const VOLUME_TOLERANCE_M3: f64 = 1e-12;

/// Bounding-box agreement after a STEP round trip, in metres.  OCCT boxes
/// include a small enlargement, so this is looser than the exact geometry.
const BOUNDS_TOLERANCE_M: f64 = 1e-6;

fn bracket_body(kernel: &OcctGeometryKernel) -> KernelBody {
    let mut model = bracket_with_hole().expect("bracket");
    model
        .regenerate(
            kernel,
            &FeatureRegistry::with_defaults(),
            Some(&bracket_parameters()),
            None,
        )
        .expect("regenerate");
    model.active_body().expect("body").clone()
}

/// Largest absolute coordinate among the file's CARTESIAN_POINT entities.
fn largest_cartesian_coordinate(step: &str) -> f64 {
    step.lines()
        .filter(|line| line.contains("CARTESIAN_POINT"))
        .filter_map(|line| {
            let start = line.rfind('(')?;
            let end = line[start..].find(')')? + start;
            Some(
                line[start + 1..end]
                    .split(',')
                    .filter_map(|value| value.trim().parse::<f64>().ok())
                    .fold(0.0_f64, |max, value| max.max(value.abs())),
            )
        })
        .fold(0.0, f64::max)
}

#[test]
fn step_export_is_millimetre_deterministic_and_round_trips() {
    let kernel = OcctGeometryKernel::new();
    let body = bracket_body(&kernel);
    let step = kernel.export_step(&body).expect("export");
    let text = String::from_utf8(step.clone()).expect("STEP is text");

    assert!(text.starts_with("ISO-10303-21;"));
    assert!(text.contains(".MILLI."), "length unit must be millimetres");
    assert!(text.contains(STEP_TIMESTAMP), "header time stamp is fixed");
    // The 80 mm bracket is written as 80, not 0.08.
    let largest = largest_cartesian_coordinate(&text);
    assert!(
        (largest - 80.0).abs() < 1e-6,
        "largest coordinate {largest}"
    );
    assert_eq!(kernel.export_step(&body).expect("export again"), step);
    assert!(!text.contains("Open CASCADE STEP translator"));

    let imported = kernel.import_step(&step).expect("import");
    let (original, restored) = (
        kernel.mass_properties(&body, 1.0).expect("mass"),
        kernel.mass_properties(&imported, 1.0).expect("mass"),
    );
    assert!(
        (original.volume_m3 - restored.volume_m3).abs() <= VOLUME_TOLERANCE_M3,
        "volume {} vs {}",
        restored.volume_m3,
        original.volume_m3
    );
    let (a, b) = (
        kernel.bounding_box(&body).expect("bounds"),
        kernel.bounding_box(&imported).expect("bounds"),
    );
    for axis in 0..3 {
        assert!((a.min[axis] - b.min[axis]).abs() <= BOUNDS_TOLERANCE_M);
        assert!((a.max[axis] - b.max[axis]).abs() <= BOUNDS_TOLERANCE_M);
    }

    assert!(kernel.import_step(b"not a step file").is_err());
}

#[test]
fn the_mock_kernel_reports_step_as_unsupported() {
    let kernel = MockGeometryKernel::new();
    let error = kernel
        .export_step(&KernelBody::new(1))
        .expect_err("unsupported");
    assert!(error.to_string().contains("does not support STEP export"));
}
