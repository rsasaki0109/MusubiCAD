//! Regenerate and tessellate part models for viewport preview.

use opencad_core::{OpenCadError, Result};
use opencad_feature::{FeatureRegistry, PartModel};
use opencad_geometry::{FaceDerivation, GeometryKernel, MeshSet, TessellationSettings, TopoRef};

#[cfg(feature = "occt")]
use opencad_kernel_occt::OcctGeometryKernel;

/// Tessellated active body with kernel face derivation history.
#[derive(Debug, Clone, PartialEq)]
pub struct TessellatedBody {
    pub mesh_set: MeshSet,
    pub face_history: Vec<FaceDerivation>,
}

pub fn tessellate_active_body(
    model: &mut PartModel,
    parameters: Option<&opencad_graph::ParamGraph>,
    semantic_refs: Option<&[TopoRef]>,
) -> Result<MeshSet> {
    Ok(tessellate_active_body_detailed(model, parameters, semantic_refs)?.mesh_set)
}

pub fn tessellate_active_body_detailed(
    model: &mut PartModel,
    parameters: Option<&opencad_graph::ParamGraph>,
    semantic_refs: Option<&[TopoRef]>,
) -> Result<TessellatedBody> {
    let registry = FeatureRegistry::with_defaults();

    #[cfg(feature = "occt")]
    {
        let kernel = OcctGeometryKernel::new();
        let report = model.regenerate(&kernel, &registry, parameters, semantic_refs)?;
        let body = model
            .active_body()
            .ok_or_else(|| OpenCadError::validation("document has no solid body to preview"))?;
        let mesh_set = kernel.tessellate(body, &TessellationSettings::default())?;
        let face_history = if report.face_history.is_empty() {
            kernel.face_derivation_history(body)
        } else {
            report.face_history
        };
        Ok(TessellatedBody {
            mesh_set,
            face_history,
        })
    }

    #[cfg(not(feature = "occt"))]
    {
        let _ = (model, parameters, semantic_refs, registry);
        Err(OpenCadError::Other(
            "OCCT backend disabled; rebuild with --features occt".into(),
        ))
    }
}
