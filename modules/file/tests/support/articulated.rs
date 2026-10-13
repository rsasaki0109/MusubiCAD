#![allow(dead_code)] // Shared by file and CLI integration targets.
use opencad_ai::{authoring_patch, design_state_revision, DesignPatch};
use opencad_assembly::*;
use opencad_core::*;
use opencad_file::*;
use opencad_geometry::{RigidTransform, TopoRef};
use serde_json::json;
use std::path::{Path, PathBuf};

pub fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn patch(ops: serde_json::Value) -> DesignPatch {
    serde_json::from_value(json!({"operations":ops})).expect("patch")
}
pub fn instance(id: &str, component: &str, translation: [f64; 3], fixed: bool) -> Instance {
    let mut i = Instance::new(
        InstanceId::new(id).expect("id"),
        ComponentId::new(component).expect("id"),
        Placement::new(RigidTransform::from_translation(translation)),
        id,
    );
    i.fixed = fixed;
    i
}
pub fn assembly(id: &str, model: AssemblyModel) -> OcadDocument {
    let mut source = OcadDocument::new(DocumentMetadata::new_assembly(
        DocumentId::new(id).expect("id"),
        id,
    ));
    source.assembly = Some(model);
    let mut out = OcadDocument::new(source.metadata.clone());
    apply_patch_to_document(
        &mut out,
        &authoring_patch(&document_design_state(&source)).expect("authoring"),
    )
    .expect("author");
    out
}
pub fn fixture(path: &Path) -> (OcadDocument, OcadDocument, OcadDocument) {
    let mut part =
        read_ocad(repository().join("examples/stl_import_plate.ocad.d")).expect("template");
    part.metadata.id = DocumentId::new("doc:art_box").expect("id");
    part.metadata.name = "Analytic 10 x 20 x 30 mm box".into();
    part.assertions.clear();
    part.attachments.clear();
    let vertices = [
        [0., 0., 0.],
        [10., 0., 0.],
        [10., 20., 0.],
        [0., 20., 0.],
        [0., 0., 30.],
        [10., 0., 30.],
        [10., 20., 30.],
        [0., 20., 30.],
    ];
    let faces = [
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
    ];
    let mut stl = String::from("solid analytic_box_mm\n");
    for face in faces {
        stl.push_str("facet normal 0 0 0\nouter loop\n");
        for vertex in face {
            let p = vertices[vertex];
            stl.push_str(&format!("vertex {} {} {}\n", p[0], p[1], p[2]));
        }
        stl.push_str("endloop\nendfacet\n");
    }
    stl.push_str("endsolid analytic_box_mm\n");
    let bytes = stl.into_bytes();
    let digest = sha256_hex(&bytes);
    let source = format!("imports/mesh_{digest}.stl");
    let mut node = serde_json::to_value(&part.feature_nodes[0]).expect("node");
    node["definition"]["source"] = json!(source);
    node["definition"]["sha256"] = json!(digest);
    part.feature_nodes[0] = serde_json::from_value(node).expect("node");
    part.attachments.insert(source, bytes);
    let mut child = AssemblyModel::new();
    child.components.push(Component::new(
        ComponentId::new("component:box").expect("id"),
        "parts/box.ocad.d",
        part.metadata.id.clone(),
    ));
    child.instances = vec![
        instance("instance:anchor", "component:box", [0., 0., -0.06], true),
        instance("instance:link", "component:box", [0., 0., 0.], false),
    ];
    let entity = |id: &str, origin| {
        MateEntity::with_local_frame(
            InstanceId::new(id).expect("id"),
            TopoRef::face(
                TopoRefId::new(format!("ref:face:{id}")).expect("ref"),
                "feature:stl_plate",
                "bottom_z",
            ),
            origin,
            [0., 0., 1.],
        )
    };
    child.mates.push(Mate::new(
        MateId::new("mate:ground").expect("id"),
        MateKind::Ground {
            instance: InstanceId::new("instance:anchor").expect("id"),
        },
    ));
    child.mates.push(Mate::new(
        MateId::new("mate:hinge").expect("id"),
        MateKind::Concentric {
            a: entity("instance:anchor", [0., 0., 0.06]),
            b: entity("instance:link", [0., 0., 0.]),
        },
    ));
    child.joints.push(AssemblyJoint::new(
        JointId::new("joint:hinge").expect("id"),
        MateId::new("mate:hinge").expect("id"),
        JointMotion::Revolute {
            lower_rad: -std::f64::consts::PI,
            upper_rad: std::f64::consts::PI,
            effort_n_m: 1.,
            velocity_rad_s: 1.,
        },
    ));
    let child = assembly("doc:art_mechanism", child);
    let child_path = path.join("parts/mechanism.ocad.d");
    write_expanded_dir(child_path.join("parts/box.ocad.d"), &part).expect("part");
    write_expanded_dir(&child_path, &child).expect("child");
    let mut model = AssemblyModel::new();
    let mut component = Component::new(
        ComponentId::new("component:mechanism").expect("id"),
        "parts/mechanism.ocad.d",
        child.metadata.id.clone(),
    );
    component.source_kind = ComponentSourceKind::Assembly;
    model.components.push(component);
    model.instances = vec![
        instance("instance:left", "component:mechanism", [0., 0., 0.], true),
        instance(
            "instance:right",
            "component:mechanism",
            [0.1, 0.2, 0.],
            true,
        ),
    ];
    model.instances[1].placement.transform.rotation = [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]];
    let mut parent = assembly("doc:art_pair", model);
    apply_patch_to_document(&mut parent, &pose_patch(&child, "90 deg"))
        .expect("articulate through patch");
    write_expanded_dir(path, &parent).expect("parent");
    (parent, child, part)
}
pub fn pose_patch(child: &OcadDocument, position: &str) -> DesignPatch {
    patch(
        json!([{"type":"set_instance_articulation","instance_id":"instance:left","articulation":{"source_doc":child.metadata.id,"source_revision":design_state_revision(&document_design_state(child)).expect("revision"),"positions":[{"joint_id":"joint:hinge","position":position}]}}]),
    )
}
pub fn load(path: &Path) -> Result<ResolvedChild> {
    let doc = read_ocad(path)?;
    let revision = design_state_revision(&document_design_state(&doc))?;
    let child = if let Some(model) = doc.assembly.clone() {
        ResolvedChild::Assembly {
            model: Box::new(model),
            doc_id: doc.metadata.id,
        }
    } else {
        ResolvedChild::Part(Box::new(ChildPart {
            doc_id: doc.metadata.id.clone(),
            parameters: doc.parameters.clone(),
            semantic_refs: doc.semantic_refs.clone(),
            part: doc.into_part_model(),
        }))
    };
    Ok(child.with_revision(revision))
}
