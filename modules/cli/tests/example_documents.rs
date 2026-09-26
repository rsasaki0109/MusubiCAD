//! Committed example documents regenerate with OCCT.

use opencad_feature::FeatureRegistry;
use opencad_file::{read_expanded_dir, validate_expanded_dir};
use opencad_geometry::GeometryKernel;
use opencad_kernel_occt::OcctGeometryKernel;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn example_robot_arm_assembly_regenerates_with_occt() {
    use opencad_assembly::{detect_interferences, regenerate_assembly, ChildPart, ResolvedChild};

    let root = workspace_root().join("examples/robot_arm_assembly.ocad.d");
    validate_expanded_dir(&root).expect("validate");
    let doc = read_expanded_dir(&root).expect("read");
    let assembly = doc.assembly.clone().expect("assembly");

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let assembly_id = opencad_core::DocumentId::new("doc:robot_arm_assembly_001").expect("id");

    let mut loader = |child_path: &std::path::Path| {
        let child = read_expanded_dir(child_path).expect("read child");
        let doc_id = child.metadata.id.clone();
        let parameters = child.parameters.clone();
        let part = child.into_part_model();
        Ok(ResolvedChild::Part(Box::new(ChildPart {
            doc_id,
            parameters,
            part,
            semantic_refs: Vec::new(),
        })))
    };
    let report = regenerate_assembly(
        &assembly,
        &assembly_id,
        &root,
        &kernel,
        &registry,
        &mut loader,
    )
    .expect("regen");

    assert_eq!(report.instance_count, 4);
    assert_eq!(report.successful_instances, 4);
    let mass = report.scene.mass.expect("mass");
    assert!(mass.mass_kg > 0.1, "robot arm mass {} kg", mass.mass_kg);
    let solve = report.mate_solve.expect("mate solve report");
    assert!(
        solve.max_error < 1e-6,
        "mate solve max error {}",
        solve.max_error
    );

    // Stacked links must not interfere; the exact common-volume computation
    // must also not fail on touching or disjoint joint hubs.
    let interferences = detect_interferences(&kernel, &report.scene, 1e-12).expect("interferences");
    assert!(
        interferences.is_empty(),
        "robot arm should have zero interference: {interferences:?}"
    );
}

#[test]
fn example_robot_arm_assembly_preview_renders_png() {
    use opencad_desktop::load_view_data;

    let root = workspace_root().join("examples/robot_arm_assembly.ocad.d");
    let data = load_view_data(root.to_str().expect("path")).expect("view data");
    assert!(
        data.scene.triangle_count() > 0,
        "assembly scene has no triangles"
    );
    assert!(!data.scene.meshes.is_empty());
    let bounds = data.scene.bounds;
    assert!(bounds.max[1] - bounds.min[1] > 0.1);
}

#[test]
fn example_bearing_carrier_regenerates_with_occt() {
    let path = workspace_root().join("examples/bearing_carrier.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let parameters = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let report = model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");

    assert_eq!(report.regenerated.len(), 9);
    assert!(model.outputs.contains_key("feature:bearing_bore"));
    assert!(model.outputs.contains_key("feature:bolt_circle"));
    // The 96 x 72 mm plate keeps its rectangle (MCAD-P7-014); it used to
    // skew toward the 80 x 60 mm sketch coordinates and weigh 0.131 kg.
    assert!(
        (0.15..=0.16).contains(&mass.mass_kg),
        "bearing carrier mass {} kg",
        mass.mass_kg
    );
    let bounds = kernel.bounding_box(body).expect("bounds");
    for (axis, extent) in [(0, 0.096), (1, 0.072)] {
        let size = bounds.max[axis] - bounds.min[axis];
        assert!((size - extent).abs() < 1e-6, "axis {axis}: {size} m");
    }
}

#[test]
fn example_robot_joint_actuator_regenerates_with_occt() {
    let path = workspace_root().join("examples/robot_joint_actuator.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let parameters = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let report = model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");

    assert_eq!(report.regenerated.len(), 22);
    assert!(model.outputs.contains_key("feature:pcd_fasteners"));
    assert!(model.outputs.contains_key("feature:radial_ribs"));
    assert!(model.outputs.contains_key("feature:mounting_holes"));
    assert!((0.60..=0.62).contains(&mass.mass_kg));
}

#[test]
fn example_bracket_face_pin_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_face_pin.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "face pin example should fuse pin onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_edge_fillet_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_edge_fillet.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&params), Some(&semantic_refs))
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 < plate_volume,
        "edge fillet example should round one edge and reduce volume: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_revolve_bushing_regenerates_with_occt() {
    let path = workspace_root().join("examples/revolve_bushing.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let parameters = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let expected = std::f64::consts::PI * (0.025_f64.powi(2) - 0.015_f64.powi(2)) * 0.02;
    assert!(
        (mass.volume_m3 - expected).abs() < 1e-8,
        "revolve bushing example volume {} vs {}",
        mass.volume_m3,
        expected
    );
}

