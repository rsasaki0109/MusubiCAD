//! OCCT integration and Mock analytic evidence for ADR-059/060.
#[path = "support/articulated.rs"]
mod support;
use opencad_assembly::*;
use opencad_file::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel, RigidTransform, TessellationSettings};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::json;
use std::path::Path;
use support::*;
fn check<K: GeometryKernel>(kernel: &K, path: &Path, parent: &OcadDocument) {
    let report = regenerate_assembly(
        parent.assembly.as_ref().expect("model"),
        &parent.metadata.id,
        path,
        kernel,
        &opencad_feature::FeatureRegistry::with_defaults(),
        &mut load,
    )
    .expect("physical scene");
    assert!(report.scene.articulated);
    assert_eq!(report.scene.leaves.len(), 4);
    assert!((report.scene.mass.expect("mass").volume_m3 - 24e-6).abs() < 1e-11);
    for leaf in &report.scene.leaves {
        assert_eq!(leaf.instance_path.len(), 2);
        assert!(!leaf.source_revision.is_empty());
        let mass = kernel.mass_properties(&leaf.body, 1.).expect("mass");
        assert!((mass.volume_m3 - 6e-6).abs() < 1e-11);
        let right = leaf.instance_path[0].as_str() == "instance:right";
        let link = leaf.instance_path[1].as_str() == "instance:link";
        let expected = if right {
            [0.09, 0.205, if link { 0.015 } else { -0.045 }]
        } else if link {
            [-0.01, 0.005, 0.015]
        } else {
            [0.005, 0.01, -0.045]
        };
        for (got, want) in mass.center_of_mass.into_iter().zip(expected) {
            assert!((got - want).abs() < 3e-7, "{got} vs {want}");
        }
        let size = kernel.bounding_box(&leaf.body).expect("bounds").size();
        let expected = if right || link {
            [0.02, 0.01, 0.03]
        } else {
            [0.01, 0.02, 0.03]
        };
        for (got, want) in size.into_iter().zip(expected) {
            assert!((got - want).abs() < 3e-7);
        }
    }
    assert!(
        detect_interferences_with_tolerance(kernel, &report.scene, Default::default())
            .expect("clear separation")
            .is_empty()
    );
    assert_eq!(
        tessellate_assembly_instances(kernel, &report.scene, &TessellationSettings::default())
            .expect("actual compound meshes")
            .len(),
        2
    );
    assert!(sample_motion_interference(
        kernel,
        parent.assembly.as_ref().expect("model"),
        &report.scene,
        3,
        Default::default()
    )
    .is_err());
    let second = regenerate_assembly(
        parent.assembly.as_ref().expect("model"),
        &parent.metadata.id,
        path,
        kernel,
        &opencad_feature::FeatureRegistry::with_defaults(),
        &mut load,
    )
    .expect("repeat");
    assert_eq!(
        second
            .scene
            .leaves
            .iter()
            .map(|l| (&l.instance_path, l.placement))
            .collect::<Vec<_>>(),
        report
            .scene
            .leaves
            .iter()
            .map(|l| (&l.instance_path, l.placement))
            .collect::<Vec<_>>()
    );
}
#[test]
fn independent_copy_analytic_frames_mass_bounds_and_reader_history() {
    let temporary = tempfile::tempdir().expect("temp");
    let path = temporary.path().join("pair.ocad.d");
    let (mut parent, child, _) = fixture(&path);
    let child_before = read_ocad(path.join("parts/mechanism.ocad.d")).expect("child");
    check(&MockGeometryKernel::new(), &path, &parent);
    check(&OcctGeometryKernel::new(), &path, &parent);
    assert_eq!(
        read_ocad(path.join("parts/mechanism.ocad.d")).expect("child"),
        child_before
    );
    let before = parent.clone();
    let mut history = DocumentHistory::default();
    apply_patch_with_history(
        &mut parent,
        &pose_patch(&child, "0 deg"),
        &mut history,
        "Change one copy",
    )
    .expect("edit");
    assert!(opencad_ai::diff_assembly_models(before.assembly.as_ref().expect("model"),parent.assembly.as_ref().expect("model")).changes.iter().any(|c|matches!(c,opencad_graph::SemanticChange::AssemblyInstanceChanged{field,..} if field=="articulation")));
    history.undo(&mut parent).expect("undo");
    assert_eq!(parent, before);
    history.redo(&mut parent).expect("redo");
    let packed = path.join("config.ocad");
    write_ocad_zip(&packed, &parent).expect("zip");
    assert_eq!(read_ocad_zip(&packed).expect("read"), parent);
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum OldKind {
        Part,
        Assembly,
    }
    let guard =
        serde_json::to_value(parent.assembly.as_ref().expect("model").components[0].source_kind)
            .expect("guard");
    assert!(serde_json::from_value::<OldKind>(guard).is_err());
    let mut value = serde_json::to_value(parent.assembly.as_ref().expect("model")).expect("source");
    value["components"][0]["source_kind"] = json!("assembly");
    assert!(serde_json::from_value::<AssemblyModel>(value).is_err());
    apply_patch_to_document(&mut parent,&patch(json!([{"type":"set_instance_articulation","instance_id":"instance:left","articulation":null}]))).expect("clear");
    assert!(!parent.assembly.as_ref().expect("model").has_articulations());
    assert_eq!(
        parent.assembly.as_ref().expect("model").components[0].source_kind,
        ComponentSourceKind::Assembly
    );
}
#[test]
fn prismatic_internal_collision_and_exact_mock_refusal() {
    let temporary = tempfile::tempdir().expect("temp");
    let path = temporary.path().join("pair.ocad.d");
    let (mut parent, mut child, _) = fixture(&path);
    child.assembly.as_mut().expect("model").joints[0].motion = JointMotion::Prismatic {
        lower_m: -0.06,
        upper_m: 0.06,
        effort_n: 1.,
        velocity_m_s: 1.,
    };
    write_ocad(path.join("parts/mechanism.ocad.d"), &child).expect("child update");
    assert!(regenerate_assembly(
        parent.assembly.as_ref().expect("model"),
        &parent.metadata.id,
        &path,
        &OcctGeometryKernel::new(),
        &opencad_feature::FeatureRegistry::with_defaults(),
        &mut load
    )
    .is_err());
    apply_patch_to_document(&mut parent, &pose_patch(&child, "25 mm")).expect("rebind");
    for native in [false, true] {
        fn translated<K: GeometryKernel>(k: &K, p: &OcadDocument, path: &Path) {
            let report = regenerate_assembly(
                p.assembly.as_ref().expect("model"),
                &p.metadata.id,
                path,
                k,
                &opencad_feature::FeatureRegistry::with_defaults(),
                &mut load,
            )
            .expect("prismatic");
            let leaf = report
                .scene
                .leaves
                .iter()
                .find(|l| {
                    l.instance_path
                        .iter()
                        .map(|id| id.as_str())
                        .eq(["instance:left", "instance:link"])
                })
                .expect("link");
            assert!(
                (k.mass_properties(&leaf.body, 1.)
                    .expect("mass")
                    .center_of_mass[2]
                    - 0.040)
                    .abs()
                    < 3e-7
            );
        }
        if native {
            translated(&OcctGeometryKernel::new(), &parent, &path)
        } else {
            translated(&MockGeometryKernel::new(), &parent, &path)
        }
    }
    apply_patch_to_document(&mut parent, &pose_patch(&child, "-45 mm")).expect("colliding pose");
    let kernel = OcctGeometryKernel::new();
    let report = regenerate_assembly(
        parent.assembly.as_ref().expect("model"),
        &parent.metadata.id,
        &path,
        &kernel,
        &opencad_feature::FeatureRegistry::with_defaults(),
        &mut load,
    )
    .expect("scene");
    let pairs = detect_interferences_with_tolerance(&kernel, &report.scene, Default::default())
        .expect("exact native");
    assert_eq!(pairs.len(), 1);
    assert!((pairs[0].common_volume_m3 - 3e-6).abs() < 1e-11);
    assert_eq!(pairs[0].first_path.as_ref().expect("path").len(), 2);
    let mock = MockGeometryKernel::new();
    let report = regenerate_assembly(
        parent.assembly.as_ref().expect("model"),
        &parent.metadata.id,
        &path,
        &mock,
        &opencad_feature::FeatureRegistry::with_defaults(),
        &mut load,
    )
    .expect("mock scene");
    assert!(detect_interferences_with_tolerance(&mock, &report.scene, Default::default()).is_err());
    assert!(mock
        .inertia_about_com(report.scene.compound_body.as_ref().expect("body"), 1.)
        .is_err());
    let moved = mock
        .transform_body(
            report.scene.compound_body.clone().expect("body"),
            RigidTransform::from_translation([1., 0., 0.]),
        )
        .expect("recursive transform");
    assert!((mock.mass_properties(&moved, 1.).expect("mass").volume_m3 - 24e-6).abs() < 1e-11);
}
#[test]
fn source_refusal_duplicate_units_and_dependency_snapshot() {
    let temporary = tempfile::tempdir().expect("temp");
    let path = temporary.path().join("pair.ocad.d");
    let (mut parent, child, _) = fixture(&path);
    let before = parent.clone();
    let mut history = DocumentHistory::default();
    let duplicate = patch(
        json!([{"type":"set_instance_articulation","instance_id":"instance:left","articulation":{"source_doc":child.metadata.id,"source_revision":"revision","positions":[{"joint_id":"joint:hinge","position":"0 deg"},{"joint_id":"joint:hinge","position":"90 deg"}]}}]),
    );
    assert!(apply_patch_with_history(&mut parent, &duplicate, &mut history, "invalid").is_err());
    assert_eq!(parent, before);
    assert!(apply_patch_to_document(&mut parent, &pose_patch(&child, "10")).is_err());
    assert_eq!(parent, before);
    assert!(serde_json::from_value::<opencad_ai::PatchOperation>(
        json!({"type":"set_instance_articulation","instance_id":"instance:left"})
    )
    .is_err());
    let snapshot = snapshot_assembly_sources(&path, &parent).expect("snapshot");
    let mut changed = child.clone();
    changed.metadata.name = "Dependency metadata changed".into();
    write_ocad(path.join("parts/mechanism.ocad.d"), &changed).expect("change");
    assert_ne!(
        snapshot,
        snapshot_assembly_sources(&path, &parent).expect("changed snapshot")
    );
}
#[test]
#[ignore]
fn write_articulated_example() {
    let path = repository().join("examples/articulated_copies.ocad.d");
    assert!(!path.exists(), "example output already exists");
    fixture(&path);
}

