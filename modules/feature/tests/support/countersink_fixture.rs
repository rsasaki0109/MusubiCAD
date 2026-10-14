use opencad_core::Length;
use opencad_feature::*;
use opencad_geometry::TopoRef;
use opencad_graph::{ParamGraph, ParameterEntry};
#[path = "seated_fixture.rs"]
mod counterbore;

pub fn fixture() -> (PartModel, ParamGraph, Vec<TopoRef>) {
    let (mut model, mut params, refs) = counterbore::fixture();
    params.remove_parameter("param:seat_depth").unwrap();
    params.remove_parameter("param:seat_diameter").unwrap();
    params
        .add_parameter(ParameterEntry::new(
            "param:seat_diameter",
            "seat_diameter",
            "12 mm",
        ))
        .unwrap();
    params
        .add_parameter(ParameterEntry::new(
            "param:seat_angle",
            "seat_angle_rad",
            "90 deg",
        ))
        .unwrap();
    let FeatureDefinition::SeatedStandardHole(def) =
        &mut model.nodes.get_mut("feature:seat").unwrap().definition
    else {
        panic!("seat")
    };
    def.seat = StandardHoleSeat::Countersink {
        diameter: Length::from_meters(0.012),
        included_angle: opencad_core::Angle::from_degrees(90.),
        diameter_expr: Some("seat_diameter".into()),
        included_angle_expr: Some("seat_angle_rad".into()),
    };
    model.nodes.get_mut("feature:seat").unwrap().name = "M6 countersink".into();
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        model.graph.ordered_ids(),
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &refs,
    )
    .unwrap();
    (model, params, refs)
}
