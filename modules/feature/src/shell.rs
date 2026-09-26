//! Shell feature: hollow a body to a uniform inward wall, leaving chosen
//! faces open (ADR-017, MCAD-P7-007).

use serde::{Deserialize, Serialize};

use opencad_core::{Length, OpenCadError, Result};
use opencad_geometry::{resolve_kernel_face_id_for_topo_ref_with_discoveries, FacePick};

use crate::topo_resolve::resolve_face_on_body;

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};

/// Thinnest accepted shell wall, in metres (ADR-017 §3).
pub const SHELL_MIN_THICKNESS_M: f64 = 1e-6;

/// Shell feature parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellFeature {
    /// Feature id whose body output is shelled.
    pub target_feature: String,
    /// Wall thickness, grown inward from the outer faces.
    pub thickness: Length,
    /// Parametric thickness expression resolved before regeneration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness_expr: Option<String>,
    /// Semantic face references (`ref:face:*`) removed to form openings.
    pub open_face_refs: Vec<String>,
}

impl ShellFeature {
    pub fn new(
        target_feature: impl Into<String>,
        thickness: Length,
        thickness_expr: Option<String>,
        open_face_refs: Vec<String>,
    ) -> Self {
        Self {
            target_feature: target_feature.into(),
            thickness,
            thickness_expr,
            open_face_refs,
        }
    }

    /// Check the stored thickness and the open face list.
    pub fn validate(&self) -> Result<()> {
        validate_thickness(self.thickness.meters())?;
        if self.open_face_refs.is_empty() {
            return Err(OpenCadError::validation(
                "shell needs at least one open face reference",
            ));
        }
        for (index, face_ref) in self.open_face_refs.iter().enumerate() {
            if !face_ref.starts_with("ref:face:") {
                return Err(OpenCadError::validation(format!(
                    "shell open face '{face_ref}' must be a 'ref:face:' reference"
                )));
            }
            if self.open_face_refs[..index].contains(face_ref) {
                return Err(OpenCadError::validation(format!(
                    "shell open face '{face_ref}' is listed twice"
                )));
            }
        }
        Ok(())
    }
}

/// Reject non-finite walls and walls thinner than [`SHELL_MIN_THICKNESS_M`].
pub fn validate_thickness(thickness_m: f64) -> Result<()> {
    if !thickness_m.is_finite() || thickness_m <= SHELL_MIN_THICKNESS_M {
        return Err(OpenCadError::validation(format!(
            "shell thickness must be greater than {SHELL_MIN_THICKNESS_M} m, got {thickness_m} m"
        )));
    }
    Ok(())
}

/// Shell executor wired to `GeometryKernel::shell_body`.
#[derive(Debug, Default)]
pub struct ShellFeatureExecutor;

impl Feature for ShellFeatureExecutor {
    fn feature_type(&self) -> &'static str {
        "shell"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Shell(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected shell feature, got {}",
                node.definition.feature_type()
            )));
        };
        def.validate()?;

        let body = ctx.body_for_feature(def.target_feature.as_str())?;
        let discoveries = ctx.face_discoveries_on(&body)?;
        // Fail closed: opening the wrong face silently changes the part, so
        // there is no role fallback (ADR-017 §4).
        let open_faces = def
            .open_face_refs
            .iter()
            .map(|face_ref| {
                let unresolved = |reason: String| {
                    OpenCadError::validation(format!(
                        "shell open face '{face_ref}' did not resolve to a face: {reason}"
                    ))
                };
                let kernel_face_id = if discoveries.is_empty() {
                    resolve_kernel_face_id_for_topo_ref_with_discoveries(
                        ctx.semantic_refs(),
                        ctx.face_history(),
                        face_ref,
                        None,
                    )
                } else {
                    resolve_face_on_body(ctx, face_ref, &discoveries)
                }
                .map_err(|error| unresolved(error.to_string()))?;
                // The kernel matches faces by geometry, so the resolved face
                // must come with its discovered point and normal.
                let discovery = discoveries
                    .iter()
                    .find(|discovery| discovery.kernel_face_id == kernel_face_id)
                    .ok_or_else(|| {
                        unresolved(format!(
                            "no discovered geometry for kernel face {kernel_face_id}"
                        ))
                    })?;
                Ok(FacePick {
                    point_m: discovery.centroid_m.map(f64::from),
                    normal: discovery.normal_m.map(f64::from),
                })
            })
            .collect::<Result<Vec<FacePick>>>()?;
        let result = ctx
            .kernel()
            .shell_body(body, def.thickness.meters(), &open_faces)?;
        Ok(FeatureOutput { body: Some(result) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regenerate::TestRegenContext;
    use opencad_geometry::KernelBody;

    fn shell(thickness_m: f64, open_face_refs: &[&str]) -> ShellFeature {
        ShellFeature::new(
            "feature:base",
            Length::from_meters(thickness_m),
            None,
            open_face_refs.iter().map(|r| r.to_string()).collect(),
        )
    }

    #[test]
    fn validation_rejects_thin_walls_and_bad_open_faces() {
        assert!(shell(0.002, &["ref:face:top"]).validate().is_ok());
        for (def, message) in [
            (shell(1e-7, &["ref:face:top"]), "greater than"),
            (shell(f64::NAN, &["ref:face:top"]), "greater than"),
            (shell(0.002, &[]), "at least one open face"),
            (shell(0.002, &["ref:edge:top"]), "'ref:face:' reference"),
            (
                shell(0.002, &["ref:face:top", "ref:face:top"]),
                "listed twice",
            ),
        ] {
            let error = def.validate().expect_err(message);
            assert!(error.to_string().contains(message), "{error}");
        }
    }

    #[test]
    fn unresolved_open_face_fails_closed() {
        let ctx = TestRegenContext::with_body("feature:base", KernelBody::new(42));
        let node = FeatureNode::new(
            "feature:shell",
            "Shell",
            FeatureDefinition::Shell(shell(0.002, &["ref:face:missing"])),
        );
        let error = ShellFeatureExecutor
            .execute(&node, &ctx)
            .expect_err("unresolved face");
        assert!(error
            .to_string()
            .contains("shell open face 'ref:face:missing' did not resolve"));
    }
}
