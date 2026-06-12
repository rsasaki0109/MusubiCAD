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

    if let Ok(kernel_face_id) = resolve_kernel_face_id_for_topo_ref(
        ctx.semantic_refs(),
        ctx.face_history(),
        face_ref,
    ) {
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
        let target = target_feature_for_face_ref(
            &ctx,
            "ref:face:bracket_top",
            "feature:extrude_base",
        )
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

            fn sketch_for_feature(&self, sketch_feature_id: &str) -> Result<&opencad_sketch::Sketch> {
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
            FilletEdgeSelector::FacePerimeter {
                kernel_face_id: 88
            }
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
}
