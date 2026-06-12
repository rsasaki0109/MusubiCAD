//! Resolve persisted TopoRefs during feature regeneration.

use opencad_core::{OpenCadError, Result};
use opencad_geometry::{
    resolve_kernel_face_id_for_topo_ref, resolve_kernel_face_id_for_topo_ref_with_discoveries,
    FilletEdgeSelector,
};

use crate::feature::RegenContext;

/// Resolve the body feature that owns a persisted face ref.
pub fn target_feature_for_face_ref(
    ctx: &dyn RegenContext,
    face_ref: &str,
    fallback: &str,
) -> Result<String> {
    if face_ref.trim().is_empty() {
        return Ok(fallback.to_string());
    }

    let topo_ref = ctx
        .semantic_refs()
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == face_ref)
        .ok_or_else(|| OpenCadError::not_found(format!("topo ref '{face_ref}'")))?;

    let created_by = topo_ref.semantic.created_by.as_str();
    if !fallback.is_empty() && fallback != created_by {
        return Err(OpenCadError::validation(format!(
            "face_ref '{face_ref}' belongs to '{created_by}', not '{fallback}'"
        )));
    }
    Ok(created_by.to_string())
}

pub fn edge_selector_for_face_ref(
    ctx: &dyn RegenContext,
    face_ref: &str,
    fallback: FilletEdgeSelector,
) -> Result<FilletEdgeSelector> {
    if face_ref.trim().is_empty() {
        return Ok(fallback);
    }

    let topo_ref = ctx
        .semantic_refs()
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == face_ref)
        .ok_or_else(|| OpenCadError::not_found(format!("topo ref '{face_ref}'")))?;

    if let Ok(kernel_face_id) =
        resolve_kernel_face_id_for_topo_ref(ctx.semantic_refs(), ctx.face_history(), face_ref)
    {
        return Ok(FilletEdgeSelector::FacePerimeter { kernel_face_id });
    }

    if !ctx.face_discoveries().is_empty() {
        if let Ok(kernel_face_id) = resolve_kernel_face_id_for_topo_ref_with_discoveries(
            ctx.semantic_refs(),
            ctx.face_history(),
            face_ref,
            Some(ctx.face_discoveries()),
        ) {
            return Ok(FilletEdgeSelector::FacePerimeter { kernel_face_id });
        }
    }

    match topo_ref.semantic.role.as_deref() {
        Some("top") => Ok(FilletEdgeSelector::TopPerimeter),
        _ => Ok(fallback),
    }
}

/// Resolve a mirror/reflection plane from a persisted face ref and regen discoveries.
pub fn plane_for_face_ref(ctx: &dyn RegenContext, face_ref: &str) -> Result<([f64; 3], [f64; 3])> {
    let topo_ref = ctx
        .semantic_refs()
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == face_ref)
        .ok_or_else(|| OpenCadError::not_found(format!("topo ref '{face_ref}'")))?;

    let role = topo_ref.semantic.role.as_deref().unwrap_or("");
    let created_by = topo_ref.semantic.created_by.as_str();

    if let Some(discovery) = ctx.face_discoveries().iter().find(|discovery| {
        discovery.role == role && discovery.feature_id.as_deref() == Some(created_by)
    }) {
        return Ok(discovery_plane(discovery));
    }

    if let Some(discovery) = ctx
        .face_discoveries()
        .iter()
        .find(|discovery| discovery.role == role)
    {
        return Ok(discovery_plane(discovery));
    }

    if let Some(normal_hint) = topo_ref.semantic.normal_hint {
        normalize_plane_normal([normal_hint[0], normal_hint[1], normal_hint[2]])?;
        return Err(OpenCadError::not_found(format!(
            "face_ref '{face_ref}' has normal hint but no matching face discovery for plane origin"
        )));
    }

    Err(OpenCadError::not_found(format!(
        "no mirror plane found for face_ref '{face_ref}'"
    )))
}

fn discovery_plane(discovery: &opencad_geometry::FaceRefDiscovery) -> ([f64; 3], [f64; 3]) {
    let origin = [
        discovery.centroid_m[0] as f64,
        discovery.centroid_m[1] as f64,
        discovery.centroid_m[2] as f64,
    ];
    let normal = [
        discovery.normal_m[0] as f64,
        discovery.normal_m[1] as f64,
        discovery.normal_m[2] as f64,
    ];
    (
        origin,
        normalize_plane_normal(normal).unwrap_or([0.0, 0.0, 1.0]),
    )
}

