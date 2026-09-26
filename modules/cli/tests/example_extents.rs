//! Every part example regenerates to its designed extents (integration
//! test: requires OCCT).
//!
//! Volume checks alone missed two orientation bugs whose volumes were
//! unchanged: a pin sketched on the bracket's top face lay on its side, and
//! a pin mirrored across the bracket's top face was mirrored across the
//! pin's own top instead.  Bounding boxes catch both.

use std::path::PathBuf;

use opencad_feature::FeatureRegistry;
use opencad_file::read_expanded_dir;
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

/// Agreement with the designed extents, in metres.  OCCT bounding boxes are
/// enlarged by about 1e-7 m.
const EXTENT_TOLERANCE_M: f64 = 1e-6;

/// Example name and its designed bounding box, `[min, max]` in millimetres.
const EXTENTS_MM: &[(&str, [[f64; 3]; 2])] = &[
    // 80 x 60 x 6 mm bracket plates.
    ("bracket", [[0.0, 0.0, 0.0], [80.0, 60.0, 6.0]]),
    ("bracket_edge_fillet", [[0.0, 0.0, 0.0], [80.0, 60.0, 6.0]]),
    ("bracket_hole_ring", [[0.0, 0.0, 0.0], [80.0, 60.0, 6.0]]),
    ("bracket_hole_row", [[0.0, 0.0, 0.0], [80.0, 60.0, 6.0]]),
    // Bosses and pins extruded boss_height = 12 mm from the XY plane.
    ("bracket_boss_join", [[0.0, 0.0, 0.0], [80.0, 60.0, 12.0]]),
    ("bracket_pin_row", [[0.0, 0.0, 0.0], [80.0, 60.0, 12.0]]),
    ("bracket_pin_ring", [[0.0, 0.0, 0.0], [80.0, 60.0, 12.0]]),
    // A 0-12 mm pin mirrored across the 6 mm top face maps onto itself.
    ("bracket_pin_mirror", [[0.0, 0.0, 0.0], [80.0, 60.0, 12.0]]),
    // A 12 mm pin standing on the 6 mm top face.
    ("bracket_face_pin", [[0.0, 0.0, 0.0], [80.0, 60.0, 18.0]]),
    // 96 x 72 mm plate with a 14 mm tall bearing boss.
    ("bearing_carrier", [[0.0, 0.0, 0.0], [96.0, 72.0, 14.0]]),
    // Radius 25 mm, 20 mm along +y: a full turn and the z <= 0 half.
    ("revolve_bushing", [[-25.0, 0.0, -25.0], [25.0, 20.0, 25.0]]),
    ("revolve_sector", [[-25.0, 0.0, -25.0], [25.0, 20.0, 0.0]]),
    (
        "robot_joint_actuator",
        [[-19.0, 0.0, 0.0], [159.0, 110.0, 32.0]],
    ),
];

#[test]
fn part_examples_regenerate_to_their_designed_extents() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let kernel = OcctGeometryKernel::new();
    let mut failures = Vec::new();
    for (name, [min_mm, max_mm]) in EXTENTS_MM {
        let doc = read_expanded_dir(examples.join(format!("{name}.ocad.d"))).expect("read");
        let parameters = doc.parameters.clone();
        let semantic_refs = doc.semantic_refs.clone();
        let mut model = doc.into_part_model();
        model
            .regenerate(
                &kernel,
                &FeatureRegistry::with_defaults(),
                Some(&parameters),
                Some(&semantic_refs),
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let bounds = kernel
            .bounding_box(model.active_body().expect("body"))
            .expect("bounds");
        for axis in 0..3 {
            for (actual, expected_mm, label) in [
                (bounds.min[axis], min_mm[axis], "min"),
                (bounds.max[axis], max_mm[axis], "max"),
            ] {
                if (actual - expected_mm * 1e-3).abs() > EXTENT_TOLERANCE_M {
                    failures.push(format!(
                        "{name}: {label}[{axis}] is {:.4} mm, designed {expected_mm} mm",
                        actual * 1e3
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
