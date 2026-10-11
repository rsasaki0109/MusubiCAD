//! File-backed, fail-closed drawing regeneration (MCAD-P10-003, ADR-034).

use std::path::Path;

use opencad_core::{DocumentKind, OpenCadError, Result};
use opencad_drawing::{
    layout_linear_dimension, resolve_linear_dimension, LinearMeasurement, Sheet, ViewMesh,
};
use opencad_feature::{edge_discover::discover_edge_refs_from_body, FeatureRegistry};
use opencad_file::{read_ocad, OcadDocument};
use opencad_geometry::{GeometryKernel, ReferenceProvenance, TessellationSettings};
use opencad_kernel_occt::OcctGeometryKernel;
use serde::{Deserialize, Serialize};

/// Regenerated dimension evidence; authored semantic bindings remain unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingDimensionResult {
    pub sheet_id: String,
    pub view_id: String,
    pub dimension_id: String,
    pub measured_length_m: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_provenance: Option<ReferenceProvenance>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingAngularDimensionResult {
    pub sheet_id: String,
    pub view_id: String,
    pub dimension_id: String,
    pub measured_angle_rad: f64,
    pub reference_provenance: Vec<ReferenceProvenance>,
}

pub struct RegeneratedDrawing {
    /// Transient point-resolved sheets for renderers, never written to .ocad.
    pub sheets: Vec<(Sheet, Vec<ViewMesh>)>,
    pub dimensions: Vec<DrawingDimensionResult>,
    pub angular_dimensions: Vec<DrawingAngularDimensionResult>,
}

