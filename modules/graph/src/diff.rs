use std::collections::BTreeMap;

use indexmap::IndexMap;
use opencad_geometry::TopoRef;
use serde::{Deserialize, Serialize};

use crate::ParamGraph;

/// High-level semantic change categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SemanticChange {
    ParameterChanged {
        id: String,
        before: String,
        after: String,
    },
    FeatureAdded {
        id: String,
        feature_type: String,
    },
    FeatureRemoved {
        id: String,
    },
    FeatureModified {
        id: String,
        field: String,
        before: String,
        after: String,
    },
    ConstraintModified {
        id: String,
        before: String,
        after: String,
    },
    MassChanged {
        before: String,
        after: String,
    },
    BboxChanged {
        before: String,
        after: String,
    },
    TopoRefAdded {
        ref_id: String,
        created_by: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<String>,
    },
    TopoRefRemoved {
        ref_id: String,
    },
    TopoRefModified {
        ref_id: String,
        field: String,
        before: String,
        after: String,
    },
    AssemblyInstanceAdded {
        id: String,
    },
    AssemblyInstanceRemoved {
        id: String,
    },
    AssemblyInstanceChanged {
        id: String,
        field: String,
        before: String,
        after: String,
    },
    AssemblyMateAdded {
        id: String,
    },
    AssemblyMateRemoved {
        id: String,
    },
    AssemblyMateChanged {
        id: String,
        before: String,
        after: String,
    },
    AssemblyConnectorAdded {
        id: String,
    },
    AssemblyConnectorRemoved {
        id: String,
    },
    AssemblyConnectorChanged {
        id: String,
        before: String,
        after: String,
    },
    DrawingSheetAdded {
        id: String,
    },
    DrawingSheetRemoved {
        id: String,
    },
    DrawingSheetChanged {
        id: String,
        before: String,
        after: String,
    },
    DrawingViewAdded {
        id: String,
    },
    DrawingViewRemoved {
        id: String,
    },
    DrawingViewChanged {
        id: String,
        before: String,
        after: String,
    },
}

/// Geometric diff summary (derived from regeneration).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometricDiff {
    pub volume_before: Option<f64>,
    pub volume_after: Option<f64>,
    pub mass_before: Option<f64>,
    pub mass_after: Option<f64>,
}

/// Combined design diff output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignDiff {
    pub diff_type: DiffType,
    pub summary: String,
    pub changes: Vec<SemanticChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometricDiff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffType {
    Text,
    Semantic,
    Geometric,
}

impl DesignDiff {
    pub fn semantic(summary: impl Into<String>, changes: Vec<SemanticChange>) -> Self {
        Self {
            diff_type: DiffType::Semantic,
            summary: summary.into(),
            changes,
            geometry: None,
        }
    }

    pub fn with_geometry(mut self, geometry: GeometricDiff) -> Self {
        self.geometry = Some(geometry);
        self
    }

    pub fn is_empty(&self) -> bool {
        if !self.changes.is_empty() {
            return false;
        }
        match &self.geometry {
            None => true,
            Some(geometry) => {
                geometry.volume_before == geometry.volume_after
                    && geometry.mass_before == geometry.mass_after
            }
        }
    }
}

/// Compare parameter expressions between two graphs.
pub fn diff_param_graphs(before: &ParamGraph, after: &ParamGraph) -> Vec<SemanticChange> {
    let mut ids: IndexMap<String, ()> = IndexMap::new();
    for id in before.parameter_ids() {
        ids.insert(id, ());
    }
    for id in after.parameter_ids() {
        ids.insert(id, ());
    }

    let mut changes = Vec::new();
    for id in ids.keys() {
        let before_expr = before.get(id).map(|entry| entry.expr.as_str());
        let after_expr = after.get(id).map(|entry| entry.expr.as_str());
        match (before_expr, after_expr) {
            (Some(before_expr), Some(after_expr)) if before_expr != after_expr => {
                changes.push(SemanticChange::ParameterChanged {
                    id: id.clone(),
                    before: before_expr.to_string(),
                    after: after_expr.to_string(),
                });
            }
            (Some(before_expr), None) => {
                changes.push(SemanticChange::ParameterChanged {
                    id: id.clone(),
                    before: before_expr.to_string(),
                    after: String::new(),
                });
            }
            (None, Some(after_expr)) => {
                changes.push(SemanticChange::ParameterChanged {
                    id: id.clone(),
                    before: String::new(),
                    after: after_expr.to_string(),
                });
            }
            _ => {}
        }
    }
    changes
}

