//! Model-driven drawing dimensions (Task-179).

use opencad_core::{DimensionId, OpenCadError, Result, TopoRefId, ViewId};
use opencad_geometry::{
    resolve_edge_ref_with_provenance, EdgeRefDiscovery, GeometryKernel, KernelBody,
    ReferenceProvenance, TopoRef, TopoRefTolerancePolicy,
};
use serde::{Deserialize, Serialize};

use crate::{DrawingView, ProjectionKind};

/// Authored measurement. Legacy points retain their original wire format;
/// semantic measurements serialize only an edge reference, never cached points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum LinearMeasurement {
    Edge {
        edge_ref: TopoRefId,
    },
    Points {
        start_model_m: [f64; 3],
        end_model_m: [f64; 3],
    },
}

/// Absolute tolerance for degenerate linear measurements, in model meters.
pub const DIMENSION_LENGTH_TOLERANCE_M: f64 = 1e-9;

/// An aligned linear dimension in the drawing Design Graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearDimension {
    /// Stable dimension identifier.
    pub id: DimensionId,
    /// Drawing view providing projection and sheet placement.
    pub view_id: ViewId,
    #[serde(flatten)]
    pub measurement: LinearMeasurement,
    /// Perpendicular offset of the dimension line from the measured edge, in meters on the sheet.
    pub offset_m: f64,
}

impl LinearDimension {
    /// Validate finite coordinates and a non-degenerate measured length.
    pub fn validate(&self) -> Result<()> {
        if !self.offset_m.is_finite() {
            return Err(OpenCadError::validation(format!(
                "drawing dimension '{}' values must be finite",
                self.id
            )));
        }
        let LinearMeasurement::Points {
            start_model_m,
            end_model_m,
        } = &self.measurement
        else {
            if let LinearMeasurement::Edge { edge_ref } = &self.measurement {
                if !edge_ref.as_str().starts_with("ref:edge:") {
                    return Err(OpenCadError::validation(format!(
                        "drawing dimension '{}' requires an edge reference, got '{}'",
                        self.id, edge_ref
                    )));
                }
            }
            return Ok(());
        };
        if !start_model_m
            .iter()
            .chain(end_model_m.iter())
            .all(|v| v.is_finite())
        {
            return Err(OpenCadError::validation(format!(
                "drawing dimension '{}' points must be finite",
                self.id
            )));
        }
        let length = self.measured_length_m()?;
        if !length.is_finite() || length <= DIMENSION_LENGTH_TOLERANCE_M {
            return Err(OpenCadError::validation(format!(
                "drawing dimension '{}' measured length must be finite and positive",
                self.id
            )));
        }
        Ok(())
    }

    /// Derived model-space distance in meters.
    pub fn measured_length_m(&self) -> Result<f64> {
        let LinearMeasurement::Points {
            start_model_m,
            end_model_m,
        } = &self.measurement
        else {
            return Err(OpenCadError::validation(format!(
                "drawing dimension '{}' requires regenerated semantic edge endpoints",
                self.id
            )));
        };
        Ok(start_model_m
            .iter()
            .zip(end_model_m.iter())
            .map(|(start, end)| (end - start).powi(2))
            .sum::<f64>()
            .sqrt())
    }
}

