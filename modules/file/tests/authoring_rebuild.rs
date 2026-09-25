//! ADR-013 acceptance: every part example is rebuilt from an empty document
//! using only structural `DesignPatch` operations, and the rebuilt document
//! regenerates to the same OCCT geometry (integration test: requires OCCT).
//!
//! Source data must match exactly.  Derived data is compared semantically:
//! sketch profiles and solve state are recomputed by regeneration, and
//! Feature Graph edge order is cosmetic for the five templates that list
//! pattern edges target-first (see `feature_graph_derivation.rs`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use opencad_ai::authoring_patch;
use opencad_feature::FeatureRegistry;
use opencad_file::expanded_dir::serialize_document_files;
use opencad_file::{apply_patch_to_document, document_design_state, read_ocad, OcadDocument};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;
use opencad_sketch::{Sketch, SolveState};

/// Volume agreement between the rebuilt and the checked-in document after
/// OCCT regeneration, in cubic meters.
const VOLUME_TOLERANCE_M3: f64 = 1e-12;

/// Bounding-box corner agreement after OCCT regeneration, in meters.
const BOUNDS_TOLERANCE_M: f64 = 1e-9;

/// Density used only to exercise mass properties, in kg/m^3.
const DENSITY_KG_PER_M3: f64 = 2700.0;

const COSMETIC_EDGE_ORDER: [&str; 5] = [
    "bracket_hole_ring.ocad.d",
    "bracket_hole_row.ocad.d",
    "bracket_pin_mirror.ocad.d",
    "bracket_pin_ring.ocad.d",
    "bracket_pin_row.ocad.d",
];

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn rebuild(source: &OcadDocument) -> OcadDocument {
    let patch = authoring_patch(&document_design_state(source)).expect("authoring patch");
    let mut rebuilt = OcadDocument::new(source.metadata.clone());
    apply_patch_to_document(&mut rebuilt, &patch).expect("apply authoring patch");
    rebuilt
}

/// Sketch source data only: derived profiles and solve state are cleared.
fn authored(sketches: &[Sketch]) -> Vec<Sketch> {
    sketches
        .iter()
        .cloned()
        .map(|mut sketch| {
            sketch.profiles.clear();
            sketch.solve_state = SolveState::Unknown;
            sketch
        })
        .collect()
}

/// Regenerate with OCCT and return `(volume_m3, bounds_min_m, bounds_max_m)`.
fn occt_geometry(doc: &OcadDocument) -> (f64, [f64; 3], [f64; 3]) {
    let parameters = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.clone().into_part_model();
    let kernel = OcctGeometryKernel::new();
    model
        .regenerate(
            &kernel,
            &FeatureRegistry::with_defaults(),
            Some(&parameters),
            Some(&semantic_refs),
        )
        .expect("regenerate");
    let body = model.active_body().expect("body");
    let mass = kernel
        .mass_properties(body, DENSITY_KG_PER_M3)
        .expect("mass");
    let bounds = kernel.bounding_box(body).expect("bounds");
    (mass.volume_m3, bounds.min, bounds.max)
}

fn part_examples() -> Vec<(String, OcadDocument)> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(examples())
        .expect("examples")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "d"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            (name, read_ocad(&path).expect("read"))
        })
        .filter(|(_, doc)| {
            !doc.feature_nodes.is_empty() && doc.assembly.is_none() && doc.drawing.is_none()
        })
        .collect()
}

#[test]
fn every_part_example_is_rebuilt_from_structural_patches() {
    let parts = part_examples();
    assert_eq!(parts.len(), 13);
    for (name, source) in parts {
        let rebuilt = rebuild(&source);
        assert_eq!(rebuilt.parameters, source.parameters, "{name}: parameters");
        assert_eq!(
            rebuilt.feature_nodes, source.feature_nodes,
            "{name}: features"
        );
        assert_eq!(rebuilt.semantic_refs, source.semantic_refs, "{name}: refs");
        assert_eq!(rebuilt.assertions, source.assertions, "{name}: assertions");
        assert_eq!(
            authored(&rebuilt.sketches),
            authored(&source.sketches),
            "{name}: sketches"
        );
        assert_eq!(
            rebuilt.feature_graph.ordered_ids(),
            source.feature_graph.ordered_ids(),
            "{name}: feature order"
        );
        if COSMETIC_EDGE_ORDER.contains(&name.as_str()) {
            let edges = |doc: &OcadDocument| -> BTreeSet<(String, String)> {
                doc.feature_graph
                    .dependency_edges()
                    .iter()
                    .map(|edge| (edge.source.clone(), edge.target.clone()))
                    .collect()
            };
            assert_eq!(edges(&rebuilt), edges(&source), "{name}: edge set");
        } else {
            assert_eq!(rebuilt.feature_graph, source.feature_graph, "{name}: graph");
        }

        let (expected, actual) = (occt_geometry(&source), occt_geometry(&rebuilt));
        assert!(expected.0 > 0.0, "{name}: empty source body");
        assert!(
            (expected.0 - actual.0).abs() <= VOLUME_TOLERANCE_M3,
            "{name}: volume {} m^3 differs from {} m^3",
            actual.0,
            expected.0
        );
        for axis in 0..3 {
            assert!(
                (expected.1[axis] - actual.1[axis]).abs() <= BOUNDS_TOLERANCE_M
                    && (expected.2[axis] - actual.2[axis]).abs() <= BOUNDS_TOLERANCE_M,
                "{name}: bounds differ on axis {axis}"
            );
        }
    }
}

#[test]
fn rebuilt_bracket_matches_checked_in_files_byte_for_byte() {
    let source = read_ocad(examples().join("bracket.ocad.d")).expect("bracket");
    let rebuilt = rebuild(&source);
    let (expected, actual) = (
        serialize_document_files(&source).expect("serialize"),
        serialize_document_files(&rebuilt).expect("serialize"),
    );
    assert_eq!(
        expected.keys().collect::<Vec<_>>(),
        actual.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in &expected {
        // Sketch solve state is derived and compared semantically above;
        // the checksum manifest differs only by the sketches.json digest.
        if Path::new(path).ends_with("sketches.json") || path == "checksums.json" {
            continue;
        }
        assert_eq!(&actual[path], bytes, "{path} differs");
    }
}
