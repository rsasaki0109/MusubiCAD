//! ADR-013 slice 3: the persisted Feature Graph of every checked-in example
//! is reproduced by `derive_feature_graph` from its feature definitions.
//!
//! Display order, entries, and the edge set must match exactly.  Edge order
//! is compared exactly only where the template already used the canonical
//! field order; elsewhere it is cosmetic, because the topological sort
//! re-sorts ready nodes by ID and so never depends on edge order.

use std::collections::BTreeSet;
use std::path::PathBuf;

use opencad_feature::derive_feature_graph;
use opencad_file::read_ocad;

/// Examples whose templates list pattern edges target-first; their edge
/// sets match but not their edge order.
const COSMETIC_EDGE_ORDER: [&str; 5] = [
    "bracket_hole_ring.ocad.d",
    "bracket_hole_row.ocad.d",
    "bracket_pin_mirror.ocad.d",
    "bracket_pin_ring.ocad.d",
    "bracket_pin_row.ocad.d",
];

#[test]
fn every_example_feature_graph_is_derivable() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut checked = 0;
    let mut entries: Vec<_> = std::fs::read_dir(&examples)
        .expect("examples")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "d"))
        .collect();
    entries.sort();

    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let doc = read_ocad(&path).expect("read example");
        if doc.feature_nodes.is_empty() {
            continue;
        }
        let persisted = &doc.feature_graph;
        let derived = derive_feature_graph(
            &doc.feature_nodes,
            persisted.ordered_ids(),
            &doc.sketches,
            &doc.semantic_refs,
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));

        assert_eq!(
            derived.ordered_ids(),
            persisted.ordered_ids(),
            "{name}: order"
        );
        for id in persisted.ordered_ids() {
            assert_eq!(derived.get(id), persisted.get(id), "{name}: entry {id}");
        }
        let edge_set = |graph: &opencad_graph::FeatureGraph| -> BTreeSet<(String, String)> {
            graph
                .dependency_edges()
                .iter()
                .map(|edge| (edge.source.clone(), edge.target.clone()))
                .collect()
        };
        assert_eq!(edge_set(&derived), edge_set(persisted), "{name}: edge set");
        assert_eq!(
            derived.recompute_order().expect("order"),
            persisted.recompute_order().expect("order"),
            "{name}: regeneration order"
        );
        if !COSMETIC_EDGE_ORDER.contains(&name.as_str()) {
            assert_eq!(&derived, persisted, "{name}: exact graph");
        }
        checked += 1;
    }
    assert_eq!(checked, 13, "every part example must be checked");
}
