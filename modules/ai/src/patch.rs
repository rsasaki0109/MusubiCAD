//! DesignPatch operations (Task-142+).

use std::collections::BTreeSet;

use opencad_core::{Assertion, AssertionKind, OpenCadError, Result, TopoRefId};
use opencad_feature::{FeatureDefinition, FeatureNode};
use opencad_geometry::{assign_named_face_ref, TopoRef};
use opencad_graph::{evaluate_param_graph, parameter_names_in_expr, ParamGraph, ParameterEntry};
use opencad_sketch::{Constraint, SketchEntity, Workplane};
use serde::{Deserialize, Serialize};

use crate::feature_patch::{
    apply_feature_operations, changes_feature_graph, validate_feature_candidate, FeaturePosition,
};
use crate::impact::serialized_value_uses_parameter;
use crate::sketch_patch::{apply_sketch_operations, validate_sketch_candidate};
use crate::state::{
    design_state_revision, design_state_revision_for_version, DesignState,
    DESIGN_STATE_REVISION_ALGORITHM, DESIGN_STATE_REVISION_VERSION,
    DESIGN_STATE_REVISION_VERSION_V1, DESIGN_STATE_REVISION_VERSION_V2,
};

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
    /// Create a parameter with an author-chosen stable ID (ADR-013).
    AddParameter {
        id: String,
        name: String,
        expr: String,
    },
    /// Remove a parameter; fails if anything still references its name.
    RemoveParameter {
        id: String,
    },
    /// Create a design assertion with an author-chosen stable ID (ADR-013).
    AddAssertion {
        assertion: Assertion,
    },
    /// Remove a design assertion by stable ID.
    RemoveAssertion {
        id: String,
    },
    /// Create an empty sketch on a workplane (ADR-013).
    AddSketch {
        id: String,
        name: String,
        workplane: Workplane,
    },
    /// Remove a sketch; fails while a sketch feature still uses it.
    RemoveSketch {
        id: String,
    },
    /// Add an entity to a sketch.  Rectangle corner and edge IDs are
    /// author-chosen like the entity ID itself.
    AddSketchEntity {
        sketch_id: String,
        entity: SketchEntity,
    },
    /// Remove an entity; fails while an entity or constraint references it.
    RemoveSketchEntity {
        sketch_id: String,
        entity_id: String,
    },
    /// Add a constraint to a sketch.
    AddSketchConstraint {
        sketch_id: String,
        constraint: Constraint,
    },
    /// Remove a constraint from a sketch.
    RemoveSketchConstraint {
        sketch_id: String,
        constraint_id: String,
    },
    /// Create a feature at a display position (ADR-013).  Its dependency
    /// edges are derived from the definition, never authored.
    AddFeature {
        node: FeatureNode,
        position: FeaturePosition,
    },
    /// Remove a feature; fails while any feature or reference still uses it.
    RemoveFeature {
        id: String,
    },
    /// Move a feature in the display order; its inputs must stay before it.
    MoveFeature {
        id: String,
        position: FeaturePosition,
    },
    /// Suppress or unsuppress a feature.
    SetFeatureSuppressed {
        id: String,
        suppressed: bool,
    },
    /// Replace a feature definition with one of the same feature type.
    ReplaceFeatureDefinition {
        id: String,
        definition: FeatureDefinition,
    },
    /// Create a semantic topology reference with its full stored shape.
    AddSemanticRef {
        topo_ref: TopoRef,
    },
    /// Remove a semantic topology reference nothing consumes.
    RemoveSemanticRef {
        ref_id: String,
    },
}

impl PatchOperation {
    /// Whether this operation creates or removes Design Graph structure
    /// (ADR-013) rather than editing an existing value.
    pub fn is_structural(&self) -> bool {
        matches!(
            self,
            Self::AddParameter { .. }
                | Self::RemoveParameter { .. }
                | Self::AddAssertion { .. }
                | Self::RemoveAssertion { .. }
                | Self::AddSketch { .. }
                | Self::RemoveSketch { .. }
                | Self::AddSketchEntity { .. }
                | Self::RemoveSketchEntity { .. }
                | Self::AddSketchConstraint { .. }
                | Self::RemoveSketchConstraint { .. }
                | Self::AddFeature { .. }
                | Self::RemoveFeature { .. }
                | Self::MoveFeature { .. }
                | Self::SetFeatureSuppressed { .. }
                | Self::ReplaceFeatureDefinition { .. }
                | Self::AddSemanticRef { .. }
                | Self::RemoveSemanticRef { .. }
        )
    }
}

