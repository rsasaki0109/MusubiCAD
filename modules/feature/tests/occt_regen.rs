//! OCCT-backed feature regeneration integration tests.

use opencad_core::{Length, TopoRefId};
use opencad_feature::{
    bearing_carrier, bracket_base_plate, bracket_edge_fillet, bracket_hole_row, bracket_pin_mirror,
    bracket_semantic_refs, bracket_with_hole, bracket_with_top_chamfer, bracket_with_top_fillet,
    profile_to_solved, robot_joint_actuator_housing, FeatureRegistry, PartModel,
};
use opencad_geometry::{
    resolve_kernel_face_id_for_topo_ref_with_discoveries, sync_semantic_refs_with_history,
    ExtrudeExtent, ExtrudeOperation, GeometryKernel, TopoRef,
};
use opencad_graph::{
    bearing_carrier_parameters, bracket_parameters, robot_joint_housing_parameters,
};
use opencad_kernel_occt::OcctGeometryKernel;

#[test]
fn occt_direct_extrude_matches_expected_volume() {
    let model = bracket_base_plate().expect("model");
    let sketch = model.sketches.get("sketch:base").expect("sketch");
    let solved = profile_to_solved(sketch, "sketch:base/profile:outer").expect("solved");
    let kernel = OcctGeometryKernel::new();
    let wire = kernel.make_wire_from_sketch(&solved).expect("wire");
    let body = kernel
        .extrude(
            wire,
            ExtrudeExtent::Distance {
                length: Length::from_meters(0.006),
            },
            ExtrudeOperation::NewBody,
            None,
            [0.0, 0.0, 1.0],
        )
        .expect("extrude");
    let mass = kernel.mass_properties(&body, 2700.0).expect("mass");
    let expected = 0.08 * 0.06 * 0.006;
    assert!(
        (mass.volume_m3 - expected).abs() < 1e-8,
        "points={:?} volume={} expected={}",
        solved.points,
        mass.volume_m3,
        expected
    );
}

#[test]
fn occt_regenerates_bracket_plate_volume() {
    let mut model = bracket_base_plate().expect("model");
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, None, None)
        .expect("regen");

    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let expected = 0.08 * 0.06 * 0.006;
    assert!(
        (mass.volume_m3 - expected).abs() < 1e-8,
        "volume={} expected={}",
        mass.volume_m3,
        expected
    );
    assert!(mass.mass_kg > 0.0);
}

#[test]
fn occt_bearing_carrier_preserves_bore_and_bolt_circle_across_hub_edit() {
    let mut model = bearing_carrier().expect("model");
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let mut parameters = bearing_carrier_parameters();
    let before_report = model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("before regen");
    assert_eq!(before_report.regenerated.len(), 9);
    let before = kernel
        .mass_properties(model.active_body().expect("before body"), 2700.0)
        .expect("before mass");

    parameters
        .set_expr("param:boss_height", "20 mm")
        .expect("edit hub height");
    let after_report = model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("after regen");
    let after = kernel
        .mass_properties(model.active_body().expect("after body"), 2700.0)
        .expect("after mass");

    assert_eq!(after_report.regenerated.len(), 9);
    assert!(model.outputs.contains_key("feature:bearing_bore"));
    assert!(model.outputs.contains_key("feature:bolt_circle"));
    let mass_delta_kg = after.mass_kg - before.mass_kg;
    assert!(
        (0.014..=0.019).contains(&mass_delta_kg),
        "mass delta {mass_delta_kg} kg"
    );
}

#[test]
fn occt_robot_joint_preserves_downstream_features_across_tower_edit() {
    let mut model = robot_joint_actuator_housing().expect("model");
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let mut parameters = robot_joint_housing_parameters();
    let before_report = model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("before regen");
    let before = kernel
        .mass_properties(model.active_body().expect("before body"), 2700.0)
        .expect("before mass");

    assert_eq!(before_report.regenerated.len(), 22);
    assert!((0.60..=0.62).contains(&before.mass_kg));

    parameters
        .set_expr("param:upper_hub_height", "42 mm")
        .expect("edit bearing tower height");
    let after_report = model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("after regen");
    let after = kernel
        .mass_properties(model.active_body().expect("after body"), 2700.0)
        .expect("after mass");

    assert_eq!(after_report.regenerated.len(), 22);
    for feature_id in [
        "feature:shaft_bore",
        "feature:counterbore",
        "feature:pcd_fasteners",
        "feature:radial_ribs",
        "feature:mounting_ears",
        "feature:mounting_holes",
    ] {
        assert!(model.outputs.contains_key(feature_id), "{feature_id}");
    }
    let mass_delta_kg = after.mass_kg - before.mass_kg;
    assert!(
        (0.043..=0.048).contains(&mass_delta_kg),
        "mass delta {mass_delta_kg} kg"
    );

    parameters
        .set_expr("param:upper_hub_height", "32 mm")
        .expect("restore tower height");
    parameters
        .set_expr("param:bolt_circle_radius", "50 mm")
        .expect("edit bolt circle radius");
    let pcd_report = model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("PCD regen");
    let pcd_mass = kernel
        .mass_properties(model.active_body().expect("PCD body"), 2700.0)
        .expect("PCD mass");
    assert_eq!(pcd_report.regenerated.len(), 22);
    assert!(model.outputs.contains_key("feature:pcd_fasteners"));
    assert!(
        (pcd_mass.mass_kg - before.mass_kg).abs() < 1e-6,
        "moving eight equal holes should preserve mass: {} vs {} kg",
        pcd_mass.mass_kg,
        before.mass_kg
    );
}

