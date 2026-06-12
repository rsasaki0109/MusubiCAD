//! Document semantic topology reference sync and assignment.

use opencad_core::{OpenCadError, Result};
use opencad_file::{apply_assign_face_ref, read_ocad, write_expanded_dir, AssignFaceRefOp, OcadDocument};
use opencad_geometry::{sync_semantic_refs_with_history, FaceRefDiscovery, TopoRef};
use opencad_render::RenderScene;

use crate::export::tessellate_active_body_detailed;
use crate::scene_query::infer_face_refs;

pub fn discover_face_refs(
    scene: &RenderScene,
    feature_nodes: &[opencad_feature::FeatureNode],
) -> Vec<FaceRefDiscovery> {
    scene
        .face_catalog
        .groups
        .iter()
        .filter_map(|group| {
            group.kernel_face_id.map(|kernel_face_id| {
                let inferred = infer_face_refs(feature_nodes, group);
                FaceRefDiscovery {
                    kernel_face_id,
                    role: group.role.as_str().to_string(),
                    normal_m: group.normal,
                    centroid_m: group.centroid,
                    feature_id: inferred.0,
                }
            })
        })
        .collect()
}

pub fn sync_document_topo_refs(doc: &mut OcadDocument) -> Result<usize> {
    let mut model = doc.clone().into_part_model();
    let tessellated = tessellate_active_body_detailed(
        &mut model,
        Some(&doc.parameters),
        Some(&doc.semantic_refs),
    )?;
    let scene = RenderScene::from_mesh_set(&tessellated.mesh_set)?;
    let discoveries = discover_face_refs(&scene, &doc.feature_nodes);
    if discoveries.is_empty() && !tessellated.mesh_set.has_triangle_face_ids() {
        return Err(OpenCadError::validation(
            "no kernel face ids available; enable OCCT tessellation to sync topo refs",
        ));
    }
    let before = doc.semantic_refs.len();
    doc.semantic_refs = sync_semantic_refs_with_history(
        &doc.semantic_refs,
        &tessellated.face_history,
        &discoveries,
    );
    Ok(doc.semantic_refs.len().saturating_sub(before))
}

pub fn sync_topo_refs_document(path: &str) -> Result<usize> {
    let mut doc = read_ocad(path)?;
    let added = sync_document_topo_refs(&mut doc)?;
    write_expanded_dir(path, &doc)?;
    Ok(added)
}

