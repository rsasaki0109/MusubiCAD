//! Feature graph regeneration pipeline (Task-098+).

use std::collections::BTreeMap;
use std::time::Instant;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use opencad_core::{sha256_hex, Length, OpenCadError, Result};
use opencad_geometry::EdgeRefDiscovery;
use opencad_geometry::{
    resolve_all_reference_provenance, CountingGeometryKernel, ExtrudeExtent, FaceDerivation,
    FaceRefDiscovery, GeometryKernel, KernelBody, ReferenceProvenance, TopoRef,
    TopoRefTolerancePolicy,
};
use opencad_graph::FeatureGraph;
use opencad_sketch::Sketch;

use opencad_graph::ParamGraph;

use crate::cache::{CachedFeature, RegenerationCache};
use crate::chamfer::ChamferFeature;
use crate::edge_discover::discover_edge_refs_from_body;
use crate::extrude::ExtrudeFeature;
use crate::face_discover::discover_face_refs_from_body;
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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RegenReport {
    pub regenerated: Vec<String>,
    pub skipped_suppressed: Vec<String>,
    /// Nodes served from the content-addressed cache instead of re-executed
    /// (MCAD-P6-002); empty for a cold regeneration.
    pub cached_nodes: Vec<String>,
    /// Face derivation pairs accumulated across modifying features in regen order.
    pub face_history: Vec<FaceDerivation>,
    pub trace: RegenerationTrace,
    /// Fail-closed provenance for every semantic reference resolved against the
    /// final regenerated topology (MCAD-P6-003).
    pub reference_provenance: Vec<ReferenceProvenance>,
}

/// Serializable evidence produced by one regeneration pass.
///
/// `elapsed_time_ms` is intentionally excluded from `trace_hash_sha256`; timing
/// is observable evidence but not deterministic identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegenerationTrace {
    pub executed_nodes: Vec<String>,
    pub skipped_nodes: Vec<String>,
    pub solver_call_count: u64,
    pub geometry_kernel_call_count: u64,
    pub elapsed_time_ms: u64,
    pub output_hashes_sha256: BTreeMap<String, String>,
    pub trace_hash_sha256: String,
}

impl RegenerationTrace {
    pub fn no_op() -> Self {
        let mut trace = Self::default();
        trace.trace_hash_sha256 = trace.deterministic_hash();
        trace
    }

    /// Construct a trace from observed execution evidence and seal its hash.
    pub fn observed(
        executed_nodes: Vec<String>,
        skipped_nodes: Vec<String>,
        solver_call_count: u64,
        geometry_kernel_call_count: u64,
        elapsed_time_ms: u64,
        output_hashes_sha256: BTreeMap<String, String>,
    ) -> Self {
        let mut trace = Self {
            executed_nodes,
            skipped_nodes,
            solver_call_count,
            geometry_kernel_call_count,
            elapsed_time_ms,
            output_hashes_sha256,
            trace_hash_sha256: String::new(),
        };
        trace.trace_hash_sha256 = trace.deterministic_hash();
        trace
    }

    fn deterministic_hash(&self) -> String {
        let payload = (
            "opencad.regeneration-trace.v1",
            &self.executed_nodes,
            &self.skipped_nodes,
            self.solver_call_count,
            self.geometry_kernel_call_count,
            &self.output_hashes_sha256,
        );
        serde_json::to_vec(&payload)
            .map(|bytes| sha256_hex(&bytes))
            .unwrap_or_default()
    }
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
    pub edge_discoveries: &'a [EdgeRefDiscovery],
}

/// Content-addressable key for one feature output (MCAD-P6-002).
///
/// The key hashes the versioned tag, kernel backend tag, feature id,
/// definition, solved source sketch, and upstream output identity. Any change
/// in a feature's inputs (or any upstream) produces a new key, so a cached
/// output is reused only when its full derivation is unchanged.
fn feature_output_key(
    feature_id: &str,
    node: &FeatureNode,
    source_sketch: Option<&Sketch>,
    upstream_hashes: &[(&str, &str)],
    backend_tag: &str,
) -> Result<String> {
    let identity = (
        "opencad.logical-feature-output.v1",
        backend_tag,
        feature_id,
        &node.definition,
        source_sketch,
        node.suppressed,
        upstream_hashes,
    );
    Ok(sha256_hex(&serde_json::to_vec(&identity)?))
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

    fn semantic_refs(&self) -> &[TopoRef] {
        self.semantic_refs
    }

    fn face_history(&self) -> &[FaceDerivation] {
        self.face_history
    }

    fn face_discoveries(&self) -> &[opencad_geometry::FaceRefDiscovery] {
        self.face_discoveries
    }

    fn edge_discoveries(&self) -> &[opencad_geometry::EdgeRefDiscovery] {
        self.edge_discoveries
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
        self.regenerate_with_cache(
            kernel,
            registry,
            parameters,
            semantic_refs,
            &mut RegenerationCache::new(),
        )
    }

    /// Regenerate, reusing content-addressed outputs for unchanged nodes
    /// (MCAD-P6-002). The cache and kernel must be reused together across calls;
    /// a failed regeneration restores the previous document outputs.
    pub fn regenerate_with_cache<K: GeometryKernel>(
        &mut self,
        kernel: &K,
        registry: &FeatureRegistry,
        parameters: Option<&ParamGraph>,
        semantic_refs: Option<&[TopoRef]>,
        cache: &mut RegenerationCache,
    ) -> Result<RegenReport> {
        let started = Instant::now();
        if let Some(params) = parameters {
            apply_parameters(self, params)?;
        }
        self.prepare_sketches()?;

        let previous_outputs = self.outputs.clone();
        self.outputs.clear();

        let order = self.graph.recompute_order()?;
        let mut report = RegenReport::default();
        let refs = semantic_refs.unwrap_or(&[]);
        let mut face_discoveries: Vec<FaceRefDiscovery> = Vec::new();
        let mut edge_discoveries: Vec<EdgeRefDiscovery> = Vec::new();
        let node_list: Vec<FeatureNode> = self.nodes.values().cloned().collect();
        let counted_kernel = CountingGeometryKernel::new(kernel);
        let solver_call_count = u64::try_from(self.sketches.len()).unwrap_or(u64::MAX);
        let mut output_hashes = BTreeMap::new();

        let result = (|| {
            for feature_id in order {
                let Some(node) = self.nodes.get(&feature_id) else {
                    continue;
                };
                if node.suppressed {
                    report.skipped_suppressed.push(feature_id);
                    continue;
                }

                let mut upstream_hashes: Vec<(&str, &str)> = self
                    .graph
                    .dependency_edges()
                    .iter()
                    .filter(|edge| edge.target == feature_id)
                    .filter_map(|edge| {
                        output_hashes
                            .get(&edge.source)
                            .map(|hash: &String| (edge.source.as_str(), hash.as_str()))
                    })
                    .collect();
                upstream_hashes.sort_unstable();
                let source_sketch = match &node.definition {
                    FeatureDefinition::Sketch(definition) => {
                        self.sketches.get(&definition.sketch_id)
                    }
                    _ => None,
                };
                let output_key = feature_output_key(
                    &feature_id,
                    node,
                    source_sketch,
                    &upstream_hashes,
                    cache.backend_tag(),
                )?;

                if let Some(cached) = cache.get(&output_key) {
                    if cached.output.body.is_some() {
                        face_discoveries = cached.face_discoveries.clone();
                        edge_discoveries = cached.edge_discoveries.clone();
                    }
                    report.face_history.extend(cached.face_history.clone());
                    output_hashes.insert(feature_id.clone(), output_key);
                    self.outputs
                        .insert(feature_id.clone(), cached.output.clone());
                    report.cached_nodes.push(feature_id);
                    continue;
                }

                let session = RegenSession {
                    kernel: &counted_kernel,
                    nodes: &self.nodes,
                    sketches: &self.sketches,
                    outputs: &self.outputs,
                    semantic_refs: refs,
                    face_history: &report.face_history,
                    face_discoveries: &face_discoveries,
                    edge_discoveries: &edge_discoveries,
                };

                let output = registry.execute(node, &session)?;
                let mut face_history_delta = Vec::new();
                if let Some(ref body) = output.body {
                    face_history_delta = counted_kernel.face_derivation_history(body);
                    report.face_history.extend(face_history_delta.clone());
                    if !refs.is_empty() {
                        face_discoveries =
                            discover_face_refs_from_body(&counted_kernel, body, &node_list)
                                .unwrap_or_default();
                        edge_discoveries =
                            discover_edge_refs_from_body(&counted_kernel, body, &node_list)
                                .unwrap_or_default();
                    }
                }
                output_hashes.insert(feature_id.clone(), output_key.clone());
                cache.insert(
                    output_key,
                    CachedFeature {
                        output: output.clone(),
                        face_history: face_history_delta,
                        face_discoveries: face_discoveries.clone(),
                        edge_discoveries: edge_discoveries.clone(),
                    },
                );
                self.outputs.insert(feature_id.clone(), output);
                report.regenerated.push(feature_id);
            }

            report.trace = RegenerationTrace::observed(
                report.regenerated.clone(),
                report.skipped_suppressed.clone(),
                solver_call_count,
                counted_kernel.call_count(),
                u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                output_hashes,
            );

            if !refs.is_empty() {
                report.reference_provenance = resolve_all_reference_provenance(
                    refs,
                    &report.face_history,
                    if face_discoveries.is_empty() {
                        None
                    } else {
                        Some(&face_discoveries)
                    },
                    if edge_discoveries.is_empty() {
                        None
                    } else {
                        Some(&edge_discoveries)
                    },
                    TopoRefTolerancePolicy::default(),
                );
            }

            Ok(report)
        })();
        if result.is_err() {
            self.outputs = previous_outputs;
        }
        result
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
    use opencad_graph::bracket_parameters;
    use opencad_sketch::{
        constraint::{Constraint, DistanceTarget},
        entity::{Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

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
    model
        .sketches
        .insert(sketch.id.as_str().to_string(), sketch);
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

/// Default semantic face refs for the bracket sample.
pub fn bracket_semantic_refs() -> Vec<TopoRef> {
    use opencad_core::TopoRefId;

    vec![
        TopoRef::face(
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
        ),
        TopoRef::edge(
            TopoRefId::new("ref:edge:bracket_top_front").expect("id"),
            "feature:extrude_base",
            "top@+y",
        ),
    ]
}

/// Bracket base plate with a centered mounting hole.
pub fn bracket_with_hole() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
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
    let mut hole = HoleFeature::on_face_ref(
        "feature:sketch_hole",
        "sketch:hole/profile:outer",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.006),
        },
        "feature:extrude_base",
        "ref:face:bracket_top",
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

/// Bracket with mounting hole and a single top-front edge fillet.
pub fn bracket_edge_fillet() -> Result<PartModel> {
    use opencad_graph::bracket_parameters;

    let mut model = bracket_with_hole()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    model.add_node(FeatureNode::new(
        "feature:fillet_front_edge",
        "Front Edge Fillet",
        FeatureDefinition::Fillet(FilletFeature::on_edge_ref(
            "feature:hole_mount",
            "ref:edge:bracket_top_front",
            Length::from_meters(0.001),
            Some("fillet_radius".into()),
        )),
    ))?;
    model.add_dependency("feature:hole_mount", "feature:fillet_front_edge")?;
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

/// Hollow bushing revolved from an XY profile (radius × axis height) around the global Y axis.
pub fn revolve_bushing() -> Result<PartModel> {
    revolve_annulus_model(
        "Bushing Profile",
        "Revolve Bushing",
        "feature:revolve_bushing",
        std::f64::consts::TAU,
        "360 deg",
    )
}

/// Half bushing (180°) revolved from the same XY annulus profile around the Y axis.
pub fn revolve_sector() -> Result<PartModel> {
    revolve_annulus_model(
        "Sector Profile",
        "Revolve Sector",
        "feature:revolve_sector",
        std::f64::consts::PI,
        "180 deg",
    )
}

fn revolve_annulus_model(
    sketch_name: &str,
    revolve_name: &str,
    revolve_feature_id: &str,
    default_angle_rad: f64,
    angle_expr: &str,
) -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_graph::revolve_parameters;
    use opencad_sketch::{
        constraint::{Constraint, DistanceTarget},
        entity::{Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    use crate::param_apply::apply_parameters;
    use crate::revolve::RevolveFeature;
    use crate::sketch_feature::SketchFeatureDef;

    let mut sketch = Sketch::new(
        SketchId::new("sketch:profile")?,
        sketch_name,
        Workplane::xy(),
    );

    let axis = "ent:axis";
    let corners = ["ent:c0", "ent:c1", "ent:c2", "ent:c3"];
    let edges = ["ent:e0", "ent:e1", "ent:e2", "ent:e3"];
    sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new(axis)?,
            construction: true,
        },
        x: Coord::literal(0.0),
        y: Coord::literal(0.0),
    }))?;
    for (id, radius, height) in [
        (corners[0], 0.015, 0.0),
        (corners[1], 0.025, 0.0),
        (corners[2], 0.025, 0.02),
        (corners[3], 0.015, 0.02),
    ] {
        sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new(id)?,
                construction: false,
            },
            x: Coord::literal(radius),
            y: Coord::literal(height),
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
        id: ConstraintId::new("con:inner_radius")?,
        target: DistanceTarget::PointToPoint {
            a: EntityId::new(axis)?,
            b: EntityId::new(corners[0])?,
        },
        expr: Expression::new("inner_radius")?,
    })?;
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new("con:outer_radius")?,
        target: DistanceTarget::PointToPoint {
            a: EntityId::new(axis)?,
            b: EntityId::new(corners[1])?,
        },
        expr: Expression::new("outer_radius")?,
    })?;
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new("con:height")?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(edges[1])?,
        },
        expr: Expression::new("height")?,
    })?;
    sketch.add_constraint(Constraint::Horizontal {
        id: ConstraintId::new("con:bottom_horizontal")?,
        line: EntityId::new(edges[0])?,
    })?;
    sketch.add_constraint(Constraint::Vertical {
        id: ConstraintId::new("con:outer_vertical")?,
        line: EntityId::new(edges[1])?,
    })?;
    sketch.add_constraint(Constraint::Horizontal {
        id: ConstraintId::new("con:top_horizontal")?,
        line: EntityId::new(edges[2])?,
    })?;
    sketch.add_constraint(Constraint::Vertical {
        id: ConstraintId::new("con:inner_vertical")?,
        line: EntityId::new(edges[3])?,
    })?;

    let params = revolve_parameters(angle_expr);
    let mut model = PartModel::new();
    model
        .sketches
        .insert(sketch.id.as_str().to_string(), sketch);
    apply_parameters(&mut model, &params)?;
    model.add_node(FeatureNode::new(
        "feature:sketch_profile",
        sketch_name,
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:profile".into(),
        }),
    ))?;
    model.add_node(FeatureNode::new(
        revolve_feature_id,
        revolve_name,
        FeatureDefinition::Revolve(RevolveFeature::with_angle(
            "feature:sketch_profile",
            "sketch:profile/profile:outer",
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            default_angle_rad,
            Some("revolve_angle_rad".into()),
        )),
    ))?;
    model.add_dependency("feature:sketch_profile", revolve_feature_id)?;
    Ok(model)
}