/// Compare persisted semantic topology references.
pub fn diff_semantic_refs(before: &[TopoRef], after: &[TopoRef]) -> Vec<SemanticChange> {
    let before_map: BTreeMap<&str, &TopoRef> = before
        .iter()
        .map(|topo_ref| (topo_ref.ref_id.as_str(), topo_ref))
        .collect();
    let after_map: BTreeMap<&str, &TopoRef> = after
        .iter()
        .map(|topo_ref| (topo_ref.ref_id.as_str(), topo_ref))
        .collect();

    let mut ids = BTreeMap::new();
    for id in before_map.keys() {
        ids.insert(*id, ());
    }
    for id in after_map.keys() {
        ids.insert(*id, ());
    }

    let mut changes = Vec::new();
    for id in ids.keys() {
        match (before_map.get(id), after_map.get(id)) {
            (Some(_), None) => changes.push(SemanticChange::TopoRefRemoved {
                ref_id: (*id).to_string(),
            }),
            (None, Some(after_ref)) => changes.push(SemanticChange::TopoRefAdded {
                ref_id: (*id).to_string(),
                created_by: after_ref.semantic.created_by.clone(),
                role: after_ref.semantic.role.clone(),
            }),
            (Some(before_ref), Some(after_ref)) if before_ref != after_ref => {
                if before_ref.semantic.created_by != after_ref.semantic.created_by {
                    changes.push(SemanticChange::TopoRefModified {
                        ref_id: (*id).to_string(),
                        field: "created_by".into(),
                        before: before_ref.semantic.created_by.clone(),
                        after: after_ref.semantic.created_by.clone(),
                    });
                }
                if before_ref.semantic.role != after_ref.semantic.role {
                    changes.push(SemanticChange::TopoRefModified {
                        ref_id: (*id).to_string(),
                        field: "role".into(),
                        before: before_ref.semantic.role.clone().unwrap_or_default(),
                        after: after_ref.semantic.role.clone().unwrap_or_default(),
                    });
                }
                if before_ref.kernel_face_id() != after_ref.kernel_face_id() {
                    changes.push(SemanticChange::TopoRefModified {
                        ref_id: (*id).to_string(),
                        field: "kernel_face_id".into(),
                        before: before_ref
                            .kernel_face_id()
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                        after: after_ref
                            .kernel_face_id()
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                    });
                }
            }
            _ => {}
        }
    }

    changes
}

/// Build a short human-readable summary from semantic changes.
pub fn build_summary(changes: &[SemanticChange]) -> String {
    if changes.is_empty() {
        return "No changes".into();
    }

    let param_changes: Vec<String> = changes
        .iter()
        .filter_map(|change| match change {
            SemanticChange::ParameterChanged { id, before, after } => {
                Some(format!("{id}: {before} -> {after}"))
            }
            _ => None,
        })
        .collect();
    if !param_changes.is_empty() && param_changes.len() == changes.len() {
        return param_changes.join(", ");
    }

    let topo_changes: Vec<String> = changes
        .iter()
        .filter_map(|change| match change {
            SemanticChange::TopoRefAdded {
                ref_id,
                created_by,
                role,
            } => Some(format!(
                "topo ref {ref_id} added ({created_by}{})",
                role.as_deref()
                    .map(|value| format!(", {value}"))
                    .unwrap_or_default()
            )),
            SemanticChange::TopoRefRemoved { ref_id } => Some(format!("topo ref {ref_id} removed")),
            SemanticChange::TopoRefModified {
                ref_id,
                field,
                before,
                after,
            } => Some(format!("topo ref {ref_id}.{field}: {before} -> {after}")),
            _ => None,
        })
        .collect();
    if !topo_changes.is_empty() && topo_changes.len() == changes.len() {
        return topo_changes.join(", ");
    }

    format!("{} change(s)", changes.len())
}

/// Format mass in SI for semantic diff output.
pub fn format_mass_kg(mass_kg: f64) -> String {
    if mass_kg.abs() < 1.0 {
        format!("{:.2} g", mass_kg * 1000.0)
    } else {
        format!("{:.3} kg", mass_kg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParamGraph, ParameterEntry};

    #[test]
    fn semantic_diff_round_trip() {
        let diff = DesignDiff::semantic(
            "Thickness increased by 3 mm",
            vec![
                SemanticChange::ParameterChanged {
                    id: "param:thickness".into(),
                    before: "6 mm".into(),
                    after: "9 mm".into(),
                },
                SemanticChange::MassChanged {
                    before: "128 g".into(),
                    after: "192 g".into(),
                },
            ],
        );
        let json = serde_json::to_string(&diff).expect("serialize");
        let restored: DesignDiff = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(diff, restored);
    }

    #[test]
    fn diff_param_graphs_detects_expr_change() {
        let before = bracket_parameters();
        let mut after = bracket_parameters();
        after.set_expr("param:width", "100 mm").expect("set expr");

        let changes = diff_param_graphs(&before, &after);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0],
            SemanticChange::ParameterChanged {
                id: "param:width".into(),
                before: "80 mm".into(),
                after: "100 mm".into(),
            }
        );
        assert_eq!(
            build_summary(&changes),
            "param:width: 80 mm -> 100 mm".to_string()
        );
    }

    fn bracket_parameters() -> ParamGraph {
        let mut graph = ParamGraph::new();
        graph
            .add_parameter(ParameterEntry::new("param:width", "width", "80 mm"))
            .expect("param");
        graph
            .add_parameter(ParameterEntry::new("param:height", "height", "60 mm"))
            .expect("param");
        graph
    }
}