/// Maximum number of operations accepted in one patch (ADR-013).
pub const MAX_PATCH_OPERATIONS: usize = 10_000;

/// Maximum byte length of an author-chosen stable ID (ADR-013).
const MAX_STABLE_ID_BYTES: usize = 128;

/// Validate an author-chosen ID against `<prefix>:[a-z0-9_]+(\.[a-z0-9_]+)*`.
pub(crate) fn validate_stable_id(id: &str, prefix: &str) -> Result<()> {
    let segment_ok = |segment: &str| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    };
    let valid = id.len() <= MAX_STABLE_ID_BYTES
        && id
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_prefix(':'))
            .is_some_and(|rest| rest.split('.').all(segment_ok));
    if valid {
        Ok(())
    } else {
        Err(OpenCadError::validation(format!(
            "invalid id '{id}': expected '{prefix}:' followed by lowercase letters, digits, '_' or '.'-separated segments, at most {MAX_STABLE_ID_BYTES} bytes"
        )))
    }
}

/// Precondition failure for a check that needs the complete design state.
fn state_required(key: String) -> (String, String) {
    (
        key,
        "patch precondition failed: complete design state is required for sketch and assertion checks"
            .to_string(),
    )
}

/// Validate a parameter name usable as an expression identifier.
fn validate_parameter_name(name: &str) -> Result<()> {
    let mut bytes = name.bytes();
    let valid = bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && parameter_names_in_expr(name) == [name];
    if valid {
        Ok(())
    } else {
        Err(OpenCadError::validation(format!(
            "invalid parameter name '{name}': expected an identifier such as 'wall_thickness'"
        )))
    }
}

/// State assertion that must hold before any patch operation is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchPrecondition {
    /// Require the complete patchable DesignState to have the given canonical
    /// revision.  The algorithm and representation version are explicit so a
    /// future canonicalization change cannot silently accept an old digest.
    RevisionEquals {
        algorithm: String,
        version: String,
        digest: String,
    },
    /// Require a parameter to exist with the exact source expression.
    ParameterExprEquals { id: String, expr: String },
    /// Require a feature node to exist.
    FeatureExists { id: String },
    /// Require a semantic topology reference to exist.
    TopoRefExists { ref_id: String },
    /// Require a sketch to exist (ADR-013).
    SketchExists { id: String },
    /// Require a sketch entity to exist in a sketch (ADR-013).
    SketchEntityExists {
        sketch_id: String,
        entity_id: String,
    },
    /// Require a design assertion to exist (ADR-013).
    AssertionExists { id: String },
}

impl PatchPrecondition {
    /// Construct a revision guard using the repository's canonical state
    /// representation and digest algorithm.
    pub fn revision_equals(state: &DesignState) -> Result<Self> {
        Ok(Self::RevisionEquals {
            algorithm: DESIGN_STATE_REVISION_ALGORITHM.to_string(),
            version: DESIGN_STATE_REVISION_VERSION.to_string(),
            digest: design_state_revision(state)?,
        })
    }
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

    /// Add a revision precondition for a complete design state snapshot.
    pub fn with_revision_precondition(mut self, state: &DesignState) -> Result<Self> {
        self.preconditions
            .push(PatchPrecondition::revision_equals(state)?);
        Ok(self)
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
        self.validate_preconditions_inner(parameters, feature_nodes, semantic_refs, None)
    }

