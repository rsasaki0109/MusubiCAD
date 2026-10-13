//! ADR-061 native/Mock analytic integration and guarded source evidence.
#[path = "support/context.rs"]
mod context_support;
#[path = "support/articulated.rs"]
mod support;
use context_support::*;
use opencad_assembly::*;
use opencad_file::*;
use opencad_geometry::{GeometryKernel, MockGeometryKernel};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::json;
use std::path::Path;
use support::*;
fn scene<K: GeometryKernel>(
    k: &K,
    p: &OcadDocument,
    path: &Path,
) -> opencad_core::Result<AssemblyRegenReport> {
    regenerate_assembly(
        p.assembly.as_ref().expect("model"),
        &p.metadata.id,
        path,
        k,
        &opencad_feature::FeatureRegistry::with_defaults(),
        &mut load,
    )
}
fn analytic<K: GeometryKernel>(k: &K, p: &OcadDocument, path: &Path, angular: bool) {
    let report = scene(k, p, path).expect("context scene");
    assert_eq!(report.scene.context_dof, Some(0));
    assert_eq!(report.scene.leaves.len(), 5);
    assert!((report.scene.mass.expect("mass").volume_m3 - 30e-6).abs() < 1e-11);
    let left = report
        .scene
        .leaves
        .iter()
        .find(|l| {
            l.instance_path
                .iter()
                .map(|i| i.as_str())
                .eq(["instance:left", "instance:link"])
        })
        .expect("left link");
    let mass = k.mass_properties(&left.body, 1.).expect("mass");
    let want = if angular {
        [0.095, -0.01, 0.015]
    } else {
        [0.105, -0.04, 0.01]
    };
    for (a, b) in mass.center_of_mass.into_iter().zip(want) {
        assert!((a - b).abs() < 3e-7, "{a} vs {b}");
    }
    let want = if angular {
        [0.01, 0.02, 0.03]
    } else {
        [0.01, 0.03, 0.02]
    };
    for (a, b) in k
        .bounding_box(&left.body)
        .expect("bounds")
        .size()
        .into_iter()
        .zip(want)
    {
        assert!((a - b).abs() < 3e-7);
    }
    let right = report
        .scene
        .leaves
        .iter()
        .find(|l| {
            l.instance_path
                .iter()
                .map(|i| i.as_str())
                .eq(["instance:right", "instance:link"])
        })
        .expect("right link");
    for (a, b) in k
        .mass_properties(&right.body, 1.)
        .expect("mass")
        .center_of_mass
        .into_iter()
        .zip([0.09, 0.205, 0.015])
    {
        assert!((a - b).abs() < 3e-7);
    }
    let mut reordered = p.clone();
    reordered
        .assembly
        .as_mut()
        .expect("model")
        .instances
        .reverse();
    let repeat = scene(k, &reordered, path).expect("warm reordered evaluation");
    assert_eq!(repeat.scene.context_dof, report.scene.context_dof);
    for (a, b) in repeat.scene.leaves.iter().zip(&report.scene.leaves) {
        assert_eq!(a.instance_path, b.instance_path);
        assert!(opencad_geometry::RigidTransform::points_near(
            a.placement.translation_m,
            b.placement.translation_m,
            1e-9
        ));
        assert!(opencad_geometry::RigidTransform::matrices_near(
            &a.placement.rotation,
            &b.placement.rotation,
            1e-9
        ));
    }
    let context = opencad_ai::assembly_assertion_context(&p.parameters, &report, 0);
    assert_eq!(context.assembly_dof, Some(0));
    assert_eq!(context.body_count, Some(5));
}
#[test]
fn parent_constraint_drives_only_one_copy_with_independent_si_oracles() {
    for angular in [false, true] {
        let temp = tempfile::tempdir().expect("temp");
        let path = temp.path().join("pair.ocad.d");
        let (mut parent, child) = context_fixture(&path, angular);
        let child_before = read_ocad(path.join("parts/mechanism.ocad.d")).expect("child");
        analytic(&MockGeometryKernel::new(), &parent, &path, angular);
        analytic(&OcctGeometryKernel::new(), &parent, &path, angular);
        assert_eq!(
            read_ocad(path.join("parts/mechanism.ocad.d")).expect("child"),
            child_before
        );
        let original = parent.clone();
        let mut history = DocumentHistory::default();
        apply_patch_with_history(
            &mut parent,
            &context_patch(&child, if angular { "90 deg" } else { "0 mm" }, angular),
            &mut history,
            "Edit parent constraint",
        )
        .expect("edit");
        let k = MockGeometryKernel::new();
        let changed = scene(&k, &parent, &path).expect("changed scene");
        let link = changed
            .scene
            .leaves
            .iter()
            .find(|l| {
                l.instance_path
                    .iter()
                    .map(|i| i.as_str())
                    .eq(["instance:left", "instance:link"])
            })
            .expect("link");
        let want = if angular {
            [0.09, 0.005, 0.015]
        } else {
            [0.105, -0.015, 0.01]
        };
        for (a, b) in k
            .mass_properties(&link.body, 1.)
            .expect("mass")
            .center_of_mass
            .into_iter()
            .zip(want)
        {
            assert!((a - b).abs() < 3e-7);
        }
        history.undo(&mut parent).expect("undo");
        assert_eq!(parent, original);
        history.redo(&mut parent).expect("redo");
        let zip = temp.path().join("pair.ocad");
        write_ocad_zip(&zip, &parent).expect("zip");
        assert_eq!(read_ocad_zip(&zip).expect("read"), parent);
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum PriorKind {
            Part,
            Assembly,
            ArticulatedAssembly,
        }
        assert!(serde_json::from_value::<PriorKind>(json!("flexible_assembly")).is_err());
        let mut bad = serde_json::to_value(parent.assembly.as_ref().expect("model")).expect("wire");
        bad["components"][0]["source_kind"] = json!("articulated_assembly");
        assert!(serde_json::from_value::<AssemblyModel>(bad).is_err());
        // One instance's source remains unchanged across all parent-only edits.
        assert_eq!(
            read_ocad(path.join("parts/mechanism.ocad.d")).expect("child"),
            child_before
        );
    }
}
#[test]
fn invalid_scopes_owners_units_conflicts_and_unfixed_outer_refuse() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("pair.ocad.d");
    let (parent, child) = context_fixture(&path, false);
    let k = MockGeometryKernel::new();
    for kind in 0..6 {
        let mut bad = parent.clone();
        let model = bad.assembly.as_mut().expect("model");
        let a = model.instances[0]
            .articulation
            .as_mut()
            .expect("configuration");
        match kind {
            0 => {
                a.context_constraints[0].moving.source_doc =
                    opencad_core::DocumentId::new("doc:wrong").expect("id")
            }
            1 => {
                a.context_constraints[0].target.connector_id =
                    opencad_core::ConnectorId::new("connector:missing").expect("id")
            }
            2 => a.context_constraints.push(a.context_constraints[0].clone()),
            3 => a.positions.push(ChildJointCoordinate {
                joint_id: opencad_core::JointId::new("joint:hinge").expect("id"),
                position: "0 mm".into(),
            }),
            4 => model.instances[2].fixed = false,
            _ => {
                a.context_constraints[0].joint_id =
                    opencad_core::JointId::new("joint:missing").expect("id")
            }
        }
        assert!(scene(&k, &bad, &path).is_err());
    }
    for expr in ["25", "90 deg", "999 mm"] {
        let mut bad = parent.clone();
        let outcome = apply_patch_to_document(&mut bad, &context_patch(&child, expr, false));
        assert!(outcome.is_err() || scene(&k, &bad, &path).is_err());
    }
    let mut singular = parent.clone();
    singular.assembly.as_mut().expect("model").instances[2]
        .placement
        .transform
        .rotation = opencad_geometry::RigidTransform::identity_rotation();
    assert!(scene(&k, &singular, &path).is_err());
    assert_eq!(read_ocad(&path).expect("unchanged"), parent);
}
#[test]
#[ignore]
fn write_parent_context_examples() {
    for (name, angular) in [
        ("parent_context_slider", false),
        ("parent_context_hinge", true),
    ] {
        let path = repository().join(format!("examples/{name}.ocad.d"));
        assert!(!path.exists());
        context_fixture(&path, angular);
    }
}