pub fn assign_face_ref_document(
    path: &str,
    kernel_face_id: u64,
    ref_id: &str,
    created_by: &str,
    role: &str,
    normal_m: [f32; 3],
) -> Result<TopoRef> {
    let mut doc = read_ocad(path)?;
    apply_assign_face_ref(
        &mut doc,
        &AssignFaceRefOp::new(ref_id, kernel_face_id, created_by, role, normal_m),
    )?;
    write_expanded_dir(path, &doc)?;
    doc.semantic_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
        .cloned()
        .ok_or_else(|| OpenCadError::validation("assigned topo ref not found after write"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::write_bracket_fixture_at;
    use tempfile::tempdir;

    #[test]
    fn sync_topo_refs_persists_kernel_faces() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let added = sync_topo_refs_document(path.to_str().expect("path")).expect("sync");
        assert!(added > 0);

        let doc = read_ocad(path.to_str().expect("path")).expect("read");
        assert!(!doc.semantic_refs.is_empty());
        assert!(doc
            .semantic_refs
            .iter()
            .any(|topo_ref| topo_ref.kernel_face_id().is_some()));
    }

    #[test]
    fn assign_face_ref_persists_custom_ref_id() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let assigned = assign_face_ref_document(
            path.to_str().expect("path"),
            0,
            "ref:face:bracket_top",
            "feature:extrude_base",
            "top",
            [0.0, 0.0, 1.0],
        )
        .expect("assign");
        assert_eq!(assigned.ref_id.as_str(), "ref:face:bracket_top");

        let restored = read_ocad(path.to_str().expect("path")).expect("read");
        assert!(restored
            .semantic_refs
            .iter()
            .any(|topo_ref| topo_ref.ref_id.as_str() == "ref:face:bracket_top"));
    }

    #[test]
    fn sync_topo_refs_rebinds_stale_ids_via_fillet_history() {
        use opencad_core::{DocumentId, DocumentMetadata, TopoRefId};
        use opencad_feature::bracket_with_top_fillet;
        use opencad_file::OcadDocument;
        use opencad_geometry::{build_src_to_post_map, sync_semantic_refs_with_history};

        let part = bracket_with_top_fillet().expect("model");
        let metadata = DocumentMetadata::new(
            DocumentId::new("doc:fillet_bracket").expect("id"),
            "Fillet Bracket",
        );
        let doc = OcadDocument::from_part_model(metadata, &part);
        let tessellated = crate::export::tessellate_active_body_detailed(
            &mut doc.clone().into_part_model(),
            Some(&doc.parameters),
            None,
        )
        .expect("tessellate");
        let (post_id, src_id) = tessellated
            .face_history
            .iter()
            .copied()
            .find(|(post, src)| post != src)
            .expect("fillet history pair with changed face id");
        let expected_id = build_src_to_post_map(&tessellated.face_history)
            .get(&src_id)
            .copied()
            .unwrap_or(post_id);

        let refs = sync_semantic_refs_with_history(
            &[TopoRef::kernel_face(
                TopoRefId::new("ref:face:test_rebind").expect("id"),
                "feature:fillet_top",
                "top",
                src_id,
                [0.0, 0.0, 1.0],
            )],
            &tessellated.face_history,
            &[],
        );

        let rebound = refs
            .iter()
            .find(|topo_ref| topo_ref.ref_id.as_str() == "ref:face:test_rebind")
            .expect("rebound ref");
        assert_eq!(rebound.kernel_face_id(), Some(expected_id));
        assert_ne!(rebound.kernel_face_id(), Some(src_id));
    }

    #[test]
    fn sync_topo_refs_rebinds_boolean_era_ids_via_full_chain() {
        use opencad_core::{DocumentId, DocumentMetadata, TopoRefId};
        use opencad_feature::bracket_with_top_fillet;
        use opencad_file::OcadDocument;
        use opencad_geometry::{build_src_to_post_map, sync_semantic_refs_with_history};

        let fillet_part = bracket_with_top_fillet().expect("fillet model");
        let fillet_metadata = DocumentMetadata::new(
            DocumentId::new("doc:fillet_bracket").expect("id"),
            "Fillet Bracket",
        );
        let fillet_doc = OcadDocument::from_part_model(fillet_metadata, &fillet_part);
        let fillet_tessellated = crate::export::tessellate_active_body_detailed(
            &mut fillet_doc.clone().into_part_model(),
            Some(&fillet_doc.parameters),
            None,
        )
        .expect("fillet tessellate");

        let boolean_src = fillet_tessellated
            .face_history
            .iter()
            .copied()
            .find(|(post, src)| post != src)
            .map(|(_, src)| src)
            .expect("boolean history in composed chain");
        let expected_id = build_src_to_post_map(&fillet_tessellated.face_history)
            .get(&boolean_src)
            .copied()
            .expect("boolean-era src should map through fillet chain");

        let refs = sync_semantic_refs_with_history(
            &[TopoRef::kernel_face(
                TopoRefId::new("ref:face:chain_top").expect("id"),
                "feature:extrude_base",
                "top",
                boolean_src,
                [0.0, 0.0, 1.0],
            )],
            &fillet_tessellated.face_history,
            &[],
        );

        let rebound = refs
            .iter()
            .find(|topo_ref| topo_ref.ref_id.as_str() == "ref:face:chain_top")
            .expect("rebound ref");
        assert_eq!(rebound.kernel_face_id(), Some(expected_id));
        assert_ne!(rebound.kernel_face_id(), Some(boolean_src));
    }

    #[test]
    fn sync_topo_refs_keeps_custom_ref_after_param_change() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        assign_face_ref_document(
            path.to_str().expect("path"),
            0,
            "ref:face:bracket_top",
            "feature:extrude_base",
            "top",
            [0.0, 0.0, 1.0],
        )
        .expect("assign");

        let mut doc = read_ocad(path.to_str().expect("path")).expect("read");
        doc.parameters
            .set_expr("param:width", "100 mm")
            .expect("set width");
        write_expanded_dir(&path, &doc).expect("write param");

        sync_topo_refs_document(path.to_str().expect("path")).expect("sync");

        let data = crate::mesh::load_view_data(path.to_str().expect("path")).expect("view");
        let top = crate::scene_query::list_face_group_infos(
            &data.scene,
            &data.feature_nodes,
            &data.semantic_refs,
            &data.face_history,
        )
        .into_iter()
        .find(|item| item.face_role == "top")
        .expect("top face group");
        assert_eq!(
            top.inferred_topo_ref_id.as_deref(),
            Some("ref:face:bracket_top")
        );
    }
}