#[test]
fn independent_copy_pair_and_parent_child_collision_paths() {
    let temporary = tempfile::tempdir().expect("temp");
    let path = temporary.path().join("pair.ocad.d");
    let (mut parent, _, part) = fixture(&path);
    // Move the ordinary right copy onto the left: only the two anchors overlap.
    parent.assembly.as_mut().expect("model").instances[1].placement =
        Placement::new(RigidTransform::identity());
    let kernel = OcctGeometryKernel::new();
    let regen = |doc: &OcadDocument| {
        regenerate_assembly(
            doc.assembly.as_ref().expect("model"),
            &doc.metadata.id,
            &path,
            &kernel,
            &opencad_feature::FeatureRegistry::with_defaults(),
            &mut load,
        )
        .expect("scene")
    };
    let scene = regen(&parent).scene;
    let pairs =
        detect_interferences_with_tolerance(&kernel, &scene, Default::default()).expect("pairs");
    assert_eq!(pairs.len(), 1);
    assert!((pairs[0].common_volume_m3 - 6e-6).abs() < 1e-11);
    assert_ne!(
        pairs[0].first_path.as_ref().expect("path")[0],
        pairs[0].second_path.as_ref().expect("path")[0]
    );
    // Restore separation, then place a parent-owned box on the articulated link.
    parent.assembly.as_mut().expect("model").instances[1].placement =
        Placement::new(RigidTransform::from_translation([0.1, 0.2, 0.]));
    let model = parent.assembly.as_mut().expect("model");
    model.components.push(Component::new(
        opencad_core::ComponentId::new("component:obstacle").expect("id"),
        "parts/mechanism.ocad.d/parts/box.ocad.d",
        part.metadata.id,
    ));
    model.instances.push(instance(
        "instance:obstacle",
        "component:obstacle",
        [-0.02, 0., 0.],
        true,
    ));
    let scene = regen(&parent).scene;
    let pairs =
        detect_interferences_with_tolerance(&kernel, &scene, Default::default()).expect("pairs");
    assert_eq!(pairs.len(), 1);
    assert!((pairs[0].common_volume_m3 - 3e-6).abs() < 1e-11);
    let mut lengths = vec![
        pairs[0].first_path.as_ref().expect("path").len(),
        pairs[0].second_path.as_ref().expect("path").len(),
    ];
    lengths.sort();
    assert_eq!(lengths, [1, 2]);
}