#[test]
fn example_revolve_sector_regenerates_with_occt() {
    let path = workspace_root().join("examples/revolve_sector.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let parameters = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&parameters), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let expected_full = std::f64::consts::PI * (0.025_f64.powi(2) - 0.015_f64.powi(2)) * 0.02;
    let expected = expected_full * 0.5;
    assert!(
        (mass.volume_m3 - expected).abs() < 1e-8,
        "revolve sector example volume {} vs {}",
        mass.volume_m3,
        expected
    );
}

#[test]
fn example_bracket_boss_join_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_boss_join.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "boss join example should fuse boss onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_hole_row_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_hole_row.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 < plate_volume,
        "hole row example should reduce plate volume: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_hole_ring_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_hole_ring.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 < plate_volume,
        "hole ring example should reduce plate volume: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_pin_row_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_pin_row.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "pin row example should fuse bosses onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_pin_ring_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_pin_ring.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "pin ring example should fuse bosses onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn example_bracket_pin_mirror_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket_pin_mirror.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass");
    let plate_volume = 0.08 * 0.06 * 0.006;
    assert!(
        mass.volume_m3 > plate_volume,
        "pin mirror example should fuse mirrored pins onto plate: {} vs {}",
        mass.volume_m3,
        plate_volume
    );
}

#[test]
fn robot_joint_incremental_regeneration_skips_unchanged_upstream() {
    use opencad_feature::RegenerationCache;

    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let mut model = opencad_feature::robot_joint_actuator_housing().expect("model");
    let mut params = opencad_graph::robot_joint_housing_parameters();
    let mut cache = RegenerationCache::with_backend("occt");

    let cold = model
        .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
        .expect("cold regen");
    assert_eq!(cold.regenerated.len(), 22);
    let cold_mass = kernel
        .mass_properties(model.active_body().expect("body"), 2700.0)
        .expect("mass")
        .mass_kg;

    // Unchanged inputs: every node served from cache, zero kernel calls.
    let warm = model
        .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
        .expect("warm regen");
    assert_eq!(warm.regenerated.len(), 0);
    assert_eq!(warm.cached_nodes.len(), 22);
    assert_eq!(warm.trace.geometry_kernel_call_count, 0);
    let warm_mass = kernel
        .mass_properties(model.active_body().expect("body"), 2700.0)
        .expect("mass")
        .mass_kg;
    assert!(
        (warm_mass - cold_mass).abs() < 1e-9,
        "cached regeneration must match cold mass: {warm_mass} vs {cold_mass}"
    );
    assert_eq!(
        cold.trace.output_hashes_sha256, warm.trace.output_hashes_sha256,
        "cached outputs must keep identical content hashes"
    );

    // Editing upper_hub_height re-executes the hub and downstream only.
    params
        .set_expr("param:upper_hub_height", "42 mm")
        .expect("edit hub height");
    let incremental = model
        .regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)
        .expect("incremental regen");
    assert!(
        incremental
            .regenerated
            .iter()
            .any(|id| id == "feature:upper_hub"),
        "upper hub must re-execute"
    );
    assert!(
        incremental
            .cached_nodes
            .iter()
            .any(|id| id == "feature:joint_base"),
        "base plate must be served from cache"
    );
    assert!(
        incremental
            .cached_nodes
            .iter()
            .any(|id| id == "feature:lower_hub"),
        "lower hub must be served from cache"
    );
    assert!(
        !incremental
            .regenerated
            .iter()
            .any(|id| id == "feature:joint_base"),
        "base plate must not re-execute"
    );
    let inc_mass = kernel
        .mass_properties(model.active_body().expect("body"), 2700.0)
        .expect("mass")
        .mass_kg;
    assert!(inc_mass > cold_mass, "longer hub must increase mass");

    // A fresh model: changing bolt_circle_radius re-executes the PCD pattern
    // and downstream while the hubs and shaft stay cached.
    let mut model2 = opencad_feature::robot_joint_actuator_housing().expect("model");
    let mut params2 = opencad_graph::robot_joint_housing_parameters();
    let mut cache2 = RegenerationCache::with_backend("occt");
    model2
        .regenerate_with_cache(&kernel, &registry, Some(&params2), None, &mut cache2)
        .expect("cold regen");
    params2
        .set_expr("param:bolt_circle_radius", "50 mm")
        .expect("edit bolt circle radius");
    let pattern = model2
        .regenerate_with_cache(&kernel, &registry, Some(&params2), None, &mut cache2)
        .expect("incremental regen");
    assert!(
        pattern
            .regenerated
            .iter()
            .any(|id| id == "feature:pcd_fasteners"),
        "PCD pattern must re-execute"
    );
    assert!(
        pattern
            .cached_nodes
            .iter()
            .any(|id| id == "feature:shaft_bore"),
        "shaft bore must be served from cache"
    );
    assert!(
        !pattern
            .regenerated
            .iter()
            .any(|id| id == "feature:shaft_bore"),
        "shaft bore must not re-execute"
    );
}