/// Bracket plate with a cylindrical boss joined via extrude `join`.
pub fn bracket_boss_join() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    use crate::extrude::ExtrudeFeature;
    use crate::sketch_feature::SketchFeatureDef;
    use opencad_graph::bracket_parameters;

    let mut model = bracket_base_plate()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    let mut boss_sketch = Sketch::new(
        SketchId::new("sketch:boss")?,
        "Boss Sketch",
        Workplane::xy(),
    );
    boss_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:boss_center")?,
            construction: false,
        },
        x: Coord::literal(0.05),
        y: Coord::literal(0.03),
    }))?;
    boss_sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new("ent:boss_circle")?,
            construction: false,
        },
        center: EntityId::new("ent:boss_center")?,
        radius: Coord::expr("hole_diameter / 2")?,
    }))?;
    boss_sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new("con:boss_radius")?,
        target: EntityId::new("ent:boss_circle")?,
        expr: Expression::new("hole_diameter / 2")?,
    })?;
    model
        .sketches
        .insert(boss_sketch.id.as_str().to_string(), boss_sketch);

    model.add_node(FeatureNode::new(
        "feature:sketch_boss",
        "Boss Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:boss".into(),
        }),
    ))?;
    let mut boss = ExtrudeFeature::join(
        "feature:sketch_boss",
        "sketch:boss/profile:outer",
        "feature:extrude_base",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.012),
        },
    );
    boss.length_expr = Some("boss_height".into());
    model.add_node(FeatureNode::new(
        "feature:boss_join",
        "Boss Join",
        FeatureDefinition::Extrude(boss),
    ))?;

    model.add_dependency("feature:sketch_boss", "feature:boss_join")?;
    model.add_dependency("feature:extrude_base", "feature:boss_join")?;
    Ok(model)
}

/// Multi-feature flanged bearing carrier with a raised hub, central bore, and
/// four-hole bolt circle.
pub fn bearing_carrier() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_graph::bearing_carrier_parameters;
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
    };

    use crate::pattern::CircularPatternFeature;

    fn circle_sketch(
        id: &str,
        name: &str,
        entity_prefix: &str,
        center: [f64; 2],
        radius_expr: &str,
    ) -> Result<Sketch> {
        let center_id = format!("ent:{entity_prefix}_center");
        let circle_id = format!("ent:{entity_prefix}_circle");
        let constraint_id = format!("con:{entity_prefix}_radius");
        let mut sketch = Sketch::new(SketchId::new(id)?, name, Workplane::xy());
        sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new(&center_id)?,
                construction: false,
            },
            x: Coord::literal(center[0]),
            y: Coord::literal(center[1]),
        }))?;
        sketch.add_entity(SketchEntity::Circle(CircleEntity {
            base: EntityBase {
                id: EntityId::new(&circle_id)?,
                construction: false,
            },
            center: EntityId::new(&center_id)?,
            radius: Coord::expr(radius_expr)?,
        }))?;
        sketch.add_constraint(Constraint::Radius {
            id: ConstraintId::new(constraint_id)?,
            target: EntityId::new(circle_id)?,
            expr: Expression::new(radius_expr)?,
        })?;
        Ok(sketch)
    }

    let parameters = bearing_carrier_parameters();
    let mut model = bracket_base_plate()?;
    apply_parameters(&mut model, &parameters)?;

    let center = [0.048, 0.036];
    let boss_sketch = circle_sketch(
        "sketch:carrier_boss",
        "Carrier Hub",
        "carrier_boss",
        center,
        "boss_outer_diameter / 2",
    )?;
    model
        .sketches
        .insert(boss_sketch.id.as_str().to_string(), boss_sketch);
    model.add_node(FeatureNode::new(
        "feature:sketch_carrier_boss",
        "Carrier Hub Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:carrier_boss".into(),
        }),
    ))?;
    let mut boss = ExtrudeFeature::join(
        "feature:sketch_carrier_boss",
        "sketch:carrier_boss/profile:outer",
        "feature:extrude_base",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.014),
        },
    );
    boss.length_expr = Some("boss_height".into());
    model.add_node(FeatureNode::new(
        "feature:carrier_boss",
        "Raised Bearing Hub",
        FeatureDefinition::Extrude(boss),
    ))?;
    model.add_dependency("feature:sketch_carrier_boss", "feature:carrier_boss")?;
    model.add_dependency("feature:extrude_base", "feature:carrier_boss")?;

    let bore_sketch = circle_sketch(
        "sketch:bearing_bore",
        "Bearing Bore",
        "bearing_bore",
        center,
        "bore_diameter / 2",
    )?;
    model
        .sketches
        .insert(bore_sketch.id.as_str().to_string(), bore_sketch);
    model.add_node(FeatureNode::new(
        "feature:sketch_bearing_bore",
        "Bearing Bore Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:bearing_bore".into(),
        }),
    ))?;
    let mut bore = HoleFeature::through(
        "feature:sketch_bearing_bore",
        "sketch:bearing_bore/profile:outer",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.022),
        },
        "feature:carrier_boss",
    );
    bore.depth_expr = Some("thickness + boss_height".into());
    model.add_node(FeatureNode::new(
        "feature:bearing_bore",
        "Central Bearing Bore",
        FeatureDefinition::Hole(bore),
    ))?;
    model.add_dependency("feature:sketch_bearing_bore", "feature:bearing_bore")?;
    model.add_dependency("feature:carrier_boss", "feature:bearing_bore")?;

    let bolt_sketch = circle_sketch(
        "sketch:bolt_hole",
        "Bolt Hole Tool",
        "bolt_hole",
        [0.078, 0.036],
        "bolt_hole_diameter / 2",
    )?;
    model
        .sketches
        .insert(bolt_sketch.id.as_str().to_string(), bolt_sketch);
    model.add_node(FeatureNode::new(
        "feature:sketch_bolt_hole",
        "Bolt Hole Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:bolt_hole".into(),
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:bolt_hole_tool",
        "Bolt Hole Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_bolt_hole".into(),
            profile_ref: "sketch:bolt_hole/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.008),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("thickness".into()),
            target_feature: None,
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:bolt_circle",
        "Four Hole Bolt Circle",
        FeatureDefinition::CircularPattern(CircularPatternFeature::cut(
            "feature:bolt_hole_tool",
            "feature:bearing_bore",
            [center[0], center[1], 0.0],
            [0.0, 0.0, 1.0],
            4,
        )),
    ))?;
    model.add_dependency("feature:sketch_bolt_hole", "feature:bolt_hole_tool")?;
    model.add_dependency("feature:bolt_hole_tool", "feature:bolt_circle")?;
    model.add_dependency("feature:bearing_bore", "feature:bolt_circle")?;

    apply_parameters(&mut model, &parameters)?;
    Ok(model)
}

