//! DesignPatch operations (Task-142+).

use opencad_core::{OpenCadError, Result, TopoRefId};
use opencad_feature::{FeatureDefinition, FeatureNode};
use opencad_geometry::{assign_named_face_ref, TopoRef};
use opencad_graph::ParamGraph;
use serde::{Deserialize, Serialize};

/// Supported feature expression fields for patch operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureExprField {
    LengthExpr,
    DepthExpr,
    RadiusExpr,
    DistanceExpr,
    SpacingExpr,
}

/// Supported semantic ref fields for patch operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureRefField {
    PlaneFaceRef,
    FaceRef,
}

impl FeatureRefField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlaneFaceRef => "plane_face_ref",
            Self::FaceRef => "face_ref",
        }
    }

    pub fn parse(field: &str) -> Result<Self> {
        match field {
            "plane_face_ref" => Ok(Self::PlaneFaceRef),
            "face_ref" => Ok(Self::FaceRef),
            _ => Err(OpenCadError::validation(format!(
                "unsupported feature ref field '{field}'; expected 'plane_face_ref' or 'face_ref'"
            ))),
        }
    }
}

impl FeatureExprField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LengthExpr => "length_expr",
            Self::DepthExpr => "depth_expr",
            Self::RadiusExpr => "radius_expr",
            Self::DistanceExpr => "distance_expr",
            Self::SpacingExpr => "spacing_expr",
        }
    }

    pub fn parse(field: &str) -> Result<Self> {
        match field {
            "length_expr" => Ok(Self::LengthExpr),
            "depth_expr" => Ok(Self::DepthExpr),
            "radius_expr" => Ok(Self::RadiusExpr),
            "distance_expr" => Ok(Self::DistanceExpr),
            "spacing_expr" => Ok(Self::SpacingExpr),
            _ => Err(OpenCadError::validation(format!(
                "unsupported feature field '{field}'; expected 'length_expr', 'depth_expr', 'radius_expr', 'distance_expr', or 'spacing_expr'"
            ))),
        }
    }
}

/// A single patch operation against design intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchOperation {
    SetParameter {
        id: String,
        expr: String,
    },
    SetFeatureExpr {
        feature_id: String,
        field: String,
        expr: String,
    },
    SetFeatureRef {
        feature_id: String,
        field: String,
        ref_id: String,
    },
    AssignFaceRef {
        ref_id: String,
        #[serde(default)]
        kernel_face_id: u64,
        created_by: String,
        role: String,
        #[serde(default = "default_normal_up")]
        normal_m: [f32; 3],
    },
    SetInstancePlacement {
        instance_id: String,
        translation_m: [f64; 3],
        rotation: [[f64; 3]; 3],
    },
    SetMateDistance {
        mate_id: String,
        distance_m: f64,
    },
    AddConnector {
        id: String,
        name: String,
        instance_id: String,
        transform: opencad_geometry::RigidTransform,
    },
    SetDrawingViewScale {
        view_id: String,
        scale: f64,
    },
    SetDrawingViewOrigin {
        view_id: String,
        origin_on_sheet_m: [f64; 2],
    },
}

/// State assertion that must hold before any patch operation is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchPrecondition {
    /// Require a parameter to exist with the exact source expression.
    ParameterExprEquals { id: String, expr: String },
    /// Require a feature node to exist.
    FeatureExists { id: String },
    /// Require a semantic topology reference to exist.
    TopoRefExists { ref_id: String },
}

/// Reviewable effect that should be verified after regeneration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExpectedEffect {
    /// Require a parameter expression in the resulting Design Graph.
    ParameterExprEquals { id: String, expr: String },
    /// Require regenerated mass delta to remain inside an inclusive kilogram range.
    MassDeltaKg { min: f64, max: f64 },
    /// Require drawing graph data to change or remain unchanged.
    DrawingChanged { expected: bool },
    /// Require the resulting assembly to contain no interference.
    NoAssemblyInterference,
}

fn default_normal_up() -> [f32; 3] {
    [0.0, 0.0, 1.0]
}

/// Semantic patch applied by agents or CLI tooling.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignPatch {
    /// Short human-readable statement of the requested design change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// Explanation of why the change is proposed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Assertions protecting the patch from stale or incompatible design state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<PatchPrecondition>,
    /// Post-regeneration effects checked by the review workflow.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_effects: Vec<ExpectedEffect>,
    pub operations: Vec<PatchOperation>,
}