#[test]
fn occt_regenerates_bracket_with_hole_reduces_volume() {
    let mut model = bracket_with_hole().expect("model");
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    let semantic_refs = bracket_semantic_refs();
    model
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");

    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    let hole_radius = 0.005;
    let hole_volume = std::f64::consts::PI * hole_radius * hole_radius * 0.006;
    let expected = plate_volume - hole_volume;
    assert!(
        mass.volume_m3 < plate_volume,
        "hole should reduce volume: {} vs plate {}",
        mass.volume_m3,
        plate_volume
    );
    assert!(
        (mass.volume_m3 - expected).abs() < 1e-7,
        "volume={} expected={}",
        mass.volume_m3,
        expected
    );
}

#[test]
fn occt_top_fillet_reduces_volume() {
    let mut model = bracket_with_top_fillet().expect("model");
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");

    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");

    let mut without_fillet = bracket_with_hole().expect("model");
    without_fillet
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let base_body = without_fillet.active_body().expect("body");
    let base_mass = kernel
        .mass_properties(base_body, 2700.0)
        .expect("base mass");

    assert!(
        mass.volume_m3 < base_mass.volume_m3,
        "fillet should reduce volume: {} vs {}",
        mass.volume_m3,
        base_mass.volume_m3
    );
    assert!(mass.mass_kg > 0.0);
}

#[test]
fn occt_regen_composes_boolean_and_fillet_history() {
    let mut model = bracket_with_top_fillet().expect("model");
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    let report = model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let final_only = kernel.face_derivation_history(body);

    assert!(
        report.face_history.len() > final_only.len(),
        "composed history should include boolean + fillet steps"
    );

    // History pairs are enumeration indices local to each operation
    // (ADR-018), so they are not merged into one cross-operation map.
    assert!(final_only.iter().all(|(post, src)| *post >= 1 && *src >= 1));
}

#[test]
fn occt_top_chamfer_reduces_volume() {
    let mut model = bracket_with_top_chamfer().expect("model");
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");

    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");

    let mut without_chamfer = bracket_with_hole().expect("model");
    without_chamfer
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let base_body = without_chamfer.active_body().expect("body");
    let base_mass = kernel
        .mass_properties(base_body, 2700.0)
        .expect("base mass");

    assert!(
        mass.volume_m3 < base_mass.volume_m3,
        "chamfer should reduce volume: {} vs {}",
        mass.volume_m3,
        base_mass.volume_m3
    );
    assert!(mass.mass_kg > 0.0);
}

#[test]
fn occt_fillet_on_face_ref_matches_top_perimeter() {
    use opencad_core::TopoRefId;
    use opencad_feature::{FeatureDefinition, FeatureNode, FilletFeature};
    use opencad_geometry::TopoRef;

    let params = bracket_parameters();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();

    let mut face_ref_model = bracket_with_hole().expect("model");
    face_ref_model
        .add_node(FeatureNode::new(
            "feature:fillet_top",
            "Top Fillet",
            FeatureDefinition::Fillet(FilletFeature::on_face_ref(
                "feature:hole_mount",
                "ref:face:bracket_top",
                Length::from_meters(0.001),
                Some("fillet_radius".into()),
            )),
        ))
        .expect("node");
    face_ref_model
        .add_dependency("feature:hole_mount", "feature:fillet_top")
        .expect("dep");

    let mut baseline_model = bracket_with_top_fillet().expect("model");
    baseline_model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let baseline_mass = kernel
        .mass_properties(baseline_model.active_body().expect("body"), 2700.0)
        .expect("mass");

    let semantic_refs = vec![TopoRef::face(
        TopoRefId::new("ref:face:bracket_top").expect("id"),
        "feature:extrude_base",
        "top",
    )];

    face_ref_model
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");
    let face_ref_mass = kernel
        .mass_properties(face_ref_model.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        (face_ref_mass.volume_m3 - baseline_mass.volume_m3).abs() < 1e-8,
        "face_ref fillet volume {} should match top perimeter {}",
        face_ref_mass.volume_m3,
        baseline_mass.volume_m3
    );
}