/// Twenty-two-node robot-joint actuator housing with stepped hubs, bearing
/// seats, patterned fasteners and ribs, plus mirrored mounting ears.
pub fn robot_joint_actuator_housing() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_graph::robot_joint_housing_parameters;
    use opencad_sketch::{
        constraint::{Constraint, DistanceTarget},
        entity::{CircleEntity, Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
        workplane::Workplane,
    };

    use crate::pattern::{CircularPatternFeature, MirrorPatternFeature};

    fn rectangle_sketch(
        id: &str,
        name: &str,
        prefix: &str,
        origin: [f64; 2],
        initial_size: [f64; 2],
        width_expr: &str,
        height_expr: &str,
    ) -> Result<Sketch> {
        let corners = [
            format!("ent:{prefix}_c0"),
            format!("ent:{prefix}_c1"),
            format!("ent:{prefix}_c2"),
            format!("ent:{prefix}_c3"),
        ];
        let edges = [
            format!("ent:{prefix}_e0"),
            format!("ent:{prefix}_e1"),
            format!("ent:{prefix}_e2"),
            format!("ent:{prefix}_e3"),
        ];
        let mut sketch = Sketch::new(SketchId::new(id)?, name, Workplane::xy());
        for (corner_id, x, y) in [
            (&corners[0], origin[0], origin[1]),
            (&corners[1], origin[0] + initial_size[0], origin[1]),
            (
                &corners[2],
                origin[0] + initial_size[0],
                origin[1] + initial_size[1],
            ),
            (&corners[3], origin[0], origin[1] + initial_size[1]),
        ] {
            sketch.add_entity(SketchEntity::Point(PointEntity {
                base: EntityBase {
                    id: EntityId::new(corner_id)?,
                    construction: false,
                },
                x: Coord::literal(x),
                y: Coord::literal(y),
            }))?;
        }
        for (edge_id, start, end) in [
            (&edges[0], &corners[0], &corners[1]),
            (&edges[1], &corners[1], &corners[2]),
            (&edges[2], &corners[2], &corners[3]),
            (&edges[3], &corners[3], &corners[0]),
        ] {
            sketch.add_entity(SketchEntity::Line(LineEntity {
                base: EntityBase {
                    id: EntityId::new(edge_id)?,
                    construction: false,
                },
                start: EntityId::new(start)?,
                end: EntityId::new(end)?,
            }))?;
        }
        sketch.add_constraint(Constraint::Distance {
            id: ConstraintId::new(format!("con:{prefix}_width"))?,
            target: DistanceTarget::LineLength {
                line: EntityId::new(&edges[0])?,
            },
            expr: Expression::new(width_expr)?,
        })?;
        sketch.add_constraint(Constraint::Distance {
            id: ConstraintId::new(format!("con:{prefix}_height"))?,
            target: DistanceTarget::LineLength {
                line: EntityId::new(&edges[1])?,
            },
            expr: Expression::new(height_expr)?,
        })?;
        Ok(sketch)
    }

    fn circle_sketch(
        id: &str,
        name: &str,
        prefix: &str,
        center: [f64; 2],
        radius_expr: &str,
    ) -> Result<Sketch> {
        let center_id = format!("ent:{prefix}_center");
        let circle_id = format!("ent:{prefix}_circle");
        let mut sketch = Sketch::new(SketchId::new(id)?, name, Workplane::xy());
        sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new(&center_id)?,
                construction: false,
            },
            x: Coord::literal(center[0]),
            y: Coord::literal(center[1]),
        }))?;
        sketch.add_entity(SketchEntity::Circle(CircleEntity {
            base: EntityBase {
                id: EntityId::new(&circle_id)?,
                construction: false,
            },
            center: EntityId::new(&center_id)?,
            radius: Coord::expr(radius_expr)?,
        }))?;
        sketch.add_constraint(Constraint::Radius {
            id: ConstraintId::new(format!("con:{prefix}_radius"))?,
            target: EntityId::new(circle_id)?,
            expr: Expression::new(radius_expr)?,
        })?;
        Ok(sketch)
    }

    fn add_sketch_feature(
        model: &mut PartModel,
        sketch: Sketch,
        feature_id: &str,
        feature_name: &str,
    ) -> Result<()> {
        let sketch_id = sketch.id.as_str().to_string();
        model.sketches.insert(sketch_id.clone(), sketch);
        model.add_node(FeatureNode::new(
            feature_id,
            feature_name,
            FeatureDefinition::Sketch(SketchFeatureDef { sketch_id }),
        ))
    }

    let parameters = robot_joint_housing_parameters();
    let center = [0.070, 0.055];
    let mut model = PartModel::new();

    add_sketch_feature(
        &mut model,
        rectangle_sketch(
            "sketch:joint_base",
            "Actuator Base",
            "joint_base",
            [0.0, 0.0],
            [0.140, 0.110],
            "width",
            "height",
        )?,
        "feature:sketch_joint_base",
        "Actuator Base Sketch",
    )?;
    model.add_node(FeatureNode::new(
        "feature:joint_base",
        "Actuator Base Plate",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_joint_base".into(),
            profile_ref: "sketch:joint_base/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.010),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("base_thickness".into()),
            target_feature: None,
        }),
    ))?;
    model.add_dependency("feature:sketch_joint_base", "feature:joint_base")?;

    add_sketch_feature(
        &mut model,
        circle_sketch(
            "sketch:lower_hub",
            "Lower Hub",
            "lower_hub",
            center,
            "lower_hub_diameter / 2",
        )?,
        "feature:sketch_lower_hub",
        "Lower Hub Sketch",
    )?;
    let mut lower_hub = ExtrudeFeature::join(
        "feature:sketch_lower_hub",
        "sketch:lower_hub/profile:outer",
        "feature:joint_base",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.020),
        },
    );
    lower_hub.length_expr = Some("lower_hub_height".into());
    model.add_node(FeatureNode::new(
        "feature:lower_hub",
        "Lower Actuator Hub",
        FeatureDefinition::Extrude(lower_hub),
    ))?;
    model.add_dependency("feature:sketch_lower_hub", "feature:lower_hub")?;
    model.add_dependency("feature:joint_base", "feature:lower_hub")?;

    add_sketch_feature(
        &mut model,
        circle_sketch(
            "sketch:upper_hub",
            "Upper Hub",
            "upper_hub",
            center,
            "upper_hub_diameter / 2",
        )?,
        "feature:sketch_upper_hub",
        "Upper Hub Sketch",
    )?;
    let mut upper_hub = ExtrudeFeature::join(
        "feature:sketch_upper_hub",
        "sketch:upper_hub/profile:outer",
        "feature:lower_hub",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.032),
        },
    );
    upper_hub.length_expr = Some("upper_hub_height".into());
    model.add_node(FeatureNode::new(
        "feature:upper_hub",
        "Upper Bearing Tower",
        FeatureDefinition::Extrude(upper_hub),
    ))?;
    model.add_dependency("feature:sketch_upper_hub", "feature:upper_hub")?;
    model.add_dependency("feature:lower_hub", "feature:upper_hub")?;

    add_sketch_feature(
        &mut model,
        circle_sketch(
            "sketch:shaft_bore",
            "Output Shaft Bore",
            "shaft_bore",
            center,
            "shaft_diameter / 2",
        )?,
        "feature:sketch_shaft_bore",
        "Output Shaft Bore Sketch",
    )?;
    let mut shaft_bore = HoleFeature::through(
        "feature:sketch_shaft_bore",
        "sketch:shaft_bore/profile:outer",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.032),
        },
        "feature:upper_hub",
    );
    shaft_bore.depth_expr = Some("upper_hub_height".into());
    model.add_node(FeatureNode::new(
        "feature:shaft_bore",
        "Through Output Shaft Bore",
        FeatureDefinition::Hole(shaft_bore),
    ))?;
    model.add_dependency("feature:sketch_shaft_bore", "feature:shaft_bore")?;
    model.add_dependency("feature:upper_hub", "feature:shaft_bore")?;

    add_sketch_feature(
        &mut model,
        circle_sketch(
            "sketch:counterbore",
            "Bearing Counterbore",
            "counterbore",
            center,
            "counterbore_diameter / 2",
        )?,
        "feature:sketch_counterbore",
        "Bearing Counterbore Sketch",
    )?;
    let mut counterbore = HoleFeature::through(
        "feature:sketch_counterbore",
        "sketch:counterbore/profile:outer",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.008),
        },
        "feature:shaft_bore",
    );
    counterbore.depth_expr = Some("counterbore_depth".into());
    model.add_node(FeatureNode::new(
        "feature:counterbore",
        "Bearing Seat Counterbore",
        FeatureDefinition::Hole(counterbore),
    ))?;
    model.add_dependency("feature:sketch_counterbore", "feature:counterbore")?;
    model.add_dependency("feature:shaft_bore", "feature:counterbore")?;

    let mut pcd_sketch = Sketch::new(
        SketchId::new("sketch:pcd_hole")?,
        "PCD Fastener Tool",
        Workplane::xy(),
    );
    pcd_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:pcd_axis_center")?,
            construction: true,
        },
        x: Coord::literal(center[0]),
        y: Coord::literal(center[1]),
    }))?;
    pcd_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:pcd_hole_center")?,
            construction: false,
        },
        x: Coord::literal(center[0] + 0.045),
        y: Coord::literal(center[1]),
    }))?;
    pcd_sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new("ent:pcd_hole_circle")?,
            construction: false,
        },
        center: EntityId::new("ent:pcd_hole_center")?,
        radius: Coord::expr("bolt_hole_diameter / 2")?,
    }))?;
    pcd_sketch.add_entity(SketchEntity::Line(LineEntity {
        base: EntityBase {
            id: EntityId::new("ent:pcd_radius_line")?,
            construction: true,
        },
        start: EntityId::new("ent:pcd_axis_center")?,
        end: EntityId::new("ent:pcd_hole_center")?,
    }))?;
    pcd_sketch.add_constraint(Constraint::Horizontal {
        id: ConstraintId::new("con:pcd_radius_horizontal")?,
        line: EntityId::new("ent:pcd_radius_line")?,
    })?;
    pcd_sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new("con:pcd_radius")?,
        target: DistanceTarget::LineLength {
            line: EntityId::new("ent:pcd_radius_line")?,
        },
        expr: Expression::new("bolt_circle_radius")?,
    })?;
    pcd_sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new("con:pcd_hole_radius")?,
        target: EntityId::new("ent:pcd_hole_circle")?,
        expr: Expression::new("bolt_hole_diameter / 2")?,
    })?;
    add_sketch_feature(
        &mut model,
        pcd_sketch,
        "feature:sketch_pcd_hole",
        "PCD Fastener Sketch",
    )?;
    model.add_node(FeatureNode::new(
        "feature:pcd_hole_tool",
        "PCD Fastener Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_pcd_hole".into(),
            profile_ref: "sketch:pcd_hole/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.010),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("base_thickness".into()),
            target_feature: None,
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pcd_fasteners",
        "Eight Hole Fastener Circle",
        FeatureDefinition::CircularPattern(CircularPatternFeature::cut(
            "feature:pcd_hole_tool",
            "feature:counterbore",
            [center[0], center[1], 0.0],
            [0.0, 0.0, 1.0],
            8,
        )),
    ))?;
    model.add_dependency("feature:sketch_pcd_hole", "feature:pcd_hole_tool")?;
    model.add_dependency("feature:pcd_hole_tool", "feature:pcd_fasteners")?;
    model.add_dependency("feature:counterbore", "feature:pcd_fasteners")?;

    add_sketch_feature(
        &mut model,
        rectangle_sketch(
            "sketch:rib",
            "Radial Rib Tool",
            "rib",
            [center[0] - 0.002, center[1] - 0.0035],
            [0.038, 0.007],
            "rib_length",
            "rib_thickness",
        )?,
        "feature:sketch_rib",
        "Radial Rib Sketch",
    )?;
    model.add_node(FeatureNode::new(
        "feature:rib_tool",
        "Radial Rib Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_rib".into(),
            profile_ref: "sketch:rib/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.015),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("rib_height".into()),
            target_feature: None,
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:radial_ribs",
        "Six Radial Reinforcement Ribs",
        FeatureDefinition::CircularPattern(CircularPatternFeature::union_on(
            "feature:rib_tool",
            "feature:pcd_fasteners",
            [center[0], center[1], 0.0],
            [0.0, 0.0, 1.0],
            6,
        )),
    ))?;
    model.add_dependency("feature:sketch_rib", "feature:rib_tool")?;
    model.add_dependency("feature:rib_tool", "feature:radial_ribs")?;
    model.add_dependency("feature:pcd_fasteners", "feature:radial_ribs")?;

    add_sketch_feature(
        &mut model,
        rectangle_sketch(
            "sketch:mounting_ear",
            "Mounting Ear Tool",
            "mounting_ear",
            [-0.019, center[1] - 0.017],
            [0.022, 0.034],
            "mounting_ear_width",
            "mounting_ear_depth",
        )?,
        "feature:sketch_mounting_ear",
        "Mounting Ear Sketch",
    )?;
    model.add_node(FeatureNode::new(
        "feature:mounting_ear_tool",
        "Mounting Ear Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_mounting_ear".into(),
            profile_ref: "sketch:mounting_ear/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.016),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("mounting_ear_height".into()),
            target_feature: None,
        }),
    ))?;
    let mut mounting_ears = MirrorPatternFeature::new(
        "feature:mounting_ear_tool",
        [center[0], center[1], 0.0],
        [1.0, 0.0, 0.0],
    );
    mounting_ears.target_feature = Some("feature:radial_ribs".into());
    model.add_node(FeatureNode::new(
        "feature:mounting_ears",
        "Mirrored Mounting Ears",
        FeatureDefinition::MirrorPattern(mounting_ears),
    ))?;
    model.add_dependency("feature:sketch_mounting_ear", "feature:mounting_ear_tool")?;
    model.add_dependency("feature:mounting_ear_tool", "feature:mounting_ears")?;
    model.add_dependency("feature:radial_ribs", "feature:mounting_ears")?;

    add_sketch_feature(
        &mut model,
        circle_sketch(
            "sketch:mounting_hole",
            "Mounting Hole Tool",
            "mounting_hole",
            [-0.008, center[1]],
            "mounting_hole_diameter / 2",
        )?,
        "feature:sketch_mounting_hole",
        "Mounting Hole Sketch",
    )?;
    model.add_node(FeatureNode::new(
        "feature:mounting_hole_tool",
        "Mounting Hole Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_mounting_hole".into(),
            profile_ref: "sketch:mounting_hole/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.016),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("mounting_ear_height".into()),
            target_feature: None,
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:mounting_holes",
        "Mirrored Mounting Holes",
        FeatureDefinition::MirrorPattern(MirrorPatternFeature::cut(
            "feature:mounting_hole_tool",
            "feature:mounting_ears",
            [center[0], center[1], 0.0],
            [1.0, 0.0, 0.0],
        )),
    ))?;
    model.add_dependency("feature:sketch_mounting_hole", "feature:mounting_hole_tool")?;
    model.add_dependency("feature:mounting_hole_tool", "feature:mounting_holes")?;
    model.add_dependency("feature:mounting_ears", "feature:mounting_holes")?;

    apply_parameters(&mut model, &parameters)?;
    Ok(model)
}

