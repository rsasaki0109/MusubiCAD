//! Imported STEP solids as fixed design inputs (ADR-016).

use opencad_core::{sha256_hex, OpenCadError, Result};
use opencad_geometry::{BooleanOp, ExtrudeOperation, RigidTransform};
use serde::{Deserialize, Serialize};

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};

/// Orthonormality and determinant tolerance for imported-solid rotations
/// (dimensionless).
pub const ROTATION_TOLERANCE: f64 = 1e-9;

fn identity() -> RigidTransform {
    RigidTransform::identity()
}

/// A solid imported from a STEP attachment of the document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedSolidFeature {
    /// Attachment path, e.g. `imports/motor.step`.
    pub source: String,
    /// Lowercase hex SHA-256 of the attachment bytes the feature expects.
    pub sha256: String,
    /// Rigid placement applied after import (metres, dimensionless rotation).
    #[serde(default = "identity")]
    pub transform: RigidTransform,
    pub operation: ExtrudeOperation,
    /// Body the solid joins or cuts; required for `join` and `cut`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_feature: Option<String>,
}

impl ImportedSolidFeature {
    /// Validate the parts of the definition that need no other state:
    /// digest format, finite placement, and a proper rotation.
    pub fn validate(&self) -> Result<()> {
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(OpenCadError::validation(format!(
                "imported solid sha256 must be 64 lowercase hex characters, got '{}'",
                self.sha256
            )));
        }
        let translation = self.transform.translation_m;
        let rotation = self.transform.rotation;
        if !translation
            .iter()
            .chain(rotation.iter().flatten())
            .all(|v| v.is_finite())
        {
            return Err(OpenCadError::validation(
                "imported solid transform must be finite",
            ));
        }
        for i in 0..3 {
            for j in 0..3 {
                let dot: f64 = (0..3).map(|k| rotation[k][i] * rotation[k][j]).sum();
                let expected = if i == j { 1.0 } else { 0.0 };
                if (dot - expected).abs() > ROTATION_TOLERANCE {
                    return Err(OpenCadError::validation(format!(
                        "imported solid rotation must be orthonormal within {ROTATION_TOLERANCE:e}"
                    )));
                }
            }
        }
        let [a, b, c] = rotation;
        let determinant = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
        if (determinant - 1.0).abs() > ROTATION_TOLERANCE {
            return Err(OpenCadError::validation(
                "imported solid rotation must be proper (determinant +1); mirroring is not supported",
            ));
        }
        match (self.operation, &self.target_feature) {
            (ExtrudeOperation::NewBody, _) | (_, Some(_)) => Ok(()),
            (operation, None) => Err(OpenCadError::validation(format!(
                "imported solid {operation:?} requires target_feature"
            ))),
        }
    }
}

/// Executor for [`ImportedSolidFeature`].
#[derive(Debug, Default)]
pub struct ImportedSolidExecutor;

impl Feature for ImportedSolidExecutor {
    fn feature_type(&self) -> &'static str {
        "imported_solid"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::ImportedSolid(def) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected imported_solid feature, got {}",
                node.definition.feature_type()
            )));
        };
        def.validate()?;
        let bytes = ctx.attachment(&def.source)?;
        let actual = sha256_hex(bytes);
        if actual != def.sha256 {
            return Err(OpenCadError::validation(format!(
                "attachment '{}' has sha256 {actual}, but feature '{}' expects {}",
                def.source, node.id, def.sha256
            )));
        }
        let kernel = ctx.kernel();
        let imported = kernel.import_step(bytes)?;
        let placed = kernel.transform_body(imported, def.transform)?;
        let body = match (def.operation, &def.target_feature) {
            (ExtrudeOperation::NewBody, _) => placed,
            (operation, Some(target)) => {
                let target = ctx.body_for_feature(target)?;
                let op = if operation == ExtrudeOperation::Join {
                    BooleanOp::Union
                } else {
                    BooleanOp::Subtract
                };
                kernel.boolean(target, placed, op)?
            }
            (_, None) => unreachable!("validated above"),
        };
        Ok(FeatureOutput { body: Some(body) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature() -> ImportedSolidFeature {
        ImportedSolidFeature {
            source: "imports/motor.step".into(),
            sha256: "a".repeat(64),
            transform: RigidTransform::identity(),
            operation: ExtrudeOperation::NewBody,
            target_feature: None,
        }
    }

    #[test]
    fn validation_rejects_bad_digests_improper_rotations_and_missing_targets() {
        assert!(feature().validate().is_ok());

        let mut digest = feature();
        digest.sha256 = "ABC".into();
        assert!(digest.validate().is_err());

        let mut mirrored = feature();
        mirrored.transform.rotation = [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert!(mirrored
            .validate()
            .unwrap_err()
            .to_string()
            .contains("determinant +1"));

        let mut scaled = feature();
        scaled.transform.rotation = [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert!(scaled
            .validate()
            .unwrap_err()
            .to_string()
            .contains("orthonormal"));

        let mut cut = feature();
        cut.operation = ExtrudeOperation::Cut;
        assert!(cut
            .validate()
            .unwrap_err()
            .to_string()
            .contains("target_feature"));
        cut.target_feature = Some("feature:plate".into());
        assert!(cut.validate().is_ok());
    }
}
