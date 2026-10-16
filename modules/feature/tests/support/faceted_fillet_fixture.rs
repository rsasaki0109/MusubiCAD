use opencad_core::Length;
use opencad_feature::*;
use opencad_geometry::TopoRef;
use opencad_graph::ParamGraph;
#[path = "nominal_thread_fixture.rs"]
mod cylinder;
pub fn fixture() -> (PartModel, ParamGraph, Vec<TopoRef>) {
    let (mut model, params, _) = cylinder::fixture();
    model.nodes.shift_remove("feature:thread");
    let sketch = model.sketches.get_mut("sketch:stock").unwrap();
    if let opencad_sketch::SketchEntity::Point(p) = &mut sketch.entities[0] {
        p.x = opencad_sketch::Coord::literal(0.);
        p.y = opencad_sketch::Coord::literal(0.);
    }
    if let opencad_sketch::SketchEntity::Circle(c) = &mut sketch.entities[1] {
        c.radius = opencad_sketch::Coord::literal(0.01);
    }
    sketch.update_profiles().unwrap();
    if let FeatureDefinition::Extrude(e) =
        &mut model.nodes.get_mut("feature:stock").unwrap().definition
    {
        e.extent = opencad_geometry::ExtrudeExtent::Distance {
            length: Length::from_meters(0.02),
        };
    }
    model
        .add_node(FeatureNode::new(
            "feature:fillet",
            "Faceted periodic circular fillet",
            FeatureDefinition::FacetedCircularFillet(FacetedCircularFilletFeature {
                target_feature: "feature:stock".into(),
                edge_ref: "ref:edge:stock_end".into(),
                representation: FacetedFilletRepresentation::GridFacetedPeriodicCosine,
                verification: FacetedFilletVerification::CanonicalGridWithNumericalBackendChecks,
                minimum_radius: Length::from_meters(0.0005),
                maximum_radius: Length::from_meters(0.0015),
                quality: FacetedFilletQuality {
                    maximum_position_error: Length::from_meters(4e-4),
                    maximum_normal_chord: 0.8,
                    ideal_volume_width_m3: 1e-11,
                    maximum_volume_difference_m3: 1e-7,
                    ideal_area_width_m2: 1e-7,
                    maximum_area_difference_m2: 1e-5,
                    ideal_first_moment_width_m4: 1e-12,
                    maximum_first_moment_difference_m4: 1e-9,
                    ideal_centroid_width: Length::from_meters(1e-6),
                    maximum_centroid_difference: Length::from_meters(2e-6),
                    max_integration_cells: 262144,
                    max_vertices: 5000,
                    max_triangles: 10000,
                },
            }),
        ))
        .unwrap();
    let refs = vec![TopoRef::edge(
        opencad_core::TopoRefId::new("ref:edge:stock_end").unwrap(),
        "feature:stock",
        "extrusion_end",
    )];
    model.graph = derive_feature_graph(
        &model.nodes.values().cloned().collect::<Vec<_>>(),
        &["feature:sketch_stock", "feature:stock", "feature:fillet"].map(String::from),
        &model.sketches.values().cloned().collect::<Vec<_>>(),
        &refs,
    )
    .unwrap();
    (model, params, refs)
}
