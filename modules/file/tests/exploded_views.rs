//! ADR-055: presentation offsets never change mechanical geometry.
use opencad_ai::{authoring_patch, DesignPatch};
use opencad_assembly::{
    exploded_meshes, AssemblyModel, Component, ExplodedView, Instance, InstanceMesh, Placement,
};
use opencad_core::{ComponentId, DocumentId, DocumentMetadata, InstanceId};
use opencad_file::{
    apply_patch_with_history, document_design_state, read_ocad, read_ocad_zip, write_expanded_dir,
    write_ocad_zip, DocumentHistory, OcadDocument,
};
use opencad_geometry::{GeometryKernel, MockGeometryKernel, TessellationSettings};
use opencad_kernel_occt::OcctGeometryKernel;
use serde_json::{json, Value};
use std::path::PathBuf;
fn patch(ops: Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":ops})).expect("patch")
}
fn view() -> Value {
    json!({"id":"view:exploded","name":"Exploded","offsets":[{"instance_id":"instance:child","translation_m":[0.1,-0.2,0.3]}]})
}
fn model() -> AssemblyModel {
    AssemblyModel {
        components: vec![Component::new(
            ComponentId::new("component:part").expect("id"),
            "parts/plate.ocad.d",
            DocumentId::new("doc:plate").expect("id"),
        )],
        instances: vec![Instance::new(
            InstanceId::new("instance:child").expect("id"),
            ComponentId::new("component:part").expect("id"),
            Placement::identity(),
            "Child",
        )],
        ..Default::default()
    }
}
fn document() -> OcadDocument {
    let mut doc = OcadDocument::new(DocumentMetadata::new_assembly(
        DocumentId::new("doc:exploded").expect("id"),
        "Exploded assembly",
    ));
    doc.assembly = Some(model());
    doc
}
fn bounds(mesh: &opencad_geometry::MeshSet) -> [[f64; 3]; 2] {
    let mut out = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for p in &mesh.positions {
        for a in 0..3 {
            out[0][a] = out[0][a].min(f64::from(p[a]));
            out[1][a] = out[1][a].max(f64::from(p[a]));
        }
    }
    out
}
fn volume(mesh: &opencad_geometry::MeshSet) -> f64 {
    mesh.indices
        .chunks_exact(3)
        .map(|t| {
            let p = mesh.positions[t[0] as usize].map(f64::from);
            let q = mesh.positions[t[1] as usize].map(f64::from);
            let r = mesh.positions[t[2] as usize].map(f64::from);
            (p[0] * (q[1] * r[2] - q[2] * r[1]) - p[1] * (q[0] * r[2] - q[2] * r[0])
                + p[2] * (q[0] * r[1] - q[1] * r[0]))
                / 6.
        })
        .sum::<f64>()
        .abs()
}
#[test]
fn both_kernels_translate_analytic_bounds_without_changing_volume_or_source() {
    fn check<K: GeometryKernel>(kernel: &K) {
        let vertices = [
            [0., 0., 0.],
            [0.08, 0., 0.],
            [0.08, 0.06, 0.],
            [0., 0.06, 0.],
            [0., 0., 0.006],
            [0.08, 0., 0.006],
            [0.08, 0.06, 0.006],
            [0., 0.06, 0.006],
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
        let solid =
            opencad_geometry::TriangleSolid::from_triangles(&triangles).expect("closed box");
        let body = kernel.import_triangle_solid(&solid).expect("body");
        let mesh = kernel
            .tessellate(&body, &TessellationSettings::default())
            .expect("mesh");
        let mut model = model();
        model
            .exploded_views
            .push(serde_json::from_value(view()).expect("view"));
        let input = vec![InstanceMesh {
            instance_id: model.instances[0].id.clone(),
            mesh_set: mesh,
        }];
        let before = input.clone();
        let mechanical = model.clone();
        assert!(exploded_meshes(&model, &[], "view:exploded").is_err());
        assert!(exploded_meshes(
            &model,
            &[input[0].clone(), input[0].clone()],
            "view:exploded"
        )
        .is_err());
        let mut invalid = input.clone();
        invalid[0].mesh_set.positions[0][0] = f32::NAN;
        assert!(exploded_meshes(&model, &invalid, "view:exploded").is_err());
        let output = exploded_meshes(&model, &input, "view:exploded").expect("presentation");
        let b = bounds(&output[0].mesh_set);
        for (actual, expected) in b
            .into_iter()
            .flatten()
            .zip([0.1, -0.2, 0.3, 0.18, -0.14, 0.306])
        {
            assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
        }
        assert!((volume(&output[0].mesh_set) - 0.08 * 0.06 * 0.006).abs() < 1e-10);
        assert!(
            (kernel.mass_properties(&body, 1.).expect("mass").volume_m3 - 0.08 * 0.06 * 0.006)
                .abs()
                < 1e-12
        );
        assert_eq!(input, before);
        assert_eq!(model, mechanical);
        assert_eq!(output[0].mesh_set.indices, input[0].mesh_set.indices);
        assert_eq!(output[0].mesh_set.normals, input[0].mesh_set.normals);
        assert_eq!(
            output,
            exploded_meshes(&model, &input, "view:exploded").expect("repeat")
        );
    }
    check(&MockGeometryKernel::new());
    check(&OcctGeometryKernel::new());
}
#[test]
fn presentation_history_roundtrips_legacy_and_new_source() {
    let mut doc = document();
    let legacy = doc.clone();
    let mut history = DocumentHistory::default();
    let p = patch(json!([{"type":"add_exploded_view","view":view()}]));
    apply_patch_with_history(&mut doc, &p, &mut history, "Explode").expect("apply");
    assert_eq!(
        doc.assembly.as_ref().expect("assembly").instances,
        legacy.assembly.as_ref().expect("legacy").instances
    );
    let after = doc.clone();
    history.undo(&mut doc).expect("undo");
    assert_eq!(doc, legacy);
    history.redo(&mut doc).expect("redo");
    assert_eq!(doc, after);
    for source in [legacy, after] {
        let dir = tempfile::tempdir().expect("temp");
        write_expanded_dir(dir.path(), &source).expect("write");
        assert_eq!(read_ocad(dir.path()).expect("read"), source);
        let path = dir.path().join("packed.ocad");
        write_ocad_zip(&path, &source).expect("zip");
        assert_eq!(read_ocad_zip(&path).expect("read zip"), source);
        let p = authoring_patch(&document_design_state(&source)).expect("author");
        let mut rebuilt = OcadDocument::new(source.metadata.clone());
        rebuilt.assembly = Some(AssemblyModel::default());
        apply_patch_with_history(&mut rebuilt, &p, &mut DocumentHistory::default(), "rebuild")
            .expect("rebuild");
        assert_eq!(rebuilt, source);
    }
}
#[test]
fn invalid_views_and_reference_removal_fail_atomically() {
    let mut doc = document();
    apply_patch_with_history(
        &mut doc,
        &patch(json!([{"type":"add_exploded_view","view":view()}])),
        &mut DocumentHistory::default(),
        "add",
    )
    .expect("add");
    let before = doc.clone();
    let mut duplicate = view();
    let first = duplicate["offsets"][0].clone();
    duplicate["offsets"]
        .as_array_mut()
        .expect("offsets")
        .push(first);
    let mut nested = view();
    nested["offsets"][0]["instance_id"] = json!("instance:child/leaf");
    let mut huge = view();
    huge["offsets"][0]["translation_m"] = json!([1e7, 0., 0.]);
    for ops in [
        json!([{"type":"set_exploded_view","view":duplicate}]),
        json!([{"type":"set_exploded_view","view":nested}]),
        json!([{"type":"set_exploded_view","view":huge}]),
        json!([{"type":"remove_instance","id":"instance:child"}]),
        json!([{"type":"add_exploded_view","view":view()}]),
        json!([{"type":"remove_exploded_view","id":"view:missing"}]),
    ] {
        assert!(apply_patch_with_history(
            &mut doc,
            &patch(ops),
            &mut DocumentHistory::default(),
            "invalid"
        )
        .is_err());
        assert_eq!(doc, before);
    }
    let mut nonfinite: ExplodedView = serde_json::from_value(view()).expect("view");
    nonfinite.offsets[0].translation_m[0] = f64::NAN;
    let mut bad = model();
    bad.exploded_views.push(nonfinite);
    assert!(opencad_assembly::validate_exploded_views(&bad).is_err());
    apply_patch_with_history(&mut doc,&patch(json!([{"type":"remove_exploded_view","id":"view:exploded"},{"type":"remove_instance","id":"instance:child"}])),&mut DocumentHistory::default(),"remove consumers").expect("remove");
}
#[test]
#[ignore = "generate exploded assembly example"]
fn generate_exploded_example() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/robot_arm_assembly.ocad.d");
    let mut doc = read_ocad(&path).expect("robot");
    doc.assembly
        .as_mut()
        .expect("assembly")
        .exploded_views
        .clear();
    let view = json!({"id":"view:exploded","name":"Arm exploded","offsets":[{"instance_id":"instance:upper_arm","translation_m":[0.1,0.,0.]},{"instance_id":"instance:forearm","translation_m":[0.2,0.,0.]},{"instance_id":"instance:gripper","translation_m":[0.3,0.,0.]}]});
    let p = patch(json!([{"type":"add_exploded_view","view":view}]));
    apply_patch_with_history(&mut doc, &p, &mut DocumentHistory::default(), "exploded")
        .expect("apply");
    doc.assembly = doc.assembly.take().map(AssemblyModel::sorted_deterministic);
    write_expanded_dir(&path, &doc).expect("write");
    std::fs::write(
        root.join("examples/agent/add_arm_exploded_view_patch.json"),
        serde_json::to_string_pretty(&p).expect("json") + "\n",
    )
    .expect("patch");
}
