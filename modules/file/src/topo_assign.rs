//! Resolve and assign face refs on `.ocad` documents.

use opencad_core::{OpenCadError, Result, TopoRefId};
use opencad_feature::FeatureRegistry;
use opencad_geometry::{
    assign_face_ref_to_refs, match_face_discovery_for_topo_ref, validate_kernel_face_on_mesh,
    GeometryKernel, TessellationSettings, TopoRef,
};
#[cfg(not(feature = "occt"))]
use opencad_geometry::assign_named_face_ref;
use opencad_render::RenderScene;

use crate::OcadDocument;

#[derive(Debug, Clone, PartialEq)]
pub struct AssignFaceRefOp {
    pub ref_id: String,
    pub kernel_face_id: u64,
    pub created_by: String,
    pub role: String,
    pub normal_m: [f32; 3],
}

impl AssignFaceRefOp {
    pub fn new(
        ref_id: impl Into<String>,
        kernel_face_id: u64,
        created_by: impl Into<String>,
        role: impl Into<String>,
        normal_m: [f32; 3],
    ) -> Self {
        Self {
            ref_id: ref_id.into(),
            kernel_face_id,
            created_by: created_by.into(),
            role: role.into(),
            normal_m,
        }
    }
}

/// Apply a face-ref assignment to an in-memory document.
pub fn apply_assign_face_ref(doc: &mut OcadDocument, op: &AssignFaceRefOp) -> Result<()> {
    #[cfg(feature = "occt")]
    {
        use opencad_kernel_occt::OcctGeometryKernel;

        let kernel = OcctGeometryKernel::new();
        let registry = FeatureRegistry::with_defaults();
        let mut model = doc.clone().into_part_model();
        model.regenerate(
            &kernel,
            &registry,
            Some(&doc.parameters),
            Some(&doc.semantic_refs),
        )?;
        let body = model.active_body().ok_or_else(|| {
            OpenCadError::validation("document has no solid body for face assignment")
        })?;
        let mesh_set = kernel.tessellate(body, &TessellationSettings::default())?;
        let effective_face_id = resolve_kernel_face_id(
            &mesh_set,
            op.kernel_face_id,
            &op.role,
            &op.created_by,
            op.normal_m,
            &doc.feature_nodes,
        )?;
        let topo_ref_id = TopoRefId::new(&op.ref_id)?;
        kernel.assign_face_ref(body, effective_face_id, topo_ref_id.clone())?;
        assign_face_ref_to_refs(
            &mut doc.semantic_refs,
            effective_face_id,
            topo_ref_id,
            &op.created_by,
            &op.role,
            op.normal_m,
        )
    }

    #[cfg(not(feature = "occt"))]
    {
        if op.kernel_face_id != 0 {
            return Err(OpenCadError::Other(
                "assign_face_ref with kernel_face_id requires OCCT backend".into(),
            ));
        }
        let topo_ref_id = TopoRefId::new(&op.ref_id)?;
        assign_named_face_ref(
            &mut doc.semantic_refs,
            topo_ref_id,
            &op.created_by,
            &op.role,
            None,
            op.normal_m,
        )
    }
}

fn resolve_kernel_face_id(
    mesh_set: &opencad_geometry::MeshSet,
    kernel_face_id: u64,
    role: &str,
    created_by: &str,
    normal_m: [f32; 3],
    feature_nodes: &[opencad_feature::FeatureNode],
) -> Result<u64> {
    if kernel_face_id != 0 && validate_kernel_face_on_mesh(mesh_set, kernel_face_id).is_ok() {
        return Ok(kernel_face_id);
    }

    let scene = RenderScene::from_mesh_set(mesh_set)?;
    let discoveries = discover_face_refs(&scene, feature_nodes);
    let mut probe = TopoRef::face(
        TopoRefId::new("ref:face:assign_probe").map_err(OpenCadError::validation)?,
        created_by,
        role,
    );
    probe.semantic.normal_hint = Some([
        normal_m[0] as f64,
        normal_m[1] as f64,
        normal_m[2] as f64,
    ]);
    if let Some(matched_id) = match_face_discovery_for_topo_ref(&probe, &discoveries) {
        return Ok(matched_id);
    }

    discoveries
        .iter()
        .find(|discovery| {
            discovery.role == role && discovery.feature_id.as_deref() == Some(created_by)
        })
        .or_else(|| discoveries.iter().find(|discovery| discovery.role == role))
        .map(|discovery| discovery.kernel_face_id)
        .ok_or_else(|| {
            OpenCadError::not_found(format!(
                "no face matching role '{role}' for feature '{created_by}'"
            ))
        })
}

fn discover_face_refs(
    scene: &RenderScene,
    feature_nodes: &[opencad_feature::FeatureNode],
) -> Vec<opencad_geometry::FaceRefDiscovery> {
    scene
        .face_catalog
        .groups
        .iter()
        .filter_map(|group| {
            group.kernel_face_id.map(|kernel_face_id| {
                let inferred = infer_face_refs(feature_nodes, group);
                opencad_geometry::FaceRefDiscovery {
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

fn infer_face_refs(
    feature_nodes: &[opencad_feature::FeatureNode],
    group: &opencad_render::FaceGroup,
) -> (Option<String>, Option<String>) {
    let role = group.role.as_str();
    let feature_id = match role {
        "top" => feature_nodes.iter().find_map(|node| {
            matches!(
                node.definition,
                opencad_feature::FeatureDefinition::Fillet(_)
                    | opencad_feature::FeatureDefinition::Chamfer(_)
            )
            .then(|| node.id.clone())
        }),
        "cylindrical" => feature_nodes.iter().find_map(|node| {
            matches!(
                node.definition,
                opencad_feature::FeatureDefinition::Hole(_)
            )
            .then(|| node.id.clone())
        }),
        _ => feature_nodes.iter().find_map(|node| {
            matches!(
                node.definition,
                opencad_feature::FeatureDefinition::Extrude(_)
            )
            .then(|| node.id.clone())
        }),
    };
    let topo_ref_id = feature_id.as_ref().map(|id| {
        let stem = id.strip_prefix("feature:").unwrap_or(id.as_str());
        format!("ref:face:{stem}_{role}")
    });
    (feature_id, topo_ref_id)
}
