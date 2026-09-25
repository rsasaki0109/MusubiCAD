//! `opencad export <doc> out.step` for parts and assemblies (integration
//! test: requires OCCT).

use std::path::{Path, PathBuf};
use std::process::Command;

use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

/// Bounding-box agreement between a re-imported STEP body and the expected
/// extent, in metres.  OCCT boxes include a small enlargement.
const BOUNDS_TOLERANCE_M: f64 = 1e-6;

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

fn export(document: &Path, output: &Path) {
    let run = Command::new(env!("CARGO_BIN_EXE_opencad"))
        .args([
            "export",
            &document.to_string_lossy(),
            &output.to_string_lossy(),
        ])
        .output()
        .expect("run opencad export");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

fn extent_of(step: &[u8]) -> [f64; 3] {
    let kernel = OcctGeometryKernel::new();
    let body = kernel.import_step(step).expect("import exported STEP");
    let bounds = kernel.bounding_box(&body).expect("bounds");
    [0, 1, 2].map(|axis| bounds.max[axis] - bounds.min[axis])
}

#[test]
fn a_part_exports_to_deterministic_millimetre_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (first, second) = (dir.path().join("a.step"), dir.path().join("b.stp"));
    export(&example("bracket.ocad.d"), &first);
    export(&example("bracket.ocad.d"), &second);
    let step = std::fs::read(&first).expect("read STEP");
    assert!(step.starts_with(b"ISO-10303-21;"));
    assert_eq!(
        step,
        std::fs::read(&second).expect("read STEP"),
        "deterministic"
    );

    // The bracket is 80 x 60 x 6 mm.
    let extent = extent_of(&step);
    for (axis, expected) in [0.08, 0.06, 0.006].into_iter().enumerate() {
        assert!(
            (extent[axis] - expected).abs() <= BOUNDS_TOLERANCE_M,
            "axis {axis}: {} m",
            extent[axis]
        );
    }
}

#[test]
fn an_assembly_exports_every_placed_instance() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = dir.path().join("assembly.step");
    export(&example("assembly_two_brackets.ocad.d"), &output);
    let step = std::fs::read(&output).expect("read STEP");
    // The `mate:spacing` distance mate solves the right bracket to x = 120 mm,
    // so the two 80 mm brackets span 200 mm: the export uses solved
    // placements, not the stored 50 mm offset.
    let extent = extent_of(&step);
    assert!(
        (extent[0] - 0.20).abs() <= BOUNDS_TOLERANCE_M,
        "x extent {} m",
        extent[0]
    );
}

#[test]
fn drawings_are_refused_for_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run = Command::new(env!("CARGO_BIN_EXE_opencad"))
        .args([
            "export",
            &example("bracket_front_view.ocad.d").to_string_lossy(),
            &dir.path().join("sheet.step").to_string_lossy(),
        ])
        .output()
        .expect("run");
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("drawings export as SVG"));
}