#[test]
fn context_solved_internal_collision_is_exact_and_scoped() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("pair.ocad.d");
    let (mut parent, child) = context_fixture(&path, false);
    apply_patch_to_document(&mut parent, &context_patch(&child, "-45 mm", false))
        .expect("constraint");
    let native = OcctGeometryKernel::new();
    let report = scene(&native, &parent, &path).expect("scene");
    let pairs = detect_interferences_with_tolerance(&native, &report.scene, Default::default())
        .expect("pairs");
    assert_eq!(pairs.len(), 1);
    assert!((pairs[0].common_volume_m3 - 3e-6).abs() < 1e-11);
    assert_eq!(
        pairs[0].first_path.as_ref().expect("path")[0].as_str(),
        "instance:left"
    );
    assert_eq!(
        pairs[0].second_path.as_ref().expect("path")[0].as_str(),
        "instance:left"
    );
    let mock = MockGeometryKernel::new();
    let report = scene(&mock, &parent, &path).expect("scene");
    assert!(detect_interferences_with_tolerance(&mock, &report.scene, Default::default()).is_err());
}
#[test]
fn context_driver_coupled_follower_has_no_duplicate_coordinate_or_dof() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("pair.ocad.d");
    let (mut parent, mut child) = context_fixture(&path, true);
    let model = child.assembly.as_mut().expect("model");
    model.instances.push(instance(
        "instance:follower",
        "component:box",
        [0.04, 0., 0.],
        false,
    ));
    let mut mate = model.mates[1].clone();
    mate.id = opencad_core::MateId::new("mate:follower").expect("id");
    if let MateKind::Concentric { a, b } = &mut mate.kind {
        a.local_origin_m = [0.04, 0., 0.06];
        b.instance = opencad_core::InstanceId::new("instance:follower").expect("id");
    }
    model.mates.push(mate);
    model.joints.push(AssemblyJoint::new(
        opencad_core::JointId::new("joint:follower").expect("id"),
        opencad_core::MateId::new("mate:follower").expect("id"),
        JointMotion::Revolute {
            lower_rad: -std::f64::consts::PI,
            upper_rad: std::f64::consts::PI,
            effort_n_m: 1.,
            velocity_rad_s: 1.,
        },
    ));
    model.couplings.push(JointCoupling::Gear {
        id: opencad_core::CouplingId::new("coupling:child_gear").expect("id"),
        driver_joint: opencad_core::JointId::new("joint:hinge").expect("id"),
        follower_joint: opencad_core::JointId::new("joint:follower").expect("id"),
        ratio: -1.,
    });
    write_ocad(path.join("parts/mechanism.ocad.d"), &child).expect("child");
    apply_patch_to_document(&mut parent, &context_patch(&child, "180 deg", true)).expect("rebind");
    fn check<K: GeometryKernel>(k: &K, parent: &OcadDocument, path: &Path) {
        let report = scene(k, parent, path).expect("coupled context");
        assert_eq!(report.scene.context_dof, Some(0));
        assert!((report.scene.mass.expect("mass").volume_m3 - 42e-6).abs() < 1e-11);
        let follower = report
            .scene
            .leaves
            .iter()
            .find(|l| {
                l.instance_path
                    .iter()
                    .map(|i| i.as_str())
                    .eq(["instance:left", "instance:follower"])
            })
            .expect("follower");
        for (a, b) in k
            .mass_properties(&follower.body, 1.)
            .expect("mass")
            .center_of_mass
            .into_iter()
            .zip([0.105, 0.05, 0.015])
        {
            assert!((a - b).abs() < 3e-7, "{a} vs {b}");
        }
    }
    check(&MockGeometryKernel::new(), &parent, &path);
    check(&OcctGeometryKernel::new(), &parent, &path);
    let mut unbound = child.clone();
    unbound.assembly.as_mut().expect("model").couplings.clear();
    write_ocad(path.join("parts/mechanism.ocad.d"), &unbound).expect("ungrounded child");
    apply_patch_to_document(&mut parent, &context_patch(&unbound, "180 deg", true))
        .expect("rebind");
    let error =
        scene(&MockGeometryKernel::new(), &parent, &path).expect_err("unconstrained coordinate");
    assert!(
        error.to_string().contains("unconstrained independent"),
        "{error}"
    );
    write_ocad(path.join("parts/mechanism.ocad.d"), &child).expect("restore coupled child");
    apply_patch_to_document(&mut parent, &context_patch(&child, "180 deg", true)).expect("rebind");
    parent.assembly.as_mut().expect("model").instances[0]
        .articulation
        .as_mut()
        .expect("config")
        .positions
        .push(ChildJointCoordinate {
            joint_id: opencad_core::JointId::new("joint:follower").expect("id"),
            position: "0 deg".into(),
        });
    assert!(scene(&MockGeometryKernel::new(), &parent, &path).is_err());
}
