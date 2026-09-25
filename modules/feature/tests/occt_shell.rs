//! OCCT shell feature (ADR-017; integration test: requires OCCT).

use opencad_core::Length;
use opencad_feature::{
    bracket_base_plate, bracket_semantic_refs, FeatureDefinition, FeatureNode, FeatureRegistry,
    PartModel, ShellFeature,
};
use opencad_geometry::GeometryKernel;
use opencad_graph::{ParamGraph, ParameterEntry};
use opencad_kernel_occt::OcctGeometryKernel;

/// Shell volume agreement with the analytic value, in cubic metres (ADR-017 §8).
const VOLUME_TOLERANCE_M3: f64 = 1e-9;

/// Bounding-box agreement, in metres.  OCCT boxes include a small
/// enlargement, so this is looser than the exact geometry.
const BOUNDS_TOLERANCE_M: f64 = 1e-6;

/// An 80 x 60 x 20 mm box: the bracket base plate, 20 mm thick.  Width and
/// height keep the sketch's literal 80 x 60 mm corners, because the plate
/// sketch only constrains edge lengths and would not stay rectangular if
/// resized.
fn box_parameters(thickness: &str) -> ParamGraph {
    let mut graph = ParamGraph::new();
    for (id, name, expr) in [
        ("param:width", "width", "80 mm"),
        ("param:height", "height", "60 mm"),
        ("param:thickness", "thickness", "20 mm"),
        ("param:wall", "wall", thickness),
    ] {
        graph
            .add_parameter(ParameterEntry::new(id, name, expr))
            .expect("parameter");
    }
    graph
}

fn shelled_box(open_face_refs: Vec<String>) -> PartModel {
    let mut model = bracket_base_plate().expect("box");
    model
        .add_node(FeatureNode::new(
            "feature:shell",
            "Shell",
            FeatureDefinition::Shell(ShellFeature::new(
                "feature:extrude_base",
                Length::from_meters(0.002),
                Some("wall".into()),
                open_face_refs,
            )),
        ))
        .expect("shell");
    model
        .add_dependency("feature:extrude_base", "feature:shell")
        .expect("edge");
    model
}

#[test]
fn shelling_a_box_with_an_open_top_matches_the_analytic_volume() {
    let kernel = OcctGeometryKernel::new();
    let mut model = shelled_box(vec!["ref:face:bracket_top".into()]);
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&box_parameters("2 mm")),
            Some(&bracket_semantic_refs()),
        )
        .expect("regenerate");
    let body = model.active_body().expect("body").clone();

    // 80*60*20 - 76*56*18 = 19 392 mm^3.
    let expected_m3 = (80.0 * 60.0 * 20.0 - 76.0 * 56.0 * 18.0) * 1e-9;
    let mass = kernel.mass_properties(&body, 1.0).expect("mass");
    let volume = mass.volume_m3;
    // Opening the top or the bottom gives the same volume; only the open top
    // keeps the floor, which puts the centre of mass below mid-height.
    assert!(
        mass.center_of_mass[2] < 0.01,
        "centre of mass z {} m: the wrong face was opened",
        mass.center_of_mass[2]
    );
    assert!(
        (volume - expected_m3).abs() <= VOLUME_TOLERANCE_M3,
        "volume {volume} m^3, expected {expected_m3} m^3"
    );

    // The wall grows inward, so the outer bounds are unchanged.
    let bounds = kernel.bounding_box(&body).expect("bounds");
    for (axis, extent) in [0.08, 0.06, 0.02].into_iter().enumerate() {
        let size = bounds.max[axis] - bounds.min[axis];
        assert!(
            (size - extent).abs() <= BOUNDS_TOLERANCE_M,
            "axis {axis}: {size} m"
        );
    }
}

#[test]
fn an_unresolved_open_face_fails_closed() {
    let kernel = OcctGeometryKernel::new();
    let mut model = shelled_box(vec!["ref:face:nowhere".into()]);
    let error = model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&box_parameters("2 mm")),
            Some(&bracket_semantic_refs()),
        )
        .expect_err("unresolved face");
    assert!(error.to_string().contains("ref:face:nowhere"), "{error}");
}

#[test]
fn a_wall_thicker_than_the_part_fails_regeneration() {
    let kernel = OcctGeometryKernel::new();
    let mut model = shelled_box(vec!["ref:face:bracket_top".into()]);
    // 35 mm walls cannot fit a 60 mm deep box; OCCT would return the input
    // unchanged, which the kernel reports as a failure.
    let result = model.regenerate(
        &kernel,
        &FeatureRegistry::with_defaults(),
        Some(&box_parameters("35 mm")),
        Some(&bracket_semantic_refs()),
    );
    let error = result.expect_err("an over-thick shell must not regenerate");
    assert!(
        error.to_string().contains("did not hollow the body"),
        "{error}"
    );
}
