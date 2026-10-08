//! Failed-regeneration inspection (MCAD-P6-006).
//!
//! [`PartModel::inspect_regeneration`] regenerates a copy of the model and,
//! when regeneration fails, names the first failing node and splits the
//! remaining features into those that completed, those the failure blocks,
//! and those regeneration never reached.  The model itself is never changed.

use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use opencad_geometry::{GeometryKernel, TopoRef};
use opencad_graph::ParamGraph;

use crate::cache::RegenerationCache;
use crate::feature::FeatureDefinition;
use crate::regenerate::{PartModel, RegenProgress, RegenReport, RegenerationStage};
use crate::registry::FeatureRegistry;

/// Where and why a regeneration stopped.
///
/// Feature lists follow the Feature Graph recompute order (insertion order
/// when the graph cannot be ordered).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegenerationFailure {
    pub stage: RegenerationStage,
    /// The failing sketch or feature; absent for the parameter and feature
    /// graph stages, which fail as a whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    pub error: String,
    /// Features that produced an output before the failure.  Their outputs
    /// stay available for inspection.
    pub completed_features: Vec<String>,
    /// Features that depend on the failing node, so cannot regenerate until
    /// it is repaired.  A failing sketch blocks the sketch features that use
    /// it; a parameter or graph failure blocks every unsuppressed feature.
    pub blocked_features: Vec<String>,
    /// Features independent of the failure that regeneration stopped before.
    pub not_reached_features: Vec<String>,
    pub skipped_suppressed: Vec<String>,
    /// The last completed upstream feature of the failing feature that has
    /// a body: the result it was building on, kept for inspection.  Absent
    /// when the failure is not in a feature or nothing upstream completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_body_feature: Option<String>,
}

/// Result of [`PartModel::inspect_regeneration`].
#[derive(Debug, Clone)]
pub struct RegenerationInspection {
    /// A copy of the model holding the outputs that were produced: every
    /// feature on success, the completed upstream features on failure.
    pub model: PartModel,
    pub outcome: std::result::Result<RegenReport, RegenerationFailure>,
}

impl PartModel {
    /// Regenerate a copy of this model and report where it fails, if it does.
    ///
    /// Read-only: `self`, including its outputs, is unchanged whatever the
    /// outcome.
    pub fn inspect_regeneration<K: GeometryKernel>(
        &self,
        kernel: &K,
        registry: &FeatureRegistry,
        parameters: Option<&ParamGraph>,
        semantic_refs: Option<&[TopoRef]>,
    ) -> RegenerationInspection {
        let mut model = self.clone();
        let mut progress = RegenProgress::default();
        let outcome = model
            .regenerate_tracked(
                kernel,
                registry,
                parameters,
                semantic_refs,
                &mut RegenerationCache::new(),
                &mut progress,
            )
            .map_err(|error| failure(&model, progress, error.to_string()));
        RegenerationInspection { model, outcome }
    }
}

fn failure(model: &PartModel, progress: RegenProgress, error: String) -> RegenerationFailure {
    let order = model
        .graph
        .recompute_order()
        .unwrap_or_else(|_| model.graph.ordered_ids().to_vec());
    let suppressed: BTreeSet<&str> = model
        .nodes
        .values()
        .filter(|node| node.suppressed)
        .map(|node| node.id.as_str())
        .collect();
    let completed: BTreeSet<&str> = progress.completed.iter().map(String::as_str).collect();

    let blocked: BTreeSet<String> = match (progress.stage, progress.node.as_deref()) {
        (RegenerationStage::Feature, Some(feature)) => downstream(model, [feature.to_string()]),
        (RegenerationStage::Sketch, Some(sketch)) => {
            let users: Vec<String> = model
                .nodes
                .values()
                .filter(|node| {
                    matches!(&node.definition, FeatureDefinition::Sketch(def) if def.sketch_id == sketch)
                })
                .map(|node| node.id.clone())
                .collect();
            let mut blocked = downstream(model, users.iter().cloned());
            blocked.extend(users);
            blocked
        }
        _ => order.iter().cloned().collect(),
    };

    let in_order = |keep: &dyn Fn(&str) -> bool| -> Vec<String> {
        order
            .iter()
            .filter(|id| keep(id.as_str()))
            .cloned()
            .collect()
    };
    let failing = progress.node.as_deref();
    let upstream_body_feature = match (progress.stage, failing) {
        (RegenerationStage::Feature, Some(feature)) => {
            let ancestors = upstream(model, feature);
            order
                .iter()
                .rev()
                .find(|id| {
                    ancestors.contains(*id)
                        && model
                            .outputs
                            .get(*id)
                            .is_some_and(|output| output.body.is_some())
                })
                .cloned()
        }
        _ => None,
    };
    RegenerationFailure {
        upstream_body_feature,
        stage: progress.stage,
        completed_features: in_order(&|id| completed.contains(id)),
        blocked_features: in_order(&|id| {
            blocked.contains(id) && !suppressed.contains(id) && Some(id) != failing
        }),
        not_reached_features: in_order(&|id| {
            !completed.contains(id)
                && !blocked.contains(id)
                && !suppressed.contains(id)
                && Some(id) != failing
        }),
        skipped_suppressed: in_order(&|id| suppressed.contains(id)),
        node: progress.node,
        error,
    }
}

/// Features that `feature` transitively depends on.
fn upstream(model: &PartModel, feature: &str) -> BTreeSet<String> {
    let edges = model.graph.dependency_edges();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([feature.to_string()]);
    while let Some(id) = queue.pop_front() {
        for edge in edges.iter().filter(|edge| edge.target == id) {
            if seen.insert(edge.source.clone()) {
                queue.push_back(edge.source.clone());
            }
        }
    }
    seen
}