// ---------------------------------------------------------------------------
// Robot arm flagship assembly parts
// ---------------------------------------------------------------------------

/// Circle sketch centered on a literal 2D point.
fn robot_arm_circle_sketch(
    id: &str,
    name: impl Into<String>,
    prefix: &str,
    center: [f64; 2],
    radius_expr: &str,
) -> Result<Sketch> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
    };

    let center_id = format!("ent:{prefix}_center");
    let circle_id = format!("ent:{prefix}_circle");
    let mut sketch = Sketch::new(SketchId::new(id)?, name, Workplane::xy());
    sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new(&center_id)?,
            construction: false,
        },
        x: Coord::literal(center[0]),
        y: Coord::literal(center[1]),
    }))?;
    sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new(&circle_id)?,
            construction: false,
        },
        center: EntityId::new(&center_id)?,
        radius: Coord::expr(radius_expr)?,
    }))?;
    sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new(format!("con:{prefix}_radius"))?,
        target: EntityId::new(circle_id)?,
        expr: Expression::new(radius_expr)?,
    })?;
    Ok(sketch)
}

/// Circle sketch centered at `(0, offset)` driven by a vertical construction line.
fn robot_arm_offset_circle_sketch(
    id: &str,
    name: impl Into<String>,
    prefix: &str,
    initial_offset: f64,
    offset_expr: &str,
    radius_expr: &str,
) -> Result<Sketch> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::{Constraint, DistanceTarget},
        entity::{CircleEntity, Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
        workplane::Workplane,
    };

    let origin_id = format!("ent:{prefix}_origin");
    let center_id = format!("ent:{prefix}_center");
    let circle_id = format!("ent:{prefix}_circle");
    let line_id = format!("ent:{prefix}_offset_line");
    let mut sketch = Sketch::new(SketchId::new(id)?, name, Workplane::xy());
    sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new(&origin_id)?,
            construction: true,
        },
        x: Coord::literal(0.0),
        y: Coord::literal(0.0),
    }))?;
    sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new(&center_id)?,
            construction: false,
        },
        x: Coord::literal(0.0),
        y: Coord::literal(initial_offset),
    }))?;
    sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new(&circle_id)?,
            construction: false,
        },
        center: EntityId::new(&center_id)?,
        radius: Coord::expr(radius_expr)?,
    }))?;
    sketch.add_entity(SketchEntity::Line(LineEntity {
        base: EntityBase {
            id: EntityId::new(&line_id)?,
            construction: true,
        },
        start: EntityId::new(&origin_id)?,
        end: EntityId::new(&center_id)?,
    }))?;
    sketch.add_constraint(Constraint::Vertical {
        id: ConstraintId::new(format!("con:{prefix}_vertical"))?,
        line: EntityId::new(&line_id)?,
    })?;
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new(format!("con:{prefix}_offset"))?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(&line_id)?,
        },
        expr: Expression::new(offset_expr)?,
    })?;
    sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new(format!("con:{prefix}_radius"))?,
        target: EntityId::new(&circle_id)?,
        expr: Expression::new(radius_expr)?,
    })?;
    Ok(sketch)
}

/// Circle sketch centered at `(x_expr, y_expr)` via two construction lines.
fn robot_arm_xy_circle_sketch(
    id: &str,
    name: impl Into<String>,
    prefix: &str,
    initial_center: [f64; 2],
    x_expr: &str,
    y_expr: &str,
    radius_expr: &str,
) -> Result<Sketch> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::{Constraint, DistanceTarget},
        entity::{CircleEntity, Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
        workplane::Workplane,
    };

    let origin_id = format!("ent:{prefix}_origin");
    let axis_id = format!("ent:{prefix}_y_axis");
    let center_id = format!("ent:{prefix}_center");
    let circle_id = format!("ent:{prefix}_circle");
    let vertical_line_id = format!("ent:{prefix}_y_line");
    let horizontal_line_id = format!("ent:{prefix}_x_line");
    let mut sketch = Sketch::new(SketchId::new(id)?, name, Workplane::xy());
    for (point_id, x, y, construction) in [
        (&origin_id, 0.0, 0.0, true),
        (&axis_id, 0.0, initial_center[1], true),
        (&center_id, initial_center[0], initial_center[1], false),
    ] {
        sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new(point_id)?,
                construction,
            },
            x: Coord::literal(x),
            y: Coord::literal(y),
        }))?;
    }
    sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new(&circle_id)?,
            construction: false,
        },
        center: EntityId::new(&center_id)?,
        radius: Coord::expr(radius_expr)?,
    }))?;
    for (line_id, start, end) in [
        (&vertical_line_id, &origin_id, &axis_id),
        (&horizontal_line_id, &axis_id, &center_id),
    ] {
        sketch.add_entity(SketchEntity::Line(LineEntity {
            base: EntityBase {
                id: EntityId::new(line_id)?,
                construction: true,
            },
            start: EntityId::new(start)?,
            end: EntityId::new(end)?,
        }))?;
    }
    sketch.add_constraint(Constraint::Vertical {
        id: ConstraintId::new(format!("con:{prefix}_y_vertical"))?,
        line: EntityId::new(&vertical_line_id)?,
    })?;
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new(format!("con:{prefix}_y_offset"))?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(&vertical_line_id)?,
        },
        expr: Expression::new(y_expr)?,
    })?;
    sketch.add_constraint(Constraint::Horizontal {
        id: ConstraintId::new(format!("con:{prefix}_x_horizontal"))?,
        line: EntityId::new(&horizontal_line_id)?,
    })?;
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new(format!("con:{prefix}_x_offset"))?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(&horizontal_line_id)?,
        },
        expr: Expression::new(x_expr)?,
    })?;
    sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new(format!("con:{prefix}_radius"))?,
        target: EntityId::new(&circle_id)?,
        expr: Expression::new(radius_expr)?,
    })?;
    Ok(sketch)
}

