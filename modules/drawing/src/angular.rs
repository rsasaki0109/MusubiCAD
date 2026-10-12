//! Derived angular annotations from explicit rays (ADR-040, MCAD-P12-001).
use crate::{DrawingView, ProjectionKind};
use opencad_core::{DimensionId, OpenCadError, Result, TopoRefId, ViewId};
use opencad_geometry::{
    resolve_edge_ref_with_provenance, EdgeRefDiscovery, GeometryKernel, KernelBody,
    ReferenceProvenance, TopoRef, TopoRefKind,
};
use serde::{Deserialize, Serialize};
const LENGTH_TOLERANCE_M: f64 = 1e-9;
const INTERSECTION_TOLERANCE_M: f64 = 1e-8;
const ANGLE_TOLERANCE_RAD: f64 = 1e-9;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RayEnd {
    Start,
    End,
}
fn default_ends() -> [RayEnd; 2] {
    [RayEnd::End; 2]
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum AngularMeasurement {
    Edges {
        edge_refs: [TopoRefId; 2],
        #[serde(default = "default_ends")]
        ray_ends: [RayEnd; 2],
    },
    Points {
        vertex_model_m: [f64; 3],
        first_model_m: [f64; 3],
        second_model_m: [f64; 3],
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AngularDimension {
    pub id: DimensionId,
    pub view_id: ViewId,
    #[serde(flatten)]
    pub measurement: AngularMeasurement,
    /// Arc radius in sheet metres, independent of view scale.
    pub arc_radius_m: f64,
}
impl AngularDimension {
    pub fn validate(&self) -> Result<()> {
        if !self.arc_radius_m.is_finite() || self.arc_radius_m <= LENGTH_TOLERANCE_M {
            return Err(invalid(
                "angular arc radius must be finite and positive in sheet metres",
            ));
        }
        match &self.measurement {
            AngularMeasurement::Edges { edge_refs, .. } => {
                if edge_refs[0] == edge_refs[1]
                    || edge_refs
                        .iter()
                        .any(|id| !id.as_str().starts_with("ref:edge:"))
                {
                    return Err(invalid(
                        "angular dimension needs two distinct semantic edge references",
                    ));
                }
            }
            AngularMeasurement::Points {
                vertex_model_m,
                first_model_m,
                second_model_m,
            } => {
                if !vertex_model_m
                    .iter()
                    .chain(first_model_m)
                    .chain(second_model_m)
                    .all(|v| v.is_finite())
                {
                    return Err(invalid("angular points must be finite model metres"));
                }
                self.measured_angle_rad()?;
            }
        }
        Ok(())
    }
    pub fn measured_angle_rad(&self) -> Result<f64> {
        let AngularMeasurement::Points {
            vertex_model_m,
            first_model_m,
            second_model_m,
        } = self.measurement
        else {
            return Err(invalid(
                "angular dimension requires regenerated semantic rays",
            ));
        };
        let a = unit(sub(first_model_m, vertex_model_m))?;
        let b = unit(sub(second_model_m, vertex_model_m))?;
        let angle = dot(a, b).clamp(-1., 1.).acos();
        if angle <= ANGLE_TOLERANCE_RAD || std::f64::consts::PI - angle <= ANGLE_TOLERANCE_RAD {
            return Err(invalid(
                "angular rays must define a nondegenerate minor angle",
            ));
        }
        Ok(angle)
    }
}
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(message)
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: [f64; 3]) -> Result<[f64; 3]> {
    let n = norm(a);
    if !n.is_finite() || n <= LENGTH_TOLERANCE_M {
        return Err(invalid("angular ray has no finite positive length"));
    }
    Ok(a.map(|v| v / n))
}
fn canonical(mut ends: [[f64; 3]; 2]) -> [[f64; 3]; 2] {
    ends.sort_by(|a, b| {
        a.iter()
            .zip(b)
            .map(|(a, b)| a.total_cmp(b))
            .find(|o| !o.is_eq())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ends
}
/// Resolve both straight edges on the final body. No cached points are persisted.
pub fn resolve_angular_dimension(
    dimension: &AngularDimension,
    refs: &[TopoRef],
    discoveries: &[EdgeRefDiscovery],
    kernel: &impl GeometryKernel,
    body: &KernelBody,
) -> Result<(AngularDimension, Vec<ReferenceProvenance>)> {
    dimension.validate()?;
    let AngularMeasurement::Edges {
        edge_refs,
        ray_ends,
    } = &dimension.measurement
    else {
        return Ok((dimension.clone(), vec![]));
    };
    let mut edges = Vec::new();
    let mut provenance = Vec::new();
    for id in edge_refs {
        let matching: Vec<_> = refs.iter().filter(|r| r.ref_id == *id).collect();
        if matching.len() != 1 || matching[0].kind != TopoRefKind::Edge {
            return Err(invalid(&format!(
                "angular dimension '{}' reference '{}' is missing, duplicated or not an edge",
                dimension.id, id
            )));
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
        if !has_hint && reference.semantic.role.is_none() {
            return Err(invalid(
                "angular reference needs a unique role or a usable geometric hint",
            ));
        }
        if !has_hint
            && discoveries
                .iter()
                .filter(|e| reference.semantic.role.as_deref() == Some(e.role.as_str()))
                .count()
                > 1
        {
            return Err(invalid(&format!(
                "angular dimension '{}' reference '{}' is ambiguous",
                dimension.id, id
            )));
        }
        let resolution = resolve_edge_ref_with_provenance(
            refs,
            id.as_str(),
            Some(discoveries),
            Default::default(),
            true,
        )
        .map_err(|error| {
            error.with_context(format!("angular dimension {} edge {}", dimension.id, id))
        })?;
        let edge_id = resolution
            .resolved_kernel_id
            .ok_or_else(|| invalid("angular edge did not resolve"))?;
        edges.push(canonical(
            kernel.straight_edge_endpoints(body, edge_id).map_err(|e| {
                e.with_context(format!(
                    "angular dimension '{}' edge '{}'",
                    dimension.id, id
                ))
            })?,
        ));
        provenance.push(resolution.provenance);
    }
    let vertex = intersection_vertex(&edges)?;
    let pick = |i: usize| edges[i][if ray_ends[i] == RayEnd::Start { 0 } else { 1 }];
    let mut resolved = dimension.clone();
    resolved.measurement = AngularMeasurement::Points {
        vertex_model_m: vertex,
        first_model_m: pick(0),
        second_model_m: pick(1),
    };
    resolved.validate()?;
    Ok((resolved, provenance))
}
fn intersection_vertex(edges: &[[[f64; 3]; 2]]) -> Result<[f64; 3]> {
    let [a, b] = edges[0];
    let [c, d] = edges[1];
    let u = unit(sub(b, a))?;
    let v = unit(sub(d, c))?;
    let n = cross(u, v);
    let n2 = dot(n, n);
    if n2 <= ANGLE_TOLERANCE_RAD.powi(2) {
        return Err(invalid("angular supporting lines are parallel"));
    }
    let w = sub(c, a);
    let t = dot(cross(w, v), n) / n2;
    let s = dot(cross(w, u), n) / n2;
    let vertex = std::array::from_fn(|i| a[i] + t * u[i]);
    let other = std::array::from_fn(|i| c[i] + s * v[i]);
    if norm(sub(vertex, other)) > INTERSECTION_TOLERANCE_M {
        return Err(invalid("angular supporting lines are skew"));
    }
    Ok(vertex)
}

#[derive(Debug, Clone, PartialEq)]
pub struct AngularLayout {
    pub vertex_m: [f64; 2],
    pub arc_start_m: [f64; 2],
    pub arc_end_m: [f64; 2],
    pub text_position_m: [f64; 2],
    pub radius_m: f64,
    pub clockwise: bool,
    pub label: String,
}
pub fn layout_angular_dimension(
    dimension: &AngularDimension,
    view: &DrawingView,
) -> Result<AngularLayout> {
    dimension.validate()?;
    if dimension.view_id != view.id
        || !view.scale.is_finite()
        || view.scale <= 0.
        || !view.origin_on_sheet_m.iter().all(|v| v.is_finite())
    {
        return Err(invalid(
            "angular dimension requires its own finite positive-scale view",
        ));
    }
    if view.projection == ProjectionKind::Isometric {
        return Err(invalid("angular dimensions require an orthographic view"));
    }
    let AngularMeasurement::Points {
        vertex_model_m,
        first_model_m,
        second_model_m,
    } = dimension.measurement
    else {
        return Err(invalid(
            "angular dimension requires regenerated semantic rays",
        ));
    };
    let a = unit(sub(first_model_m, vertex_model_m))?;
    let b = unit(sub(second_model_m, vertex_model_m))?;
    let normal = unit(cross(a, b))?;
    let projected = view.projection.project_point(normal);
    if projected[0].hypot(projected[1]) > ANGLE_TOLERANCE_RAD {
        return Err(invalid(
            "angular measurement plane is foreshortened in this view",
        ));
    }
    let vertex = view.projection.project_point(vertex_model_m);
    let vertex = std::array::from_fn(|i| view.origin_on_sheet_m[i] + vertex[i] * view.scale);
    let a = view.projection.project_point(a);
    let b = view.projection.project_point(b);
    let bisector = [a[0] + b[0], a[1] + b[1]];
    let length = bisector[0].hypot(bisector[1]);
    let radius = dimension.arc_radius_m;
    Ok(AngularLayout {
        vertex_m: vertex,
        arc_start_m: std::array::from_fn(|i| vertex[i] + a[i] * radius),
        arc_end_m: std::array::from_fn(|i| vertex[i] + b[i] * radius),
        text_position_m: std::array::from_fn(|i| {
            vertex[i] + bisector[i] / length * (radius + 0.003)
        }),
        radius_m: radius,
        clockwise: a[0] * b[1] - a[1] * b[0] < 0.,
        label: format!("{:.2}°", dimension.measured_angle_rad()?.to_degrees()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelReference;
    use opencad_core::DocumentId;
    fn points(angle: f64) -> AngularDimension {
        AngularDimension {
            id: DimensionId::new("dim:angle").expect("id"),
            view_id: ViewId::new("view:front").expect("id"),
            arc_radius_m: 0.01,
            measurement: AngularMeasurement::Points {
                vertex_model_m: [0., 0., 0.],
                first_model_m: [0.02, 0., 0.],
                second_model_m: [0.03 * angle.cos(), 0.03 * angle.sin(), 0.],
            },
        }
    }
    fn view() -> DrawingView {
        DrawingView::new(
            ViewId::new("view:front").expect("id"),
            "Front",
            ModelReference::new("part.ocad.d", DocumentId::new("doc:part").expect("id")),
            ProjectionKind::Front,
            0.5,
            [0.04, 0.05],
        )
    }
    #[test]
    fn analytic_angles_sheet_scale_and_refusals() {
        for deg in [30_f64, 45., 90., 135.] {
            let dim = points(deg.to_radians());
            dim.validate().expect("valid");
            let mut view = view();
            assert!((dim.measured_angle_rad().expect("angle") - deg.to_radians()).abs() < 1e-12);
            let layout = layout_angular_dimension(&dim, &view).expect("layout");
            assert_eq!(layout.label, format!("{deg:.2}°"));
            assert!((layout.arc_start_m[0] - 0.05).abs() < 1e-12);
            assert!((layout.vertex_m[1] - 0.05).abs() < 1e-12);
            view.scale = 2.;
            let scaled =
                layout_angular_dimension(&dim, &view).expect("scale does not change the arc");
            for (a, b) in layout.arc_start_m.into_iter().zip(scaled.arc_start_m) {
                assert!((a - b).abs() < 1e-12);
            }
            for (a, b) in layout.arc_end_m.into_iter().zip(scaled.arc_end_m) {
                assert!((a - b).abs() < 1e-12);
            }
            assert_eq!(layout.label, scaled.label);
            view.projection = ProjectionKind::Top;
            assert!(layout_angular_dimension(&dim, &view).is_err());
            view.projection = ProjectionKind::Isometric;
            assert!(layout_angular_dimension(&dim, &view).is_err());
            let value = serde_json::to_value(&dim).expect("json");
            assert_eq!(dim, serde_json::from_value(value).expect("round trip"));
        }
        for angle in [0., std::f64::consts::PI] {
            assert!(points(angle).validate().is_err());
        }
        let mut bad = points(1.);
        bad.arc_radius_m = f64::NAN;
        assert!(bad.validate().is_err());
    }
    #[test]
    fn parallel_and_skew_supporting_lines_are_refused() {
        let base = [[0., 0., 0.], [1., 0., 0.]];
        assert!(intersection_vertex(&[base, [[0., 1., 0.], [1., 1., 0.]]]).is_err());
        assert!(intersection_vertex(&[base, [[0., 0., 1.], [0., 1., 1.]]]).is_err());
        let vertex =
            intersection_vertex(&[base, [[0.5, -1., 0.], [0.5, 1., 0.]]]).expect("intersect");
        assert!((vertex[0] - 0.5).abs() < 1e-12);
    }
    #[test]
    fn mock_semantic_rays_are_canonical_and_fail_closed() {
        let kernel = opencad_geometry::MockGeometryKernel::new();
        let body = KernelBody::new(80);
        let discoveries = vec![
            EdgeRefDiscovery {
                kernel_edge_id: 1,
                role: "front".into(),
                midpoint_m: [0.04, 0., 0.08],
                tangent_m: [1., 0., 0.],
                length_m: 0.08,
                feature_id: Some("feature:plate".into()),
            },
            EdgeRefDiscovery {
                kernel_edge_id: 4,
                role: "left".into(),
                midpoint_m: [0., 0.04, 0.08],
                tangent_m: [0., 1., 0.],
                length_m: 0.08,
                feature_id: Some("feature:plate".into()),
            },
        ];
        let refs = vec![
            TopoRef::edge(
                TopoRefId::new("ref:edge:front").expect("id"),
                "feature:plate",
                "front",
            ),
            TopoRef::edge(
                TopoRefId::new("ref:edge:left").expect("id"),
                "feature:plate",
                "left",
            ),
        ];
        let mut dim = points(1.);
        dim.measurement = AngularMeasurement::Edges {
            edge_refs: [refs[0].ref_id.clone(), refs[1].ref_id.clone()],
            ray_ends: [RayEnd::End; 2],
        };
        let authored = serde_json::to_value(&dim).expect("json");
        let (resolved, provenance) =
            resolve_angular_dimension(&dim, &refs, &discoveries, &kernel, &body).expect("resolve");
        assert!(
            (resolved.measured_angle_rad().expect("angle") - std::f64::consts::FRAC_PI_2).abs()
                < 1e-12
        );
        assert_eq!(provenance.len(), 2);
        assert_eq!(authored, serde_json::to_value(&dim).expect("unchanged"));
        assert_eq!(
            dim,
            serde_json::from_value(authored.clone()).expect("round trip")
        );
        let mut mixed = authored;
        mixed["vertex_model_m"] = serde_json::json!([0, 0, 0]);
        assert!(serde_json::from_value::<AngularDimension>(mixed).is_err());
        assert!(resolve_angular_dimension(&dim, &[], &discoveries, &kernel, &body).is_err());
        let mut ambiguous = discoveries.clone();
        let mut extra = ambiguous[0].clone();
        extra.kernel_edge_id = 99;
        extra.length_m = 0.02;
        ambiguous.push(extra);
        assert!(resolve_angular_dimension(&dim, &refs, &ambiguous, &kernel, &body).is_err());
        let mut wrong = refs.clone();
        wrong[0].kind = TopoRefKind::Face;
        assert!(resolve_angular_dimension(&dim, &wrong, &discoveries, &kernel, &body).is_err());
        let AngularMeasurement::Edges { ray_ends, .. } = &mut dim.measurement else {
            panic!("edges")
        };
        ray_ends[0] = RayEnd::Start;
        assert!(resolve_angular_dimension(&dim, &refs, &discoveries, &kernel, &body).is_err());
    }
}