#[test]
fn invalid_owners_missing_joints_patterns_and_order_refuse_or_agree() {
    let temporary = tempfile::tempdir().expect("temp");
    let path = temporary.path().join("pair.ocad.d");
    let (parent, child, _) = fixture(&path);
    let kernel = MockGeometryKernel::new();
    let regen = |doc: &OcadDocument| {
        regenerate_assembly(
            doc.assembly.as_ref().expect("model"),
            &doc.metadata.id,
            &path,
            &kernel,
            &opencad_feature::FeatureRegistry::with_defaults(),
            &mut load,
        )
    };
    let mut invalid = parent.clone();
    invalid.assembly.as_mut().expect("model").instances[0]
        .articulation
        .as_mut()
        .expect("pose")
        .source_doc = opencad_core::DocumentId::new("doc:wrong_owner").expect("id");
    assert!(regen(&invalid).is_err());
    let mut invalid = parent.clone();
    invalid.assembly.as_mut().expect("model").instances[0]
        .articulation
        .as_mut()
        .expect("pose")
        .positions[0]
        .joint_id = opencad_core::JointId::new("joint:missing").expect("id");
    assert!(regen(&invalid).is_err());
    let mut invalid = parent.clone();
    invalid
        .assembly
        .as_mut()
        .expect("model")
        .patterns
        .push(AssemblyPattern {
            id: opencad_core::PatternId::new("pattern:unsupported").expect("id"),
            component: opencad_core::ComponentId::new("component:mechanism").expect("id"),
            count: 2,
            spacing_m: 1.,
            direction_m: [1., 0., 0.],
            base_placement: Placement::new(RigidTransform::identity()),
            name_prefix: "copy".into(),
        });
    assert!(regen(&invalid).is_err());
    let mut permuted = parent.clone();
    permuted
        .assembly
        .as_mut()
        .expect("model")
        .instances
        .reverse();
    let canonical = regen(&parent).expect("scene");
    let reordered = regen(&permuted).expect("scene");
    let identities = |r: AssemblyRegenReport| {
        r.scene
            .leaves
            .into_iter()
            .map(|l| {
                (
                    l.instance_path,
                    l.source_doc,
                    l.source_revision,
                    l.placement,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(identities(canonical), identities(reordered));
    // Even a requested in-range value cannot hide an illegal authored zero.
    let mut child = child;
    child.assembly.as_mut().expect("model").joints[0].motion = JointMotion::Revolute {
        lower_rad: 0.5,
        upper_rad: 2.,
        effort_n_m: 1.,
        velocity_rad_s: 1.,
    };
    write_ocad(path.join("parts/mechanism.ocad.d"), &child).expect("child");
    let mut invalid = parent;
    apply_patch_to_document(&mut invalid, &pose_patch(&child, "90 deg")).expect("rebind");
    assert!(regen(&invalid).is_err());
}

#[test]
fn mixed_child_source_epochs_refuse_the_whole_scene() {
    let temporary = tempfile::tempdir().expect("temp");
    let path = temporary.path().join("pair.ocad.d");
    let (parent, _, _) = fixture(&path);
    let mut reads = 0;
    let mut mixed = |p: &Path| {
        let mut child = load(p)?;
        if p.ends_with("mechanism.ocad.d") {
            reads += 1;
            if reads == 2 {
                if let ResolvedChild::Versioned { revision, .. } = &mut child {
                    *revision = "changed-child-epoch".into();
                }
            }
        }
        Ok(child)
    };
    assert!(regenerate_assembly(
        parent.assembly.as_ref().expect("model"),
        &parent.metadata.id,
        &path,
        &MockGeometryKernel::new(),
        &opencad_feature::FeatureRegistry::with_defaults(),
        &mut mixed
    )
    .is_err());
    assert_eq!(read_ocad(&path).expect("unchanged"), parent);
}
