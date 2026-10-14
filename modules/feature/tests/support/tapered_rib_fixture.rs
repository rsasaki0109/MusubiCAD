use opencad_core::{Angle, EntityId, Length, SketchId, TopoRefId};
use opencad_feature::*;
use opencad_geometry::TopoRef;
use opencad_graph::{ParamGraph, ParameterEntry};
use opencad_sketch::{
    entity::{Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
    Sketch, Workplane,
};
#[path = "seated_fixture.rs"]
mod stock;
pub fn fixture() -> (PartModel, ParamGraph, Vec<TopoRef>) {
    let (mut model, _, _) = stock::fixture();
    model.nodes.shift_remove("feature:seat");
    model.nodes.shift_remove("feature:sketch_seat");
    model.sketches.shift_remove("sketch:seat");
    let mut sketch = Sketch::new(
        SketchId::new("sketch:rib").unwrap(),
        "Rib centreline",
        Workplane::Custom {
            origin: [0., 0., 0.01],
            normal: [0., 0., 1.],
            x_axis: [1., 0., 0.],
        },
    );
    for (id, x) in [("ent:rib_start", 0.005), ("ent:rib_end", 0.035)] {
        sketch
            .add_entity(SketchEntity::Point(PointEntity {
                base: EntityBase {
                    id: EntityId::new(id).unwrap(),
                    construction: false,
                },
                x: Coord::literal(x),
                y: Coord::literal(0.015),
            }))
            .unwrap();
    }
    sketch
        .add_entity(SketchEntity::Line(LineEntity {
            base: EntityBase {
                id: EntityId::new("ent:rib_line").unwrap(),
                construction: true,
            },
            start: EntityId::new("ent:rib_start").unwrap(),
            end: EntityId::new("ent:rib_end").unwrap(),
        }))
        .unwrap();
    model.add_sketch(sketch).unwrap();
    model
        .add_node(FeatureNode::new(
            "feature:sketch_rib",
            "Rib centreline",
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: "sketch:rib".into(),
            }),
        ))
        .unwrap();
    model
        .add_node(FeatureNode::new(
            "feature:rib",
            "Square-ended tapered rib",
            FeatureDefinition::TaperedRib(TaperedRibFeature {
                target_feature: "feature:extrude_base".into(),
                sketch_feature: "feature:sketch_rib".into(),
                centerline_ref: "ent:rib_line".into(),
                face_ref: "ref:face:rib_base".into(),
                base_thickness: Length::from_meters(0.004),
                height: Length::from_meters(0.02),
                angle: Angle::from_radians(0.05_f64.atan()),
                base_thickness_expr: Some("rib_thickness".into()),
                height_expr: Some("rib_height".into()),
                angle_expr: Some("rib_angle_rad".into()),
                ends: TaperedRibEnds::Square,
            }),
        ))
        .unwrap();
    let mut reference = TopoRef::face(
        TopoRefId::new("ref:face:rib_base").unwrap(),
        "feature:extrude_base",
        "extrusion_end",
    );
    reference.semantic.normal_hint = Some([0., 0., 1.]);
    let refs = vec![reference];
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &[
            "feature:sketch_base",
            "feature:extrude_base",
            "feature:sketch_rib",
            "feature:rib",
        ]
        .map(String::from),
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &refs,
    )
    .unwrap();
    let mut params = ParamGraph::new();
    for (id, name, expr) in [
        ("param:rib_thickness", "rib_thickness", "4 mm"),
        ("param:rib_height", "rib_height", "20 mm"),
        ("param:rib_angle", "rib_angle_rad", "2.862405226111748 deg"),
    ] {
        params
            .add_parameter(ParameterEntry::new(id, name, expr))
            .unwrap();
    }
    (model, params, refs)
}