fn normalize_plane_normal(normal_m: [f64; 3]) -> Result<[f64; 3]> {
    let length =
        (normal_m[0] * normal_m[0] + normal_m[1] * normal_m[1] + normal_m[2] * normal_m[2]).sqrt();
    if length <= 1e-12 {
        return Err(OpenCadError::validation(
            "face plane normal must be a non-zero vector",
        ));
    }
    Ok([
        normal_m[0] / length,
        normal_m[1] / length,
        normal_m[2] / length,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::RegenContext;
    use crate::regenerate::TestRegenContext;
    use opencad_core::TopoRefId;
    use opencad_geometry::{KernelBody, TopoRef};

    struct RefContext {
        inner: TestRegenContext,
        semantic_refs: Vec<TopoRef>,
    }

    impl RegenContext for RefContext {
        fn kernel(&self) -> &dyn opencad_geometry::GeometryKernel {
            self.inner.kernel()
        }

        fn sketch_for_feature(&self, sketch_feature_id: &str) -> Result<&opencad_sketch::Sketch> {
            self.inner.sketch_for_feature(sketch_feature_id)
        }

        fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody> {
            self.inner.body_for_feature(feature_id)
        }

        fn semantic_refs(&self) -> &[TopoRef] {
            &self.semantic_refs
        }
    }

    #[test]
    fn target_feature_for_face_ref_returns_created_by() {
        let ctx = RefContext {
            inner: TestRegenContext::with_body("feature:base", KernelBody::new(42)),
            semantic_refs: vec![TopoRef::face(
                TopoRefId::new("ref:face:bracket_top").expect("id"),
                "feature:extrude_base",
                "top",
            )],
        };
        let target =
            target_feature_for_face_ref(&ctx, "ref:face:bracket_top", "feature:extrude_base")
                .expect("target");
        assert_eq!(target, "feature:extrude_base");
    }

    #[test]
    fn face_ref_target_mismatch_is_rejected() {
        let ctx = RefContext {
            inner: TestRegenContext::with_body("feature:base", KernelBody::new(42)),
            semantic_refs: vec![TopoRef::face(
                TopoRefId::new("ref:face:bracket_top").expect("id"),
                "feature:extrude_base",
                "top",
            )],
        };
        let err = target_feature_for_face_ref(&ctx, "ref:face:bracket_top", "feature:hole_mount")
            .expect_err("mismatch");
        assert!(err.to_string().contains("belongs to"));
    }

    #[test]
    fn face_ref_resolves_via_fingerprint_discoveries() {
        use opencad_geometry::FaceRefDiscovery;

        struct RefContext {
            inner: TestRegenContext,
            semantic_refs: Vec<TopoRef>,
            face_discoveries: Vec<FaceRefDiscovery>,
        }

        impl RegenContext for RefContext {
            fn kernel(&self) -> &dyn opencad_geometry::GeometryKernel {
                self.inner.kernel()
            }

            fn sketch_for_feature(
                &self,
                sketch_feature_id: &str,
            ) -> Result<&opencad_sketch::Sketch> {
                self.inner.sketch_for_feature(sketch_feature_id)
            }

            fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody> {
                self.inner.body_for_feature(feature_id)
            }

            fn semantic_refs(&self) -> &[TopoRef] {
                &self.semantic_refs
            }

            fn face_discoveries(&self) -> &[FaceRefDiscovery] {
                &self.face_discoveries
            }
        }

        let ctx = RefContext {
            inner: TestRegenContext::with_body("feature:base", KernelBody::new(42)),
            semantic_refs: vec![TopoRef::face(
                TopoRefId::new("ref:face:bracket_top").expect("id"),
                "feature:extrude_base",
                "top",
            )],
            face_discoveries: vec![FaceRefDiscovery {
                kernel_face_id: 88,
                role: "top".into(),
                normal_m: [0.0, 0.0, 1.0],
                centroid_m: [0.0, 0.0, 0.006],
                feature_id: Some("feature:extrude_base".into()),
            }],
        };
        let selector =
            edge_selector_for_face_ref(&ctx, "ref:face:bracket_top", FilletEdgeSelector::All)
                .expect("selector");
        assert_eq!(
            selector,
            FilletEdgeSelector::FacePerimeter { kernel_face_id: 88 }
        );
    }

    #[test]
    fn face_ref_without_kernel_id_falls_back_to_top_perimeter() {
        let ctx = RefContext {
            inner: TestRegenContext::with_body("feature:base", KernelBody::new(42)),
            semantic_refs: vec![TopoRef::face(
                TopoRefId::new("ref:face:bracket_top").expect("id"),
                "feature:extrude_base",
                "top",
            )],
        };
        let selector =
            edge_selector_for_face_ref(&ctx, "ref:face:bracket_top", FilletEdgeSelector::All)
                .expect("selector");
        assert_eq!(selector, FilletEdgeSelector::TopPerimeter);
    }

    #[test]
    fn plane_for_face_ref_uses_discovery_centroid_and_normal() {
        use opencad_geometry::FaceRefDiscovery;

        struct RefContext {
            inner: TestRegenContext,
            semantic_refs: Vec<TopoRef>,
            face_discoveries: Vec<FaceRefDiscovery>,
        }

        impl RegenContext for RefContext {
            fn kernel(&self) -> &dyn opencad_geometry::GeometryKernel {
                self.inner.kernel()
            }

            fn sketch_for_feature(
                &self,
                sketch_feature_id: &str,
            ) -> Result<&opencad_sketch::Sketch> {
                self.inner.sketch_for_feature(sketch_feature_id)
            }

            fn body_for_feature(&self, feature_id: &str) -> Result<KernelBody> {
                self.inner.body_for_feature(feature_id)
            }

            fn semantic_refs(&self) -> &[TopoRef] {
                &self.semantic_refs
            }

            fn face_discoveries(&self) -> &[FaceRefDiscovery] {
                &self.face_discoveries
            }
        }

        let ctx = RefContext {
            inner: TestRegenContext::empty(),
            semantic_refs: vec![TopoRef::face(
                TopoRefId::new("ref:face:bracket_top").expect("id"),
                "feature:extrude_base",
                "top",
            )],
            face_discoveries: vec![FaceRefDiscovery {
                kernel_face_id: 12,
                role: "top".into(),
                normal_m: [0.0, 0.0, 1.0],
                centroid_m: [0.04, 0.03, 0.006],
                feature_id: Some("feature:extrude_base".into()),
            }],
        };
        let (origin, normal) = plane_for_face_ref(&ctx, "ref:face:bracket_top").expect("plane");
        assert!((origin[0] - 0.04).abs() < 1e-9);
        assert!((origin[2] - 0.006).abs() < 1e-9);
        assert!((normal[2] - 1.0).abs() < 1e-9);
    }
}
