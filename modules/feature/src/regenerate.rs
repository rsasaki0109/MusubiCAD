//! Feature graph regeneration pipeline (Task-098+).

use indexmap::IndexMap;

use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::{ExtrudeExtent, FaceDerivation, FaceRefDiscovery, GeometryKernel, KernelBody, TopoRef};
use opencad_graph::FeatureGraph;
use opencad_sketch::Sketch;

use opencad_graph::ParamGraph;

use crate::face_discover::discover_face_refs_from_body;
use crate::extrude::ExtrudeFeature;
use crate::chamfer::ChamferFeature;
use crate::feature::{FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use crate::fillet::FilletFeature;
use crate::hole::HoleFeature;
use crate::param_apply::apply_parameters;
use crate::registry::FeatureRegistry;
use crate::sketch_bridge::prepare_sketch;
use crate::sketch_feature::{validate_sketch, SketchFeatureDef};

/// Part model: feature graph, definitions, and sketch storage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartModel {
    pub graph: FeatureGraph,
    pub nodes: IndexMap<String, FeatureNode>,
    pub sketches: IndexMap<String, Sketch>,
    pub outputs: IndexMap<String, FeatureOutput>,
}

/// Summary of a regeneration pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegenReport {
    pub regenerated: Vec<String>,
    pub skipped_suppressed: Vec<String>,
    /// Face derivation pairs accumulated across modifying features in regen order.
    pub face_history: Vec<FaceDerivation>,
}

/// Session context passed to feature executors during regeneration.
pub struct RegenSession<'a, K: GeometryKernel> {
    pub kernel: &'a K,
    pub nodes: &'a IndexMap<String, FeatureNode>,
    pub sketches: &'a IndexMap<String, Sketch>,
    pub outputs: &'a IndexMap<String, FeatureOutput>,
    pub semantic_refs: &'a [TopoRef],
    pub face_history: &'a [FaceDerivation],
    pub face_discoveries: &'a [FaceRefDiscovery],
}

impl<K: GeometryKernel> RegenContext for RegenSession<'_, K> {
    fn kernel(&self) -> &dyn GeometryKernel {
        self.kernel
    }

    fn sketch_for_feature(&self, sketch_feature_id: &str) -> Result<&Sketch> {
        let node = self
            .nodes
            .get(sketch_feature_id)
            .ok_or_else(|| OpenCadError::not_found(format!("feature '{sketch_feature_id}'")))?;
        let FeatureDefinition::Sketch(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "feature '{sketch_feature_id}' is not a sketch"
            )));
        };
        self.sketches.get(&def.sketch_id).ok_or_else(|| {
            OpenCadError::not_found(format!("sketch '{}'", def.sketch_id))
        })
    }

    fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody> {
        self.outputs
            .get(feature_id)
            .and_then(|o| o.body.clone())
            .ok_or_else(|| OpenCadError::not_found(format!("body for feature '{feature_id}'")))
    }

    fn semantic_refs(&self) -> &[TopoRef] {
        self.semantic_refs
    }

    fn face_history(&self) -> &[FaceDerivation] {
        self.face_history
    }

    fn face_discoveries(&self) -> &[opencad_geometry::FaceRefDiscovery] {
        self.face_discoveries
    }
}