/// Rectangle bar running along `+Y` from the origin, centered on the Y axis.
fn robot_arm_bar_sketch(
    id: &str,
    name: impl Into<String>,
    prefix: &str,
    initial_size: [f64; 2],
    length_expr: &str,
    width_expr: &str,
) -> Result<Sketch> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::{Constraint, DistanceTarget},
        entity::{Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
        workplane::Workplane,
    };

    let half_width = initial_size[1] / 2.0;
    let corners = [
        (format!("ent:{prefix}_c0"), -half_width, 0.0),
        (format!("ent:{prefix}_c1"), -half_width, initial_size[0]),
        (format!("ent:{prefix}_c2"), half_width, initial_size[0]),
        (format!("ent:{prefix}_c3"), half_width, 0.0),
    ];
    let edges = [
        format!("ent:{prefix}_e0"),
        format!("ent:{prefix}_e1"),
        format!("ent:{prefix}_e2"),
        format!("ent:{prefix}_e3"),
    ];
    let mut sketch = Sketch::new(SketchId::new(id)?, name, Workplane::xy());
    for (corner_id, x, y) in &corners {
        sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new(corner_id)?,
                construction: false,
            },
            x: Coord::literal(*x),
            y: Coord::literal(*y),
        }))?;
    }
    for (index, (start, end)) in [(0usize, 1usize), (1, 2), (2, 3), (3, 0)]
        .iter()
        .enumerate()
    {
        sketch.add_entity(SketchEntity::Line(LineEntity {
            base: EntityBase {
                id: EntityId::new(&edges[index])?,
                construction: false,
            },
            start: EntityId::new(&corners[*start].0)?,
            end: EntityId::new(&corners[*end].0)?,
        }))?;
    }
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new(format!("con:{prefix}_length"))?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(&edges[0])?,
        },
        expr: Expression::new(length_expr)?,
    })?;
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new(format!("con:{prefix}_width"))?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(&edges[1])?,
        },
        expr: Expression::new(width_expr)?,
    })?;
    Ok(sketch)
}

fn robot_arm_add_sketch_feature(
    model: &mut PartModel,
    sketch: Sketch,
    feature_id: &str,
    feature_name: impl Into<String>,
) -> Result<()> {
    let sketch_id = sketch.id.as_str().to_string();
    model.sketches.insert(sketch_id.clone(), sketch);
    model.add_node(FeatureNode::new(
        feature_id,
        feature_name,
        FeatureDefinition::Sketch(SketchFeatureDef { sketch_id }),
    ))
}

struct RobotArmLinkSpec<'a> {
    prefix: &'a str,
    link_name: &'a str,
    length_expr: &'a str,
    width_expr: &'a str,
    thickness_expr: &'a str,
    proximal_hub_radius_expr: &'a str,
    proximal_bore_radius_expr: &'a str,
    distal_hub_radius_expr: &'a str,
    distal_bore_radius_expr: &'a str,
    proximal_hub_name: &'a str,
    distal_hub_name: &'a str,
    initial_length: f64,
    initial_width: f64,
    initial_thickness: f64,
}

fn robot_arm_link(spec: RobotArmLinkSpec<'_>, parameters: &ParamGraph) -> Result<PartModel> {
    let prefix = spec.prefix;
    let mut model = PartModel::new();

    let sketch_bar = format!("sketch:{prefix}_bar");
    let feature_sketch_bar = format!("feature:sketch_{prefix}_bar");
    let feature_extrude = format!("feature:{prefix}_extrude");
    robot_arm_add_sketch_feature(
        &mut model,
        robot_arm_bar_sketch(
            &sketch_bar,
            format!("{} Bar", spec.link_name),
            &format!("{prefix}_bar"),
            [spec.initial_length, spec.initial_width],
            spec.length_expr,
            spec.width_expr,
        )?,
        &feature_sketch_bar,
        format!("{} Bar Sketch", spec.link_name),
    )?;
    model.add_node(FeatureNode::new(
        &feature_extrude,
        format!("{} Body", spec.link_name),
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: feature_sketch_bar.clone(),
            profile_ref: format!("{sketch_bar}/profile:outer"),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(spec.initial_thickness),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some(spec.thickness_expr.into()),
            target_feature: None,
        }),
    ))?;
    model.add_dependency(&feature_sketch_bar, &feature_extrude)?;

    let mut body = feature_extrude.clone();

    for (role, hub_radius_expr, hub_name, offset_expr, initial_offset) in [
        (
            "proximal",
            spec.proximal_hub_radius_expr,
            spec.proximal_hub_name,
            "0 mm",
            0.0,
        ),
        (
            "distal",
            spec.distal_hub_radius_expr,
            spec.distal_hub_name,
            spec.length_expr,
            spec.initial_length,
        ),
    ] {
        let sketch_id = format!("sketch:{prefix}_{role}_hub");
        let feature_sketch = format!("feature:sketch_{prefix}_{role}_hub");
        let feature_hub = format!("feature:{prefix}_{role}_hub");
        let hub_sketch = if role == "proximal" {
            robot_arm_circle_sketch(
                &sketch_id,
                format!("{hub_name} Hub"),
                &format!("{prefix}_{role}_hub"),
                [0.0, initial_offset],
                hub_radius_expr,
            )?
        } else {
            robot_arm_offset_circle_sketch(
                &sketch_id,
                format!("{hub_name} Hub"),
                &format!("{prefix}_{role}_hub"),
                initial_offset,
                offset_expr,
                hub_radius_expr,
            )?
        };
        robot_arm_add_sketch_feature(
            &mut model,
            hub_sketch,
            &feature_sketch,
            format!("{hub_name} Hub Sketch"),
        )?;
        let mut hub = ExtrudeFeature::join(
            feature_sketch.clone(),
            format!("{sketch_id}/profile:outer"),
            body.clone(),
            ExtrudeExtent::Distance {
                length: Length::from_meters(spec.initial_thickness),
            },
        );
        hub.length_expr = Some(spec.thickness_expr.into());
        model.add_node(FeatureNode::new(
            &feature_hub,
            format!("{hub_name} Hub"),
            FeatureDefinition::Extrude(hub),
        ))?;
        model.add_dependency(&feature_sketch, &feature_hub)?;
        model.add_dependency(&body, &feature_hub)?;
        body = feature_hub;
    }

    for (role, bore_radius_expr, bore_name, initial_offset) in [
        (
            "proximal",
            spec.proximal_bore_radius_expr,
            spec.proximal_hub_name,
            0.0,
        ),
        (
            "distal",
            spec.distal_bore_radius_expr,
            spec.distal_hub_name,
            spec.initial_length,
        ),
    ] {
        let sketch_id = format!("sketch:{prefix}_{role}_bore");
        let feature_sketch = format!("feature:sketch_{prefix}_{role}_bore");
        let feature_bore = format!("feature:{prefix}_{role}_bore");
        let bore_sketch = if role == "proximal" {
            robot_arm_circle_sketch(
                &sketch_id,
                format!("{bore_name} Bore"),
                &format!("{prefix}_{role}_bore"),
                [0.0, initial_offset],
                bore_radius_expr,
            )?
        } else {
            robot_arm_offset_circle_sketch(
                &sketch_id,
                format!("{bore_name} Bore"),
                &format!("{prefix}_{role}_bore"),
                initial_offset,
                spec.length_expr,
                bore_radius_expr,
            )?
        };
        robot_arm_add_sketch_feature(
            &mut model,
            bore_sketch,
            &feature_sketch,
            format!("{bore_name} Bore Sketch"),
        )?;
        let mut bore = HoleFeature::through(
            feature_sketch.clone(),
            format!("{sketch_id}/profile:outer"),
            ExtrudeExtent::Distance {
                length: Length::from_meters(spec.initial_thickness),
            },
            body.clone(),
        );
        bore.depth_expr = Some(spec.thickness_expr.into());
        model.add_node(FeatureNode::new(
            &feature_bore,
            format!("{bore_name} Bore"),
            FeatureDefinition::Hole(bore),
        ))?;
        model.add_dependency(&feature_sketch, &feature_bore)?;
        model.add_dependency(&body, &feature_bore)?;
        body = feature_bore;
    }

    apply_parameters(&mut model, parameters)?;
    Ok(model)
}

/// Robot-arm base pedestal: round mounting plate, turret column, and bolt circle.
pub fn robot_arm_base() -> Result<PartModel> {
    use crate::pattern::CircularPatternFeature;

    let parameters = opencad_graph::robot_arm_base_parameters();
    let mut model = PartModel::new();

    robot_arm_add_sketch_feature(
        &mut model,
        robot_arm_circle_sketch(
            "sketch:base_plate",
            "Base Plate",
            "base_plate",
            [0.0, 0.0],
            "base_plate_diameter / 2",
        )?,
        "feature:sketch_base_plate",
        "Base Plate Sketch",
    )?;
    model.add_node(FeatureNode::new(
        "feature:base_plate",
        "Base Plate",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_base_plate".into(),
            profile_ref: "sketch:base_plate/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.010),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("base_plate_thickness".into()),
            target_feature: None,
        }),
    ))?;
    model.add_dependency("feature:sketch_base_plate", "feature:base_plate")?;

    robot_arm_add_sketch_feature(
        &mut model,
        robot_arm_circle_sketch(
            "sketch:turret",
            "Turret Column",
            "turret",
            [0.0, 0.0],
            "turret_diameter / 2",
        )?,
        "feature:sketch_turret",
        "Turret Column Sketch",
    )?;
    let mut turret = ExtrudeFeature::join(
        "feature:sketch_turret",
        "sketch:turret/profile:outer",
        "feature:base_plate",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.030),
        },
    );
    turret.length_expr = Some("turret_height".into());
    model.add_node(FeatureNode::new(
        "feature:turret",
        "Turret Column",
        FeatureDefinition::Extrude(turret),
    ))?;
    model.add_dependency("feature:sketch_turret", "feature:turret")?;
    model.add_dependency("feature:base_plate", "feature:turret")?;

    robot_arm_add_sketch_feature(
        &mut model,
        robot_arm_circle_sketch(
            "sketch:bolt_hole",
            "Bolt Hole Tool",
            "bolt_hole",
            [0.050, 0.0],
            "bolt_hole_diameter / 2",
        )?,
        "feature:sketch_bolt_hole",
        "Bolt Hole Sketch",
    )?;
    model.add_node(FeatureNode::new(
        "feature:bolt_hole_tool",
        "Bolt Hole Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_bolt_hole".into(),
            profile_ref: "sketch:bolt_hole/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.010),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("base_plate_thickness".into()),
            target_feature: None,
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:bolt_circle",
        "Four Hole Bolt Circle",
        FeatureDefinition::CircularPattern(CircularPatternFeature::cut(
            "feature:bolt_hole_tool",
            "feature:turret",
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            4,
        )),
    ))?;
    model.add_dependency("feature:sketch_bolt_hole", "feature:bolt_hole_tool")?;
    model.add_dependency("feature:bolt_hole_tool", "feature:bolt_circle")?;
    model.add_dependency("feature:turret", "feature:bolt_circle")?;

    apply_parameters(&mut model, &parameters)?;
    Ok(model)
}

