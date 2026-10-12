//! Immutable explicit-unit STL solid input (ADR-049).

use opencad_core::{sha256_hex, LengthUnit, OpenCadError, Result};
use opencad_geometry::{read_stl_solid, ExtrudeOperation, RigidTransform};
use serde::{Deserialize, Serialize};

use crate::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedMeshFeature {
    pub source: String,
    pub sha256: String,
    pub source_unit: LengthUnit,
    pub transform: RigidTransform,
}

impl ImportedMeshFeature {
    pub fn validate(&self) -> Result<()> {
        let valid_source = self
            .source
            .strip_prefix("imports/")
            .and_then(|name| name.strip_suffix(".stl"))
            .is_some_and(|stem| {
                !stem.is_empty()
                    && stem.split(['.', '-']).all(|part| {
                        !part.is_empty()
                            && part
                                .bytes()
                                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    })
            });
        if !valid_source {
            return Err(OpenCadError::validation(
                "imported mesh requires an imports/*.stl attachment",
            ));
        }
        crate::ImportedSolidFeature {
            source: self.source.clone(),
            sha256: self.sha256.clone(),
            transform: self.transform,
            operation: ExtrudeOperation::NewBody,
            target_feature: None,
        }
        .validate()
    }
}

#[derive(Debug, Default)]
pub struct ImportedMeshExecutor;

impl Feature for ImportedMeshExecutor {
    fn feature_type(&self) -> &'static str {
        "imported_mesh"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::ImportedMesh(def) = &node.definition else {
            return Err(OpenCadError::validation("expected imported_mesh feature"));
        };
        def.validate()?;
        let bytes = ctx.attachment(&def.source)?;
        if sha256_hex(bytes) != def.sha256 {
            return Err(OpenCadError::validation(
                "imported mesh attachment digest differs",
            ));
        }
        let mesh = read_stl_solid(bytes, def.source_unit)?.transformed(def.transform)?;
        let body = ctx.kernel().import_triangle_solid(&mesh)?;
        Ok(FeatureOutput {
            body: Some(body),
            datum: None,
        })
    }
}