impl PartModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_sketch(&mut self, sketch: Sketch) -> Result<()> {
        if self.sketches.contains_key(sketch.id.as_str()) {
            return Err(OpenCadError::validation(format!(
                "sketch '{}' already exists",
                sketch.id
            )));
        }
        self.sketches.insert(sketch.id.as_str().to_string(), sketch);
        Ok(())
    }

    pub fn add_node(&mut self, node: FeatureNode) -> Result<()> {
        if self.nodes.contains_key(&node.id) {
            return Err(OpenCadError::validation(format!(
                "feature '{}' already exists",
                node.id
            )));
        }
        self.graph.add_feature(opencad_graph::FeatureEntry::new(
            node.id.clone(),
            node.name.clone(),
            node.definition.feature_type(),
        ))?;
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn add_dependency(&mut self, source: &str, target: &str) -> Result<()> {
        self.graph.add_dependency(source, target)
    }

    pub fn prepare_sketches(&mut self) -> Result<()> {
        for sketch in self.sketches.values_mut() {
            prepare_sketch(sketch)?;
            validate_sketch(sketch)?;
        }
        Ok(())
    }

    pub fn regenerate<K: GeometryKernel>(
        &mut self,
        kernel: &K,
        registry: &FeatureRegistry,
        parameters: Option<&ParamGraph>,
        semantic_refs: Option<&[TopoRef]>,
    ) -> Result<RegenReport> {
        if let Some(params) = parameters {
            apply_parameters(self, params)?;
        }
        self.prepare_sketches()?;
        self.outputs.clear();

        let order = self.graph.recompute_order()?;
        let mut report = RegenReport::default();
        let refs = semantic_refs.unwrap_or(&[]);
        let mut face_discoveries: Vec<FaceRefDiscovery> = Vec::new();
        let node_list: Vec<FeatureNode> = self.nodes.values().cloned().collect();

        for feature_id in order {
            let Some(node) = self.nodes.get(&feature_id) else {
                continue;
            };
            if node.suppressed {
                report.skipped_suppressed.push(feature_id);
                continue;
            }

            let session = RegenSession {
                kernel,
                nodes: &self.nodes,
                sketches: &self.sketches,
                outputs: &self.outputs,
                semantic_refs: refs,
                face_history: &report.face_history,
                face_discoveries: &face_discoveries,
            };

            let output = registry.execute(node, &session)?;
            if let Some(ref body) = output.body {
                report
                    .face_history
                    .extend(kernel.face_derivation_history(body));
                if !refs.is_empty() {
                    face_discoveries =
                        discover_face_refs_from_body(kernel, body, &node_list).unwrap_or_default();
                }
            }
            self.outputs.insert(feature_id.clone(), output);
            report.regenerated.push(feature_id);
        }

        Ok(report)
    }

    pub fn active_body(&self) -> Option<&KernelBody> {
        self.graph
            .ordered_ids()
            .iter()
            .rev()
            .find_map(|id| self.outputs.get(id).and_then(|o| o.body.as_ref()))
    }
}

/// Build the bracket base plate model used in architecture samples.
pub fn bracket_base_plate() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::{Constraint, DistanceTarget},
        entity::{Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };
    use opencad_graph::bracket_parameters;

    use crate::param_apply::apply_parameters;

    let mut sketch = Sketch::new(
        SketchId::new("sketch:base")?,
        "Base Sketch",
        Workplane::xy(),
    );

    let corners = ["ent:c0", "ent:c1", "ent:c2", "ent:c3"];
    let edges = ["ent:e0", "ent:e1", "ent:e2", "ent:e3"];
    for (id, x, y) in [
        (corners[0], 0.0, 0.0),
        (corners[1], 0.08, 0.0),
        (corners[2], 0.08, 0.06),
        (corners[3], 0.0, 0.06),
    ] {
        sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new(id)?,
                construction: false,
            },
            x: Coord::literal(x),
            y: Coord::literal(y),
        }))?;
    }
    for (id, start, end) in [
        (edges[0], corners[0], corners[1]),
        (edges[1], corners[1], corners[2]),
        (edges[2], corners[2], corners[3]),
        (edges[3], corners[3], corners[0]),
    ] {
        sketch.add_entity(SketchEntity::Line(LineEntity {
            base: EntityBase {
                id: EntityId::new(id)?,
                construction: false,
            },
            start: EntityId::new(start)?,
            end: EntityId::new(end)?,
        }))?;
    }
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new("con:width")?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(edges[0])?,
        },
        expr: Expression::new("width")?,
    })?;
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new("con:height")?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(edges[1])?,
        },
        expr: Expression::new("height")?,
    })?;
    let mut model = PartModel::new();
    model.sketches.insert(sketch.id.as_str().to_string(), sketch);
    apply_parameters(&mut model, &bracket_parameters())?;
    model.add_node(FeatureNode::new(
        "feature:sketch_base",
        "Base Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:base".into(),
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:extrude_base",
        "Extrude Base",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_base".into(),
            profile_ref: "sketch:base/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("thickness".into()),
            target_feature: None,
        }),
    ))?;
    model.add_dependency("feature:sketch_base", "feature:extrude_base")?;
    Ok(model)
}

