#![allow(dead_code)]
use crate::support::*;
use opencad_assembly::*;
use opencad_core::*;
use opencad_file::*;
use opencad_geometry::RigidTransform;
use serde_json::json;
use std::path::Path;

pub fn context_fixture(path: &Path, angular: bool) -> (OcadDocument, OcadDocument) {
    let (mut parent, mut child, part) = fixture(path);
    if !angular {
        child.assembly.as_mut().expect("model").joints[0].motion = JointMotion::Prismatic {
            lower_m: -0.06,
            upper_m: 0.06,
            effort_n: 1.,
            velocity_m_s: 1.,
        };
    }
    let frame = if angular {
        RigidTransform {
            translation_m: [0., 0., 0.],
            rotation: [[0., 0., 1.], [0., 1., 0.], [-1., 0., 0.]],
        }
    } else {
        RigidTransform::identity()
    };
    child
        .assembly
        .as_mut()
        .expect("model")
        .connectors
        .push(Connector::new(
            ConnectorId::new("connector:link_tip").expect("id"),
            "tip",
            InstanceId::new("instance:link").expect("id"),
            frame,
        ));
    write_ocad(path.join("parts/mechanism.ocad.d"), &child).expect("child");
    let model = parent.assembly.as_mut().expect("model");
    model.components.push(Component::new(
        ComponentId::new("component:target").expect("id"),
        "parts/mechanism.ocad.d/parts/box.ocad.d",
        part.metadata.id,
    ));
    let mut target = instance("instance:target", "component:target", [0.2, 0., 0.], true);
    let rotation = if angular {
        [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]]
    } else {
        [[1., 0., 0.], [0., 0., -1.], [0., 1., 0.]]
    };
    model.instances[0].placement.transform = RigidTransform {
        translation_m: [0.1, 0., 0.],
        rotation,
    };
    if !angular {
        target.placement.transform.rotation = rotation;
    }
    model.instances.push(target);
    model.connectors.push(Connector::new(
        ConnectorId::new("connector:target").expect("id"),
        "target",
        InstanceId::new("instance:target").expect("id"),
        frame,
    ));
    apply_patch_to_document(
        &mut parent,
        &context_patch(&child, if angular { "180 deg" } else { "25 mm" }, angular),
    )
    .expect("context patch");
    write_ocad(path, &parent).expect("parent");
    (parent, child)
}
pub fn context_patch(child: &OcadDocument, value: &str, angular: bool) -> opencad_ai::DesignPatch {
    let relation = if angular {
        json!({"type":"signed_angle","angle":value,"turns":0})
    } else {
        json!({"type":"signed_distance","distance":value})
    };
    patch(
        json!([{"type":"set_instance_articulation","instance_id":"instance:left","articulation":{"source_doc":child.metadata.id,"source_revision":opencad_ai::design_state_revision(&document_design_state(child)).expect("revision"),"positions":[],"context_constraints":[{"id":"mate:parent_context","joint_id":"joint:hinge","moving":{"instance_path":["instance:left","instance:link"],"source_doc":child.metadata.id,"connector_id":"connector:link_tip"},"target":{"instance_path":["instance:target"],"source_doc":"doc:art_pair","connector_id":"connector:target"},"relation":relation}]}}]),
    )
}
