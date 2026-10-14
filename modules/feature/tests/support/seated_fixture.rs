use opencad_core::{EntityId, Length, SketchId, TopoRefId};
use opencad_feature::*;
use opencad_geometry::{ExtrudeExtent, TopoRef};
use opencad_graph::{ParamGraph, ParameterEntry};
use opencad_sketch::{
    entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
    Sketch, Workplane,
};
pub fn fixture() -> (PartModel, ParamGraph, Vec<TopoRef>) {
    let mut model = bracket_base_plate().unwrap();
    // The bracket solver's free translation is avoided by a literal admitted
    // rectangle, so independent world contact/first-moment checks have an oracle.
    let base = model.sketches.get_mut("sketch:base").unwrap();
    base.constraints.clear();
    for entity in &mut base.entities {
        if let SketchEntity::Point(p) = entity {
            let (x, y) = match p.base.id.as_str() {
                "ent:c0" => (0., 0.),
                "ent:c1" => (0.04, 0.),
                "ent:c2" => (0.04, 0.03),
                "ent:c3" => (0., 0.03),
                _ => panic!("corner"),
            };
            p.x = Coord::literal(x);
            p.y = Coord::literal(y);
        }
    }
    let FeatureDefinition::Extrude(def) = &mut model
        .nodes
        .get_mut("feature:extrude_base")
        .unwrap()
        .definition
    else {
        panic!("stock")
    };
    def.extent = ExtrudeExtent::Distance {
        length: Length::from_meters(0.01),
    };
    def.length_expr = None;
    let mut sketch = Sketch::new(
        SketchId::new("sketch:seat").unwrap(),
        "M6 entry",
        Workplane::Custom {
            origin: [0., 0., 0.01],
            normal: [0., 0., 1.],
            x_axis: [1., 0., 0.],
        },
    );
    sketch
        .add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new("ent:seat_center").unwrap(),
                construction: false,
            },
            x: Coord::literal(0.012),
            y: Coord::literal(0.009),
        }))
        .unwrap();
    sketch
        .add_entity(SketchEntity::Circle(CircleEntity {
            base: EntityBase {
                id: EntityId::new("ent:seat_circle").unwrap(),
                construction: false,
            },
            center: EntityId::new("ent:seat_center").unwrap(),
            radius: Coord::literal(0.0033),
        }))
        .unwrap();
    model.add_sketch(sketch).unwrap();
    model
        .add_node(FeatureNode::new(
            "feature:sketch_seat",
            "M6 bore sketch",
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: "sketch:seat".into(),
            }),
        ))
        .unwrap();
    model
        .add_node(FeatureNode::new(
            "feature:seat",
            "M6 counterbore",
            FeatureDefinition::SeatedStandardHole(SeatedStandardHoleFeature {
                sketch_feature: "feature:sketch_seat".into(),
                profile_ref: "sketch:seat/profile:outer".into(),
                target_feature: "feature:extrude_base".into(),
                face_ref: "ref:face:plate_entry".into(),
                bore_depth: Length::from_meters(0.01),
                bore_depth_expr: Some("bore_depth".into()),
                standard: HoleStandard::Iso273 {
                    size: MetricSize::M6,
                    fit: ClearanceFit::Normal,
                },
                seat: StandardHoleSeat::Counterbore {
                    diameter: Length::from_meters(0.011),
                    depth: Length::from_meters(0.004),
                    diameter_expr: Some("seat_diameter".into()),
                    depth_expr: Some("seat_depth".into()),
                },
            }),
        ))
        .unwrap();
    let mut reference = TopoRef::face(
        TopoRefId::new("ref:face:plate_entry").unwrap(),
        "feature:extrude_base",
        "top",
    );
    reference.semantic.normal_hint = Some([0., 0., 1.]);
    reference.geometric_fingerprint = Some(opencad_geometry::GeometricFingerprint {
        surface_type: "plane".into(),
        kernel_face_id: None,
        kernel_edge_id: None,
        area_range: None,
        bbox_hint: Some([[0., 0., 0.01], [0.04, 0.03, 0.01]]),
        adjacent_feature_ids: Vec::new(),
    });
    let refs = vec![reference];
    let order = [
        "feature:sketch_base",
        "feature:extrude_base",
        "feature:sketch_seat",
        "feature:seat",
    ]
    .map(String::from);
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &order,
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &refs,
    )
    .unwrap();
    let mut params = ParamGraph::new();
    for (name, value) in [
        ("bore_depth", "10 mm"),
        ("seat_diameter", "11 mm"),
        ("seat_depth", "4 mm"),
    ] {
        params
            .add_parameter(ParameterEntry::new(format!("param:{name}"), name, value))
            .unwrap();
    }
    (model, params, refs)
}