/// Bracket base plate with a centered mounting hole.
pub fn bracket_with_hole() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{Coord, EntityBase, CircleEntity, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    use opencad_graph::bracket_parameters;

    let mut model = bracket_base_plate()?;
    let mut hole_sketch = Sketch::new(
        SketchId::new("sketch:hole")?,
        "Mounting Hole",
        Workplane::xy(),
    );
    hole_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:hole_center")?,
            construction: false,
        },
        x: Coord::literal(0.04),
        y: Coord::literal(0.03),
    }))?;
    hole_sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new("ent:hole_circle")?,
            construction: false,
        },
        center: EntityId::new("ent:hole_center")?,
        radius: Coord::expr("hole_diameter / 2")?,
    }))?;
    hole_sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new("con:hole_radius")?,
        target: EntityId::new("ent:hole_circle")?,
        expr: Expression::new("hole_diameter / 2")?,
    })?;
    model
        .sketches
        .insert(hole_sketch.id.as_str().to_string(), hole_sketch);

    apply_parameters(&mut model, &bracket_parameters())?;

    model.add_node(FeatureNode::new(
        "feature:sketch_hole",
        "Hole Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:hole".into(),
        }),
    ))?;
    let mut hole = HoleFeature::through(
        "feature:sketch_hole",
        "sketch:hole/profile:outer",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.006),
        },
        "feature:extrude_base",
    );
    hole.depth_expr = Some("thickness".into());
    model.add_node(FeatureNode::new(
        "feature:hole_mount",
        "Mounting Hole",
        FeatureDefinition::Hole(hole),
    ))?;
    model.add_dependency("feature:sketch_hole", "feature:hole_mount")?;
    model.add_dependency("feature:extrude_base", "feature:hole_mount")?;
    Ok(model)
}

/// Bracket with mounting hole and top-edge fillet.
pub fn bracket_with_top_fillet() -> Result<PartModel> {
    use opencad_graph::bracket_parameters;

    let mut model = bracket_with_hole()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    model.add_node(FeatureNode::new(
        "feature:fillet_top",
        "Top Edge Fillet",
        FeatureDefinition::Fillet(FilletFeature::top_perimeter(
            "feature:hole_mount",
            Length::from_meters(0.001),
            Some("fillet_radius".into()),
        )),
    ))?;
    model.add_dependency("feature:hole_mount", "feature:fillet_top")?;
    Ok(model)
}

/// Bracket with mounting hole and top-edge chamfer.
pub fn bracket_with_top_chamfer() -> Result<PartModel> {
    use opencad_graph::bracket_parameters;

    let mut model = bracket_with_hole()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    model.add_node(FeatureNode::new(
        "feature:chamfer_top",
        "Top Edge Chamfer",
        FeatureDefinition::Chamfer(ChamferFeature::top_perimeter(
            "feature:hole_mount",
            Length::from_meters(0.0005),
            Some("chamfer_distance".into()),
        )),
    ))?;
    model.add_dependency("feature:hole_mount", "feature:chamfer_top")?;
    Ok(model)
}

#[cfg(test)]
pub(crate) struct TestRegenContext {
    kernel: opencad_geometry::MockGeometryKernel,
    sketches: IndexMap<String, Sketch>,
    pub(crate) outputs: IndexMap<String, FeatureOutput>,
    nodes: IndexMap<String, FeatureNode>,
}

#[cfg(test)]
impl TestRegenContext {
    pub(crate) fn empty() -> Self {
        Self {
            kernel: opencad_geometry::MockGeometryKernel::new(),
            sketches: IndexMap::new(),
            outputs: IndexMap::new(),
            nodes: IndexMap::new(),
        }
    }

    pub(crate) fn with_body(feature_id: impl Into<String>, body: KernelBody) -> Self {
        let mut ctx = Self::empty();
        ctx.outputs.insert(
            feature_id.into(),
            FeatureOutput {
                body: Some(body),
            },
        );
        ctx
    }
}

