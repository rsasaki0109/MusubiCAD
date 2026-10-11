//! Explicit operations between two existing body outputs (MCAD-P11-006).
use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};
use opencad_core::{OpenCadError, Result};
use opencad_geometry::BooleanOp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyBooleanFeature {
    /// Existing tool body. Subtraction removes this body from the target.
    pub source_feature: String,
    pub target_feature: String,
    pub operation: BooleanOp,
}

impl BodyBooleanFeature {
    pub fn validate(&self) -> Result<()> {
        for id in [&self.source_feature, &self.target_feature] {
            if !id.starts_with("feature:") || id.len() <= "feature:".len() {
                return Err(OpenCadError::validation(
                    "body_boolean inputs must be feature IDs",
                ));
            }
        }
        if self.source_feature == self.target_feature {
            return Err(OpenCadError::validation(
                "body_boolean needs two distinct feature inputs",
            ));
        }
        Ok(())
    }
}

pub struct BodyBooleanExecutor;
impl Feature for BodyBooleanExecutor {
    fn feature_type(&self) -> &'static str {
        "body_boolean"
    }
    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::BodyBoolean(def) = &node.definition else {
            return Err(OpenCadError::validation("expected body_boolean"));
        };
        def.validate()?;
        let target = ctx.body_for_feature(&def.target_feature)?;
        let source = ctx.body_for_feature(&def.source_feature)?;
        let body = ctx.kernel().body_boolean(target, source, def.operation)?;
        Ok(FeatureOutput {
            body: Some(body),
            datum: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn definitions_validate_and_round_trip_without_implicit_operands() {
        for operation in [BooleanOp::Union, BooleanOp::Subtract, BooleanOp::Intersect] {
            let def = BodyBooleanFeature {
                source_feature: "feature:source".into(),
                target_feature: "feature:target".into(),
                operation,
            };
            def.validate().expect("distinct inputs");
            let value = serde_json::to_value(&def).expect("json");
            assert_eq!(def, serde_json::from_value(value).expect("round trip"));
            let mut bad = def.clone();
            bad.source_feature = bad.target_feature.clone();
            assert!(bad.validate().is_err());
            bad.source_feature = "feature:".into();
            assert!(bad.validate().is_err());
        }
        assert!(serde_json::from_value::<BodyBooleanFeature>(serde_json::json!({"source_feature":"feature:a","target_feature":"feature:b","operation":"unknown"})).is_err());
    }
}
