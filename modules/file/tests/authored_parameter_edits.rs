//! Authored parts keep their shape when a driving parameter changes
//! (MCAD-P7-008; integration test: requires OCCT).
//!
//! A rectangle constrained only by its two side lengths leaves the solver
//! free to skew it once a length changes, so the authoring examples also
//! constrain the edges horizontal and vertical.

use std::path::PathBuf;

use opencad_ai::DesignPatch;
use opencad_feature::FeatureRegistry;
use opencad_file::{apply_patch_to_document, read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

/// Bounding-box agreement, in metres.  OCCT boxes include a small
/// enlargement, so this is looser than the exact geometry.
const BOUNDS_TOLERANCE_M: f64 = 1e-6;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example_patch(name: &str) -> DesignPatch {
    serde_json::from_str(
        &std::fs::read_to_string(repo().join("examples/agent").join(name)).expect("read"),
    )
    .expect("patch json")
}

fn authored(name: &str) -> OcadDocument {
    let metadata = read_ocad(repo().join("examples/bracket.ocad.d"))
        .expect("bracket")
        .metadata;
    let mut doc = OcadDocument::new(metadata);
    apply_patch_to_document(&mut doc, &example_patch(name)).expect("author");
    doc
}

/// Size of the regenerated body along x, y, and z, in metres.
fn extents(doc: &OcadDocument) -> [f64; 3] {
    let kernel = OcctGeometryKernel::new();
    let parameters = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.clone().into_part_model();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&parameters),
            Some(&semantic_refs),
        )
        .expect("regenerate");
    let bounds = kernel
        .bounding_box(model.active_body().expect("body"))
        .expect("bounds");
    [0, 1, 2].map(|axis| bounds.max[axis] - bounds.min[axis])
}

fn assert_extents(actual: [f64; 3], expected: [f64; 3], label: &str) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() <= BOUNDS_TOLERANCE_M,
            "{label}: axis {axis} is {} m, expected {} m",
            actual[axis],
            expected[axis]
        );
    }
}

#[test]
fn authored_rectangles_stay_rectangular_after_parameter_edits() {
    for (name, before, width_param, depth_param) in [
        (
            "author_plate_from_empty_patch.json",
            [0.06, 0.04, 0.005],
            "param:width",
            "param:depth",
        ),
        (
            "add_shell_patch.json",
            [0.06, 0.04, 0.02],
            "param:width",
            "param:depth",
        ),
    ] {
        let doc = authored(name);
        assert_extents(extents(&doc), before, name);

        let mut edited = doc.clone();
        apply_patch_to_document(
            &mut edited,
            &DesignPatch::set_parameter(width_param, "90 mm"),
        )
        .expect("width");
        apply_patch_to_document(
            &mut edited,
            &DesignPatch::set_parameter(depth_param, "25 mm"),
        )
        .expect("depth");
        assert_extents(extents(&edited), [0.09, 0.025, before[2]], name);
    }
}
