//! Fillet and chamfer on side-face references (ADR-018; integration test:
//! requires OCCT).
//!
//! Before kernel IDs became enumeration indices, only the top face worked:
//! tessellation face IDs came from a deep copy of the solid and never named
//! faces of the stored body, so every other face selected no edges, and the
//! address-keyed discoveries made the chosen face vary between runs.

use opencad_core::{Length, TopoRefId};
use opencad_feature::{
    bracket_semantic_refs, bracket_with_hole, ChamferFeature, FeatureDefinition, FeatureNode,
    FeatureRegistry, FilletFeature,
};
use opencad_geometry::{GeometryKernel, TopoRef};
use opencad_graph::bracket_parameters;
use opencad_kernel_occt::OcctGeometryKernel;

/// Agreement with the analytic volume difference, in cubic metres
/// (0.01 mm^3).
const VOLUME_TOLERANCE_M3: f64 = 1e-11;

/// Fillet radius and chamfer distance, in metres.
const EDGE_SIZE_M: f64 = 0.001;

fn bracket_volume(
    kernel: &OcctGeometryKernel,
    edge_feature: Option<(&str, FeatureDefinition)>,
) -> f64 {
    let mut model = bracket_with_hole().expect("bracket");
    let mut refs = bracket_semantic_refs();
    if let Some((role, definition)) = edge_feature {
        refs.push(TopoRef::face(
            TopoRefId::new("ref:face:side").expect("id"),
            "feature:extrude_base",
            role,
        ));
        model
            .add_node(FeatureNode::new("feature:edge", "Edge", definition))
            .expect("node");
        model
            .add_dependency("feature:hole_mount", "feature:edge")
            .expect("edge");
    }
    model
        .regenerate(
            kernel,
            &FeatureRegistry::with_defaults(),
            Some(&bracket_parameters()),
            Some(&refs),
        )
        .unwrap_or_else(|error| panic!("regenerate: {error}"));
    kernel
        .mass_properties(model.active_body().expect("body"), 1.0)
        .expect("mass")
        .volume_m3
}

fn fillet() -> FeatureDefinition {
    FeatureDefinition::Fillet(FilletFeature::on_face_ref(
        "feature:hole_mount",
        "ref:face:side",
        Length::from_meters(EDGE_SIZE_M),
        None,
    ))
}

fn chamfer() -> FeatureDefinition {
    FeatureDefinition::Chamfer(ChamferFeature::on_face_ref(
        "feature:hole_mount",
        "ref:face:side",
        Length::from_meters(EDGE_SIZE_M),
        None,
    ))
}

/// The 80 x 60 x 6 mm bracket's -y face (80 x 6 mm) and +x face
/// (60 x 6 mm) have perimeters differing by 2 x 20 mm of straight edge, with
/// identical corners, so the removed volumes differ by the straight-edge
/// cross-section times 40 mm.
#[test]
fn side_face_refs_select_that_faces_perimeter() {
    let kernel = OcctGeometryKernel::new();
    let base = bracket_volume(&kernel, None);
    let extra_length_m = 0.04;

    let fillet_x = base - bracket_volume(&kernel, Some(("+x", fillet())));
    let fillet_y = base - bracket_volume(&kernel, Some(("-y", fillet())));
    let fillet_section = (1.0 - std::f64::consts::FRAC_PI_4) * EDGE_SIZE_M * EDGE_SIZE_M;
    assert!(fillet_x > 0.0, "the +x fillet removed {fillet_x} m^3");
    assert!(
        (fillet_y - fillet_x - fillet_section * extra_length_m).abs() <= VOLUME_TOLERANCE_M3,
        "fillet removal -y {fillet_y} m^3 vs +x {fillet_x} m^3"
    );
    // The bracket is symmetric about x = 40 mm.
    let fillet_minus_x = base - bracket_volume(&kernel, Some(("-x", fillet())));
    assert!((fillet_minus_x - fillet_x).abs() <= VOLUME_TOLERANCE_M3);

    let chamfer_x = base - bracket_volume(&kernel, Some(("+x", chamfer())));
    let chamfer_y = base - bracket_volume(&kernel, Some(("-y", chamfer())));
    let chamfer_section = 0.5 * EDGE_SIZE_M * EDGE_SIZE_M;
    assert!(chamfer_x > 0.0, "the +x chamfer removed {chamfer_x} m^3");
    assert!(
        (chamfer_y - chamfer_x - chamfer_section * extra_length_m).abs() <= VOLUME_TOLERANCE_M3,
        "chamfer removal -y {chamfer_y} m^3 vs +x {chamfer_x} m^3"
    );
}