/// Features that transitively depend on any of `start`, excluding `start`.
fn downstream(model: &PartModel, start: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    let edges = model.graph.dependency_edges();
    let mut seen = BTreeSet::new();
    let mut queue: VecDeque<String> = start.into_iter().collect();
    while let Some(id) = queue.pop_front() {
        for edge in edges.iter().filter(|edge| edge.source == id) {
            if seen.insert(edge.target.clone()) {
                queue.push_back(edge.target.clone());
            }
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extrude::ExtrudeFeature;
    use crate::feature::FeatureNode;
    use crate::regenerate::bracket_with_hole;
    use opencad_core::Length;
    use opencad_geometry::{ExtrudeExtent, MockGeometryKernel};
    use opencad_sketch::entity::SketchEntity;

    fn inspect(model: &PartModel, parameters: Option<&ParamGraph>) -> RegenerationInspection {
        model.inspect_regeneration(
            &MockGeometryKernel::new(),
            &FeatureRegistry::with_defaults(),
            parameters,
            None,
        )
    }

    fn broken_extrude(id: &str) -> FeatureNode {
        FeatureNode::new(
            id,
            "Broken Extrude",
            FeatureDefinition::Extrude(ExtrudeFeature {
                sketch_feature: "feature:missing_sketch".into(),
                profile_ref: "sketch:missing/profile:outer".into(),
                extent: ExtrudeExtent::Distance {
                    length: Length::from_meters(0.01),
                },
                operation: opencad_geometry::ExtrudeOperation::NewBody,
                length_expr: None,
                target_feature: None,
            }),
        )
    }

    #[test]
    fn a_regenerating_model_reports_every_feature() {
        let model = bracket_with_hole().expect("model");
        let inspection = inspect(&model, None);
        let report = inspection.outcome.expect("regenerates");
        assert_eq!(report.regenerated.len(), 4);
        assert_eq!(inspection.model.outputs.len(), 4);
        assert!(
            model.outputs.is_empty(),
            "the model itself is not regenerated"
        );
    }

    #[test]
    fn a_failing_feature_is_named_with_completed_blocked_and_unreached_features() {
        let mut model = bracket_with_hole().expect("model");
        model
            .add_node(broken_extrude("feature:broken"))
            .expect("broken");
        model
            .add_dependency("feature:extrude_base", "feature:broken")
            .expect("edge");
        model
            .add_node(broken_extrude("feature:after"))
            .expect("after");
        model
            .add_dependency("feature:broken", "feature:after")
            .expect("edge");
        let before = model.clone();

        let inspection = inspect(&model, None);
        let failure = inspection.outcome.expect_err("fails");
        assert_eq!(failure.stage, RegenerationStage::Feature);
        assert_eq!(failure.node.as_deref(), Some("feature:broken"));
        assert!(
            failure.error.contains("feature 'feature:broken'"),
            "{failure:?}"
        );
        assert!(failure.completed_features.starts_with(&[
            "feature:sketch_base".to_string(),
            "feature:extrude_base".to_string()
        ]));
        assert_eq!(failure.blocked_features, vec!["feature:after".to_string()]);
        assert_eq!(
            failure.upstream_body_feature.as_deref(),
            Some("feature:extrude_base")
        );

        // Every feature lands in exactly one list.
        let mut all: Vec<String> = failure
            .completed_features
            .iter()
            .chain(&failure.blocked_features)
            .chain(&failure.not_reached_features)
            .cloned()
            .chain(failure.node.clone())
            .collect();
        all.sort();
        let mut expected: Vec<String> = model.nodes.keys().cloned().collect();
        expected.sort();
        assert_eq!(all, expected);

        // Completed upstream outputs stay available for inspection, and the
        // inspected model is unchanged.
        let produced: Vec<&String> = inspection.model.outputs.keys().collect();
        assert_eq!(
            produced,
            failure.completed_features.iter().collect::<Vec<_>>()
        );
        assert_eq!(model, before);
    }

    #[test]
    fn a_failing_sketch_blocks_the_features_that_use_it() {
        // Without its circle the hole sketch has no profile to cut.
        let mut model = bracket_with_hole().expect("model");
        let sketch = model.sketches.get_mut("sketch:hole").expect("sketch");
        sketch
            .entities
            .retain(|entity| !matches!(entity, SketchEntity::Circle(_)));
        sketch.constraints.clear();
        sketch.profiles.clear();

        let failure = inspect(&model, None).outcome.expect_err("fails");
        assert_eq!(failure.stage, RegenerationStage::Sketch);
        assert_eq!(failure.node.as_deref(), Some("sketch:hole"));
        assert!(failure.completed_features.is_empty());
        assert_eq!(
            failure.blocked_features,
            vec![
                "feature:sketch_hole".to_string(),
                "feature:hole_mount".to_string()
            ]
        );
        assert_eq!(
            failure.not_reached_features,
            vec![
                "feature:sketch_base".to_string(),
                "feature:extrude_base".to_string()
            ]
        );
    }

    #[test]
    fn a_parameter_failure_blocks_every_feature() {
        let model = bracket_with_hole().expect("model");
        let mut parameters = opencad_graph::bracket_parameters();
        parameters
            .set_expr("param:thickness", "undefined_length * 2")
            .expect("expr");

        let failure = inspect(&model, Some(&parameters))
            .outcome
            .expect_err("fails");
        assert_eq!(failure.stage, RegenerationStage::Parameters);
        assert_eq!(failure.node, None);
        assert!(failure.completed_features.is_empty());
        assert_eq!(failure.blocked_features.len(), 4);
        assert!(failure.not_reached_features.is_empty());
    }
}