/// Validate every sheet/view/dimension, even when exporting only the first sheet.
pub fn regenerate_drawing(path: &str, doc: &OcadDocument) -> Result<RegeneratedDrawing> {
    let drawing = doc
        .drawing
        .as_ref()
        .ok_or_else(|| OpenCadError::validation("document has no drawing model"))?;
    drawing.validate(&doc.metadata.id)?;
    let root = if Path::new(path).extension().and_then(|ext| ext.to_str()) == Some("ocad") {
        Path::new(path).parent().unwrap_or_else(|| Path::new("."))
    } else {
        Path::new(path)
    };
    let mut result = RegeneratedDrawing {
        sheets: Vec::new(),
        dimensions: Vec::new(),
        angular_dimensions: Vec::new(),
    };
    for mut sheet in drawing.clone().sorted_deterministic().sheets {
        let mut meshes = Vec::new();
        for view in &sheet.views {
            let reference_path = root.join(&view.model.source_path);
            let referenced = read_ocad(&reference_path)
                .map_err(|e| e.with_context(format!("drawing view '{}'", view.id)))?;
            if referenced.metadata.id != view.model.source_doc {
                return Err(OpenCadError::validation(format!(
                    "drawing view '{}' expected document '{}' but found '{}'",
                    view.id, view.model.source_doc, referenced.metadata.id
                )));
            }
            let semantic_dimensions = sheet.angular_dimensions.iter().any(|d| {
                d.view_id == view.id
                    && matches!(
                        d.measurement,
                        opencad_drawing::AngularMeasurement::Edges { .. }
                    )
            }) || sheet.dimensions.iter().any(|dimension| {
                dimension.view_id == view.id
                    && matches!(dimension.measurement, LinearMeasurement::Edge { .. })
            });
            let mesh = match referenced.metadata.kind {
                DocumentKind::Part => {
                    let kernel = OcctGeometryKernel::new();
                    let mut model = referenced.clone().into_part_model();
                    let report = model.regenerate(
                        &kernel,
                        &FeatureRegistry::with_defaults(),
                        Some(&referenced.parameters),
                        Some(&referenced.semantic_refs),
                    )?;
                    let body = model.active_body().ok_or_else(|| {
                        OpenCadError::validation(format!(
                            "drawing view '{}' model has no active body",
                            view.id
                        ))
                    })?;
                    let nodes = model.nodes.values().cloned().collect::<Vec<_>>();
                    let discoveries = if semantic_dimensions {
                        discover_edge_refs_from_body(&kernel, body, &nodes)?
                    } else {
                        Vec::new()
                    };
                    for dimension in sheet.dimensions.iter_mut().filter(|d| d.view_id == view.id) {
                        if let LinearMeasurement::Edge { edge_ref } = &dimension.measurement {
                            if let Some(provenance) = report
                                .reference_provenance
                                .iter()
                                .find(|r| r.ref_id == edge_ref.as_str())
                            {
                                if provenance.status.is_unresolved() {
                                    return Err(OpenCadError::validation(format!(
                                        "drawing dimension '{}' edge reference '{}' is {:?}: {}",
                                        dimension.id,
                                        edge_ref,
                                        provenance.status,
                                        provenance.reason
                                    )));
                                }
                            }
                        }
                        let (resolved, provenance) = resolve_linear_dimension(
                            dimension,
                            &referenced.semantic_refs,
                            &discoveries,
                            &kernel,
                            body,
                        )?;
                        layout_linear_dimension(&resolved, view)?;
                        result.dimensions.push(DrawingDimensionResult {
                            sheet_id: sheet.id.as_str().into(),
                            view_id: view.id.as_str().into(),
                            dimension_id: dimension.id.as_str().into(),
                            measured_length_m: resolved.measured_length_m()?,
                            reference_provenance: provenance,
                        });
                        *dimension = resolved;
                    }
                    for dimension in sheet
                        .angular_dimensions
                        .iter_mut()
                        .filter(|d| d.view_id == view.id)
                    {
                        let (resolved, provenance) = opencad_drawing::resolve_angular_dimension(
                            dimension,
                            &referenced.semantic_refs,
                            &discoveries,
                            &kernel,
                            body,
                        )?;
                        opencad_drawing::layout_angular_dimension(&resolved, view)?;
                        result
                            .angular_dimensions
                            .push(DrawingAngularDimensionResult {
                                sheet_id: sheet.id.as_str().into(),
                                view_id: view.id.as_str().into(),
                                dimension_id: dimension.id.as_str().into(),
                                measured_angle_rad: resolved.measured_angle_rad()?,
                                reference_provenance: provenance,
                            });
                        *dimension = resolved;
                    }
                    kernel.tessellate(body, &TessellationSettings::default())?
                }
                DocumentKind::Assembly => {
                    if semantic_dimensions {
                        return Err(OpenCadError::validation(format!("drawing view '{}' semantic edge dimensions require a part model; assembly instance bindings are not supported", view.id)));
                    }
                    for dimension in sheet.dimensions.iter().filter(|d| d.view_id == view.id) {
                        layout_linear_dimension(dimension, view)?;
                        result.dimensions.push(DrawingDimensionResult {
                            sheet_id: sheet.id.as_str().into(),
                            view_id: view.id.as_str().into(),
                            dimension_id: dimension.id.as_str().into(),
                            measured_length_m: dimension.measured_length_m()?,
                            reference_provenance: None,
                        });
                    }
                    for dimension in sheet
                        .angular_dimensions
                        .iter()
                        .filter(|d| d.view_id == view.id)
                    {
                        opencad_drawing::layout_angular_dimension(dimension, view)?;
                        result
                            .angular_dimensions
                            .push(DrawingAngularDimensionResult {
                                sheet_id: sheet.id.as_str().into(),
                                view_id: view.id.as_str().into(),
                                dimension_id: dimension.id.as_str().into(),
                                measured_angle_rad: dimension.measured_angle_rad()?,
                                reference_provenance: vec![],
                            });
                    }
                    crate::export::document_mesh(
                        reference_path.to_str().ok_or_else(|| {
                            OpenCadError::validation("drawing model path must be valid UTF-8")
                        })?,
                        referenced,
                    )?
                }
                DocumentKind::Drawing => {
                    return Err(OpenCadError::validation(format!(
                        "drawing view '{}' must reference a part or assembly",
                        view.id
                    )))
                }
            };
            meshes.push(ViewMesh {
                view_id: view.id.clone(),
                mesh_set: mesh,
            });
        }
        result.sheets.push((sheet, meshes));
    }
    Ok(result)
}
