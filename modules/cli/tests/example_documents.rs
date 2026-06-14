//! Committed example documents regenerate with OCCT.

use opencad_feature::FeatureRegistry;
use opencad_file::{read_expanded_dir, validate_expanded_dir};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn example_bracket_face_pin_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_face_pin.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            Some(&semantic_refs),
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "face pin example should fuse pin onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_edge_fillet_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_edge_fillet.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            Some(&semantic_refs),
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 < plate_volume,
        "edge fillet example should round one edge and reduce volume: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_revolve_bushing_regenerates_with_occt() {
    let path = workspace_root().join("examples/revolve_bushing.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let parameters = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let expected = std::f64::consts::PI * (0.025_f64.powi(2) - 0.015_f64.powi(2)) * 0.02;
    assert!(
        (mass.volume_m3 - expected).abs() < 1e-8,
        "revolve bushing example volume {} vs {}",
        mass.volume_m3,
        expected
    );
}

#[test]
fn example_revolve_sector_regenerates_with_occt() {
    let path = workspace_root().join("examples/revolve_sector.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let parameters = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let expected_full = std::f64::consts::PI * (0.025_f64.powi(2) - 0.015_f64.powi(2)) * 0.02;
    let expected = expected_full * 0.5;
    assert!(
        (mass.volume_m3 - expected).abs() < 1e-8,
        "revolve sector example volume {} vs {}",
        mass.volume_m3,
        expected
    );
}

#[test]
fn example_bracket_boss_join_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_boss_join.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "boss join example should fuse boss onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_hole_row_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_hole_row.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 < plate_volume,
        "hole row example should reduce plate volume: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_hole_ring_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_hole_ring.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 < plate_volume,
        "hole ring example should reduce plate volume: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_pin_row_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_pin_row.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "pin row example should fuse bosses onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_pin_ring_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_pin_ring.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "pin ring example should fuse bosses onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_pin_mirror_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_pin_mirror.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "pin mirror example should fuse mirrored pins onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    assert!(model.active_body().is_some());
}