/// Resolve one semantic measurement against the referenced model's final body.
/// The returned points are transient; callers must retain the authored dimension
/// in the document. Ambiguity is never broken by kernel enumeration order.
pub fn resolve_linear_dimension(
    dimension: &LinearDimension,
    semantic_refs: &[TopoRef],
    discoveries: &[EdgeRefDiscovery],
    kernel: &impl GeometryKernel,
    body: &KernelBody,
) -> Result<(LinearDimension, Option<ReferenceProvenance>)> {
    dimension.validate()?;
    let LinearMeasurement::Edge { edge_ref } = &dimension.measurement else {
        return Ok((dimension.clone(), None));
    };
    let context = format!(
        "drawing dimension '{}' edge reference '{}'",
        dimension.id, edge_ref
    );
    let matching: Vec<_> = semantic_refs
        .iter()
        .filter(|r| r.ref_id == *edge_ref)
        .collect();
    if matching.len() > 1
        || matching
            .first()
            .is_some_and(|r| r.kind != opencad_geometry::TopoRefKind::Edge)
    {
        return Err(OpenCadError::validation(format!(
            "{context} is duplicated or is not an edge"
        )));
    }
    let resolution = resolve_edge_ref_with_provenance(
        semantic_refs,
        edge_ref.as_str(),
        Some(discoveries),
        TopoRefTolerancePolicy::default(),
        true,
    )
    .map_err(|error| error.with_context(&context))?;
    let id = resolution
        .resolved_kernel_id
        .ok_or_else(|| OpenCadError::validation(format!("{context} did not resolve")))?;
    let mut endpoints = kernel
        .edge_endpoints(body, id)
        .map_err(|error| error.with_context(&context))?;
    // Canonical witness ordering is independent of OCCT's edge orientation.
    endpoints.sort_by(|a, b| {
        a.iter()
            .zip(b)
            .map(|(a, b)| a.total_cmp(b))
            .find(|order| !order.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut resolved = dimension.clone();
    resolved.measurement = LinearMeasurement::Points {
        start_model_m: endpoints[0],
        end_model_m: endpoints[1],
    };
    resolved
        .validate()
        .map_err(|error| error.with_context(&context))?;
    Ok((resolved, Some(resolution.provenance)))
}

/// Sheet-space geometry and derived label for an aligned dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionLayout {
    /// Projected first measurement point in sheet meters.
    pub witness_start_m: [f64; 2],
    /// Projected second measurement point in sheet meters.
    pub witness_end_m: [f64; 2],
    /// First endpoint of the offset dimension line in sheet meters.
    pub line_start_m: [f64; 2],
    /// Second endpoint of the offset dimension line in sheet meters.
    pub line_end_m: [f64; 2],
    /// Text anchor at the dimension-line midpoint in sheet meters.
    pub text_position_m: [f64; 2],
    /// Derived millimeter label.
    pub label: String,
}

/// Project and place one model-driven linear dimension on its drawing view.
pub fn layout_linear_dimension(
    dimension: &LinearDimension,
    view: &DrawingView,
) -> Result<DimensionLayout> {
    dimension.validate()?;
    if !view.scale.is_finite()
        || view.scale <= 0.0
        || !view.origin_on_sheet_m.iter().all(|v| v.is_finite())
    {
        return Err(OpenCadError::validation(format!(
            "dimension '{}' requires finite view placement and a positive finite scale",
            dimension.id
        )));
    }
    if dimension.view_id != view.id {
        return Err(OpenCadError::validation(format!(
            "dimension '{}' references view '{}', not '{}'",
            dimension.id, dimension.view_id, view.id
        )));
    }
    let LinearMeasurement::Points {
        start_model_m,
        end_model_m,
    } = dimension.measurement
    else {
        return Err(OpenCadError::validation(format!(
            "drawing dimension '{}' requires regenerated semantic edge endpoints",
            dimension.id
        )));
    };
    let start = place_model_point(start_model_m, view.projection, view);
    let end = place_model_point(end_model_m, view.projection, view);
    let delta = [end[0] - start[0], end[1] - start[1]];
    let projected_length = delta[0].hypot(delta[1]);
    if projected_length <= DIMENSION_LENGTH_TOLERANCE_M * view.scale {
        return Err(OpenCadError::validation(format!(
            "dimension '{}' endpoints overlap in view '{}'",
            dimension.id, view.id
        )));
    }
    let normal = [-delta[1] / projected_length, delta[0] / projected_length];
    let offset = [
        normal[0] * dimension.offset_m,
        normal[1] * dimension.offset_m,
    ];
    let line_start = [start[0] + offset[0], start[1] + offset[1]];
    let line_end = [end[0] + offset[0], end[1] + offset[1]];
    Ok(DimensionLayout {
        witness_start_m: start,
        witness_end_m: end,
        line_start_m: line_start,
        line_end_m: line_end,
        text_position_m: [
            (line_start[0] + line_end[0]) * 0.5,
            (line_start[1] + line_end[1]) * 0.5,
        ],
        label: format!("{:.2} mm", dimension.measured_length_m()? * 1000.0),
    })
}

fn place_model_point(
    point_m: [f64; 3],
    projection: ProjectionKind,
    view: &DrawingView,
) -> [f64; 2] {
    let projected = projection.project_point(point_m);
    [
        view.origin_on_sheet_m[0] + projected[0] * view.scale,
        view.origin_on_sheet_m[1] + projected[1] * view.scale,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelReference;
    use opencad_core::DocumentId;

    fn semantic_dimension() -> LinearDimension {
        LinearDimension {
            id: DimensionId::new("dim:width").expect("id"),
            view_id: ViewId::new("view:front").expect("id"),
            measurement: LinearMeasurement::Edge {
                edge_ref: TopoRefId::new("ref:edge:width").expect("ref"),
            },
            offset_m: -0.01,
        }
    }

    #[test]
    fn semantic_dimensions_resolve_mock_endpoints_without_persisting_them() {
        let kernel = opencad_geometry::MockGeometryKernel::new();
        let body = KernelBody::new(80);
        let discoveries = kernel.discover_body_edges(&body).expect("edges");
        let references = vec![TopoRef::edge(
            TopoRefId::new("ref:edge:width").expect("ref"),
            "feature:base",
            "top@+y",
        )];
        let dimension = semantic_dimension();
        let before = serde_json::to_value(&dimension).expect("json");
        assert!(before.get("start_model_m").is_none());
        assert!(dimension.measured_length_m().is_err());
        let (resolved, provenance) =
            resolve_linear_dimension(&dimension, &references, &discoveries, &kernel, &body)
                .expect("resolve");
        assert!((resolved.measured_length_m().expect("length") - 0.08).abs() < 1e-12);
        assert_eq!(
            provenance.expect("provenance").status,
            opencad_geometry::ReferenceStatus::Fingerprint
        );
        assert_eq!(serde_json::to_value(&dimension).expect("json"), before);
        assert_eq!(
            resolved.measurement,
            LinearMeasurement::Points {
                start_model_m: [0.0, 0.08, 0.08],
                end_model_m: [0.08, 0.08, 0.08]
            }
        );
    }

    #[test]
    fn missing_ambiguous_and_stale_edge_references_fail_closed() {
        let kernel = opencad_geometry::MockGeometryKernel::new();
        let body = KernelBody::new(80);
        let discoveries = kernel.discover_body_edges(&body).expect("edges");
        let dimension = semantic_dimension();
        let references = vec![TopoRef::edge(
            TopoRefId::new("ref:edge:width").expect("ref"),
            "feature:base",
            "top@+y",
        )];
        let missing = resolve_linear_dimension(&dimension, &[], &discoveries, &kernel, &body)
            .expect_err("missing")
            .to_string();
        assert!(
            missing.contains("dim:width")
                && missing.contains("ref:edge:width")
                && missing.contains("Missing"),
            "{missing}"
        );
        let mut tied = discoveries.clone();
        let mut duplicate = discoveries[2].clone();
        duplicate.kernel_edge_id = 99;
        tied.push(duplicate);
        for candidates in [tied.clone(), tied.into_iter().rev().collect()] {
            let message =
                resolve_linear_dimension(&dimension, &references, &candidates, &kernel, &body)
                    .expect_err("ambiguous")
                    .to_string();
            assert!(
                message.contains("Ambiguous") && message.contains("dim:width"),
                "{message}"
            );
        }
        let stale = vec![TopoRef::kernel_edge(
            TopoRefId::new("ref:edge:width").expect("ref"),
            "feature:base",
            "top@+y",
            4,
            [0.04, 0.08, 0.08],
            [1.0, 0.0, 0.0],
        )];
        let (_, provenance) =
            resolve_linear_dimension(&dimension, &stale, &discoveries, &kernel, &body)
                .expect("stale id falls back to unique geometry");
        assert_eq!(provenance.expect("provenance").resolved_kernel_id, Some(3));
        assert!(resolve_linear_dimension(&dimension, &stale, &[], &kernel, &body).is_err());
    }

    #[test]
    fn wire_migration_retains_legacy_points_and_rejects_mixed_bindings() {
        let legacy = r#"{"id":"dim:width","view_id":"view:front","start_model_m":[0,0,0],"end_model_m":[0.08,0,0],"offset_m":-0.01}"#;
        let dimension: LinearDimension = serde_json::from_str(legacy).expect("legacy");
        assert!(matches!(
            dimension.measurement,
            LinearMeasurement::Points { .. }
        ));
        assert!((dimension.measured_length_m().expect("length") - 0.08).abs() < 1e-12);
        let semantic = semantic_dimension();
        let value = serde_json::to_value(&semantic).expect("json");
        assert_eq!(
            serde_json::from_value::<LinearDimension>(value.clone()).expect("roundtrip"),
            semantic
        );
        let mut mixed = value;
        mixed["start_model_m"] = serde_json::json!([0, 0, 0]);
        mixed["end_model_m"] = serde_json::json!([0.08, 0, 0]);
        assert!(serde_json::from_value::<LinearDimension>(mixed).is_err());
    }

    #[test]
    fn derives_model_length_and_sheet_layout() -> Result<()> {
        let view = DrawingView::new(
            ViewId::new("view:front")?,
            "Front",
            ModelReference::new("part.ocad.d", DocumentId::new("doc:part")?),
            ProjectionKind::Front,
            0.5,
            [0.01, 0.02],
        );
        let dimension = LinearDimension {
            id: DimensionId::new("dim:width")?,
            view_id: view.id.clone(),
            measurement: crate::LinearMeasurement::Points {
                start_model_m: [0.0, 0.0, 0.0],
                end_model_m: [0.08, 0.0, 0.0],
            },
            offset_m: 0.01,
        };
        let layout = layout_linear_dimension(&dimension, &view)?;
        assert_eq!(layout.label, "80.00 mm");
        assert!((layout.line_end_m[0] - 0.05).abs() < 1.0e-9);
        assert!((layout.line_start_m[1] - 0.03).abs() < 1.0e-9);
        Ok(())
    }

    #[test]
    fn linear_dimension_round_trip() -> Result<()> {
        let dimension = LinearDimension {
            id: DimensionId::new("dim:width")?,
            view_id: ViewId::new("view:front")?,
            measurement: crate::LinearMeasurement::Points {
                start_model_m: [0.0, 0.0, 0.0],
                end_model_m: [0.08, 0.0, 0.0],
            },
            offset_m: -0.01,
        };
        let json = serde_json::to_string(&dimension).expect("serialize");
        let restored: LinearDimension = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(dimension, restored);
        Ok(())
    }
}