/// Robot-arm upper link with a shoulder hub and an elbow hub.
pub fn robot_arm_upper_arm() -> Result<PartModel> {
    robot_arm_link(
        RobotArmLinkSpec {
            prefix: "upper_arm",
            link_name: "Upper Arm",
            length_expr: "upper_arm_length",
            width_expr: "upper_arm_width",
            thickness_expr: "upper_arm_thickness",
            proximal_hub_radius_expr: "shoulder_hub_diameter / 2",
            proximal_bore_radius_expr: "shoulder_bore_diameter / 2",
            distal_hub_radius_expr: "upper_arm_elbow_hub_diameter / 2",
            distal_bore_radius_expr: "upper_arm_elbow_bore_diameter / 2",
            proximal_hub_name: "Shoulder",
            distal_hub_name: "Elbow",
            initial_length: 0.160,
            initial_width: 0.026,
            initial_thickness: 0.014,
        },
        &opencad_graph::robot_arm_upper_arm_parameters(),
    )
}

/// Robot-arm forearm link with an elbow hub and a wrist hub.
pub fn robot_arm_forearm() -> Result<PartModel> {
    robot_arm_link(
        RobotArmLinkSpec {
            prefix: "forearm",
            link_name: "Forearm",
            length_expr: "forearm_length",
            width_expr: "forearm_width",
            thickness_expr: "forearm_thickness",
            proximal_hub_radius_expr: "forearm_elbow_hub_diameter / 2",
            proximal_bore_radius_expr: "forearm_elbow_bore_diameter / 2",
            distal_hub_radius_expr: "wrist_hub_diameter / 2",
            distal_bore_radius_expr: "wrist_bore_diameter / 2",
            proximal_hub_name: "Elbow",
            distal_hub_name: "Wrist",
            initial_length: 0.110,
            initial_width: 0.024,
            initial_thickness: 0.012,
        },
        &opencad_graph::robot_arm_forearm_parameters(),
    )
}

/// Robot-arm wrist gripper: body block, wrist bore, and two gripper fingers.
pub fn robot_arm_gripper() -> Result<PartModel> {
    let parameters = opencad_graph::robot_arm_gripper_parameters();
    let mut model = PartModel::new();

    robot_arm_add_sketch_feature(
        &mut model,
        robot_arm_bar_sketch(
            "sketch:gripper_body",
            "Gripper Body",
            "gripper_body",
            [0.040, 0.036],
            "gripper_length",
            "gripper_width",
        )?,
        "feature:sketch_gripper_body",
        "Gripper Body Sketch",
    )?;
    model.add_node(FeatureNode::new(
        "feature:gripper_body",
        "Gripper Body",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_gripper_body".into(),
            profile_ref: "sketch:gripper_body/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.014),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("gripper_thickness".into()),
            target_feature: None,
        }),
    ))?;
    model.add_dependency("feature:sketch_gripper_body", "feature:gripper_body")?;

    robot_arm_add_sketch_feature(
        &mut model,
        robot_arm_circle_sketch(
            "sketch:wrist_bore",
            "Wrist Bore",
            "wrist_bore",
            [0.0, 0.0],
            "gripper_wrist_bore_diameter / 2",
        )?,
        "feature:sketch_wrist_bore",
        "Wrist Bore Sketch",
    )?;
    let mut wrist_bore = HoleFeature::through(
        "feature:sketch_wrist_bore",
        "sketch:wrist_bore/profile:outer",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.014),
        },
        "feature:gripper_body",
    );
    wrist_bore.depth_expr = Some("gripper_thickness".into());
    model.add_node(FeatureNode::new(
        "feature:wrist_bore",
        "Wrist Bore",
        FeatureDefinition::Hole(wrist_bore),
    ))?;
    model.add_dependency("feature:sketch_wrist_bore", "feature:wrist_bore")?;
    model.add_dependency("feature:gripper_body", "feature:wrist_bore")?;

    let mut body = "feature:wrist_bore".to_string();
    for (side, label, initial_x) in [("left", "Left", -0.008), ("right", "Right", 0.008)] {
        let sketch_id = format!("sketch:finger_{side}");
        let feature_sketch = format!("feature:sketch_finger_{side}");
        let feature_finger = format!("feature:finger_{side}");
        robot_arm_add_sketch_feature(
            &mut model,
            robot_arm_xy_circle_sketch(
                &sketch_id,
                format!("{label} Finger"),
                &format!("finger_{side}"),
                [initial_x, 0.040],
                "finger_spacing / 2",
                "gripper_length",
                "finger_diameter / 2",
            )?,
            &feature_sketch,
            format!("{label} Finger Sketch"),
        )?;
        let mut finger = ExtrudeFeature::join(
            feature_sketch.clone(),
            format!("{sketch_id}/profile:outer"),
            body.clone(),
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.018),
            },
        );
        finger.length_expr = Some("finger_length".into());
        model.add_node(FeatureNode::new(
            &feature_finger,
            format!("{label} Finger"),
            FeatureDefinition::Extrude(finger),
        ))?;
        model.add_dependency(&feature_sketch, &feature_finger)?;
        model.add_dependency(&body, &feature_finger)?;
        body = feature_finger;
    }

    apply_parameters(&mut model, &parameters)?;
    Ok(model)
}

/// Bracket plate with a pin boss sketched on the top face via `face_ref` workplane.
pub fn bracket_face_pin() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    use crate::extrude::ExtrudeFeature;
    use crate::sketch_feature::SketchFeatureDef;
    use opencad_graph::bracket_parameters;

    let mut model = bracket_base_plate()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    let mut pin_sketch = Sketch::new(
        SketchId::new("sketch:face_pin")?,
        "Face Pin Sketch",
        Workplane::face_ref("ref:face:bracket_top"),
    );
    pin_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:face_pin_center")?,
            construction: false,
        },
        x: Coord::literal(0.0),
        y: Coord::literal(0.0),
    }))?;
    pin_sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new("ent:face_pin_circle")?,
            construction: false,
        },
        center: EntityId::new("ent:face_pin_center")?,
        radius: Coord::expr("hole_diameter / 2")?,
    }))?;
    pin_sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new("con:face_pin_radius")?,
        target: EntityId::new("ent:face_pin_circle")?,
        expr: Expression::new("hole_diameter / 2")?,
    })?;
    model
        .sketches
        .insert(pin_sketch.id.as_str().to_string(), pin_sketch);

    model.add_node(FeatureNode::new(
        "feature:sketch_face_pin",
        "Face Pin Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:face_pin".into(),
        }),
    ))?;
    let mut pin = ExtrudeFeature::join(
        "feature:sketch_face_pin",
        "sketch:face_pin/profile:outer",
        "feature:extrude_base",
        ExtrudeExtent::Distance {
            length: Length::from_meters(0.006),
        },
    );
    pin.length_expr = Some("boss_height".into());
    model.add_node(FeatureNode::new(
        "feature:face_pin_join",
        "Face Pin Join",
        FeatureDefinition::Extrude(pin),
    ))?;

    model.add_dependency("feature:sketch_face_pin", "feature:face_pin_join")?;
    model.add_dependency("feature:extrude_base", "feature:face_pin_join")?;
    Ok(model)
}

/// Bracket base plate with a linear cut pattern of pin holes (`spacing_expr: hole_pitch`).
pub fn bracket_hole_row() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    use crate::extrude::ExtrudeFeature;
    use crate::pattern::LinearPatternFeature;
    use crate::sketch_feature::SketchFeatureDef;
    use opencad_graph::bracket_parameters;

    let mut model = bracket_base_plate()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    let mut pin_sketch = Sketch::new(SketchId::new("sketch:pin")?, "Pin Sketch", Workplane::xy());
    pin_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:pin_center")?,
            construction: false,
        },
        x: Coord::literal(0.01),
        y: Coord::literal(0.01),
    }))?;
    pin_sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new("ent:pin_circle")?,
            construction: false,
        },
        center: EntityId::new("ent:pin_center")?,
        radius: Coord::expr("hole_diameter / 2")?,
    }))?;
    pin_sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new("con:pin_radius")?,
        target: EntityId::new("ent:pin_circle")?,
        expr: Expression::new("hole_diameter / 2")?,
    })?;
    model
        .sketches
        .insert(pin_sketch.id.as_str().to_string(), pin_sketch);

    model.add_node(FeatureNode::new(
        "feature:sketch_pin",
        "Pin Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:pin".into(),
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pin_tool",
        "Pin Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_pin".into(),
            profile_ref: "sketch:pin/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("thickness".into()),
            target_feature: None,
        }),
    ))?;
    let mut pattern = LinearPatternFeature::cut(
        "feature:pin_tool",
        "feature:extrude_base",
        [1.0, 0.0, 0.0],
        Length::from_meters(0.02),
        2,
    );
    pattern.spacing_expr = Some("hole_pitch".into());
    model.add_node(FeatureNode::new(
        "feature:pin_holes",
        "Pin Hole Row",
        FeatureDefinition::LinearPattern(pattern),
    ))?;

    model.add_dependency("feature:sketch_pin", "feature:pin_tool")?;
    model.add_dependency("feature:extrude_base", "feature:pin_holes")?;
    model.add_dependency("feature:pin_tool", "feature:pin_holes")?;
    Ok(model)
}

/// Bracket base plate with a circular cut pattern of pin holes.
pub fn bracket_hole_ring() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    use crate::extrude::ExtrudeFeature;
    use crate::pattern::CircularPatternFeature;
    use crate::sketch_feature::SketchFeatureDef;
    use opencad_graph::bracket_parameters;

    let mut model = bracket_base_plate()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    let mut pin_sketch = Sketch::new(SketchId::new("sketch:pin")?, "Pin Sketch", Workplane::xy());
    pin_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:pin_center")?,
            construction: false,
        },
        x: Coord::literal(0.01),
        y: Coord::literal(0.01),
    }))?;
    pin_sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new("ent:pin_circle")?,
            construction: false,
        },
        center: EntityId::new("ent:pin_center")?,
        radius: Coord::expr("hole_diameter / 2")?,
    }))?;
    pin_sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new("con:pin_radius")?,
        target: EntityId::new("ent:pin_circle")?,
        expr: Expression::new("hole_diameter / 2")?,
    })?;
    model
        .sketches
        .insert(pin_sketch.id.as_str().to_string(), pin_sketch);

    model.add_node(FeatureNode::new(
        "feature:sketch_pin",
        "Pin Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:pin".into(),
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pin_tool",
        "Pin Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_pin".into(),
            profile_ref: "sketch:pin/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("thickness".into()),
            target_feature: None,
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pin_hole_ring",
        "Pin Hole Ring",
        FeatureDefinition::CircularPattern(CircularPatternFeature::cut(
            "feature:pin_tool",
            "feature:extrude_base",
            [0.04, 0.03, 0.0],
            [0.0, 0.0, 1.0],
            4,
        )),
    ))?;

    model.add_dependency("feature:sketch_pin", "feature:pin_tool")?;
    model.add_dependency("feature:extrude_base", "feature:pin_hole_ring")?;
    model.add_dependency("feature:pin_tool", "feature:pin_hole_ring")?;
    Ok(model)
}

