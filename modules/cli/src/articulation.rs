//! Shared source inspection and dependency gate for articulated copies.
use opencad_ai::{design_state_revision, DesignPatch, PatchOperation};
use opencad_core::{OpenCadError, Result};
use opencad_file::{
    document_design_state, read_ocad, snapshot_assembly_sources, AssemblySourceSnapshot,
    OcadDocument,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRequest {
    pub path: String,
}
#[derive(Serialize)]
pub struct SourceInspection {
    pub source_doc: String,
    pub source_revision: String,
    pub independent_joints: Vec<String>,
    pub can_articulate_child: bool,
    pub unsupported_reason: Option<String>,
    pub revision_precondition: opencad_ai::PatchPrecondition,
}
pub fn inspect(request: &SourceRequest) -> Result<SourceInspection> {
    let doc = read_ocad(&request.path)?;
    let model = doc
        .assembly
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("articulation source must be an assembly"))?;
    snapshot_assembly_sources(Path::new(&request.path), &doc)?;
    let admission = (|| -> Result<Vec<String>> {
        model.require_rigid_kinematics()?;
        if !model.patterns.is_empty() || model.instances.len() > 256 {
            return Err(OpenCadError::validation(
                "child patterns/link budget unsupported",
            ));
        }
        let solved = crate::joint_animation::solved_assembly(model)?;
        let tree = opencad_assembly::kinematic_tree(&solved)?;
        tree.pose(&std::collections::BTreeMap::new())?;
        let mut joints = Vec::new();
        for joint in &solved.joints {
            let links: Vec<_> = tree
                .links
                .iter()
                .filter(|l| {
                    l.joint.as_ref().is_some_and(|j| {
                        j.mate.as_ref() == Some(&joint.mate) && j.kind.is_movable()
                    })
                })
                .collect();
            if links.len() == 1 && !tree.is_follower(links[0].instance.as_str()) {
                joints.push(joint.id.as_str().to_owned());
            }
        }
        joints.sort();
        Ok(joints)
    })();
    let (joints, can_articulate_child, unsupported_reason) = match admission {
        Ok(joints) => (joints, true, None),
        Err(error) => (Vec::new(), false, Some(error.to_string())),
    };
    Ok(SourceInspection {
        source_doc: doc.metadata.id.as_str().to_owned(),
        source_revision: design_state_revision(&document_design_state(&doc))?,
        independent_joints: joints,
        can_articulate_child,
        unsupported_reason,
        revision_precondition: opencad_ai::PatchPrecondition::revision_equals(
            &document_design_state(&doc),
        )?,
    })
}
pub fn command(args: Vec<String>) -> Result<()> {
    if args.len() != 1 {
        return Err(OpenCadError::validation(
            "usage: musubicad inspect-articulation-source <assembly>",
        ));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&inspect(&SourceRequest {
            path: args[0].clone()
        })?)?
    );
    Ok(())
}
pub fn requires_verification(doc: &OcadDocument, patch: &DesignPatch) -> bool {
    doc.assembly.as_ref().is_some_and(|m|m.has_articulations()) || patch.operations.iter().any(|op|matches!(op,PatchOperation::SetInstanceArticulation{..}) || matches!(op,PatchOperation::AddInstance{instance} if instance.articulation.is_some()) || matches!(op,PatchOperation::AddComponent{component} if component.source_kind==opencad_assembly::ComponentSourceKind::ArticulatedAssembly))
}
pub fn before_verification(
    path: &str,
    before: &OcadDocument,
    patch: &DesignPatch,
) -> Result<Option<Vec<AssemblySourceSnapshot>>> {
    if !requires_verification(before, patch) {
        return Ok(None);
    }
    let mut candidate = before.clone();
    opencad_file::apply_patch_to_document(&mut candidate, patch)?;
    Ok(Some(snapshot_assembly_sources(
        Path::new(path),
        &candidate,
    )?))
}
pub fn ensure_sources_unchanged(
    path: &str,
    before: &OcadDocument,
    candidate: &OcadDocument,
    snapshot: &[AssemblySourceSnapshot],
) -> Result<()> {
    let fresh = read_ocad(path)?;
    if fresh != *before || snapshot_assembly_sources(Path::new(path), candidate)? != snapshot {
        return Err(OpenCadError::validation(
            "parent/child source changed during articulated verification; nothing was written",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../file/tests/support/articulated.rs"]
mod test_support;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_snapshot_gate_rejects_child_and_parent_changes_before_commit() {
        let temporary = tempfile::tempdir().expect("temp");
        let path = temporary.path().join("pair.ocad.d");
        let (before, mut child, _) = super::test_support::fixture(&path);
        let patch = super::test_support::pose_patch(&child, "0 deg");
        let snapshot = before_verification(path.to_str().expect("path"), &before, &patch)
            .expect("capture")
            .expect("required");
        let mut candidate = before.clone();
        opencad_file::apply_patch_to_document(&mut candidate, &patch).expect("candidate");
        assert!(ensure_sources_unchanged(
            path.to_str().expect("path"),
            &before,
            &candidate,
            &snapshot
        )
        .is_ok());
        child.metadata.name = "Changed while verifying".into();
        opencad_file::write_ocad(path.join("parts/mechanism.ocad.d"), &child).expect("change");
        assert!(ensure_sources_unchanged(
            path.to_str().expect("path"),
            &before,
            &candidate,
            &snapshot
        )
        .is_err());
        assert_eq!(read_ocad(&path).expect("parent unchanged"), before);
        let snapshot = before_verification(path.to_str().expect("path"), &before, &patch)
            .expect("capture")
            .expect("required");
        let mut changed = before.clone();
        changed.metadata.name = "Another parent edit".into();
        opencad_file::write_ocad(&path, &changed).expect("parent change");
        assert!(ensure_sources_unchanged(
            path.to_str().expect("path"),
            &before,
            &candidate,
            &snapshot
        )
        .is_err());
        assert_eq!(read_ocad(&path).expect("other edit retained"), changed);
    }
}
