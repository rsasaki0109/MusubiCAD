//! Semantic full-circle radius and diameter annotations (ADR-041).
use crate::{DrawingView, ProjectionKind};
use opencad_core::{DimensionId, OpenCadError, Result, TopoRefId, ViewId};
use opencad_geometry::{
    resolve_edge_ref_with_provenance, CircularEdgeGeometry, EdgeRefDiscovery, GeometryKernel,
    KernelBody, ReferenceProvenance, TopoRef, TopoRefKind,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadialKind {
    Radius,
    Diameter,
}
/// Only semantic bindings are authored; circle witnesses are disposable regen data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadialDimension {
    pub id: DimensionId,
    pub view_id: ViewId,
    pub edge_ref: TopoRefId,
    pub kind: RadialKind,
    /// Direction in sheet coordinates, normalized during layout.
    pub leader_direction: [f64; 2],
    /// Extra leader length beyond the circumference, in sheet metres.
    pub leader_extension_m: f64,
}
impl RadialDimension {
    pub fn validate(&self) -> Result<()> {
        let norm = self.leader_direction[0].hypot(self.leader_direction[1]);
        if !self.edge_ref.as_str().starts_with("ref:edge:")
            || !norm.is_finite()
            || norm <= 1e-9
            || !self.leader_extension_m.is_finite()
            || self.leader_extension_m <= 1e-9
        {
            return Err(OpenCadError::validation("radial dimension requires an edge ref, finite nonzero leader direction and positive extension in sheet metres"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRadialDimension {
    pub dimension: RadialDimension,
    pub circle: CircularEdgeGeometry,
    pub reference_provenance: ReferenceProvenance,
}
impl ResolvedRadialDimension {
    pub fn measured_length_m(&self) -> f64 {
        self.circle.radius_m
            * if self.dimension.kind == RadialKind::Diameter {
                2.
            } else {
                1.
            }
    }
}
pub fn resolve_radial_dimension(
    dimension: &RadialDimension,
    refs: &[TopoRef],
    discoveries: &[EdgeRefDiscovery],
    kernel: &impl GeometryKernel,
    body: &KernelBody,
) -> Result<ResolvedRadialDimension> {
    dimension.validate()?;
    let id = &dimension.edge_ref;
    let matching: Vec<_> = refs.iter().filter(|r| r.ref_id == *id).collect();
    if matching.len() != 1 || matching[0].kind != TopoRefKind::Edge {
        return Err(OpenCadError::validation(
            "radial reference is missing, duplicated or not an edge",
        ));
    }
    let reference = matching[0];
    let has_hint = reference
        .geometric_fingerprint
        .as_ref()
        .is_some_and(|hint| {
            hint.bbox_hint.is_some()
                || hint.kernel_edge_id.is_some_and(|id| {
                    discoveries.iter().any(|e| {
                        e.kernel_edge_id == id
                            && reference
                                .semantic
                                .role
                                .as_deref()
                                .map_or(true, |role| role == e.role)
                    })
                })
        });
    if !has_hint
        && (reference.semantic.role.is_none()
            || discoveries
                .iter()
                .filter(|e| reference.semantic.role.as_deref() == Some(e.role.as_str()))
                .count()
                > 1)
    {
        return Err(OpenCadError::validation("radial reference needs a unique role or usable geometric hint; ambiguous roles are refused"));
    }
    let resolution = resolve_edge_ref_with_provenance(
        refs,
        id.as_str(),
        Some(discoveries),
        Default::default(),
        true,
    )?;
    let edge_id = resolution
        .resolved_kernel_id
        .ok_or_else(|| OpenCadError::validation("radial edge did not resolve"))?;
    let circle = kernel.circular_edge_geometry(body, edge_id).map_err(|e| {
        e.with_context(format!("radial dimension '{}' edge '{}'", dimension.id, id))
    })?;
    validate_circle(&circle)?;
    Ok(ResolvedRadialDimension {
        dimension: dimension.clone(),
        circle,
        reference_provenance: resolution.provenance,
    })
}
fn validate_circle(circle: &CircularEdgeGeometry) -> Result<()> {
    let norm = circle.normal.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !circle
        .center_m
        .iter()
        .chain(&circle.normal)
        .all(|v| v.is_finite())
        || !(circle.radius_m * 2000.).is_finite()
        || circle.radius_m <= 1e-9
        || (norm - 1.).abs() > 1e-9
    {
        return Err(OpenCadError::validation(
            "circle witness requires finite SI center/radius and unit normal",
        ));
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq)]
pub struct RadialLayout {
    pub line_start_m: [f64; 2],
    pub circle_point_m: [f64; 2],
    pub text_position_m: [f64; 2],
    pub label: String,
}
pub fn layout_radial_dimension(
    dimension: &RadialDimension,
    circle: &CircularEdgeGeometry,
    view: &DrawingView,
) -> Result<RadialLayout> {
    dimension.validate()?;
    validate_circle(circle)?;
    if view.id != dimension.view_id
        || !view.scale.is_finite()
        || view.scale <= 0.
        || !view.origin_on_sheet_m.iter().all(|v| v.is_finite())
        || view.projection == ProjectionKind::Isometric
    {
        return Err(OpenCadError::validation(
            "radial dimension requires its own finite orthographic view",
        ));
    }
    let normal = view.projection.project_point(circle.normal);
    if normal[0].hypot(normal[1]) > 1e-9 {
        return Err(OpenCadError::validation(
            "radial circle plane is foreshortened in this view",
        ));
    }
    let center = view.projection.project_point(circle.center_m);
    let center: [f64; 2] =
        std::array::from_fn(|i| view.origin_on_sheet_m[i] + center[i] * view.scale);
    let norm = dimension.leader_direction[0].hypot(dimension.leader_direction[1]);
    let direction = dimension.leader_direction.map(|v| v / norm);
    let radius = circle.radius_m * view.scale;
    let diameter = dimension.kind == RadialKind::Diameter;
    let layout = RadialLayout {
        line_start_m: std::array::from_fn(|i| {
            center[i] - if diameter { direction[i] * radius } else { 0. }
        }),
        circle_point_m: std::array::from_fn(|i| center[i] + direction[i] * radius),
        text_position_m: std::array::from_fn(|i| {
            center[i] + direction[i] * (radius + dimension.leader_extension_m)
        }),
        label: format!(
            "{} {:.2} mm",
            if diameter { "⌀" } else { "R" },
            circle.radius_m * 1000. * if diameter { 2. } else { 1. }
        ),
    };
    if !layout
        .line_start_m
        .iter()
        .chain(&layout.circle_point_m)
        .chain(&layout.text_position_m)
        .all(|v| (v * 1000.).is_finite())
    {
        return Err(OpenCadError::validation(
            "radial layout overflows sheet millimetres",
        ));
    }
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelReference;
    use opencad_core::DocumentId;
    fn dim() -> RadialDimension {
        RadialDimension {
            id: DimensionId::new("dim:radius").expect("id"),
            view_id: ViewId::new("view:front").expect("id"),
            edge_ref: TopoRefId::new("ref:edge:circle").expect("id"),
            kind: RadialKind::Radius,
            leader_direction: [3., 4.],
            leader_extension_m: 0.01,
        }
    }
    fn view() -> DrawingView {
        DrawingView::new(
            ViewId::new("view:front").expect("id"),
            "Front",
            ModelReference::new("part.ocad.d", DocumentId::new("doc:part").expect("id")),
            ProjectionKind::Front,
            0.5,
            [0.05, 0.06],
        )
    }
    #[test]
    fn analytic_layouts_use_model_mm_and_sheet_scaled_witnesses() {
        for radius in [0.001, 0.005, 0.02] {
            let circle = CircularEdgeGeometry {
                center_m: [0.02, 0.03, 0.01],
                normal: [0., 0., 1.],
                radius_m: radius,
            };
            for kind in [RadialKind::Radius, RadialKind::Diameter] {
                let mut d = dim();
                d.kind = kind;
                let layout = layout_radial_dimension(&d, &circle, &view()).expect("layout");
                let c = [0.06, 0.075];
                for i in 0..2 {
                    let u = [0.6, 0.8][i];
                    assert!((layout.circle_point_m[i] - c[i] - u * radius * 0.5).abs() < 1e-12);
                    assert!(
                        (layout.text_position_m[i] - c[i] - u * (radius * 0.5 + 0.01)).abs()
                            < 1e-12
                    );
                    assert!(
                        (layout.line_start_m[i] - c[i]
                            + if kind == RadialKind::Diameter {
                                u * radius * 0.5
                            } else {
                                0.
                            })
                        .abs()
                            < 1e-12
                    );
                }
                assert_eq!(
                    layout.label,
                    format!(
                        "{} {:.2} mm",
                        if kind == RadialKind::Radius {
                            "R"
                        } else {
                            "⌀"
                        },
                        radius * 1000. * if kind == RadialKind::Radius { 1. } else { 2. }
                    )
                );
            }
        }
    }
    #[test]
    fn mock_semantic_circle_and_fail_closed_refusal() {
        let kernel = opencad_geometry::MockGeometryKernel::new();
        let body = KernelBody(20);
        let ref_ = TopoRef::edge(
            TopoRefId::new("ref:edge:circle").expect("id"),
            "feature:circle",
            "circle",
        );
        let discovery = EdgeRefDiscovery {
            kernel_edge_id: 5,
            role: "circle".into(),
            midpoint_m: [0.02, 0.01, 0.02],
            tangent_m: [0., 0., 0.],
            length_m: 0.,
            feature_id: Some("feature:circle".into()),
        };
        let refs = [ref_];
        let edges = [discovery.clone()];
        let d = dim();
        let resolved =
            resolve_radial_dimension(&d, &refs, &edges, &kernel, &body).expect("resolve");
        assert!((resolved.measured_length_m() - 0.01).abs() < 1e-12);
        assert_eq!(resolved.dimension, d);
        assert!(resolve_radial_dimension(&d, &[], &edges, &kernel, &body).is_err());
        assert!(resolve_radial_dimension(
            &d,
            &refs,
            &[discovery.clone(), discovery.clone()],
            &kernel,
            &body
        )
        .is_err());
        let mut line = discovery;
        line.kernel_edge_id = 1;
        assert!(resolve_radial_dimension(&d, &refs, &[line], &kernel, &body).is_err());
        let json = serde_json::to_value(&d).expect("json");
        assert_eq!(d, serde_json::from_value(json.clone()).expect("round trip"));
        let mut forged = json;
        forged["radius_m"] = serde_json::json!(0.02);
        assert!(serde_json::from_value::<RadialDimension>(forged).is_err());
    }
    #[test]
    fn invalid_values_and_projection_are_refused() {
        let circle = CircularEdgeGeometry {
            center_m: [0.; 3],
            normal: [0., 0., 1.],
            radius_m: 0.01,
        };
        let d = dim();
        for projection in [
            ProjectionKind::Top,
            ProjectionKind::Right,
            ProjectionKind::Isometric,
        ] {
            let mut v = view();
            v.projection = projection;
            assert!(layout_radial_dimension(&d, &circle, &v).is_err());
        }
        for value in [0., -1., f64::INFINITY, f64::NAN] {
            let mut d = d.clone();
            d.leader_extension_m = value;
            assert!(d.validate().is_err());
            let mut c = circle.clone();
            c.radius_m = value;
            assert!(layout_radial_dimension(&dim(), &c, &view()).is_err());
        }
        let mut huge = circle.clone();
        huge.radius_m = f64::MAX;
        assert!(layout_radial_dimension(&d, &huge, &view()).is_err());
        let mut huge = circle.clone();
        huge.center_m = [f64::MAX; 3];
        assert!(layout_radial_dimension(&d, &huge, &view()).is_err());
        let mut d = d;
        d.leader_direction = [0.; 2];
        assert!(d.validate().is_err());
        let mut c = circle;
        c.normal = [0., 0., 2.];
        assert!(layout_radial_dimension(&dim(), &c, &view()).is_err());
    }
}