#[test]
fn occt_edge_ref_fillet_affects_less_volume_than_top_perimeter() {
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    let semantic_refs = opencad_feature::bracket_semantic_refs();

    let mut without_fillet = bracket_with_hole().expect("model");
    without_fillet
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");
    let base_mass = kernel
        .mass_properties(without_fillet.active_body().expect("body"), 2700.0)
        .expect("base mass");

    let mut edge_model = bracket_edge_fillet().expect("model");
    edge_model
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");
    let edge_mass = kernel
        .mass_properties(edge_model.active_body().expect("body"), 2700.0)
        .expect("edge mass");

    let mut full_model = bracket_with_top_fillet().expect("model");
    full_model
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");
    let full_mass = kernel
        .mass_properties(full_model.active_body().expect("body"), 2700.0)
        .expect("full mass");

    assert!(
        edge_mass.volume_m3 < base_mass.volume_m3,
        "edge fillet should reduce volume: {} vs {}",
        edge_mass.volume_m3,
        base_mass.volume_m3
    );
    assert!(
        edge_mass.volume_m3 > full_mass.volume_m3,
        "single-edge fillet {} should remove less material than top perimeter {}",
        edge_mass.volume_m3,
        full_mass.volume_m3
    );
}

#[test]
fn occt_face_sketch_pin_joins_onto_plate() {
    use opencad_feature::{apply_parameters, bracket_base_plate, bracket_face_pin};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    let refs = opencad_feature::bracket_semantic_refs();

    let mut plate = bracket_base_plate().expect("plate");
    apply_parameters(&mut plate, &params).expect("apply");
    plate
        .regenerate(&kernel, &registry, Some(&params), Some(&refs))
        .expect("regen");
    let plate_mass = kernel
        .mass_properties(plate.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut pinned = bracket_face_pin().expect("model");
    pinned
        .regenerate(&kernel, &registry, Some(&params), Some(&refs))
        .expect("regen");
    let pinned_mass = kernel
        .mass_properties(pinned.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        pinned_mass.volume_m3 > plate_mass.volume_m3,
        "face sketch pin should fuse onto plate: {} vs {}",
        pinned_mass.volume_m3,
        plate_mass.volume_m3
    );
}

#[test]
fn occt_revolve_bushing_has_annulus_volume() {
    use opencad_feature::revolve_bushing;
    use opencad_graph::revolve_parameters;

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = revolve_parameters("360 deg");
    let mut model = revolve_bushing().expect("model");
    model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let mass = kernel
        .mass_properties(model.active_body().expect("body"), 2700.0)
        .expect("mass");

    let outer = 0.025_f64;
    let inner = 0.015_f64;
    let height = 0.02_f64;
    let expected = std::f64::consts::PI * (outer.powi(2) - inner.powi(2)) * height;
    assert!(
        (mass.volume_m3 - expected).abs() < 1e-8,
        "revolve bushing volume {} should match annulus {}",
        mass.volume_m3,
        expected
    );
}

#[test]
fn occt_revolve_sector_is_half_bushing_volume() {
    use opencad_feature::{revolve_bushing, revolve_sector};
    use opencad_graph::revolve_parameters;

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let full_params = revolve_parameters("360 deg");
    let sector_params = revolve_parameters("180 deg");

    let mut full = revolve_bushing().expect("model");
    full.regenerate(&kernel, &registry, Some(&full_params), None)
        .expect("regen");
    let full_mass = kernel
        .mass_properties(full.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut sector = revolve_sector().expect("model");
    sector
        .regenerate(&kernel, &registry, Some(&sector_params), None)
        .expect("regen");
    let sector_mass = kernel
        .mass_properties(sector.active_body().expect("body"), 2700.0)
        .expect("mass");

    let outer = 0.025_f64;
    let inner = 0.015_f64;
    let height = 0.02_f64;
    let expected_full = std::f64::consts::PI * (outer.powi(2) - inner.powi(2)) * height;
    let expected_sector = expected_full * 0.5;

    assert!(
        (sector_mass.volume_m3 - expected_sector).abs() < 1e-8,
        "180° sector volume {} should match half annulus {}",
        sector_mass.volume_m3,
        expected_sector
    );
    assert!(
        sector_mass.volume_m3 < full_mass.volume_m3,
        "sector {} should be smaller than full revolve {}",
        sector_mass.volume_m3,
        full_mass.volume_m3
    );
}

#[test]
fn occt_join_extrude_fuses_onto_plate() {
    use opencad_feature::{apply_parameters, bracket_base_plate, bracket_boss_join};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut plate = bracket_base_plate().expect("plate");
    apply_parameters(&mut plate, &params).expect("apply");
    plate
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let plate_mass = kernel
        .mass_properties(plate.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut joined = bracket_boss_join().expect("model");
    joined
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let joined_mass = kernel
        .mass_properties(joined.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        joined_mass.volume_m3 > plate_mass.volume_m3,
        "join extrude should fuse boss onto plate: {} vs {}",
        joined_mass.volume_m3,
        plate_mass.volume_m3
    );
}

#[test]
fn occt_boss_join_radius_follows_hole_diameter() {
    use opencad_feature::{apply_parameters, bracket_boss_join};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();

    let default_params = bracket_parameters();
    let mut large_params = bracket_parameters();
    large_params
        .set_expr("param:hole_diameter", "20 mm")
        .expect("hole_diameter");

    let mut default_model = bracket_boss_join().expect("model");
    default_model
        .regenerate(&kernel, &registry, Some(&default_params), None)
        .expect("regen");
    let default_mass = kernel
        .mass_properties(default_model.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut large_model = bracket_boss_join().expect("model");
    apply_parameters(&mut large_model, &large_params).expect("apply");
    large_model
        .regenerate(&kernel, &registry, Some(&large_params), None)
        .expect("regen");
    let large_mass = kernel
        .mass_properties(large_model.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        large_mass.volume_m3 > default_mass.volume_m3,
        "larger hole_diameter should grow boss join volume: {} vs {}",
        large_mass.volume_m3,
        default_mass.volume_m3
    );
}

#[test]
fn occt_linear_pattern_unions_translated_bodies() {
    use opencad_feature::{apply_parameters, FeatureDefinition, FeatureNode, LinearPatternFeature};

    let mut model = bracket_base_plate().expect("model");
    let params = bracket_parameters();
    apply_parameters(&mut model, &params).expect("apply");
    model
        .add_node(FeatureNode::new(
            "feature:plate_row",
            "Plate Row",
            FeatureDefinition::LinearPattern(LinearPatternFeature::new(
                "feature:extrude_base",
                [0.0, 0.0, 1.0],
                Length::from_meters(0.006),
                2,
            )),
        ))
        .expect("pattern");
    model
        .add_dependency("feature:extrude_base", "feature:plate_row")
        .expect("dep");

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let patterned_mass = kernel
        .mass_properties(model.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut single_model = bracket_base_plate().expect("model");
    apply_parameters(&mut single_model, &params).expect("apply");
    single_model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let single_mass = kernel
        .mass_properties(single_model.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        patterned_mass.volume_m3 > single_mass.volume_m3,
        "patterned plates should increase volume: {} vs {}",
        patterned_mass.volume_m3,
        single_mass.volume_m3
    );
}

#[test]
fn occt_linear_union_pattern_fuses_onto_target() {
    use opencad_feature::{FeatureDefinition, FeatureNode, LinearPatternFeature};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut plate = pin_tool_plate();
    plate
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let plate_mass = kernel
        .mass_properties(plate.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut fused = pin_tool_plate();
    fused
        .add_node(FeatureNode::new(
            "feature:pin_pair",
            "Pin Pair",
            FeatureDefinition::LinearPattern(LinearPatternFeature::union_on(
                "feature:pin_tool",
                "feature:extrude_base",
                [1.0, 0.0, 0.0],
                Length::from_meters(0.02),
                2,
            )),
        ))
        .expect("pattern");
    fused
        .add_dependency("feature:extrude_base", "feature:pin_pair")
        .expect("dep");
    fused
        .add_dependency("feature:pin_tool", "feature:pin_pair")
        .expect("dep");
    fused
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let fused_mass = kernel
        .mass_properties(fused.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        fused_mass.volume_m3 > plate_mass.volume_m3,
        "linear union should fuse patterned tools onto plate: {} vs {}",
        fused_mass.volume_m3,
        plate_mass.volume_m3
    );
}

#[test]
fn occt_circular_union_pattern_fuses_onto_target() {
    use opencad_feature::{CircularPatternFeature, FeatureDefinition, FeatureNode};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut plate = pin_tool_plate_at(0.04, 0.03);
    plate
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let plate_mass = kernel
        .mass_properties(plate.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut fused = pin_tool_plate_at(0.04, 0.03);
    fused
        .add_node(FeatureNode::new(
            "feature:pin_ring",
            "Pin Ring",
            FeatureDefinition::CircularPattern(CircularPatternFeature::union_on(
                "feature:pin_tool",
                "feature:extrude_base",
                [0.04, 0.03, 0.0],
                [0.0, 0.0, 1.0],
                4,
            )),
        ))
        .expect("pattern");
    fused
        .add_dependency("feature:extrude_base", "feature:pin_ring")
        .expect("dep");
    fused
        .add_dependency("feature:pin_tool", "feature:pin_ring")
        .expect("dep");
    fused
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let fused_mass = kernel
        .mass_properties(fused.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        fused_mass.volume_m3 > plate_mass.volume_m3,
        "circular union should fuse patterned tools onto plate: {} vs {}",
        fused_mass.volume_m3,
        plate_mass.volume_m3
    );
}

fn pin_tool_plate() -> opencad_feature::PartModel {
    pin_tool_plate_at(0.01, 0.01)
}

fn pin_tool_plate_at(center_x: f64, center_y: f64) -> opencad_feature::PartModel {
    use opencad_core::{ConstraintId, EntityId, Expression, SketchId};
    use opencad_feature::{
        apply_parameters, ExtrudeFeature, FeatureDefinition, FeatureNode, SketchFeatureDef,
    };
    use opencad_geometry::{ExtrudeExtent, ExtrudeOperation};
    use opencad_sketch::{
        constraint::Constraint,
        entity::{CircleEntity, Coord, EntityBase, PointEntity, SketchEntity},
        workplane::Workplane,
        Sketch,
    };

    let mut model = bracket_base_plate().expect("plate");
    apply_parameters(&mut model, &bracket_parameters()).expect("apply");

    let mut pin_sketch = Sketch::new(
        SketchId::new("sketch:pin").expect("id"),
        "Pin Sketch",
        Workplane::xy(),
    );
    pin_sketch
        .add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new("ent:pin_center").expect("id"),
                construction: false,
            },
            x: Coord::literal(center_x),
            y: Coord::literal(center_y),
        }))
        .expect("point");
    pin_sketch
        .add_entity(SketchEntity::Circle(CircleEntity {
            base: EntityBase {
                id: EntityId::new("ent:pin_circle").expect("id"),
                construction: false,
            },
            center: EntityId::new("ent:pin_center").expect("id"),
            radius: Coord::literal(0.002),
        }))
        .expect("circle");
    pin_sketch
        .add_constraint(Constraint::Radius {
            id: ConstraintId::new("con:pin_radius").expect("id"),
            target: EntityId::new("ent:pin_circle").expect("id"),
            expr: Expression::new("2 mm").expect("expr"),
        })
        .expect("radius");
    model
        .sketches
        .insert(pin_sketch.id.as_str().to_string(), pin_sketch);

    model
        .add_node(FeatureNode::new(
            "feature:sketch_pin",
            "Pin Sketch",
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: "sketch:pin".into(),
            }),
        ))
        .expect("sketch feature");
    model
        .add_node(FeatureNode::new(
            "feature:pin_tool",
            "Pin Tool",
            FeatureDefinition::Extrude(ExtrudeFeature {
                sketch_feature: "feature:sketch_pin".into(),
                profile_ref: "sketch:pin/profile:outer".into(),
                extent: ExtrudeExtent::Distance {
                    length: Length::from_meters(0.006),
                },
                operation: ExtrudeOperation::NewBody,
                length_expr: Some("thickness".into()),
                target_feature: None,
            }),
        ))
        .expect("pin tool");
    model
        .add_dependency("feature:sketch_pin", "feature:pin_tool")
        .expect("dep");
    model
}

#[test]
fn occt_linear_cut_pattern_subtracts_more_than_single_cut() {
    use opencad_feature::{FeatureDefinition, FeatureNode, LinearPatternFeature};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut single_cut = pin_tool_plate();
    single_cut
        .add_node(FeatureNode::new(
            "feature:pin_hole",
            "Single Pin Hole",
            FeatureDefinition::LinearPattern(LinearPatternFeature::cut(
                "feature:pin_tool",
                "feature:extrude_base",
                [1.0, 0.0, 0.0],
                Length::from_meters(0.02),
                1,
            )),
        ))
        .expect("pattern");
    single_cut
        .add_dependency("feature:extrude_base", "feature:pin_hole")
        .expect("dep");
    single_cut
        .add_dependency("feature:pin_tool", "feature:pin_hole")
        .expect("dep");
    single_cut
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let single_mass = kernel
        .mass_properties(single_cut.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut double_cut = pin_tool_plate();
    double_cut
        .add_node(FeatureNode::new(
            "feature:pin_holes",
            "Double Pin Holes",
            FeatureDefinition::LinearPattern(LinearPatternFeature::cut(
                "feature:pin_tool",
                "feature:extrude_base",
                [1.0, 0.0, 0.0],
                Length::from_meters(0.02),
                2,
            )),
        ))
        .expect("pattern");
    double_cut
        .add_dependency("feature:extrude_base", "feature:pin_holes")
        .expect("dep");
    double_cut
        .add_dependency("feature:pin_tool", "feature:pin_holes")
        .expect("dep");
    double_cut
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let double_mass = kernel
        .mass_properties(double_cut.active_body().expect("body"), 2700.0)
        .expect("mass");

    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        single_mass.volume_m3 < plate_volume,
        "single cut should reduce plate volume: {} vs {}",
        single_mass.volume_m3,
        plate_volume
    );
    assert!(
        double_mass.volume_m3 < single_mass.volume_m3,
        "double cut should remove more material: {} vs {}",
        double_mass.volume_m3,
        single_mass.volume_m3
    );
}

#[test]
fn occt_circular_pattern_unions_rotated_bodies() {
    use opencad_feature::{CircularPatternFeature, FeatureDefinition, FeatureNode};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut single = pin_tool_plate_at(0.042, 0.03);
    single
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let single_mass = kernel
        .mass_properties(
            single
                .outputs
                .get("feature:pin_tool")
                .and_then(|output| output.body.as_ref())
                .expect("pin tool"),
            2700.0,
        )
        .expect("mass");

    let mut ring = pin_tool_plate_at(0.042, 0.03);
    ring.add_node(FeatureNode::new(
        "feature:pin_ring",
        "Pin Ring",
        FeatureDefinition::CircularPattern(CircularPatternFeature::new(
            "feature:pin_tool",
            [0.04, 0.03, 0.0],
            [0.0, 0.0, 1.0],
            4,
        )),
    ))
    .expect("pattern");
    ring.add_dependency("feature:pin_tool", "feature:pin_ring")
        .expect("dep");
    ring.regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let ring_mass = kernel
        .mass_properties(ring.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        ring_mass.volume_m3 > single_mass.volume_m3 * 3.0,
        "circular union should combine multiple pins: {} vs {}",
        ring_mass.volume_m3,
        single_mass.volume_m3
    );
}

#[test]
fn occt_circular_cut_pattern_reduces_plate_volume() {
    use opencad_feature::{CircularPatternFeature, FeatureDefinition, FeatureNode};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut plate = pin_tool_plate();
    plate
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let plate_mass = kernel
        .mass_properties(
            plate
                .outputs
                .get("feature:extrude_base")
                .and_then(|output| output.body.as_ref())
                .expect("plate"),
            2700.0,
        )
        .expect("mass");

    let mut cut = pin_tool_plate();
    cut.add_node(FeatureNode::new(
        "feature:pin_hole_ring",
        "Pin Hole Ring",
        FeatureDefinition::CircularPattern({
            let mut pattern = CircularPatternFeature::new(
                "feature:pin_tool",
                [0.04, 0.03, 0.0],
                [0.0, 0.0, 1.0],
                4,
            );
            pattern.operation = opencad_feature::PatternOperation::Cut;
            pattern.target_feature = Some("feature:extrude_base".into());
            pattern
        }),
    ))
    .expect("pattern");
    cut.add_dependency("feature:extrude_base", "feature:pin_hole_ring")
        .expect("dep");
    cut.add_dependency("feature:pin_tool", "feature:pin_hole_ring")
        .expect("dep");
    cut.regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let cut_mass = kernel
        .mass_properties(cut.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        cut_mass.volume_m3 < plate_mass.volume_m3,
        "circular cut should reduce plate volume: {} vs {}",
        cut_mass.volume_m3,
        plate_mass.volume_m3
    );
}

#[test]
fn occt_linear_pattern_spacing_expr_resolves_before_regen() {
    use opencad_feature::{FeatureDefinition, FeatureNode, LinearPatternFeature};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut explicit = pin_tool_plate();
    explicit
        .add_node(FeatureNode::new(
            "feature:pin_holes",
            "Explicit Pitch Holes",
            FeatureDefinition::LinearPattern(LinearPatternFeature::cut(
                "feature:pin_tool",
                "feature:extrude_base",
                [1.0, 0.0, 0.0],
                Length::from_meters(0.02),
                2,
            )),
        ))
        .expect("pattern");
    explicit
        .add_dependency("feature:extrude_base", "feature:pin_holes")
        .expect("dep");
    explicit
        .add_dependency("feature:pin_tool", "feature:pin_holes")
        .expect("dep");
    explicit
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let explicit_mass = kernel
        .mass_properties(explicit.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut expr_model = pin_tool_plate();
    let mut pattern = LinearPatternFeature::cut(
        "feature:pin_tool",
        "feature:extrude_base",
        [1.0, 0.0, 0.0],
        Length::from_meters(0.01),
        2,
    );
    pattern.spacing_expr = Some("hole_pitch".into());
    expr_model
        .add_node(FeatureNode::new(
            "feature:pin_holes",
            "Parametric Pitch Holes",
            FeatureDefinition::LinearPattern(pattern),
        ))
        .expect("pattern");
    expr_model
        .add_dependency("feature:extrude_base", "feature:pin_holes")
        .expect("dep");
    expr_model
        .add_dependency("feature:pin_tool", "feature:pin_holes")
        .expect("dep");
    expr_model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let expr_mass = kernel
        .mass_properties(expr_model.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        (expr_mass.volume_m3 - explicit_mass.volume_m3).abs() < 1e-8,
        "spacing_expr should match explicit spacing: {} vs {}",
        expr_mass.volume_m3,
        explicit_mass.volume_m3
    );
}

#[test]
fn occt_mirror_pattern_unions_reflected_bodies() {
    use opencad_feature::{FeatureDefinition, FeatureNode, MirrorPatternFeature};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut single = pin_tool_plate_at(0.041, 0.03);
    single
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let single_mass = kernel
        .mass_properties(
            single
                .outputs
                .get("feature:pin_tool")
                .and_then(|output| output.body.as_ref())
                .expect("pin tool"),
            2700.0,
        )
        .expect("mass");

    let mut mirrored = pin_tool_plate_at(0.041, 0.03);
    mirrored
        .add_node(FeatureNode::new(
            "feature:pin_pair",
            "Pin Pair",
            FeatureDefinition::MirrorPattern(MirrorPatternFeature::new(
                "feature:pin_tool",
                [0.04, 0.0, 0.0],
                [1.0, 0.0, 0.0],
            )),
        ))
        .expect("pattern");
    mirrored
        .add_dependency("feature:pin_tool", "feature:pin_pair")
        .expect("dep");
    mirrored
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let pair_mass = kernel
        .mass_properties(mirrored.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        pair_mass.volume_m3 > single_mass.volume_m3 * 1.5,
        "mirror union should combine source and reflection: {} vs {}",
        pair_mass.volume_m3,
        single_mass.volume_m3
    );
}

#[test]
fn occt_mirror_cut_pattern_reduces_plate_volume() {
    use opencad_feature::{FeatureDefinition, FeatureNode, MirrorPatternFeature};

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();

    let mut plate = pin_tool_plate();
    plate
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let plate_mass = kernel
        .mass_properties(
            plate
                .outputs
                .get("feature:extrude_base")
                .and_then(|output| output.body.as_ref())
                .expect("plate"),
            2700.0,
        )
        .expect("mass");

    let mut cut = pin_tool_plate();
    cut.add_node(FeatureNode::new(
        "feature:pin_hole_pair",
        "Pin Hole Pair",
        FeatureDefinition::MirrorPattern(MirrorPatternFeature::cut(
            "feature:pin_tool",
            "feature:extrude_base",
            [0.04, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        )),
    ))
    .expect("pattern");
    cut.add_dependency("feature:extrude_base", "feature:pin_hole_pair")
        .expect("dep");
    cut.add_dependency("feature:pin_tool", "feature:pin_hole_pair")
        .expect("dep");
    cut.regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let cut_mass = kernel
        .mass_properties(cut.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        cut_mass.volume_m3 < plate_mass.volume_m3,
        "mirror cut should reduce plate volume: {} vs {}",
        cut_mass.volume_m3,
        plate_mass.volume_m3
    );
}

#[test]
fn occt_hole_with_face_ref_matches_without_when_refs_present() {
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    let semantic_refs = bracket_semantic_refs();

    let mut with_refs = bracket_with_hole().expect("model");
    with_refs
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");
    let with_refs_mass = kernel
        .mass_properties(with_refs.active_body().expect("body"), 2700.0)
        .expect("mass");

    let mut without_refs = bracket_with_hole().expect("model");
    without_refs
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let without_refs_mass = kernel
        .mass_properties(without_refs.active_body().expect("body"), 2700.0)
        .expect("mass");

    assert!(
        (with_refs_mass.volume_m3 - without_refs_mass.volume_m3).abs() < 1e-8,
        "face_ref hole should match fallback target: {} vs {}",
        with_refs_mass.volume_m3,
        without_refs_mass.volume_m3
    );
}

#[test]
fn occt_mirror_pattern_uses_plane_face_ref() {
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let params = bracket_parameters();
    let semantic_refs = bracket_semantic_refs();

    let mut single = bracket_pin_mirror().expect("model");
    single
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");
    let single_mass = kernel
        .mass_properties(
            single
                .outputs
                .get("feature:pin_tool")
                .and_then(|output| output.body.as_ref())
                .expect("pin tool"),
            2700.0,
        )
        .expect("mass");

    let mirrored_mass = kernel
        .mass_properties(single.active_body().expect("body"), 2700.0)
        .expect("mass");

    let plate_mass = kernel
        .mass_properties(
            single
                .outputs
                .get("feature:extrude_base")
                .and_then(|output| output.body.as_ref())
                .expect("plate"),
            2700.0,
        )
        .expect("mass");

    assert!(
        mirrored_mass.volume_m3 > plate_mass.volume_m3,
        "plane_face_ref mirror should fuse pins onto plate: {} vs {}",
        mirrored_mass.volume_m3,
        plate_mass.volume_m3
    );
    assert!(
        mirrored_mass.volume_m3 > single_mass.volume_m3 * 1.5,
        "plane_face_ref mirror should union source and reflection: {} vs {}",
        mirrored_mass.volume_m3,
        single_mass.volume_m3
    );
}

type ModelBuilder = fn() -> opencad_core::Result<PartModel>;

struct SemanticReferenceRegression {
    name: &'static str,
    seed_model: ModelBuilder,
    edited_model: ModelBuilder,
    parameter_id: &'static str,
    edited_expression: &'static str,
}

fn discovered_faces(
    kernel: &OcctGeometryKernel,
    model: &PartModel,
) -> Vec<opencad_geometry::FaceRefDiscovery> {
    let body = model.active_body().expect("active body");
    let nodes = model.nodes.values().cloned().collect::<Vec<_>>();
    opencad_feature::face_discover::discover_face_refs_from_body(kernel, body, &nodes)
        .expect("face discoveries")
}

fn sync_top_face_reference(
    kernel: &OcctGeometryKernel,
    model: &mut PartModel,
    registry: &FeatureRegistry,
    parameters: &opencad_graph::ParamGraph,
    semantic_refs: &[TopoRef],
) -> (opencad_feature::RegenReport, Vec<TopoRef>) {
    let report = model
        .regenerate(kernel, registry, Some(parameters), Some(semantic_refs))
        .expect("regenerate");
    let discoveries = discovered_faces(kernel, model);
    let synced = sync_semantic_refs_with_history(semantic_refs, &report.face_history, &discoveries);
    (report, synced)
}

fn assert_top_ref_survives_feature_parameter_edit(case: SemanticReferenceRegression) {
    const REF_ID: &str = "ref:face:bracket_top";
    const CREATED_BY: &str = "feature:extrude_base";

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let mut parameters = bracket_parameters();
    let initial_refs = vec![TopoRef::face(
        TopoRefId::new(REF_ID).expect("ref id"),
        CREATED_BY,
        "top",
    )];

    // Seed the semantic reference from the common source feature. This models
    // a persisted ref before a later boolean/fillet, chamfer, or pattern node
    // is regenerated.
    let mut seed = (case.seed_model)().expect("seed model");
    let (_seed_report, seeded_refs) =
        sync_top_face_reference(&kernel, &mut seed, &registry, &parameters, &initial_refs);
    let seeded = seeded_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == REF_ID)
        .expect("seeded semantic ref");
    assert_eq!(seeded.ref_id.as_str(), REF_ID, "{}: seed ref id", case.name);
    assert!(
        seeded.kernel_face_id().is_some(),
        "{}: seed should resolve a current kernel face",
        case.name
    );

    let mut model = (case.edited_model)().expect("edited model");
    let (_before_report, before_refs) =
        sync_top_face_reference(&kernel, &mut model, &registry, &parameters, &seeded_refs);
    let before = before_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == REF_ID)
        .expect("pre-edit semantic ref");
    assert_eq!(
        before.ref_id.as_str(),
        REF_ID,
        "{}: pre-edit ref id",
        case.name
    );
    assert!(
        before.kernel_face_id().is_some(),
        "{}: pre-edit kernel face",
        case.name
    );

    parameters
        .set_expr(case.parameter_id, case.edited_expression)
        .expect("edit parameter");
    let (after_report, after_refs) =
        sync_top_face_reference(&kernel, &mut model, &registry, &parameters, &before_refs);
    assert!(
        !after_report.face_history.is_empty(),
        "{}: edit must produce face derivation history",
        case.name
    );
    assert!(
        after_report
            .face_history
            .iter()
            .any(|(post_id, source_id)| post_id != source_id),
        "{}: edit must expose a topology rebind candidate",
        case.name
    );

    let after = after_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == REF_ID)
        .expect("post-edit semantic ref");
    assert_eq!(
        after.ref_id.as_str(),
        REF_ID,
        "{}: post-edit ref id",
        case.name
    );
    let after_discoveries = discovered_faces(&kernel, &model);

    // Resolution is checked against the edited regeneration history rather
    // than comparing geometry floats. The stored semantic ref must resolve to
    // the ID belonging to the edited body's current kernel topology.
    let resolved = resolve_kernel_face_id_for_topo_ref_with_discoveries(
        &before_refs,
        &after_report.face_history,
        REF_ID,
        Some(&after_discoveries),
    )
    .expect("resolve edited kernel face");
    assert!(
        after_discoveries
            .iter()
            .any(|discovery| discovery.kernel_face_id == resolved),
        "{}: resolved ID must belong to the edited current topology",
        case.name
    );
}

#[test]
fn occt_semantic_toporef_survives_boolean_fillet_chamfer_and_linear_pattern_edits() {
    let cases = [
        SemanticReferenceRegression {
            name: "boolean hole",
            seed_model: bracket_base_plate,
            edited_model: bracket_with_hole,
            parameter_id: "param:hole_diameter",
            edited_expression: "12 mm",
        },
        SemanticReferenceRegression {
            name: "fillet",
            seed_model: bracket_with_hole,
            edited_model: bracket_with_top_fillet,
            parameter_id: "param:fillet_radius",
            edited_expression: "2 mm",
        },
        SemanticReferenceRegression {
            name: "chamfer",
            seed_model: bracket_with_hole,
            edited_model: bracket_with_top_chamfer,
            parameter_id: "param:chamfer_distance",
            edited_expression: "1 mm",
        },
        SemanticReferenceRegression {
            name: "linear pattern",
            seed_model: bracket_base_plate,
            edited_model: bracket_hole_row,
            parameter_id: "param:hole_pitch",
            edited_expression: "25 mm",
        },
    ];

    for case in cases {
        assert_top_ref_survives_feature_parameter_edit(case);
    }
}
