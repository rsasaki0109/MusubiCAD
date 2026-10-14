use opencad_core::{Angle, EntityId, Length, SketchId, TopoRefId};
use opencad_feature::*;
use opencad_geometry::{ExtrudeExtent, ExtrudeOperation, GeometricFingerprint, TopoRef};
use opencad_graph::{ParamGraph, ParameterEntry};
use opencad_sketch::{
    entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
    Sketch, Workplane,
};
pub fn fixture() -> (PartModel, ParamGraph, Vec<TopoRef>) {
    let mut model = PartModel::new();
    let mut sketch = Sketch::new(
        SketchId::new("sketch:stock").unwrap(),
        "Exact cylinder",
        Workplane::xy(),
    );
    sketch
        .add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new("ent:center").unwrap(),
                construction: false,
            },
            x: Coord::literal(0.012),
            y: Coord::literal(0.009),
        }))
        .unwrap();
    sketch
        .add_entity(SketchEntity::Circle(CircleEntity {
            base: EntityBase {
                id: EntityId::new("ent:circle").unwrap(),
                construction: false,
            },
            center: EntityId::new("ent:center").unwrap(),
            radius: Coord::literal(0.02),
        }))
        .unwrap();
    model.add_sketch(sketch).unwrap();
    for node in [
        FeatureNode::new(
            "feature:sketch_stock",
            "Cylinder sketch",
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: "sketch:stock".into(),
            }),
        ),
        FeatureNode::new(
            "feature:stock",
            "Original cylinder",
            FeatureDefinition::Extrude(ExtrudeFeature {
                sketch_feature: "feature:sketch_stock".into(),
                profile_ref: "sketch:stock/profile:outer".into(),
                extent: ExtrudeExtent::Distance {
                    length: Length::from_meters(0.03),
                },
                operation: ExtrudeOperation::NewBody,
                length_expr: None,
                target_feature: None,
            }),
        ),
        FeatureNode::new(
            "feature:draft",
            "Cylindrical draft",
            FeatureDefinition::CylindricalDraft(CylindricalDraftFeature {
                target_feature: "feature:stock".into(),
                face_ref: "ref:face:stock_side".into(),
                angle: Angle::from_degrees(7.),
                angle_expr: Some("draft_angle_rad".into()),
                neutral_plane: Workplane::Custom {
                    origin: [0., 0., 0.015],
                    normal: [0., 0., 1.],
                    x_axis: [1., 0., 0.],
                },
            }),
        ),
    ] {
        model.add_node(node).unwrap();
    }
    let mut reference = TopoRef::face(
        TopoRefId::new("ref:face:stock_side").unwrap(),
        "feature:stock",
        "cylindrical",
    );
    reference.semantic.normal_hint = Some([1., 0., 0.]);
    reference.geometric_fingerprint = Some(GeometricFingerprint {
        surface_type: "cylinder".into(),
        kernel_face_id: None,
        kernel_edge_id: None,
        area_range: None,
        bbox_hint: None,
        adjacent_feature_ids: Vec::new(),
    });
    let refs = vec![reference];
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &["feature:sketch_stock", "feature:stock", "feature:draft"].map(String::from),
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &refs,
    )
    .unwrap();
    let mut params = ParamGraph::new();
    params
        .add_parameter(ParameterEntry::new(
            "param:draft_angle",
            "draft_angle_rad",
            "7 deg",
        ))
        .unwrap();
    (model, params, refs)
}