    /// Verify every precondition against the complete patchable design state.
    ///
    /// Revision checks intentionally use the complete state rather than only
    /// the operation's target.  This prevents a patch authored against one
    /// assembly/drawing combination from being applied to another state that
    /// happens to share the same parameter and feature values.
    pub fn validate_preconditions_for_state(&self, state: &DesignState) -> Result<()> {
        self.validate_preconditions_inner(
            &state.parameters,
            &state.feature_nodes,
            &state.semantic_refs,
            Some(state),
        )
    }

    fn validate_preconditions_inner(
        &self,
        parameters: &ParamGraph,
        feature_nodes: &[FeatureNode],
        semantic_refs: &[TopoRef],
        state: Option<&DesignState>,
    ) -> Result<()> {
        let mut failures = Vec::new();
        for precondition in &self.preconditions {
            match precondition {
                PatchPrecondition::RevisionEquals {
                    algorithm,
                    version,
                    digest,
                } => {
                    let Some(state) = state else {
                        failures.push((
                            "revision".to_string(),
                            "patch precondition failed: complete design state is required for revision check"
                                .to_string(),
                        ));
                        continue;
                    };
                    if algorithm != DESIGN_STATE_REVISION_ALGORITHM {
                        failures.push((
                            "revision:algorithm".to_string(),
                            format!(
                                "patch precondition failed: unsupported design-state revision algorithm '{algorithm}' (expected '{DESIGN_STATE_REVISION_ALGORITHM}')"
                            ),
                        ));
                        continue;
                    }
                    if version != DESIGN_STATE_REVISION_VERSION_V1
                        && version != DESIGN_STATE_REVISION_VERSION_V2
                    {
                        failures.push((
                            "revision:version".to_string(),
                            format!(
                                "patch precondition failed: unsupported design-state revision version '{version}' (expected '{DESIGN_STATE_REVISION_VERSION}')"
                            ),
                        ));
                        continue;
                    }
                    // v1 cannot observe sketch or assertion edits, so it may
                    // not guard a patch that creates or removes structure.
                    if version == DESIGN_STATE_REVISION_VERSION_V1 && self.is_structural() {
                        failures.push((
                            "revision:version".to_string(),
                            format!(
                                "patch precondition failed: design-state revision version '{version}' is too weak for structural operations (use '{DESIGN_STATE_REVISION_VERSION_V2}')"
                            ),
                        ));
                        continue;
                    }
                    let actual = design_state_revision_for_version(state, version)?;
                    if digest != &actual {
                        failures.push((
                            "revision:digest".to_string(),
                            format!(
                                "patch precondition failed: design-state revision mismatch (expected '{digest}', found '{actual}')"
                            ),
                        ));
                    }
                }
                PatchPrecondition::ParameterExprEquals { id, expr } => {
                    match parameters.get(id) {
                        None => failures.push((
                            format!("parameter:{id}"),
                            format!(
                                "patch precondition failed: parameter '{id}' does not exist"
                            ),
                        )),
                        Some(actual) if actual.expr != *expr => failures.push((
                            format!("parameter:{id}"),
                            format!(
                                "patch precondition failed: parameter '{id}' expected expression '{expr}', found '{}'",
                                actual.expr
                            ),
                        )),
                        Some(_) => {}
                    }
                }
                PatchPrecondition::FeatureExists { id } => {
                    if !feature_nodes.iter().any(|node| node.id == *id) {
                        failures.push((
                            format!("feature:{id}"),
                            format!("patch precondition failed: feature '{id}' does not exist"),
                        ));
                    }
                }
                PatchPrecondition::TopoRefExists { ref_id } => {
                    if !semantic_refs
                        .iter()
                        .any(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
                    {
                        failures.push((
                            format!("topo_ref:{ref_id}"),
                            format!(
                                "patch precondition failed: topology reference '{ref_id}' does not exist"
                            ),
                        ));
                    }
                }
                PatchPrecondition::SketchExists { id } => match state {
                    None => failures.push(state_required(format!("sketch:{id}"))),
                    Some(state) => {
                        if !state.sketches.iter().any(|sketch| sketch.id.as_str() == id) {
                            failures.push((
                                format!("sketch:{id}"),
                                format!("patch precondition failed: sketch '{id}' does not exist"),
                            ));
                        }
                    }
                },
                PatchPrecondition::SketchEntityExists {
                    sketch_id,
                    entity_id,
                } => match state {
                    None => failures.push(state_required(format!("sketch:{sketch_id}"))),
                    Some(state) => {
                        let exists = state
                            .sketches
                            .iter()
                            .find(|sketch| sketch.id.as_str() == sketch_id)
                            .is_some_and(|sketch| sketch.find_entity(entity_id).is_some());
                        if !exists {
                            failures.push((
                                format!("sketch:{sketch_id}/{entity_id}"),
                                format!(
                                    "patch precondition failed: entity '{entity_id}' does not exist in sketch '{sketch_id}'"
                                ),
                            ));
                        }
                    }
                },
                PatchPrecondition::AssertionExists { id } => match state {
                    None => failures.push(state_required(format!("assertion:{id}"))),
                    Some(state) => {
                        if !state.assertions.iter().any(|assertion| assertion.id == *id) {
                            failures.push((
                                format!("assertion:{id}"),
                                format!(
                                    "patch precondition failed: assertion '{id}' does not exist"
                                ),
                            ));
                        }
                    }
                },
            }
        }
        failures.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        failures.dedup_by(|left, right| left.1 == right.1);
        if failures.is_empty() {
            Ok(())
        } else {
            Err(OpenCadError::validation(
                failures
                    .iter()
                    .map(|(_, message)| message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            ))
        }
    }

    /// Whether any operation creates or removes structure (ADR-013).
    pub fn is_structural(&self) -> bool {
        self.operations.iter().any(PatchOperation::is_structural)
    }

    /// Whether any operation changes Feature Graph inputs, so persisted
    /// `feature_graph` data must be re-derived after applying the patch.
    pub fn changes_feature_graph(&self) -> bool {
        self.operations.iter().any(changes_feature_graph)
    }

    pub fn apply_to_parameters(&self, graph: &mut ParamGraph) -> Result<()> {
        let mut added = Vec::new();
        let mut removed = BTreeSet::new();
        for operation in &self.operations {
            match operation {
                PatchOperation::SetParameter { id, expr } => {
                    graph.set_expr(id, expr.as_str()).map_err(|_| {
                        OpenCadError::validation(format!("unknown parameter '{id}'"))
                    })?;
                }
                PatchOperation::AddParameter { id, name, expr } => {
                    validate_stable_id(id, "param")?;
                    validate_parameter_name(name)?;
                    if removed.contains(id.as_str()) {
                        return Err(OpenCadError::validation(format!(
                            "parameter id '{id}' was removed earlier in this patch and cannot be reused"
                        )));
                    }
                    if let Some(existing) = graph.find_by_name(name) {
                        return Err(OpenCadError::validation(format!(
                            "parameter name '{name}' is already used by '{}'",
                            existing.id
                        )));
                    }
                    graph.add_parameter(ParameterEntry::new(id, name, expr))?;
                    added.push(id.clone());
                }
                PatchOperation::RemoveParameter { id } => {
                    graph.remove_parameter(id).map_err(|_| {
                        OpenCadError::validation(format!("unknown parameter '{id}'"))
                    })?;
                    added.retain(|added_id| added_id != id);
                    removed.insert(id.as_str());
                }
                PatchOperation::SetFeatureExpr { .. }
                | PatchOperation::SetFeatureRef { .. }
                | PatchOperation::AssignFaceRef { .. }
                | PatchOperation::SetInstancePlacement { .. }
                | PatchOperation::SetMateDistance { .. }
                | PatchOperation::AddConnector { .. }
                | PatchOperation::SetDrawingViewScale { .. }
                | PatchOperation::SetDrawingViewOrigin { .. }
                | PatchOperation::AddAssertion { .. }
                | PatchOperation::RemoveAssertion { .. }
                | PatchOperation::AddSketch { .. }
                | PatchOperation::RemoveSketch { .. }
                | PatchOperation::AddSketchEntity { .. }
                | PatchOperation::RemoveSketchEntity { .. }
                | PatchOperation::AddSketchConstraint { .. }
                | PatchOperation::RemoveSketchConstraint { .. }
                | PatchOperation::AddFeature { .. }
                | PatchOperation::RemoveFeature { .. }
                | PatchOperation::MoveFeature { .. }
                | PatchOperation::SetFeatureSuppressed { .. }
                | PatchOperation::ReplaceFeatureDefinition { .. }
                | PatchOperation::AddSemanticRef { .. }
                | PatchOperation::RemoveSemanticRef { .. } => {}
            }
        }
        // Dependency edges for new parameters are derived once, after every
        // operation, so a patch may add parameters in any order.
        for id in added {
            let Some(entry) = graph.get(&id) else {
                continue;
            };
            let sources: Vec<String> = parameter_names_in_expr(&entry.expr)
                .iter()
                .filter_map(|name| graph.find_by_name(name))
                .map(|source| source.id.clone())
                .filter(|source| *source != id)
                .collect();
            for source in sources {
                graph.add_dependency(source, id.as_str())?;
            }
        }
        Ok(())
    }

    /// Apply assertion add/remove operations in order.
    pub fn apply_to_assertions(&self, assertions: &mut Vec<Assertion>) -> Result<()> {
        let mut removed = BTreeSet::new();
        for operation in &self.operations {
            match operation {
                PatchOperation::AddAssertion { assertion } => {
                    validate_stable_id(&assertion.id, "assertion")?;
                    assertion.validate()?;
                    if removed.contains(assertion.id.as_str()) {
                        return Err(OpenCadError::validation(format!(
                            "assertion id '{}' was removed earlier in this patch and cannot be reused",
                            assertion.id
                        )));
                    }
                    if assertions
                        .iter()
                        .any(|existing| existing.id == assertion.id)
                    {
                        return Err(OpenCadError::validation(format!(
                            "assertion '{}' already exists",
                            assertion.id
                        )));
                    }
                    assertions.push(assertion.clone());
                }
                PatchOperation::RemoveAssertion { id } => {
                    let index = assertions
                        .iter()
                        .position(|assertion| assertion.id == *id)
                        .ok_or_else(|| {
                            OpenCadError::validation(format!("unknown assertion '{id}'"))
                        })?;
                    assertions.remove(index);
                    removed.insert(id.as_str());
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Apply the patch to a complete design state after verifying its
    /// preconditions against the unmodified state.
    ///
    /// The candidate is staged, parameter-evaluated, and checked with
    /// [`Self::validate_structural_candidate`] before `state` is replaced, so a
    /// failure leaves `state` untouched.  This is the only application path
    /// that accepts structural operations, because removal checks need every
    /// collection that can hold a reference.
    pub fn apply_to_state(&self, state: &mut DesignState) -> Result<()> {
        if self.operations.len() > MAX_PATCH_OPERATIONS {
            return Err(OpenCadError::validation(format!(
                "patch has {} operations; at most {MAX_PATCH_OPERATIONS} are allowed",
                self.operations.len()
            )));
        }
        self.validate_preconditions_for_state(state)?;
        self.validate_document_context(state.assembly.as_ref(), state.drawing.as_ref())?;
        let mut next = state.clone();
        // Structural sketch, feature, and reference operations are staged
        // before value edits, so a value edit may target an object created
        // earlier in the same patch.
        apply_sketch_operations(&self.operations, &mut next.sketches)?;
        apply_feature_operations(&self.operations, &mut next)?;
        self.apply_to_document_in_place(
            &mut next.parameters,
            &mut next.feature_nodes,
            &mut next.semantic_refs,
            next.assembly.as_mut(),
            next.drawing.as_mut(),
        )?;
        self.apply_to_assertions(&mut next.assertions)?;
        // Structural checks run first so a dangling reference is reported
        // with its complete dependent list rather than as an evaluation error.
        self.validate_structural_candidate(state, &next)?;
        evaluate_param_graph(&next.parameters)?;
        *state = next;
        Ok(())
    }

    /// Validate references in the final candidate of a structural patch.
    ///
    /// Intermediate states may be invalid; only the final state is checked
    /// (ADR-013 §3).  Removal never cascades: a removed parameter whose name
    /// is still referenced fails with every dependent listed in sorted order.
    pub fn validate_structural_candidate(
        &self,
        before: &DesignState,
        after: &DesignState,
    ) -> Result<()> {
        if !self.is_structural() {
            return Ok(());
        }
        let mut failures = BTreeSet::new();

        for operation in &self.operations {
            let PatchOperation::RemoveParameter { id } = operation else {
                continue;
            };
            let Some(removed) = before.parameters.get(id) else {
                continue;
            };
            let name = removed.name.as_str();
            if after.parameters.find_by_name(name).is_some() {
                continue;
            }
            let mut dependents = BTreeSet::new();
            for entry in after.parameters.entries() {
                if parameter_names_in_expr(&entry.expr)
                    .iter()
                    .any(|item| item == name)
                {
                    dependents.insert(format!("parameter {}", entry.id));
                }
            }
            for node in &after.feature_nodes {
                if serialized_value_uses_parameter(&node.definition, name) {
                    dependents.insert(format!("feature {}", node.id));
                }
            }
            for sketch in &after.sketches {
                if serialized_value_uses_parameter(sketch, name) {
                    dependents.insert(format!("sketch {}", sketch.id.as_str()));
                }
            }
            for assertion in &after.assertions {
                if matches!(
                    &assertion.kind,
                    AssertionKind::ParameterRange { parameter_name, .. } if parameter_name == name
                ) {
                    dependents.insert(format!("assertion {}", assertion.id));
                }
            }
            if !dependents.is_empty() {
                failures.insert(format!(
                    "cannot remove parameter '{id}' ('{name}'): still referenced by {}",
                    dependents.into_iter().collect::<Vec<_>>().join(", ")
                ));
            }
        }

        for operation in &self.operations {
            let PatchOperation::AddAssertion { assertion } = operation else {
                continue;
            };
            if !after.assertions.iter().any(|item| item.id == assertion.id) {
                continue;
            }
            match &assertion.kind {
                AssertionKind::ParameterRange { parameter_name, .. }
                    if after.parameters.find_by_name(parameter_name).is_none() =>
                {
                    failures.insert(format!(
                        "assertion '{}' references unknown parameter '{parameter_name}'",
                        assertion.id
                    ));
                }
                AssertionKind::RequiredReference { ref_id }
                    if !after
                        .semantic_refs
                        .iter()
                        .any(|topo_ref| topo_ref.ref_id.as_str() == ref_id) =>
                {
                    failures.insert(format!(
                        "assertion '{}' references unknown semantic reference '{ref_id}'",
                        assertion.id
                    ));
                }
                _ => {}
            }
        }

        validate_sketch_candidate(&self.operations, after, &mut failures);
        validate_feature_candidate(&self.operations, after, &mut failures);

        if failures.is_empty() {
            Ok(())
        } else {
            Err(OpenCadError::validation(
                failures.into_iter().collect::<Vec<_>>().join("; "),
            ))
        }
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
        if self.is_structural() {
            return Err(OpenCadError::validation(
                "structural patch operations require the complete design state; use build_patch_candidate or DesignPatch::apply_to_state",
            ));
        }
        let current_state = DesignState::with_models(
            parameters.clone(),
            feature_nodes.to_vec(),
            semantic_refs.clone(),
            assembly.as_deref().cloned(),
            drawing.as_deref().cloned(),
        );
        self.validate_preconditions_for_state(&current_state)?;
        self.validate_document_context(assembly.as_deref(), drawing.as_deref())?;
        let mut next_parameters = parameters.clone();
        let mut next_feature_nodes = feature_nodes.to_vec();
        let mut next_semantic_refs = semantic_refs.clone();
        let mut next_assembly = assembly.as_deref().cloned();
        let mut next_drawing = drawing.as_deref().cloned();

        self.apply_to_document_in_place(
            &mut next_parameters,
            &mut next_feature_nodes,
            &mut next_semantic_refs,
            next_assembly.as_mut(),
            next_drawing.as_mut(),
        )?;

        *parameters = next_parameters;
        feature_nodes.clone_from_slice(&next_feature_nodes);
        *semantic_refs = next_semantic_refs;
        if let (Some(target), Some(next)) = (assembly, next_assembly) {
            *target = next;
        }
        if let (Some(target), Some(next)) = (drawing, next_drawing) {
            *target = next;
        }
        Ok(())
    }

    fn validate_document_context(
        &self,
        assembly: Option<&opencad_assembly::AssemblyModel>,
        drawing: Option<&opencad_drawing::DrawingModel>,
    ) -> Result<()> {
        for operation in &self.operations {
            match operation {
                PatchOperation::SetInstancePlacement { .. }
                | PatchOperation::SetMateDistance { .. }
                | PatchOperation::AddConnector { .. }
                    if assembly.is_none() =>
                {
                    return Err(OpenCadError::validation(
                        "assembly patch operation requires an assembly model",
                    ));
                }
                PatchOperation::SetDrawingViewScale { .. }
                | PatchOperation::SetDrawingViewOrigin { .. }
                    if drawing.is_none() =>
                {
                    return Err(OpenCadError::validation(
                        "drawing patch operation requires a drawing model",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn apply_to_document_in_place(
        &self,
        parameters: &mut ParamGraph,
        feature_nodes: &mut [FeatureNode],
        semantic_refs: &mut Vec<TopoRef>,
        assembly: Option<&mut opencad_assembly::AssemblyModel>,
        drawing: Option<&mut opencad_drawing::DrawingModel>,
    ) -> Result<()> {
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
    use crate::{design_state_revision, dry_run_patch_state, DesignState};
    use opencad_assembly::AssemblyModel;
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
    fn revision_precondition_is_serializable_and_checks_complete_state() {
        let base = DesignState::new(bracket_parameters(), Vec::new());
        let precondition = PatchPrecondition::revision_equals(&base).expect("revision");
        let patch = DesignPatch::set_parameter("param:width", "100 mm").with_review_metadata(
            "Increase width",
            "Fit the enclosure",
            vec![precondition.clone()],
            Vec::new(),
        );
        let encoded = serde_json::to_vec(&patch).expect("serialize patch");
        let decoded: DesignPatch = serde_json::from_slice(&encoded).expect("deserialize patch");
        assert_eq!(decoded, patch);
        assert_eq!(
            serde_json::to_value(precondition).expect("precondition json")["type"],
            "revision_equals"
        );

        // The parameter graph is unchanged, but a complete-state change in an
        // optional model must still make the revision stale.
        let current = DesignState::with_models(
            base.parameters.clone(),
            base.feature_nodes.clone(),
            base.semantic_refs.clone(),
            Some(AssemblyModel::default()),
            None,
        );
        let report = dry_run_patch_state(&current, &patch);
        let mut parameters = current.parameters.clone();
        let mut features = current.feature_nodes.clone();
        let mut refs = current.semantic_refs.clone();
        let mut assembly = current.assembly.clone();
        let error = patch
            .apply_to_document(
                &mut parameters,
                &mut features,
                &mut refs,
                assembly.as_mut(),
                None,
            )
            .expect_err("stale complete-state revision");
        assert!(!report.validation.is_ok());
        assert_eq!(report.validation.messages[0].message, error.to_string());
        assert_eq!(
            design_state_revision(&base).expect("base revision"),
            match &patch.preconditions[0] {
                PatchPrecondition::RevisionEquals { digest, .. } => digest.clone(),
                _ => panic!("expected revision precondition"),
            }
        );
    }

    #[test]
    fn precondition_failures_are_sorted_and_deduplicated() {
        let state = DesignState::new(bracket_parameters(), Vec::new());
        let patch = DesignPatch::set_parameter("param:width", "100 mm").with_review_metadata(
            "test",
            "test",
            vec![
                PatchPrecondition::FeatureExists {
                    id: "feature:z_missing".into(),
                },
                PatchPrecondition::ParameterExprEquals {
                    id: "param:width".into(),
                    expr: "70 mm".into(),
                },
                PatchPrecondition::FeatureExists {
                    id: "feature:z_missing".into(),
                },
            ],
            Vec::new(),
        );
        let reversed = DesignPatch {
            preconditions: patch.preconditions.iter().cloned().rev().collect(),
            ..patch.clone()
        };
        let first = dry_run_patch_state(&state, &patch);
        let second = dry_run_patch_state(&state, &reversed);
        assert_eq!(
            first.validation.messages[0].message,
            second.validation.messages[0].message
        );
        assert_eq!(
            first.validation.messages[0].message,
            "validation failed: patch precondition failed: feature 'feature:z_missing' does not exist; patch precondition failed: parameter 'param:width' expected expression '70 mm', found '80 mm'"
        );
    }

    #[test]
    fn top_level_document_patch_commits_all_operations() {
        let part = bracket_with_hole().expect("model");
        let mut params = bracket_parameters();
        let mut nodes: Vec<FeatureNode> = part.nodes.into_values().collect();
        let mut refs = Vec::new();
        let patch = DesignPatch::new(vec![
            PatchOperation::SetParameter {
                id: "param:width".into(),
                expr: "100 mm".into(),
            },
            PatchOperation::SetFeatureExpr {
                feature_id: "feature:extrude_base".into(),
                field: FeatureExprField::LengthExpr.as_str().into(),
                expr: "thickness * 2".into(),
            },
        ]);

        patch
            .apply_to_document(&mut params, &mut nodes, &mut refs, None, None)
            .expect("patch");
        assert_eq!(params.get("param:width").expect("width").expr, "100 mm");
        let extrude = nodes
            .iter()
            .find(|node| node.id == "feature:extrude_base")
            .expect("extrude");
        let FeatureDefinition::Extrude(extrude) = &extrude.definition else {
            panic!("expected extrude")
        };
        assert_eq!(extrude.length_expr.as_deref(), Some("thickness * 2"));
    }

    #[test]
    fn top_level_document_patch_failure_does_not_commit_prior_cross_group_operations() {
        let part = bracket_with_hole().expect("model");
        let mut params = bracket_parameters();
        let mut nodes: Vec<FeatureNode> = part.nodes.into_values().collect();
        let mut refs = Vec::new();
        let before = (params.clone(), nodes.clone(), refs.clone());
        let patch = DesignPatch::new(vec![
            PatchOperation::SetParameter {
                id: "param:width".into(),
                expr: "100 mm".into(),
            },
            PatchOperation::SetFeatureExpr {
                feature_id: "feature:extrude_base".into(),
                field: FeatureExprField::LengthExpr.as_str().into(),
                expr: "thickness * 2".into(),
            },
            PatchOperation::SetFeatureExpr {
                feature_id: "feature:hole_mount".into(),
                field: FeatureExprField::LengthExpr.as_str().into(),
                expr: "thickness".into(),
            },
        ]);

        patch
            .apply_to_document(&mut params, &mut nodes, &mut refs, None, None)
            .expect_err("unsupported field");
        assert_eq!((params, nodes, refs), before);
    }

    #[test]
    fn top_level_document_patch_failure_does_not_commit_optional_assembly_group() {
        let mut params = bracket_parameters();
        let mut nodes = Vec::new();
        let mut refs = Vec::new();
        let mut assembly = AssemblyModel::default();
        let before = (
            params.clone(),
            nodes.clone(),
            refs.clone(),
            assembly.clone(),
        );
        let patch = DesignPatch::new(vec![
            PatchOperation::SetParameter {
                id: "param:width".into(),
                expr: "100 mm".into(),
            },
            PatchOperation::SetInstancePlacement {
                instance_id: "instance:missing".into(),
                translation_m: [0.1, 0.0, 0.0],
                rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
        ]);

        patch
            .apply_to_document(
                &mut params,
                &mut nodes,
                &mut refs,
                Some(&mut assembly),
                None,
            )
            .expect_err("unknown assembly instance");
        assert_eq!((params, nodes, refs, assembly), before);
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
