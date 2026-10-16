//! Sketch feature: stores and validates 2D profile input (Task-088).

use serde::{Deserialize, Serialize};

use opencad_core::{OpenCadError, Result};
use opencad_sketch::Sketch;

use crate::feature::{Feature, FeatureDefinition, FeatureNode, FeatureOutput, RegenContext};

/// Sketch feature definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchFeatureDef {
    pub sketch_id: String,
}

/// Sketch feature executor.
#[derive(Debug, Default)]
pub struct SketchFeature;

impl Feature for SketchFeature {
    fn feature_type(&self) -> &'static str {
        "sketch"
    }

    fn execute(&self, node: &FeatureNode, ctx: &dyn RegenContext) -> Result<FeatureOutput> {
        let FeatureDefinition::Sketch(_) = &node.definition else {
            return Err(OpenCadError::validation(format!(
                "expected sketch feature, got {}",
                node.definition.feature_type()
            )));
        };
        // A datum sketch consumes construction geometry even without a solid
        // feature downstream. Keep the legacy data-free executor contract.
        if let Ok(sketch) = ctx.sketch_for_feature(&node.id) {
            if matches!(
                sketch.workplane,
                opencad_sketch::Workplane::DatumPlane { .. }
            ) {
                crate::sketch_bridge::resolve_workplane(&sketch.workplane, ctx)?;
            }
        }
        Ok(FeatureOutput::default())
    }
}

/// Validate that a sketch has at least one profile.
///
/// A sketch may hold only an open chain, such as a sweep path (ADR-023).
/// Consumers that need a closed profile (extrude, hole, revolve, loft,
/// sweep section) check closure when they resolve their `profile_ref`.
pub fn validate_sketch(sketch: &Sketch) -> Result<()> {
    if sketch.profiles.is_empty() {
        return Err(OpenCadError::validation(format!(
            "sketch '{}' has no profiles; call update_profiles() after solving",
            sketch.id
        )));
    }
    Ok(())
}

/// ADR-102: an unconsumed authored admitted sketch may still be a draft with no
/// profiles. Keep profile-consuming and unsupported sketch admission unchanged.
pub(crate) fn admits_unconsumed_editor_draft(
    sketch: &Sketch,
    nodes: &indexmap::IndexMap<String, FeatureNode>,
) -> bool {
    use opencad_sketch::{SketchEntity, Workplane};
    let admitted_plane = match &sketch.workplane {
        Workplane::Global { .. } => true,
        Workplane::DatumPlane { feature_id } => {
            crate::datum::admitted_sketch_datum_chain(feature_id, nodes.values()).is_ok()
        }
        _ => false,
    };
    if !sketch.profiles.is_empty()
        || !admitted_plane
        || sketch.entities.iter().any(|e| match e {
            SketchEntity::Point(_)
            | SketchEntity::Line(_)
            | SketchEntity::Circle(_)
            | SketchEntity::Ellipse(_) => false,
            SketchEntity::Arc(arc) => !admitted_arc_draft(sketch, arc),
            _ => true,
        })
    {
        return false;
    }
    let owners: Vec<_> = nodes.values().filter(|n| !n.suppressed
        && matches!(&n.definition,FeatureDefinition::Sketch(s) if s.sketch_id==sketch.id.as_str())).collect();
    if owners.len() != 1
        || nodes.values().any(|n| {
            !n.suppressed
                && n.definition
                    .feature_inputs()
                    .iter()
                    .any(|(_, id)| *id == owners[0].id)
        })
    {
        return false;
    }
    // No-parameter regeneration can skip apply_parameters' solver. Do not turn
    // that shortcut into admission of contradictory or invalid source drafts.
    crate::param_apply::solved_sketch_copy(sketch, &indexmap::IndexMap::new()).is_ok()
}

/// ADR-104: only a resolved explicit-endpoint arc in the existing portable
/// subset may extend the profile-free construction draft admission.
fn admitted_arc_draft(sketch: &Sketch, arc: &opencad_sketch::ArcEntity) -> bool {
    use opencad_geometry::{ProfileSegment, SolvedSketch};
    use opencad_sketch::{Coord, SketchEntity};
    let check = || -> Option<()> {
        let point = |id: &opencad_core::EntityId| match sketch.find_entity(id.as_str())? {
            SketchEntity::Point(p) => match (&p.x, &p.y) {
                (Coord::Literal(x), Coord::Literal(y)) => Some([*x, *y]),
                _ => None,
            },
            _ => None,
        };
        let center = point(&arc.center)?;
        let start = point(arc.start_point.as_ref()?)?;
        let end = point(arc.end_point.as_ref()?)?;
        let Coord::Literal(radius) = arc.radius else {
            return None;
        };
        let (a, b) = opencad_sketch::solve::arc_angles(arc).ok()?;
        let sweep = (b - a).rem_euclid(std::f64::consts::TAU);
        let mid = a + sweep / 2.;
        opencad_geometry::validate_arc_sector_profile(&SolvedSketch {
            profile_ref: "editor construction arc admission".into(),
            points: Vec::new(),
            closed: true,
            circle: None,
            ellipse: None,
            cubic_bezier: None,
            placement: None,
            segments: vec![
                ProfileSegment::Arc {
                    start_m: start,
                    mid_m: [
                        center[0] + radius * mid.cos(),
                        center[1] + radius * mid.sin(),
                    ],
                    end_m: end,
                },
                ProfileSegment::Line {
                    start_m: end,
                    end_m: center,
                },
                ProfileSegment::Line {
                    start_m: center,
                    end_m: start,
                },
            ],
        })
        .ok()
    };
    check().is_some()
}