impl DesignPatch {
    pub fn new(operations: Vec<PatchOperation>) -> Self {
        Self {
            operations,
            ..Self::default()
        }
    }

    /// Attach review intent, rationale, preconditions, and expected effects.
    pub fn with_review_metadata(
        mut self,
        intent: impl Into<String>,
        rationale: impl Into<String>,
        preconditions: Vec<PatchPrecondition>,
        expected_effects: Vec<ExpectedEffect>,
    ) -> Self {
        self.intent = Some(intent.into());
        self.rationale = Some(rationale.into());
        self.preconditions = preconditions;
        self.expected_effects = expected_effects;
        self
    }

    pub fn set_parameter(id: impl Into<String>, expr: impl Into<String>) -> Self {
        Self {
            operations: vec![PatchOperation::SetParameter {
                id: id.into(),
                expr: expr.into(),
            }],
            ..Self::default()
        }
    }

    pub fn set_parameters(
        operations: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            operations: operations
                .into_iter()
                .map(|(id, expr)| PatchOperation::SetParameter {
                    id: id.into(),
                    expr: expr.into(),
                })
                .collect(),
            ..Self::default()
        }
    }

    pub fn set_feature_expr(
        feature_id: impl Into<String>,
        field: FeatureExprField,
        expr: impl Into<String>,
    ) -> Self {
        Self {
            operations: vec![PatchOperation::SetFeatureExpr {
                feature_id: feature_id.into(),
                field: field.as_str().to_string(),
                expr: expr.into(),
            }],
            ..Self::default()
        }
    }

    pub fn set_feature_ref(
        feature_id: impl Into<String>,
        field: FeatureRefField,
        ref_id: impl Into<String>,
    ) -> Self {
        Self {
            operations: vec![PatchOperation::SetFeatureRef {
                feature_id: feature_id.into(),
                field: field.as_str().to_string(),
                ref_id: ref_id.into(),
            }],
            ..Self::default()
        }
    }

    pub fn assign_face_ref(
        ref_id: impl Into<String>,
        created_by: impl Into<String>,
        role: impl Into<String>,
    ) -> Self {
        Self {
            operations: vec![PatchOperation::AssignFaceRef {
                ref_id: ref_id.into(),
                kernel_face_id: 0,
                created_by: created_by.into(),
                role: role.into(),
                normal_m: default_normal_up(),
            }],
            ..Self::default()
        }
    }

    /// Verify every precondition against the unmodified Design Graph state.
    pub fn validate_preconditions(
        &self,
        parameters: &ParamGraph,
        feature_nodes: &[FeatureNode],
        semantic_refs: &[TopoRef],
    ) -> Result<()> {
        for precondition in &self.preconditions {
            match precondition {
                PatchPrecondition::ParameterExprEquals { id, expr } => {
                    let actual = parameters.get(id).ok_or_else(|| {
                        OpenCadError::validation(format!(
                            "patch precondition failed: parameter '{id}' does not exist"
                        ))
                    })?;
                    if actual.expr != *expr {
                        return Err(OpenCadError::validation(format!(
                            "patch precondition failed: parameter '{id}' expected expression '{expr}', found '{}'",
                            actual.expr
                        )));
                    }
                }
                PatchPrecondition::FeatureExists { id } => {
                    if !feature_nodes.iter().any(|node| node.id == *id) {
                        return Err(OpenCadError::validation(format!(
                            "patch precondition failed: feature '{id}' does not exist"
                        )));
                    }
                }
                PatchPrecondition::TopoRefExists { ref_id } => {
                    if !semantic_refs
                        .iter()
                        .any(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
                    {
                        return Err(OpenCadError::validation(format!(
                            "patch precondition failed: topology reference '{ref_id}' does not exist"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn apply_to_parameters(&self, graph: &mut ParamGraph) -> Result<()> {
        for operation in &self.operations {
            match operation {
                PatchOperation::SetParameter { id, expr } => {
                    graph.set_expr(id, expr.as_str()).map_err(|_| {
                        OpenCadError::validation(format!("unknown parameter '{id}'"))
                    })?;
                }
                PatchOperation::SetFeatureExpr { .. } => {}
                PatchOperation::SetFeatureRef { .. } => {}
                PatchOperation::AssignFaceRef { .. } => {}
                PatchOperation::SetInstancePlacement { .. }
                | PatchOperation::SetMateDistance { .. }
                | PatchOperation::AddConnector { .. }
                | PatchOperation::SetDrawingViewScale { .. }
                | PatchOperation::SetDrawingViewOrigin { .. } => {}
            }
        }
        Ok(())
    }

    pub fn apply_to_semantic_refs(&self, semantic_refs: &mut Vec<TopoRef>) -> Result<()> {
        for operation in &self.operations {
            let PatchOperation::AssignFaceRef {
                ref_id,
                kernel_face_id,
                created_by,
                role,
                normal_m,
            } = operation
            else {
                continue;
            };
            let topo_ref_id = TopoRefId::new(ref_id)?;
            let kernel_face_id = (*kernel_face_id != 0).then_some(*kernel_face_id);
            assign_named_face_ref(
                semantic_refs,
                topo_ref_id,
                created_by,
                role,
                kernel_face_id,
                *normal_m,
            )?;
        }
        Ok(())
    }

    pub fn apply_to_features(&self, feature_nodes: &mut [FeatureNode]) -> Result<()> {
        for operation in &self.operations {
            match operation {
                PatchOperation::SetFeatureExpr {
                    feature_id,
                    field,
                    expr,
                } => {
                    let field = FeatureExprField::parse(field)?;
                    let node = feature_nodes
                        .iter_mut()
                        .find(|node| node.id == *feature_id)
                        .ok_or_else(|| {
                            OpenCadError::validation(format!("unknown feature '{feature_id}'"))
                        })?;
                    apply_feature_expr(node, field, expr)?;
                }
                PatchOperation::SetFeatureRef {
                    feature_id,
                    field,
                    ref_id,
                } => {
                    let field = FeatureRefField::parse(field)?;
                    let node = feature_nodes
                        .iter_mut()
                        .find(|node| node.id == *feature_id)
                        .ok_or_else(|| {
                            OpenCadError::validation(format!("unknown feature '{feature_id}'"))
                        })?;
                    apply_feature_ref(node, field, ref_id)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn apply_to_document(
        &self,
        parameters: &mut ParamGraph,
        feature_nodes: &mut [FeatureNode],
        semantic_refs: &mut Vec<TopoRef>,
        assembly: Option<&mut opencad_assembly::AssemblyModel>,
        drawing: Option<&mut opencad_drawing::DrawingModel>,
    ) -> Result<()> {
        self.validate_preconditions(parameters, feature_nodes, semantic_refs)?;
        self.apply_to_parameters(parameters)?;
        self.apply_to_features(feature_nodes)?;
        self.apply_to_semantic_refs(semantic_refs)?;
        if let Some(assembly) = assembly {
            crate::assembly::apply_assembly_patch(assembly, &self.operations)?;
        }
        if let Some(drawing) = drawing {
            crate::drawing::apply_drawing_patch(drawing, &self.operations)?;
        }
        Ok(())
    }

    pub fn has_assign_face_ref(&self) -> bool {
        self.operations
            .iter()
            .any(|op| matches!(op, PatchOperation::AssignFaceRef { .. }))
    }
}

fn apply_feature_ref(node: &mut FeatureNode, field: FeatureRefField, ref_id: &str) -> Result<()> {
    match (&mut node.definition, field) {
        (FeatureDefinition::MirrorPattern(pattern), FeatureRefField::PlaneFaceRef) => {
            pattern.plane_face_ref = Some(ref_id.to_string());
            Ok(())
        }
        (FeatureDefinition::Hole(hole), FeatureRefField::FaceRef) => {
            hole.face_ref = Some(ref_id.to_string());
            Ok(())
        }
        (definition, field) => Err(OpenCadError::validation(format!(
            "feature '{}' ({}) does not support '{}'",
            node.id,
            definition.feature_type(),
            field.as_str()
        ))),
    }
}

fn apply_feature_expr(node: &mut FeatureNode, field: FeatureExprField, expr: &str) -> Result<()> {
    match (&mut node.definition, field) {
        (FeatureDefinition::Extrude(extrude), FeatureExprField::LengthExpr) => {
            extrude.length_expr = Some(expr.to_string());
            Ok(())
        }
        (FeatureDefinition::Hole(hole), FeatureExprField::DepthExpr) => {
            hole.depth_expr = Some(expr.to_string());
            Ok(())
        }
        (FeatureDefinition::Fillet(fillet), FeatureExprField::RadiusExpr) => {
            fillet.radius_expr = Some(expr.to_string());
            Ok(())
        }
        (FeatureDefinition::Chamfer(chamfer), FeatureExprField::DistanceExpr) => {
            chamfer.distance_expr = Some(expr.to_string());
            Ok(())
        }
        (FeatureDefinition::LinearPattern(pattern), FeatureExprField::SpacingExpr) => {
            pattern.spacing_expr = Some(expr.to_string());
            Ok(())
        }
        (definition, field) => Err(OpenCadError::validation(format!(
            "feature '{}' ({}) does not support '{}'",
            node.id,
            definition.feature_type(),
            field.as_str()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_feature::{
        bracket_with_hole, bracket_with_top_chamfer, bracket_with_top_fillet, FeatureDefinition,
        FeatureNode, LinearPatternFeature, MirrorPatternFeature,
    };
    use opencad_graph::{bracket_parameters, evaluate_param_graph};

    #[test]
    fn set_parameter_patch_updates_graph() {
        let mut params = bracket_parameters();
        let patch = DesignPatch::set_parameter("param:width", "100 mm");
        patch.apply_to_parameters(&mut params).expect("patch");
        let values = evaluate_param_graph(&params).expect("eval");
        assert!((values["width"] - 0.1).abs() < 1e-9);
    }

    #[test]
    fn set_parameters_applies_multiple_values() {
        let mut params = bracket_parameters();
        let patch =
            DesignPatch::set_parameters([("param:width", "100 mm"), ("param:thickness", "8 mm")]);
        patch.apply_to_parameters(&mut params).expect("patch");
        let values = evaluate_param_graph(&params).expect("eval");
        assert!((values["width"] - 0.1).abs() < 1e-9);
        assert!((values["thickness"] - 0.008).abs() < 1e-9);
    }

    #[test]
    fn set_feature_expr_updates_extrude_length_expr() {
        let part = bracket_with_hole().expect("model");
        let mut nodes: Vec<FeatureNode> = part.nodes.into_values().collect();
        let patch = DesignPatch::set_feature_expr(
            "feature:extrude_base",
            FeatureExprField::LengthExpr,
            "thickness * 2",
        );
        patch.apply_to_features(&mut nodes).expect("patch");
        let node = nodes
            .iter()
            .find(|node| node.id == "feature:extrude_base")
            .expect("extrude");
        let FeatureDefinition::Extrude(extrude) = &node.definition else {
            panic!("expected extrude");
        };
        assert_eq!(extrude.length_expr.as_deref(), Some("thickness * 2"));
    }

    #[test]
    fn set_feature_expr_rejects_unsupported_field() {
        let part = bracket_with_hole().expect("model");
        let mut nodes: Vec<FeatureNode> = part.nodes.into_values().collect();
        let patch = DesignPatch::set_feature_expr(
            "feature:hole_mount",
            FeatureExprField::LengthExpr,
            "thickness",
        );
        let err = patch.apply_to_features(&mut nodes).expect_err("field");
        assert!(err.to_string().contains("does not support"));
    }

    #[test]
    fn set_feature_expr_updates_fillet_radius_expr() {
        let part = bracket_with_top_fillet().expect("model");
        let mut nodes: Vec<FeatureNode> = part.nodes.into_values().collect();
        let patch = DesignPatch::set_feature_expr(
            "feature:fillet_top",
            FeatureExprField::RadiusExpr,
            "fillet_radius * 2",
        );
        patch.apply_to_features(&mut nodes).expect("patch");
        let node = nodes
            .iter()
            .find(|node| node.id == "feature:fillet_top")
            .expect("fillet");
        let FeatureDefinition::Fillet(fillet) = &node.definition else {
            panic!("expected fillet");
        };
        assert_eq!(fillet.radius_expr.as_deref(), Some("fillet_radius * 2"));
    }

    #[test]
    fn set_feature_expr_updates_chamfer_distance_expr() {
        let part = bracket_with_top_chamfer().expect("model");
        let mut nodes: Vec<FeatureNode> = part.nodes.into_values().collect();
        let patch = DesignPatch::set_feature_expr(
            "feature:chamfer_top",
            FeatureExprField::DistanceExpr,
            "chamfer_distance * 2",
        );
        patch.apply_to_features(&mut nodes).expect("patch");
        let node = nodes
            .iter()
            .find(|node| node.id == "feature:chamfer_top")
            .expect("chamfer");
        let FeatureDefinition::Chamfer(chamfer) = &node.definition else {
            panic!("expected chamfer");
        };
        assert_eq!(
            chamfer.distance_expr.as_deref(),
            Some("chamfer_distance * 2")
        );
    }

    #[test]
    fn set_feature_expr_updates_linear_pattern_spacing_expr() {
        let mut nodes = vec![FeatureNode::new(
            "feature:hole_row",
            "Hole Row",
            FeatureDefinition::LinearPattern(LinearPatternFeature::new(
                "feature:hole_mount",
                [1.0, 0.0, 0.0],
                opencad_core::Length::from_meters(0.01),
                3,
            )),
        )];
        let patch = DesignPatch::set_feature_expr(
            "feature:hole_row",
            FeatureExprField::SpacingExpr,
            "hole_pitch",
        );
        patch.apply_to_features(&mut nodes).expect("patch");
        let FeatureDefinition::LinearPattern(pattern) = &nodes[0].definition else {
            panic!("expected linear pattern");
        };
        assert_eq!(pattern.spacing_expr.as_deref(), Some("hole_pitch"));
    }

    #[test]
    fn set_feature_ref_updates_mirror_plane_face_ref() {
        let mut nodes = vec![FeatureNode::new(
            "feature:pin_mirror",
            "Pin Mirror",
            FeatureDefinition::MirrorPattern(MirrorPatternFeature::new(
                "feature:pin_tool",
                [0.04, 0.0, 0.0],
                [1.0, 0.0, 0.0],
            )),
        )];
        let patch = DesignPatch::set_feature_ref(
            "feature:pin_mirror",
            FeatureRefField::PlaneFaceRef,
            "ref:face:bracket_top",
        );
        patch.apply_to_features(&mut nodes).expect("patch");
        let FeatureDefinition::MirrorPattern(pattern) = &nodes[0].definition else {
            panic!("expected mirror pattern");
        };
        assert_eq!(
            pattern.plane_face_ref.as_deref(),
            Some("ref:face:bracket_top")
        );
    }

    #[test]
    fn assign_face_ref_adds_semantic_ref() {
        let part = bracket_with_hole().expect("model");
        let mut params = bracket_parameters();
        let mut nodes: Vec<FeatureNode> = part.nodes.into_values().collect();
        let mut semantic_refs = Vec::new();
        let patch =
            DesignPatch::assign_face_ref("ref:face:bracket_top", "feature:extrude_base", "top");
        patch
            .apply_to_document(&mut params, &mut nodes, &mut semantic_refs, None, None)
            .expect("patch");
        assert_eq!(semantic_refs.len(), 1);
        assert_eq!(semantic_refs[0].ref_id.as_str(), "ref:face:bracket_top");
        assert_eq!(semantic_refs[0].semantic.role.as_deref(), Some("top"));
    }

    #[test]
    fn stale_parameter_precondition_rejects_patch_before_mutation() {
        let mut params = bracket_parameters();
        let mut nodes = Vec::new();
        let mut refs = Vec::new();
        let patch = DesignPatch::set_parameter("param:width", "100 mm").with_review_metadata(
            "Increase width",
            "Fit the larger enclosure",
            vec![PatchPrecondition::ParameterExprEquals {
                id: "param:width".into(),
                expr: "75 mm".into(),
            }],
            vec![ExpectedEffect::ParameterExprEquals {
                id: "param:width".into(),
                expr: "100 mm".into(),
            }],
        );
        patch
            .apply_to_document(&mut params, &mut nodes, &mut refs, None, None)
            .expect_err("stale patch");
        assert_eq!(params.get("param:width").expect("width").expr, "80 mm");
    }

    #[test]
    fn legacy_operations_only_patch_deserializes() {
        let patch: DesignPatch = serde_json::from_str(
            r#"{"operations":[{"type":"set_parameter","id":"param:width","expr":"100 mm"}]}"#,
        )
        .expect("legacy patch");
        assert!(patch.intent.is_none());
        assert!(patch.preconditions.is_empty());
        assert_eq!(patch.operations.len(), 1);
    }
}