/// Bracket base plate with a linear union pattern of pin bosses (`spacing_expr: hole_pitch`).
pub fn bracket_pin_row() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    use crate::extrude::ExtrudeFeature;
    use crate::pattern::LinearPatternFeature;
    use crate::sketch_feature::SketchFeatureDef;
    use opencad_graph::bracket_parameters;

    let mut model = bracket_base_plate()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    let mut pin_sketch = Sketch::new(SketchId::new("sketch:pin")?, "Pin Sketch", Workplane::xy());
    pin_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:pin_center")?,
            construction: false,
        },
        x: Coord::literal(0.01),
        y: Coord::literal(0.01),
    }))?;
    pin_sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new("ent:pin_circle")?,
            construction: false,
        },
        center: EntityId::new("ent:pin_center")?,
        radius: Coord::expr("hole_diameter / 2")?,
    }))?;
    pin_sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new("con:pin_radius")?,
        target: EntityId::new("ent:pin_circle")?,
        expr: Expression::new("hole_diameter / 2")?,
    })?;
    model
        .sketches
        .insert(pin_sketch.id.as_str().to_string(), pin_sketch);

    model.add_node(FeatureNode::new(
        "feature:sketch_pin",
        "Pin Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:pin".into(),
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pin_tool",
        "Pin Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_pin".into(),
            profile_ref: "sketch:pin/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("boss_height".into()),
            target_feature: None,
        }),
    ))?;
    let mut pattern = LinearPatternFeature::union_on(
        "feature:pin_tool",
        "feature:extrude_base",
        [1.0, 0.0, 0.0],
        Length::from_meters(0.02),
        2,
    );
    pattern.spacing_expr = Some("hole_pitch".into());
    model.add_node(FeatureNode::new(
        "feature:pin_bosses",
        "Pin Boss Row",
        FeatureDefinition::LinearPattern(pattern),
    ))?;

    model.add_dependency("feature:sketch_pin", "feature:pin_tool")?;
    model.add_dependency("feature:extrude_base", "feature:pin_bosses")?;
    model.add_dependency("feature:pin_tool", "feature:pin_bosses")?;
    Ok(model)
}

/// Bracket base plate with a circular union pattern of pin bosses around plate center.
pub fn bracket_pin_ring() -> Result<PartModel> {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    use crate::extrude::ExtrudeFeature;
    use crate::pattern::CircularPatternFeature;
    use crate::sketch_feature::SketchFeatureDef;
    use opencad_graph::bracket_parameters;

    let mut model = bracket_base_plate()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    let mut pin_sketch = Sketch::new(SketchId::new("sketch:pin")?, "Pin Sketch", Workplane::xy());
    pin_sketch.add_entity(SketchEntity::Point(PointEntity {
        base: EntityBase {
            id: EntityId::new("ent:pin_center")?,
            construction: false,
        },
        x: Coord::literal(0.04),
        y: Coord::literal(0.03),
    }))?;
    pin_sketch.add_entity(SketchEntity::Circle(CircleEntity {
        base: EntityBase {
            id: EntityId::new("ent:pin_circle")?,
            construction: false,
        },
        center: EntityId::new("ent:pin_center")?,
        radius: Coord::expr("hole_diameter / 2")?,
    }))?;
    pin_sketch.add_constraint(Constraint::Radius {
        id: ConstraintId::new("con:pin_radius")?,
        target: EntityId::new("ent:pin_circle")?,
        expr: Expression::new("hole_diameter / 2")?,
    })?;
    model
        .sketches
        .insert(pin_sketch.id.as_str().to_string(), pin_sketch);

    model.add_node(FeatureNode::new(
        "feature:sketch_pin",
        "Pin Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:pin".into(),
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pin_tool",
        "Pin Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_pin".into(),
            profile_ref: "sketch:pin/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("boss_height".into()),
            target_feature: None,
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pin_ring",
        "Pin Boss Ring",
        FeatureDefinition::CircularPattern(CircularPatternFeature::union_on(
            "feature:pin_tool",
            "feature:extrude_base",
            [0.04, 0.03, 0.0],
            [0.0, 0.0, 1.0],
            4,
        )),
    ))?;

    model.add_dependency("feature:sketch_pin", "feature:pin_tool")?;
    model.add_dependency("feature:extrude_base", "feature:pin_ring")?;
    model.add_dependency("feature:pin_tool", "feature:pin_ring")?;
    Ok(model)
}

