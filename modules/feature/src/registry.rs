//! Feature type registry (Task-087+).

use std::collections::HashMap;

use opencad_core::{OpenCadError, Result};

use crate::chamfer::ChamferFeatureExecutor;
use crate::extrude::ExtrudeFeatureExecutor;
use crate::feature::{Feature, FeatureNode, FeatureOutput, RegenContext};
use crate::fillet::FilletFeatureExecutor;
use crate::hole::HoleFeatureExecutor;
use crate::pattern::{
    CircularPatternFeatureExecutor, LinearPatternFeatureExecutor, MirrorPatternFeatureExecutor,
};
use crate::sketch_feature::SketchFeature;

/// Maps feature type names to executors.
#[derive(Default)]
pub struct FeatureRegistry {
    executors: HashMap<&'static str, Box<dyn Feature>>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(SketchFeature));
        registry.register(Box::new(ExtrudeFeatureExecutor));
        registry.register(Box::new(HoleFeatureExecutor));
        registry.register(Box::new(FilletFeatureExecutor));
        registry.register(Box::new(ChamferFeatureExecutor));
        registry.register(Box::new(LinearPatternFeatureExecutor));
        registry.register(Box::new(CircularPatternFeatureExecutor));
        registry.register(Box::new(MirrorPatternFeatureExecutor));
        registry
    }

    pub fn register(&mut self, executor: Box<dyn Feature>) {
        self.executors
            .insert(executor.feature_type(), executor);
    }

    pub fn execute(
        &self,
        node: &FeatureNode,
        ctx: &dyn RegenContext,
    ) -> Result<FeatureOutput> {
        let feature_type = node.definition.feature_type();
        let executor = self.executors.get(feature_type).ok_or_else(|| {
            OpenCadError::validation(format!("unknown feature type '{feature_type}'"))
        })?;
        executor.execute(node, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::{FeatureDefinition, FeatureNode};
    use crate::regenerate::TestRegenContext;
    use crate::sketch_feature::SketchFeatureDef;

    #[test]
    fn default_registry_has_sketch_and_extrude() {
        let registry = FeatureRegistry::with_defaults();
        let node = FeatureNode::new(
            "feature:sketch_base",
            "Sketch",
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: "sketch:base".into(),
            }),
        );
        let ctx = TestRegenContext::empty();
        let output = registry.execute(&node, &ctx).expect("execute");
        assert!(output.body.is_none());
    }
}