#[cfg(test)]
impl RegenContext for TestRegenContext {
    fn kernel(&self) -> &dyn GeometryKernel {
        &self.kernel
    }

    fn sketch_for_feature(&self, sketch_feature_id: &str) -> Result<&Sketch> {
        let node = self
            .nodes
            .get(sketch_feature_id)
            .ok_or_else(|| OpenCadError::not_found(format!("feature '{sketch_feature_id}'")))?;
        let FeatureDefinition::Sketch(def) = &node.definition else {
            return Err(OpenCadError::validation("not a sketch feature"));
        };
        self.sketches
            .get(&def.sketch_id)
            .ok_or_else(|| OpenCadError::not_found(format!("sketch '{}'", def.sketch_id)))
    }

    fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody> {
        self.outputs
            .get(feature_id)
            .and_then(|o| o.body.clone())
            .ok_or_else(|| OpenCadError::not_found(format!("body for feature '{feature_id}'")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_geometry::MockGeometryKernel;

    #[test]
    fn regenerates_bracket_base_plate() {
        let mut model = bracket_base_plate().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let report = model.regenerate(&kernel, &registry, None, None).expect("regen");
        assert_eq!(report.regenerated.len(), 2);
        let body = model.active_body().expect("body");
        assert!(body.0 > 0);
    }

    #[test]
    fn regenerate_preserves_bracket_profile() {
        let mut model = bracket_base_plate().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        model.regenerate(&kernel, &registry, None, None).expect("regen");
        let sketch = model.sketches.get("sketch:base").expect("sketch");
        let solved =
            crate::sketch_bridge::profile_to_solved(sketch, "sketch:base/profile:outer")
                .expect("solved");
        assert_eq!(solved.points.len(), 4, "{:?}", solved.points);
    }

    #[test]
    fn bracket_profile_has_four_corners() {
        let model = bracket_base_plate().expect("model");
        let sketch = model.sketches.get("sketch:base").expect("sketch");
        let solved =
            crate::sketch_bridge::profile_to_solved(sketch, "sketch:base/profile:outer")
                .expect("solved");
        assert_eq!(solved.points.len(), 4, "{:?}", solved.points);
        assert!((solved.points[1][0] - 0.08).abs() < 1e-6);
        assert!((solved.points[1][1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn extrude_volume_is_positive_with_mock_kernel() {
        let mut model = bracket_base_plate().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        model.regenerate(&kernel, &registry, None, None).expect("regen");
        let body = model.active_body().expect("body");
        let mass = kernel.mass_properties(body, 2700.0).expect("mass");
        assert!(mass.volume_m3 > 0.0);
        assert!(mass.mass_kg > 0.0);
    }

    #[test]
    fn suppressed_features_are_skipped() {
        let mut model = bracket_base_plate().expect("model");
        model
            .nodes
            .get_mut("feature:extrude_base")
            .expect("node")
            .suppressed = true;
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let report = model.regenerate(&kernel, &registry, None, None).expect("regen");
        assert_eq!(report.regenerated, vec!["feature:sketch_base"]);
        assert_eq!(
            report.skipped_suppressed,
            vec!["feature:extrude_base"]
        );
        assert!(model.active_body().is_none());
    }

    #[test]
    fn regenerates_bracket_with_top_fillet() {
        let mut model = bracket_with_top_fillet().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let params = opencad_graph::bracket_parameters();
        let report = model
            .regenerate(&kernel, &registry, Some(&params), None)
            .expect("regen");
        assert_eq!(report.regenerated.len(), 5);
        assert!(model.active_body().is_some());
    }

    #[test]
    fn regenerates_bracket_with_top_chamfer() {
        let mut model = bracket_with_top_chamfer().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let params = opencad_graph::bracket_parameters();
        let report = model
            .regenerate(&kernel, &registry, Some(&params), None)
            .expect("regen");
        assert_eq!(report.regenerated.len(), 5);
        assert!(model.active_body().is_some());
    }
}