/// Pin tool mirrored across the bracket top face using `plane_face_ref`.
pub fn bracket_pin_mirror() -> Result<PartModel> {
    use crate::pattern::MirrorPatternFeature;
    use opencad_graph::bracket_parameters;

    let mut model = bracket_base_plate()?;
    apply_parameters(&mut model, &bracket_parameters())?;

    let pin_sketch = {
        use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
        use opencad_sketch::{
            constraint::Constraint,
            entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
            workplane::Workplane,
            Sketch,
        };

        let mut pin_sketch =
            Sketch::new(SketchId::new("sketch:pin")?, "Pin Sketch", Workplane::xy());
        pin_sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new("ent:pin_center")?,
                construction: false,
            },
            x: Coord::literal(0.05),
            y: Coord::literal(0.03),
        }))?;
        pin_sketch.add_entity(SketchEntity::Circle(CircleEntity {
            base: EntityBase {
                id: EntityId::new("ent:pin_circle")?,
                construction: false,
            },
            center: EntityId::new("ent:pin_center")?,
            radius: Coord::expr("hole_diameter / 2")?,
        }))?;
        pin_sketch.add_constraint(Constraint::Radius {
            id: ConstraintId::new("con:pin_radius")?,
            target: EntityId::new("ent:pin_circle")?,
            expr: Expression::new("hole_diameter / 2")?,
        })?;
        pin_sketch
    };

    model
        .sketches
        .insert(pin_sketch.id.as_str().to_string(), pin_sketch);

    model.add_node(FeatureNode::new(
        "feature:sketch_pin",
        "Pin Sketch",
        FeatureDefinition::Sketch(SketchFeatureDef {
            sketch_id: "sketch:pin".into(),
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pin_tool",
        "Pin Tool",
        FeatureDefinition::Extrude(ExtrudeFeature {
            sketch_feature: "feature:sketch_pin".into(),
            profile_ref: "sketch:pin/profile:outer".into(),
            extent: ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            operation: opencad_geometry::ExtrudeOperation::NewBody,
            length_expr: Some("boss_height".into()),
            target_feature: None,
        }),
    ))?;
    model.add_node(FeatureNode::new(
        "feature:pin_mirror",
        "Pin Mirror",
        FeatureDefinition::MirrorPattern(MirrorPatternFeature::union_across_face_ref(
            "feature:pin_tool",
            "feature:extrude_base",
            "ref:face:bracket_top",
        )),
    ))?;

    model.add_dependency("feature:sketch_pin", "feature:pin_tool")?;
    model.add_dependency("feature:extrude_base", "feature:pin_mirror")?;
    model.add_dependency("feature:pin_tool", "feature:pin_mirror")?;
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
        ctx.outputs
            .insert(feature_id.into(), FeatureOutput { body: Some(body) });
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
        let report = model
            .regenerate(&kernel, &registry, None, None)
            .expect("regen");
        assert_eq!(report.regenerated.len(), 2);
        let body = model.active_body().expect("body");
        assert!(body.0 > 0);
    }

    #[test]
    fn regenerates_multi_feature_bearing_carrier() {
        let mut model = bearing_carrier().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let parameters = opencad_graph::bearing_carrier_parameters();
        let report = model
            .regenerate(&kernel, &registry, Some(&parameters), None)
            .expect("regen");
        assert_eq!(report.regenerated.len(), 9);
        assert_eq!(
            report.regenerated.last().map(String::as_str),
            Some("feature:bolt_circle")
        );
        assert!(model.active_body().is_some());
    }

    #[test]
    fn regenerates_twenty_two_node_robot_joint_housing() {
        let mut model = robot_joint_actuator_housing().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let parameters = opencad_graph::robot_joint_housing_parameters();
        let report = model
            .regenerate(&kernel, &registry, Some(&parameters), None)
            .expect("regen");
        assert_eq!(report.regenerated.len(), 22);
        assert_eq!(
            report.regenerated.last().map(String::as_str),
            Some("feature:mounting_holes")
        );
        assert!(model.active_body().is_some());
        assert_eq!(report.trace.executed_nodes.len(), 22);
        assert_eq!(report.trace.solver_call_count, 9);
        assert!(report.trace.geometry_kernel_call_count > 0);
        assert_eq!(report.trace.output_hashes_sha256.len(), 22);
        assert!(!report.trace.trace_hash_sha256.is_empty());
    }

    #[test]
    fn regenerates_four_part_robot_arm_models() {
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();

        let mut base = robot_arm_base().expect("base model");
        let base_report = base
            .regenerate(
                &kernel,
                &registry,
                Some(&opencad_graph::robot_arm_base_parameters()),
                None,
            )
            .expect("base regen");
        assert_eq!(base_report.regenerated.len(), 7);
        assert!(base.active_body().is_some());

        let mut upper = robot_arm_upper_arm().expect("upper arm model");
        let upper_report = upper
            .regenerate(
                &kernel,
                &registry,
                Some(&opencad_graph::robot_arm_upper_arm_parameters()),
                None,
            )
            .expect("upper arm regen");
        assert_eq!(upper_report.regenerated.len(), 10);
        assert_eq!(
            upper_report.regenerated.last().map(String::as_str),
            Some("feature:upper_arm_distal_bore")
        );
        assert!(upper.active_body().is_some());

        let mut forearm = robot_arm_forearm().expect("forearm model");
        let forearm_report = forearm
            .regenerate(
                &kernel,
                &registry,
                Some(&opencad_graph::robot_arm_forearm_parameters()),
                None,
            )
            .expect("forearm regen");
        assert_eq!(forearm_report.regenerated.len(), 10);
        assert_eq!(
            forearm_report.regenerated.last().map(String::as_str),
            Some("feature:forearm_distal_bore")
        );
        assert!(forearm.active_body().is_some());

        let mut gripper = robot_arm_gripper().expect("gripper model");
        let gripper_report = gripper
            .regenerate(
                &kernel,
                &registry,
                Some(&opencad_graph::robot_arm_gripper_parameters()),
                None,
            )
            .expect("gripper regen");
        assert_eq!(gripper_report.regenerated.len(), 8);
        assert_eq!(
            gripper_report.regenerated.last().map(String::as_str),
            Some("feature:finger_right")
        );
        assert!(gripper.active_body().is_some());
    }

    #[test]
    fn incremental_regeneration_serves_every_unchanged_node_from_cache() {
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let mut model = robot_joint_actuator_housing().expect("model");
        let params = opencad_graph::robot_joint_housing_parameters();
        let mut cache = RegenerationCache::with_backend("mock");

        let cold = model
            .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
            .expect("cold regen");
        assert_eq!(cold.regenerated.len(), 22);
        assert!(cold.cached_nodes.is_empty());
        assert!(cold.trace.geometry_kernel_call_count > 0);

        let warm = model
            .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
            .expect("warm regen");
        assert_eq!(
            warm.regenerated.len(),
            0,
            "unchanged nodes must not re-execute"
        );
        assert_eq!(warm.cached_nodes.len(), 22);
        assert_eq!(
            warm.trace.geometry_kernel_call_count, 0,
            "no kernel calls for cached nodes"
        );
        assert_eq!(
            cold.trace.output_hashes_sha256, warm.trace.output_hashes_sha256,
            "cached outputs must have identical content hashes"
        );
    }

    #[test]
    fn incremental_upper_hub_change_only_recomputes_downstream() {
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let mut model = robot_joint_actuator_housing().expect("model");
        let mut params = opencad_graph::robot_joint_housing_parameters();
        let mut cache = RegenerationCache::with_backend("mock");

        model
            .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
            .expect("cold regen");

        params
            .set_expr("param:upper_hub_height", "42 mm")
            .expect("edit upper hub height");
        let report = model
            .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
            .expect("incremental regen");

        assert!(
            report
                .regenerated
                .iter()
                .any(|id| id == "feature:upper_hub"),
            "upper hub must re-execute"
        );
        assert!(
            report
                .cached_nodes
                .iter()
                .any(|id| id == "feature:joint_base"),
            "base plate must be served from cache"
        );
        assert!(
            report
                .cached_nodes
                .iter()
                .any(|id| id == "feature:lower_hub"),
            "lower hub must be served from cache"
        );
        assert!(
            !report
                .regenerated
                .iter()
                .any(|id| id == "feature:joint_base"),
            "base plate must not re-execute"
        );
    }

    #[test]
    fn incremental_bolt_circle_change_recomputes_pattern_downstream_only() {
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let mut model = robot_joint_actuator_housing().expect("model");
        let mut params = opencad_graph::robot_joint_housing_parameters();
        let mut cache = RegenerationCache::with_backend("mock");

        model
            .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
            .expect("cold regen");

        params
            .set_expr("param:bolt_circle_radius", "50 mm")
            .expect("edit bolt circle radius");
        let report = model
            .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
            .expect("incremental regen");

        assert!(
            report
                .regenerated
                .iter()
                .any(|id| id == "feature:pcd_fasteners"),
            "PCD pattern must re-execute"
        );
        assert!(
            report
                .cached_nodes
                .iter()
                .any(|id| id == "feature:joint_base"),
            "base plate must be served from cache"
        );
        assert!(
            report
                .cached_nodes
                .iter()
                .any(|id| id == "feature:shaft_bore"),
            "shaft bore must be served from cache"
        );
        assert!(
            !report
                .regenerated
                .iter()
                .any(|id| id == "feature:shaft_bore"),
            "shaft bore must not re-execute"
        );
    }

    #[test]
    fn failed_incremental_regeneration_preserves_previous_outputs() {
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let mut model = bracket_with_hole().expect("model");
        model
            .regenerate(&kernel, &registry, None, None)
            .expect("cold regen");
        let previous = model.outputs.clone();

        // A node referencing a missing sketch makes execution fail.
        model
            .add_node(FeatureNode::new(
                "feature:broken",
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
            ))
            .expect("add broken node");
        model
            .add_dependency("feature:extrude_base", "feature:broken")
            .expect("add dependency");

        let result = model.regenerate(&kernel, &registry, None, None);
        assert!(result.is_err(), "broken model must fail regeneration");
        assert_eq!(
            model.outputs, previous,
            "failed regeneration must restore the previous document outputs"
        );
    }

    #[test]
    fn robot_arm_upper_arm_elbow_position_follows_length_param() {
        let mut model = robot_arm_upper_arm().expect("model");
        let mut params = opencad_graph::robot_arm_upper_arm_parameters();
        params
            .set_expr("param:upper_arm_length", "200 mm")
            .expect("edit length");
        crate::param_apply::apply_parameters(&mut model, &params).expect("apply");

        let sketch = model
            .sketches
            .get("sketch:upper_arm_distal_hub")
            .expect("distal hub sketch");
        let center = sketch
            .find_entity("ent:upper_arm_distal_hub_center")
            .and_then(|entity| match entity {
                opencad_sketch::SketchEntity::Point(point) => {
                    match (point.x.clone(), point.y.clone()) {
                        (opencad_sketch::Coord::Literal(x), opencad_sketch::Coord::Literal(y)) => {
                            Some((x, y))
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
            .expect("distal hub center");
        assert!((center.1 - 0.200).abs() < 1e-6, "distal hub y {}", center.1);
    }

    #[test]
    fn robot_joint_trace_order_and_hash_are_deterministic() {
        let registry = FeatureRegistry::with_defaults();
        let parameters = opencad_graph::robot_joint_housing_parameters();
        let mut first = robot_joint_actuator_housing().expect("first model");
        let mut second = robot_joint_actuator_housing().expect("second model");
        let first_trace = first
            .regenerate(
                &MockGeometryKernel::new(),
                &registry,
                Some(&parameters),
                None,
            )
            .expect("first regen")
            .trace;
        let second_trace = second
            .regenerate(
                &MockGeometryKernel::new(),
                &registry,
                Some(&parameters),
                None,
            )
            .expect("second regen")
            .trace;

        assert_eq!(first_trace.executed_nodes, second_trace.executed_nodes);
        assert_eq!(
            first_trace.output_hashes_sha256,
            second_trace.output_hashes_sha256
        );
        assert_eq!(
            first_trace.trace_hash_sha256,
            second_trace.trace_hash_sha256
        );
        assert_eq!(
            first_trace.geometry_kernel_call_count,
            second_trace.geometry_kernel_call_count
        );
    }

    #[test]
    fn logical_output_hash_changes_when_parameterized_sketch_changes() {
        let registry = FeatureRegistry::with_defaults();
        let mut baseline = bracket_base_plate().expect("baseline model");
        let baseline_parameters = opencad_graph::bracket_parameters();
        let baseline_trace = baseline
            .regenerate(
                &MockGeometryKernel::new(),
                &registry,
                Some(&baseline_parameters),
                None,
            )
            .expect("baseline regen")
            .trace;

        let mut edited = bracket_base_plate().expect("edited model");
        let mut edited_parameters = opencad_graph::bracket_parameters();
        edited_parameters
            .set_expr("param:width", "100 mm")
            .expect("edit width");
        let edited_trace = edited
            .regenerate(
                &MockGeometryKernel::new(),
                &registry,
                Some(&edited_parameters),
                None,
            )
            .expect("edited regen")
            .trace;

        assert_ne!(
            baseline_trace.output_hashes_sha256["feature:sketch_base"],
            edited_trace.output_hashes_sha256["feature:sketch_base"]
        );
        assert_ne!(
            baseline_trace.output_hashes_sha256["feature:extrude_base"],
            edited_trace.output_hashes_sha256["feature:extrude_base"]
        );
    }

    #[test]
    fn regenerate_preserves_bracket_profile() {
        let mut model = bracket_base_plate().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        model
            .regenerate(&kernel, &registry, None, None)
            .expect("regen");
        let sketch = model.sketches.get("sketch:base").expect("sketch");
        let solved = crate::sketch_bridge::profile_to_solved(sketch, "sketch:base/profile:outer")
            .expect("solved");
        assert_eq!(solved.points.len(), 4, "{:?}", solved.points);
    }

    #[test]
    fn bracket_profile_has_four_corners() {
        let model = bracket_base_plate().expect("model");
        let sketch = model.sketches.get("sketch:base").expect("sketch");
        let solved = crate::sketch_bridge::profile_to_solved(sketch, "sketch:base/profile:outer")
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
        model
            .regenerate(&kernel, &registry, None, None)
            .expect("regen");
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
        let report = model
            .regenerate(&kernel, &registry, None, None)
            .expect("regen");
        assert_eq!(report.regenerated, vec!["feature:sketch_base"]);
        assert_eq!(report.skipped_suppressed, vec!["feature:extrude_base"]);
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

    #[test]
    fn regenerates_bracket_hole_row() {
        let mut model = bracket_hole_row().expect("model");
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
    fn regenerates_bracket_hole_ring() {
        let mut model = bracket_hole_ring().expect("model");
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
    fn regenerates_revolve_bushing() {
        let mut model = revolve_bushing().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let params = opencad_graph::revolve_parameters("360 deg");
        let report = model
            .regenerate(&kernel, &registry, Some(&params), None)
            .expect("regen");
        assert_eq!(report.regenerated.len(), 2);
        assert!(model.active_body().is_some());
    }

    #[test]
    fn regenerates_revolve_sector() {
        let mut model = revolve_sector().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let params = opencad_graph::revolve_parameters("180 deg");
        let report = model
            .regenerate(&kernel, &registry, Some(&params), None)
            .expect("regen");
        assert_eq!(report.regenerated.len(), 2);
        assert!(model.active_body().is_some());
    }

    #[test]
    fn regenerates_bracket_face_pin() {
        let mut model = bracket_face_pin().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let params = opencad_graph::bracket_parameters();
        let refs = bracket_semantic_refs();
        let report = model
            .regenerate(&kernel, &registry, Some(&params), Some(&refs))
            .expect("regen");
        assert_eq!(report.regenerated.len(), 4);
        assert!(model.active_body().is_some());
    }

    #[test]
    fn regenerates_bracket_boss_join() {
        let mut model = bracket_boss_join().expect("model");
        let kernel = MockGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let params = opencad_graph::bracket_parameters();
        let report = model
            .regenerate(&kernel, &registry, Some(&params), None)
            .expect("regen");
        assert_eq!(report.regenerated.len(), 4);
        assert!(model.active_body().is_some());
    }

    #[test]
    fn regenerates_bracket_pin_row() {
        let mut model = bracket_pin_row().expect("model");
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
    fn regenerates_bracket_pin_ring() {
        let mut model = bracket_pin_ring().expect("model");
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