#[test]
fn robot_joint_actuator_assertions_pass_after_regeneration() {
    use opencad_ai::{evaluate_assertions, required_assertions_pass, AssertionContext};
    use opencad_core::{Assertion, AssertionKind, AssertionSeverity};
    use opencad_feature::robot_joint_actuator_housing;
    use opencad_graph::robot_joint_housing_parameters;

    let mut model = robot_joint_actuator_housing().expect("model");
    let params = robot_joint_housing_parameters();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let report = model
        .regenerate(&kernel, &registry, Some(&params), None)
        .expect("regen");
    let body = model.active_body().expect("body");
    let mass = kernel.mass_properties(body, 2700.0).expect("mass").mass_kg;
    let bounds = kernel.bounding_box(body).expect("bounds");
    let bounding_box_size_m = [
        bounds.max[0] - bounds.min[0],
        bounds.max[1] - bounds.min[1],
        bounds.max[2] - bounds.min[2],
    ];

    // The actuator acceptance: mass range, overall bounds, parameter range, and
    // body count, all unit-explicit and Required (MCAD-P6-004).
    let assertions = vec![
        Assertion::new(
            "assertion:mass",
            "Actuator mass",
            AssertionSeverity::Required,
            AssertionKind::MassRange {
                min_kg: 0.58,
                max_kg: 0.65,
            },
        ),
        Assertion::new(
            "assertion:param",
            "Upper hub height",
            AssertionSeverity::Required,
            AssertionKind::ParameterRange {
                parameter_name: "upper_hub_height".into(),
                min_m: 0.020,
                max_m: 0.050,
            },
        ),
        Assertion::new(
            "assertion:body_count",
            "Body count",
            AssertionSeverity::Required,
            AssertionKind::BodyCount { expected: 1 },
        ),
        Assertion::new(
            "assertion:bbox",
            "Overall bounds",
            AssertionSeverity::Required,
            AssertionKind::BoundingBoxWithin {
                max_m: [0.3, 0.3, 0.2],
            },
        ),
    ];
    let values = opencad_graph::evaluate_param_graph(&params).expect("params");
    let context = AssertionContext {
        parameter_values: values.into_iter().collect(),
        mass_kg: Some(mass),
        bounding_box_size_m: Some(bounding_box_size_m),
        body_count: Some(1),
        reference_provenance: report.reference_provenance.clone(),
        assembly_dof: None,
        interference_count: None,
    };
    let results = evaluate_assertions(&assertions, &context);
    assert!(
        required_assertions_pass(&results),
        "actuator assertions must pass: {results:?}"
    );
    assert_eq!(results.len(), 4);

    // Valid B-Rep that violates a Required assertion must be rejected.
    let violating = Assertion::new(
        "assertion:mass_violating",
        "Impossible mass",
        AssertionSeverity::Required,
        AssertionKind::MassRange {
            min_kg: 0.01,
            max_kg: 0.02,
        },
    );
    let results = evaluate_assertions(&[violating], &context);
    assert!(!required_assertions_pass(&results));
}

#[test]
fn example_bracket_regenerates_with_occt() {
    let path = workspace_root().join("examples/bracket.ocad.d");
    validate_expanded_dir(&path).expect("validate");
    let doc = read_expanded_dir(&path).expect("read");
    let params = doc.parameters.clone();
    let semantic_refs = doc.semantic_refs.clone();
    let mut model = doc.into_part_model();
    let kernel = OcctGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let report = model
        .regenerate(
            &kernel,
            &registry,
            Some(&params),
            if semantic_refs.is_empty() {
                None
            } else {
                Some(&semantic_refs)
            },
        )
        .expect("regen");
    assert!(model.active_body().is_some());

    // MCAD-P6-003: every persisted semantic reference resolves with exact
    // provenance on a stable fixture and is pinned to a current kernel face.
    assert!(
        !report.reference_provenance.is_empty(),
        "bracket regenerated with semantic refs must report reference provenance"
    );
    for provenance in &report.reference_provenance {
        assert!(
            !provenance.status.is_unresolved(),
            "bracket reference '{}' must resolve exactly, got {:?}: {}",
            provenance.ref_id,
            provenance.status,
            provenance.reason
        );
        assert!(
            provenance.resolved_kernel_id.is_some(),
            "bracket reference '{}' must pin a kernel id",
            provenance.ref_id
        );
    }
}
