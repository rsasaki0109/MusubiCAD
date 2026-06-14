//! Feature graph regeneration pipeline (Task-098+).

use indexmap::IndexMap;

use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::{
    ExtrudeExtent, FaceDerivation, FaceRefDiscovery, GeometryKernel, KernelBody, TopoRef,
};
use opencad_geometry::EdgeRefDiscovery;
use opencad_graph::FeatureGraph;
use opencad_sketch::Sketch;

use opencad_graph::ParamGraph;

use crate::chamfer::ChamferFeature;
use crate::extrude::ExtrudeFeature;
use crate::edge_discover::discover_edge_refs_from_body;
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
    pub edge_discoveries: &'a [EdgeRefDiscovery],
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
        if let Some(params) = parameters {
            apply_parameters(self, params)?;
        }
        self.prepare_sketches()?;
        self.outputs.clear();

        let order = self.graph.recompute_order()?;
        let mut report = RegenReport::default();
        let refs = semantic_refs.unwrap_or(&[]);
        let mut face_discoveries: Vec<FaceRefDiscovery> = Vec::new();
        let mut edge_discoveries: Vec<EdgeRefDiscovery> = Vec::new();
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
                edge_discoveries: &edge_discoveries,
            };

            let output = registry.execute(node, &session)?;
            if let Some(ref body) = output.body {
                report
                    .face_history
                    .extend(kernel.face_derivation_history(body));
                if !refs.is_empty() {
                    face_discoveries =
                        discover_face_refs_from_body(kernel, body, &node_list).unwrap_or_default();
                    edge_discoveries =
                        discover_edge_refs_from_body(kernel, body, &node_list).unwrap_or_default();
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
