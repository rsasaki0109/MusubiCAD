//! ADR-056: semantic gear/rack displacements, source/history and true native collisions.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_assembly::*;
use opencad_core::*;
use opencad_file::{
    apply_patch_with_history, document_design_state, read_ocad, read_ocad_zip, write_expanded_dir,
    write_ocad_zip, DocumentHistory, OcadDocument,
};
use opencad_geometry::{
    GeometryKernel, MockGeometryKernel, RigidTransform, TopoRef, TriangleSolid,
};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};
fn patch(ops: Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":ops})).expect("patch")
}
fn gear(ratio: f64) -> Value {
    json!({"type":"gear","id":"coupling:gear","driver_joint":"joint:shoulder","follower_joint":"joint:elbow","ratio":ratio})
}
fn arm() -> AssemblyModel {
    robot_arm_assembly_model().expect("arm")
}
fn with_gear() -> AssemblyModel {
    let mut m = arm();
    m.couplings
        .push(serde_json::from_value(gear(-0.5)).expect("gear"));
    m
}
fn model_doc(model: AssemblyModel) -> OcadDocument {
    let mut d = OcadDocument::new(DocumentMetadata::new_assembly(
        DocumentId::new("doc:coupled").expect("id"),
        "Coupled assembly",
    ));
    d.assembly = Some(model);
    d
}
fn box_solid(min: [f64; 3], max: [f64; 3]) -> TriangleSolid {
    let [x, y, z] = min;
    let [a, b, c] = max;
    let vertices = [
        [x, y, z],
        [a, y, z],
        [a, b, z],
        [x, b, z],
        [x, y, c],
        [a, y, c],
        [a, b, c],
        [x, b, c],
    ];
    let triangles = [
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ]
    .map(|t| t.map(|i| vertices[i]));
    TriangleSolid::from_triangles(&triangles).expect("box")
}
#[test]
fn signed_gear_zero_pose_and_chain_order_are_analytic() {
    let model = with_gear();
    let before = model.clone();
    let tree = kinematic_tree(&model).expect("tree");
    let input = BTreeMap::from([("instance:upper_arm".into(), std::f64::consts::FRAC_PI_2)]);
    let positions = tree.resolve_positions(&input).expect("derived");
    assert!((positions["instance:forearm"] + std::f64::consts::FRAC_PI_4).abs() < 1e-12);
    let explicit = BTreeMap::from([
        ("instance:upper_arm".into(), std::f64::consts::FRAC_PI_2),
        ("instance:forearm".into(), -std::f64::consts::FRAC_PI_4),
    ]);
    for ((left_id, left), (right_id, right)) in tree
        .pose(&input)
        .expect("pose")
        .into_iter()
        .zip(tree.pose(&explicit).expect("explicit"))
    {
        assert_eq!(left_id, right_id);
        assert!(RigidTransform::points_near(
            left.translation_m,
            right.translation_m,
            1e-9
        ));
        for (x, y) in left
            .rotation
            .into_iter()
            .flatten()
            .zip(right.rotation.into_iter().flatten())
        {
            assert!((x - y).abs() < 1e-9);
        }
    }
    let zero = tree.pose(&BTreeMap::new()).expect("zero");
    for (id, pose) in zero {
        let expected = model
            .instances
            .iter()
            .find(|i| i.id == id)
            .expect("instance")
            .placement
            .transform;
        assert!(RigidTransform::points_near(
            pose.translation_m,
            expected.translation_m,
            1e-9
        ));
    }
    assert_eq!(model, before);
    let mut chain = model.clone();
    chain.couplings.push(serde_json::from_value(json!({"type":"gear","id":"coupling:wrist","driver_joint":"joint:elbow","follower_joint":"joint:wrist","ratio":0.25})).expect("chain"));
    let first = kinematic_tree(&chain)
        .expect("tree")
        .resolve_positions(&input)
        .expect("chain");
    chain.couplings.reverse();
    let second = kinematic_tree(&chain)
        .expect("tree")
        .resolve_positions(&input)
        .expect("order");
    assert_eq!(
        first.keys().collect::<Vec<_>>(),
        second.keys().collect::<Vec<_>>()
    );
    for (id, value) in &first {
        assert!((value - second[id]).abs() < 1e-12);
    }
    assert!((first["instance:gripper"] + std::f64::consts::PI / 16.).abs() < 1e-12);
}
#[test]
fn continuous_pairs_are_unwrapped_and_finite_followers_duplicate_owners_and_fixed_sources_refuse() {
    let mut model = with_gear();
    for joint in &mut model.joints {
        if matches!(joint.id.as_str(), "joint:shoulder" | "joint:elbow") {
            joint.motion = JointMotion::Continuous;
        }
    }
    let input = BTreeMap::from([("instance:upper_arm".to_owned(), 3. * std::f64::consts::PI)]);
    let tree = kinematic_tree(&model).expect("continuous tree");
    let positions = tree.resolve_positions(&input).expect("unwrapped");
    assert!((positions["instance:forearm"] + 1.5 * std::f64::consts::PI).abs() < 1e-12);
    tree.pose(&input).expect("unwrapped pose");
    let mut limited = with_gear();
    limited
        .joints
        .iter_mut()
        .find(|j| j.id.as_str() == "joint:shoulder")
        .expect("driver")
        .motion = JointMotion::Continuous;
    assert!(ordered_couplings(&limited).is_err());
    let mut duplicate = with_gear();
    duplicate.couplings.push(serde_json::from_value(json!({"type":"gear","id":"coupling:other","driver_joint":"joint:wrist","follower_joint":"joint:elbow","ratio":0.25})).expect("other"));
    assert!(ordered_couplings(&duplicate).is_err());
    let mut fixed = with_gear();
    fixed
        .joints
        .iter_mut()
        .find(|j| j.id.as_str() == "joint:shoulder")
        .expect("driver")
        .motion = JointMotion::Fixed;
    assert!(ordered_couplings(&fixed).is_err());
}
#[test]
fn invalid_semantics_and_coordinate_conflicts_fail_closed() {
    let mut cases = Vec::new();
    let valid = with_gear();
    for ratio in [0., 1e-15, f64::NAN, 1e7, -10.] {
        let mut m = arm();
        m.couplings.push(JointCoupling::Gear {
            id: CouplingId::new("coupling:gear").expect("id"),
            driver_joint: JointId::new("joint:shoulder").expect("id"),
            follower_joint: JointId::new("joint:elbow").expect("id"),
            ratio,
        });
        cases.push(m);
    }
    let mut missing = valid.clone();
    missing.joints.retain(|j| j.id.as_str() != "joint:elbow");
    cases.push(missing);
    let mut suppressed = valid.clone();
    suppressed
        .mates
        .iter_mut()
        .find(|m| m.id.as_str() == "mate:elbow")
        .expect("mate")
        .suppressed = true;
    cases.push(suppressed);
    let mut duplicate = valid.clone();
    duplicate.couplings.push(duplicate.couplings[0].clone());
    cases.push(duplicate);
    let mut cycle = valid.clone();
    cycle.couplings.push(serde_json::from_value(json!({"type":"gear","id":"coupling:cycle","driver_joint":"joint:elbow","follower_joint":"joint:shoulder","ratio":-0.5})).expect("cycle"));
    cases.push(cycle);
    let mut wrong = arm();
    wrong.couplings.push(serde_json::from_value(json!({"type":"rack","id":"coupling:rack","driver_joint":"joint:shoulder","follower_joint":"joint:elbow","metres_per_radian":0.02})).expect("wrong"));
    cases.push(wrong);
    for model in cases {
        assert!(ordered_couplings(&model).is_err());
    }
    let tree = kinematic_tree(&valid).expect("tree");
    assert!(tree
        .pose(&BTreeMap::from([
            ("instance:upper_arm".into(), 1.),
            ("instance:forearm".into(), 1.)
        ]))
        .is_err());
    assert!(tree
        .pose(&BTreeMap::from([("instance:upper_arm".into(), f64::NAN)]))
        .is_err());
}
fn rack_model() -> AssemblyModel {
    let mut model = AssemblyModel::default();
    model.components.push(Component::new(
        ComponentId::new("component:box").expect("id"),
        "parts/box.ocad.d",
        DocumentId::new("doc:box").expect("id"),
    ));
    for (name, placement) in [
        ("root", [0., 0., 0.]),
        ("driver", [0., 0., 0.]),
        ("rack", [0.12, 0., 0.]),
    ] {
        let mut instance = Instance::new(
            InstanceId::new(format!("instance:{name}")).expect("id"),
            ComponentId::new("component:box").expect("id"),
            Placement::new(RigidTransform::from_translation(placement)),
            name,
        );
        instance.fixed = name == "root";
        model.instances.push(instance);
    }
    model.mates.push(Mate::new(
        MateId::new("mate:ground").expect("id"),
        MateKind::Ground {
            instance: InstanceId::new("instance:root").expect("id"),
        },
    ));
    for (name, origin, axis, motion) in [
        (
            "driver",
            [0., 0., 0.],
            [0., 0., 1.],
            JointMotion::Revolute {
                lower_rad: 0.,
                upper_rad: 1.,
                effort_n_m: 1.,
                velocity_rad_s: 1.,
            },
        ),
        (
            "rack",
            [0.12, 0., 0.],
            [1., 0., 0.],
            JointMotion::Prismatic {
                lower_m: -0.08,
                upper_m: 0.,
                effort_n: 1.,
                velocity_m_s: 1.,
            },
        ),
    ] {
        let attachment = |instance: &str, point| {
            MateEntity::with_local_frame(
                InstanceId::new(instance).expect("id"),
                TopoRef::face(
                    TopoRefId::new(format!("ref:face:{name}_axis")).expect("id"),
                    "feature:box",
                    "axis",
                ),
                point,
                axis,
            )
        };
        let mate = MateId::new(format!("mate:{name}")).expect("id");
        model.mates.push(Mate::new(
            mate.clone(),
            MateKind::Concentric {
                a: attachment("instance:root", origin),
                b: attachment(&format!("instance:{name}"), [0.; 3]),
            },
        ));
        model.joints.push(AssemblyJoint::new(
            JointId::new(format!("joint:{name}")).expect("id"),
            mate,
            motion,
        ));
    }
    model.couplings.push(serde_json::from_value(json!({"type":"rack","id":"coupling:rack","driver_joint":"joint:driver","follower_joint":"joint:rack","metres_per_radian":-0.08})).expect("coupling"));
    model
}
#[test]
fn both_kernels_rack_pose_matches_analytic_bounds_centre_and_volume() {
    let model = rack_model();
    let tree = kinematic_tree(&model).expect("tree");
    let input = BTreeMap::from([("instance:driver".into(), 0.5)]);
    let values = tree.resolve_positions(&input).expect("positions");
    assert!((values["instance:rack"] + 0.04).abs() < 1e-12);
    let pose = tree
        .pose(&input)
        .expect("pose")
        .into_iter()
        .find(|(i, _)| i.as_str() == "instance:rack")
        .expect("rack")
        .1;
    assert!((pose.translation_m[0] - 0.08).abs() < 1e-9);
    let driver_pose = tree
        .pose(&input)
        .expect("pose")
        .into_iter()
        .find(|(i, _)| i.as_str() == "instance:driver")
        .expect("driver")
        .1;
    fn check<K: GeometryKernel>(kernel: &K, pose: RigidTransform, driver_pose: RigidTransform) {
        let solid = box_solid([0.; 3], [0.08, 0.06, 0.006]);
        let body = kernel.import_triangle_solid(&solid).expect("body");
        let rotated = kernel
            .transform_body(body.clone(), driver_pose)
            .expect("rotation");
        let angular = kernel.bounding_box(&rotated).expect("bounds");
        let (sin, cos) = 0.5_f64.sin_cos();
        assert!((angular.min[0] + 0.06 * sin).abs() < 1e-6);
        assert!((angular.max[0] - 0.08 * cos).abs() < 1e-6);
        assert!((angular.max[1] - (0.08 * sin + 0.06 * cos)).abs() < 1e-6);
        let properties = kernel.mass_properties(&rotated, 1.).expect("mass");
        assert!((properties.volume_m3 - 2.88e-5).abs() < 1e-12);
        assert!((properties.center_of_mass[0] - (0.04 * cos - 0.03 * sin)).abs() < 1e-8);
        let moved = kernel
            .transform_body(body.clone(), pose)
            .expect("placement");
        let bounds = kernel.bounding_box(&moved).expect("bounds");
        assert!((bounds.min[0] - 0.08).abs() < 1e-6);
        assert!((bounds.max[0] - 0.16).abs() < 1e-6);
        let mass = kernel.mass_properties(&moved, 1.).expect("mass");
        assert!((mass.volume_m3 - 2.88e-5).abs() < 1e-12);
        assert!((mass.center_of_mass[0] - 0.12).abs() < 1e-8);
        assert!(
            (kernel
                .mass_properties(&body, 1.)
                .expect("original")
                .center_of_mass[0]
                - 0.04)
                .abs()
                < 1e-8
        );
    }
    check(&MockGeometryKernel::new(), pose, driver_pose);
    check(&OcctGeometryKernel::new(), pose, driver_pose);
}
fn collision_scene<K: GeometryKernel>(kernel: &K) -> AssemblyScene {
    let data = [
        ("root", [0., 0., -1.], [0.08, 0.06, -0.994]),
        ("driver", [0., -0.03, 0.], [0.08, 0.03, 0.006]),
        ("rack", [0.12, -0.03, 0.], [0.20, 0.03, 0.006]),
    ];
    let instances = data
        .into_iter()
        .map(|(id, min, max)| InstanceRegenResult {
            instance_id: InstanceId::new(format!("instance:{id}")).expect("id"),
            status: InstanceRegenStatus::Ok,
            body: Some(
                kernel
                    .import_triangle_solid(&box_solid(min, max))
                    .expect("body"),
            ),
        })
        .collect();
    AssemblyScene {
        leaves: Vec::new(),
        articulated: false,

        instances,
        compound_body: None,
        bounding_box: None,
        mass: None,
    }
}
#[test]
fn coupled_sibling_collision_is_detected_inside_moving_union_and_mock_refuses_exact_boolean() {
    let model = rack_model();
    let kernel = OcctGeometryKernel::new();
    let report = sample_motion_interference(
        &kernel,
        &model,
        &collision_scene(&kernel),
        3,
        AssemblyInterferenceTolerance::default(),
    )
    .expect("sweep");
    assert_eq!(report.rest_count, 0);
    assert_eq!(report.joints, 1);
    assert_eq!(report.poses_checked, 3);
    assert_eq!(report.max_count, 1);
    assert!(report
        .worst
        .expect("collision")
        .pairs
        .contains(&["instance:driver".into(), "instance:rack".into()]));
    let mock = MockGeometryKernel::new();
    let err = sample_motion_interference(
        &mock,
        &model,
        &collision_scene(&mock),
        3,
        AssemblyInterferenceTolerance::default(),
    )
    .expect_err("unsupported exact Boolean");
    assert!(err.to_string().contains("Mock booleans"), "{err}");
}
#[test]
fn coupling_source_roundtrips_history_authoring_and_removal_are_atomic() {
    let mut doc = model_doc(arm());
    let before = doc.clone();
    let mut history = DocumentHistory::default();
    let p = patch(json!([{"type":"add_joint_coupling","coupling":gear(-0.5)}]));
    apply_patch_with_history(&mut doc, &p, &mut history, "couple").expect("apply");
    let after = doc.clone();
    history.undo(&mut doc).expect("undo");
    assert_eq!(doc, before);
    history.redo(&mut doc).expect("redo");
    assert_eq!(doc, after);
    for ops in [
        json!([{"type":"remove_joint","id":"joint:elbow"}]),
        json!([{"type":"set_joint_coupling","coupling":gear(-10.)}]),
        json!([{"type":"add_joint_coupling","coupling":gear(-0.5)}]),
    ] {
        assert!(apply_patch_with_history(
            &mut doc,
            &patch(ops),
            &mut DocumentHistory::default(),
            "invalid"
        )
        .is_err());
        assert_eq!(doc, after);
    }
    for source in [before, after] {
        let temp = tempfile::tempdir().expect("temp");
        write_expanded_dir(temp.path(), &source).expect("write");
        assert_eq!(read_ocad(temp.path()).expect("read"), source);
        let zip = temp.path().join("model.ocad");
        write_ocad_zip(&zip, &source).expect("zip");
        assert_eq!(read_ocad_zip(&zip).expect("read zip"), source);
        let p = authoring_patch(&document_design_state(&source)).expect("author");
        let mut rebuilt = model_doc(AssemblyModel::default());
        apply_patch_with_history(&mut rebuilt, &p, &mut DocumentHistory::default(), "author")
            .expect("rebuild");
        assert_eq!(rebuilt, source);
    }
}
#[test]
fn legacy_reader_rejects_coupled_wire_and_invalid_guards_refuse() {
    let source = with_gear();
    let wire = serde_json::to_value(&source).expect("wire");
    let follower = wire["joints"]
        .as_array()
        .expect("joints")
        .iter()
        .find(|j| j["id"] == "joint:elbow")
        .expect("follower");
    assert_eq!(follower["type"], "coupled");
    assert!(serde_json::from_value::<AssemblyJoint>(follower.clone()).is_err());
    assert_eq!(
        serde_json::from_value::<AssemblyModel>(wire.clone()).expect("current"),
        source
    );
    let mut orphan = wire.clone();
    orphan["couplings"] = json!([]);
    assert!(serde_json::from_value::<AssemblyModel>(orphan).is_err());
    let mut missing = wire.clone();
    let joints = missing["joints"].as_array_mut().expect("joints");
    let index = joints
        .iter()
        .position(|j| j["id"] == "joint:elbow")
        .expect("index");
    joints[index] = serde_json::to_value(
        source
            .joints
            .iter()
            .find(|j| j.id.as_str() == "joint:elbow")
            .expect("joint"),
    )
    .expect("legacy");
    assert!(serde_json::from_value::<AssemblyModel>(missing).is_err());
    let mut wrong = wire;
    for joint in wrong["joints"].as_array_mut().expect("joints") {
        if joint["type"] == "coupled" {
            joint["coupling_id"] = json!("coupling:missing");
        }
    }
    assert!(serde_json::from_value::<AssemblyModel>(wrong).is_err());
}
#[test]
#[ignore = "generate coupled arm example"]
fn generate_coupled_arm_example() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut doc = read_ocad(root.join("examples/robot_arm_assembly.ocad.d")).expect("arm");
    doc.metadata.id = DocumentId::new("doc:coupled_arm").expect("id");
    doc.metadata.name = "Coupled arm".into();
    let p = patch(json!([{"type":"add_joint_coupling","coupling":gear(-0.5)}]));
    apply_patch_with_history(&mut doc, &p, &mut DocumentHistory::default(), "couple")
        .expect("apply");
    let dest = root.join("examples/coupled_arm.ocad.d");
    write_expanded_dir(&dest, &doc).expect("write");
    fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).expect("mkdir");
        for entry in std::fs::read_dir(from).expect("dir") {
            let entry = entry.expect("entry");
            if entry.file_type().expect("type").is_dir() {
                copy_tree(&entry.path(), &to.join(entry.file_name()));
            } else {
                std::fs::copy(entry.path(), to.join(entry.file_name())).expect("copy");
            }
        }
    }
    copy_tree(
        &root.join("examples/robot_arm_assembly.ocad.d/parts"),
        &dest.join("parts"),
    );
    std::fs::write(
        root.join("examples/agent/add_arm_gear_coupling_patch.json"),
        serde_json::to_string_pretty(&p).expect("json") + "\n",
    )
    .expect("patch");
}
