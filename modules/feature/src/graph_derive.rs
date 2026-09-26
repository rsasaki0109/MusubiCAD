//! Derive the persisted Feature Graph from feature definitions (ADR-013 §4).
//!
//! `feature_graph.edges` and its entries are a projection of the feature
//! nodes: every edge comes from an input field of a definition, or from the
//! feature that created a semantic reference the definition consumes.  The
//! authored display order is the only independent input.  Edge order has no
//! effect on regeneration (the topological sort re-sorts ready nodes by ID),
//! so edges are emitted in a canonical order: display order of the consuming
//! feature, then [`FeatureDefinition::feature_inputs`] field order, then
//! reference inputs, with duplicates removed.  A reference input adds an
//! edge only when its creating feature is not already upstream through direct
//! inputs, so references never add redundant transitive edges.

use std::collections::{BTreeMap, BTreeSet};

use opencad_core::{OpenCadError, Result};
use opencad_geometry::TopoRef;
use opencad_graph::{FeatureEntry, FeatureGraph};
use opencad_sketch::{Sketch, Workplane};

use crate::feature::{FeatureDefinition, FeatureNode};

impl FeatureDefinition {
    /// Feature IDs this definition consumes directly, as `(field, feature_id)`
    /// in canonical field order: sketch, source, then target.
    pub fn feature_inputs(&self) -> Vec<(&'static str, &str)> {
        if let Self::Sweep(def) = self {
            return [
                ("sketch_feature", Some(&def.sketch_feature)),
                ("path_sketch_feature", Some(&def.path_sketch_feature)),
                ("target_feature", def.target_feature.as_ref()),
            ]
            .into_iter()
            .filter_map(|(field, id)| id.map(|id| (field, id.as_str())))
            .collect();
        }
        if let Self::Loft(def) = self {
            return def
                .sections
                .iter()
                .map(|section| ("sections", section.sketch_feature.as_str()))
                .chain(
                    def.target_feature
                        .as_deref()
                        .map(|id| ("target_feature", id)),
                )
                .collect();
        }
        let candidates: [(&'static str, Option<&String>); 2] = match self {
            Self::Sketch(_) => [("", None), ("", None)],
            Self::Extrude(def) => [
                ("sketch_feature", Some(&def.sketch_feature)),
                ("target_feature", def.target_feature.as_ref()),
            ],
            Self::Revolve(def) => [
                ("sketch_feature", Some(&def.sketch_feature)),
                ("target_feature", def.target_feature.as_ref()),
            ],
            Self::Hole(def) => [
                ("sketch_feature", Some(&def.sketch_feature)),
                ("target_feature", Some(&def.target_feature)),
            ],
            Self::Fillet(def) => [("target_feature", Some(&def.target_feature)), ("", None)],
            Self::Chamfer(def) => [("target_feature", Some(&def.target_feature)), ("", None)],
            Self::Shell(def) => [("target_feature", Some(&def.target_feature)), ("", None)],
            Self::Loft(_) => [("", None), ("", None)],
            Self::Sweep(_) => [("", None), ("", None)],
            Self::HelixSweep(def) => [
                ("sketch_feature", Some(&def.sketch_feature)),
                ("target_feature", def.target_feature.as_ref()),
            ],
            Self::LinearPattern(def) => [
                ("source_feature", Some(&def.source_feature)),
                ("target_feature", def.target_feature.as_ref()),
            ],
            Self::CircularPattern(def) => [
                ("source_feature", Some(&def.source_feature)),
                ("target_feature", def.target_feature.as_ref()),
            ],
            Self::MirrorPattern(def) => [
                ("source_feature", Some(&def.source_feature)),
                ("target_feature", def.target_feature.as_ref()),
            ],
            Self::ImportedSolid(def) => {
                [("target_feature", def.target_feature.as_ref()), ("", None)]
            }
        };
        present(candidates)
    }

    /// Semantic reference IDs this definition consumes, as `(field, ref_id)`.
    pub fn reference_inputs(&self) -> Vec<(&'static str, &str)> {
        if let Self::Shell(def) = self {
            return def
                .open_face_refs
                .iter()
                .map(|face_ref| ("open_face_refs", face_ref.as_str()))
                .collect();
        }
        let candidates: [(&'static str, Option<&String>); 2] = match self {
            Self::Hole(def) => [("face_ref", def.face_ref.as_ref()), ("", None)],
            Self::Fillet(def) => [
                ("face_ref", def.face_ref.as_ref()),
                ("edge_ref", def.edge_ref.as_ref()),
            ],
            Self::Chamfer(def) => [
                ("face_ref", def.face_ref.as_ref()),
                ("edge_ref", def.edge_ref.as_ref()),
            ],
            Self::MirrorPattern(def) => {
                [("plane_face_ref", def.plane_face_ref.as_ref()), ("", None)]
            }
            Self::Sketch(_)
            | Self::Extrude(_)
            | Self::Revolve(_)
            | Self::LinearPattern(_)
            | Self::CircularPattern(_)
            | Self::ImportedSolid(_)
            | Self::Shell(_)
            | Self::Loft(_)
            | Self::Sweep(_)
            | Self::HelixSweep(_) => [("", None), ("", None)],
        };
        present(candidates)
    }

    /// Closed profiles this definition consumes, as `(sketch_feature,
    /// profile_ref)`: one for extrudes, holes, and revolves, one per loft
    /// section (ADR-022).
    pub fn profile_inputs(&self) -> Vec<(&str, &str)> {
        match self {
            Self::Extrude(def) => vec![(def.sketch_feature.as_str(), def.profile_ref.as_str())],
            Self::Hole(def) => vec![(def.sketch_feature.as_str(), def.profile_ref.as_str())],
            Self::Revolve(def) => vec![(def.sketch_feature.as_str(), def.profile_ref.as_str())],
            Self::Sweep(def) => vec![(def.sketch_feature.as_str(), def.profile_ref.as_str())],
            Self::HelixSweep(def) => {
                vec![(def.sketch_feature.as_str(), def.profile_ref.as_str())]
            }
            Self::Loft(def) => def
                .sections
                .iter()
                .map(|section| {
                    (
                        section.sketch_feature.as_str(),
                        section.profile_ref.as_str(),
                    )
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Face references this definition removes from the body, such as shell
    /// openings (ADR-019).  Provenance reports them as consumed.
    pub fn consumed_face_refs(&self) -> Vec<&str> {
        match self {
            Self::Shell(def) => def.open_face_refs.iter().map(String::as_str).collect(),
            _ => Vec::new(),
        }
    }

    /// Parametric expressions this definition evaluates, as `(field, expr)`.
    pub fn expressions(&self) -> Vec<(&'static str, &str)> {
        if let Self::HelixSweep(def) = self {
            return present([
                ("pitch_expr", def.pitch_expr.as_ref()),
                ("height_expr", def.height_expr.as_ref()),
            ]);
        }
        let (field, expr) = match self {
            Self::Extrude(def) => ("length_expr", def.length_expr.as_ref()),
            Self::Revolve(def) => ("angle_expr", def.angle_expr.as_ref()),
            Self::Hole(def) => ("depth_expr", def.depth_expr.as_ref()),
            Self::Fillet(def) => ("radius_expr", def.radius_expr.as_ref()),
            Self::Chamfer(def) => ("distance_expr", def.distance_expr.as_ref()),
            Self::Shell(def) => ("thickness_expr", def.thickness_expr.as_ref()),
            Self::LinearPattern(def) => ("spacing_expr", def.spacing_expr.as_ref()),
            Self::Sketch(_)
            | Self::CircularPattern(_)
            | Self::MirrorPattern(_)
            | Self::ImportedSolid(_)
            | Self::Loft(_)
            | Self::Sweep(_)
            | Self::HelixSweep(_) => return Vec::new(),
        };
        expr.map(|expr| vec![(field, expr.as_str())])
            .unwrap_or_default()
    }
}

/// Keep the `(field, value)` pairs whose value is present.
fn present<'a, const N: usize>(
    candidates: [(&'static str, Option<&'a String>); N],
) -> Vec<(&'static str, &'a str)> {
    candidates
        .into_iter()
        .filter_map(|(field, value)| value.map(|value| (field, value.as_str())))
        .collect()
}

/// Derive the complete Feature Graph for `nodes` in display `order`.
///
/// Fails when `order` is not a permutation of the node IDs, when an input
/// names an unknown feature, when the dependencies are cyclic, or when the
/// display order places a feature before one of its inputs.
pub fn derive_feature_graph(
    nodes: &[FeatureNode],
    order: &[String],
    sketches: &[Sketch],
    semantic_refs: &[TopoRef],
) -> Result<FeatureGraph> {
    let by_id: BTreeMap<&str, &FeatureNode> =
        nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    if by_id.len() != nodes.len() {
        return Err(OpenCadError::validation("feature node IDs must be unique"));
    }
    let ordered: BTreeSet<&str> = order.iter().map(String::as_str).collect();
    if ordered.len() != order.len() || ordered != by_id.keys().copied().collect() {
        return Err(OpenCadError::validation(
            "feature order must list every feature node exactly once",
        ));
    }

    let created_by: BTreeMap<&str, &str> = semantic_refs
        .iter()
        .map(|topo_ref| {
            (
                topo_ref.ref_id.as_str(),
                topo_ref.semantic.created_by.as_str(),
            )
        })
        .collect();
    let sketch_face_ref = |sketch_id: &str| {
        sketches
            .iter()
            .find(|sketch| sketch.id.as_str() == sketch_id)
            .and_then(|sketch| match &sketch.workplane {
                Workplane::FaceRef { face_ref } => Some(face_ref.as_str()),
                _ => None,
            })
    };

    let mut graph = FeatureGraph::new();
    for id in order {
        let node = by_id[id.as_str()];
        let mut entry = FeatureEntry::new(&node.id, &node.name, node.definition.feature_type());
        entry.suppressed = node.suppressed;
        graph.add_feature(entry)?;
    }

    // Direct inputs first: they alone define the body history chain.
    let mut inputs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for id in order {
        let node = by_id[id.as_str()];
        for (field, source) in node.definition.feature_inputs() {
            if !by_id.contains_key(source) {
                return Err(OpenCadError::validation(format!(
                    "feature '{}' {field} references unknown feature '{source}'",
                    node.id
                )));
            }
            inputs.entry(node.id.as_str()).or_default().push(source);
        }
    }
    let is_ancestor = |ancestor: &str, of: &str| {
        let mut stack: Vec<&str> = inputs.get(of).cloned().unwrap_or_default();
        let mut visited = BTreeSet::new();
        while let Some(current) = stack.pop() {
            if current == ancestor {
                return true;
            }
            if visited.insert(current) {
                stack.extend(inputs.get(current).into_iter().flatten().copied());
            }
        }
        false
    };

    let mut seen = BTreeSet::new();
    for id in order {
        let node = by_id[id.as_str()];
        let mut sources: Vec<&str> = inputs.get(node.id.as_str()).cloned().unwrap_or_default();
        let mut refs = node.definition.reference_inputs();
        // A sketch's face-ref workplane is resolved when a consumer of the
        // sketch executes, so it is an input of that consumer, not of the
        // (geometry-free) sketch feature itself.
        for (field, input) in node.definition.feature_inputs() {
            if field != "sketch_feature" {
                continue;
            }
            if let Some(FeatureDefinition::Sketch(def)) =
                by_id.get(input).map(|sketch| &sketch.definition)
            {
                if let Some(face_ref) = sketch_face_ref(&def.sketch_id) {
                    refs.push(("workplane.face_ref", face_ref));
                }
            }
        }
        for (_, ref_id) in refs {
            // A reference adds an edge only when its creating feature is not
            // already upstream through direct inputs; a reference created by
            // an unknown feature or by the consumer itself adds no ordering
            // constraint (reference existence is validated by the patch
            // layer).
            if let Some(source) = created_by.get(ref_id).copied() {
                if source != node.id
                    && by_id.contains_key(source)
                    && !is_ancestor(source, node.id.as_str())
                {
                    sources.push(source);
                }
            }
        }
        for source in sources {
            if seen.insert((source, node.id.as_str())) {
                graph.add_dependency(source, node.id.as_str())?;
            }
        }
    }

    graph
        .recompute_order()
        .map_err(|_| OpenCadError::validation("feature dependencies contain a cycle"))?;
    let position: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect();
    let violations: BTreeSet<String> = graph
        .dependency_edges()
        .iter()
        .filter(|edge| position[edge.source.as_str()] > position[edge.target.as_str()])
        .map(|edge| {
            format!(
                "feature order places '{}' before its input '{}'",
                edge.target, edge.source
            )
        })
        .collect();
    if !violations.is_empty() {
        return Err(OpenCadError::validation(
            violations.into_iter().collect::<Vec<_>>().join("; "),
        ));
    }
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bracket_with_hole;

    fn bracket() -> (Vec<FeatureNode>, Vec<String>, Vec<Sketch>) {
        let part = bracket_with_hole().expect("bracket");
        (
            part.nodes.values().cloned().collect(),
            part.graph.ordered_ids().to_vec(),
            part.sketches.values().cloned().collect(),
        )
    }

    #[test]
    fn derivation_reproduces_the_bracket_template_graph() {
        let part = bracket_with_hole().expect("bracket");
        let (nodes, order, sketches) = bracket();
        let derived = derive_feature_graph(&nodes, &order, &sketches, &[]).expect("derive");
        assert_eq!(derived, part.graph);
    }

    #[test]
    fn derivation_rejects_bad_order_unknown_inputs_and_misordering() {
        let (nodes, order, sketches) = bracket();

        let missing = derive_feature_graph(&nodes, &order[1..], &sketches, &[]);
        assert!(missing.unwrap_err().to_string().contains("exactly once"));

        let mut reversed = order.clone();
        reversed.reverse();
        let misordered = derive_feature_graph(&nodes, &reversed, &sketches, &[]);
        assert!(misordered
            .unwrap_err()
            .to_string()
            .contains("before its input"));

        let mut broken = nodes.clone();
        for node in &mut broken {
            if let FeatureDefinition::Hole(hole) = &mut node.definition {
                hole.target_feature = "feature:nowhere".into();
            }
        }
        let unknown = derive_feature_graph(&broken, &order, &sketches, &[]);
        assert!(unknown
            .unwrap_err()
            .to_string()
            .contains("target_feature references unknown feature 'feature:nowhere'"));
    }

    /// Guard against an input field being added to a definition without
    /// being registered here: every serialized `*_feature` and `*_ref`
    /// string field must be reported by `feature_inputs`/`reference_inputs`.
    #[test]
    fn every_serialized_input_field_is_registered() {
        let part = crate::robot_joint_actuator_housing().expect("flagship");
        let mut samples: Vec<FeatureDefinition> = part
            .nodes
            .values()
            .map(|node| node.definition.clone())
            .collect();
        samples.push(FeatureDefinition::ImportedSolid(
            crate::ImportedSolidFeature {
                source: "imports/motor.step".into(),
                sha256: "0".repeat(64),
                transform: opencad_geometry::RigidTransform::identity(),
                operation: opencad_geometry::ExtrudeOperation::Cut,
                target_feature: Some("feature:plate".into()),
            },
        ));
        samples.push(FeatureDefinition::Sweep(crate::SweepFeature {
            sketch_feature: "feature:sketch_profile".into(),
            profile_ref: "sketch:profile/profile:outer".into(),
            path_sketch_feature: "feature:sketch_path".into(),
            operation: opencad_geometry::ExtrudeOperation::Cut,
            target_feature: Some("feature:plate".into()),
        }));
        samples.push(FeatureDefinition::HelixSweep(crate::HelixSweepFeature {
            sketch_feature: "feature:sketch_wire".into(),
            profile_ref: "sketch:wire/profile:outer".into(),
            axis_origin_m: [0.0, 0.0, 0.0],
            axis_direction_m: [0.0, 0.0, 1.0],
            pitch_m: 0.005,
            pitch_expr: Some("pitch".into()),
            height_m: 0.02,
            height_expr: None,
            operation: opencad_geometry::ExtrudeOperation::Join,
            target_feature: Some("feature:rod".into()),
        }));
        samples.push(FeatureDefinition::Loft(crate::LoftFeature {
            sections: vec![
                crate::LoftSection {
                    sketch_feature: "feature:sketch_low".into(),
                    profile_ref: "sketch:low/profile:outer".into(),
                },
                crate::LoftSection {
                    sketch_feature: "feature:sketch_high".into(),
                    profile_ref: "sketch:high/profile:outer".into(),
                },
            ],
            operation: opencad_geometry::ExtrudeOperation::Join,
            target_feature: Some("feature:plate".into()),
        }));
        samples.push(FeatureDefinition::Shell(crate::ShellFeature::new(
            "feature:plate",
            opencad_core::Length::from_meters(0.002),
            None,
            vec!["ref:face:plate_top".into()],
        )));
        for extra in [
            crate::bracket_edge_fillet(),
            crate::bracket_with_top_chamfer(),
            crate::bracket_pin_mirror(),
            crate::bracket_hole_row(),
            crate::revolve_bushing(),
        ] {
            samples.extend(
                extra
                    .expect("template")
                    .nodes
                    .values()
                    .map(|node| node.definition.clone()),
            );
        }
        let mut covered_types = BTreeSet::new();
        for definition in samples {
            covered_types.insert(definition.feature_type());
            let serialized = serde_json::to_value(&definition).expect("serialize");
            let object = serialized.as_object().expect("object");
            let registered: BTreeSet<&str> = definition
                .feature_inputs()
                .into_iter()
                .chain(definition.reference_inputs())
                .map(|(field, _)| field)
                .collect();
            for (key, value) in object {
                let is_input = key.ends_with("_feature") || key.ends_with("_ref");
                let is_input_list = key.ends_with("_refs") && value.is_array();
                if (is_input && value.is_string() && key != "profile_ref") || is_input_list {
                    assert!(
                        registered.contains(key.as_str()),
                        "{} field '{key}' is not registered as a graph input",
                        definition.feature_type()
                    );
                }
            }
        }
        for feature_type in [
            "sketch",
            "extrude",
            "revolve",
            "hole",
            "fillet",
            "chamfer",
            "linear_pattern",
            "circular_pattern",
            "mirror_pattern",
            "imported_solid",
            "shell",
            "loft",
            "sweep",
            "helix_sweep",
        ] {
            assert!(
                covered_types.contains(feature_type),
                "no sample covers '{feature_type}'"
            );
        }
    }
}
