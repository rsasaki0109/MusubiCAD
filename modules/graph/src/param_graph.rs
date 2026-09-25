use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use opencad_core::OpenCadError;
use opencad_core::Result;

use crate::dependency::{topological_sort, DependencyEdge};
use crate::design_graph::{DesignGraph, GraphNode, GraphNodeKind};

/// Parameter-to-parameter or parameter-to-consumer dependency graph.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParamGraph {
    parameters: IndexMap<String, ParameterEntry>,
    edges: Vec<DependencyEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterEntry {
    pub id: String,
    pub name: String,
    pub expr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default)]
    pub dirty: bool,
}

impl ParameterEntry {
    pub fn new(id: impl Into<String>, name: impl Into<String>, expr: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            expr: expr.into(),
            role: None,
            dirty: false,
        }
    }
}

impl ParamGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_parameter(&mut self, entry: ParameterEntry) -> Result<()> {
        if self.parameters.contains_key(&entry.id) {
            return Err(OpenCadError::validation(format!(
                "parameter '{}' already exists",
                entry.id
            )));
        }
        self.parameters.insert(entry.id.clone(), entry);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ParameterEntry> {
        self.parameters.get(id)
    }

    /// Find a parameter by its expression-visible name.
    pub fn find_by_name(&self, name: &str) -> Option<&ParameterEntry> {
        self.parameters.values().find(|entry| entry.name == name)
    }

    /// All parameter entries in stored order.
    pub fn entries(&self) -> impl Iterator<Item = &ParameterEntry> {
        self.parameters.values()
    }

    /// Remove a parameter and every dependency edge that touches it.
    ///
    /// The remaining parameters keep their relative order, so serialization
    /// stays deterministic.  Callers are responsible for proving that no
    /// expression still references the removed name.
    pub fn remove_parameter(&mut self, id: &str) -> Result<ParameterEntry> {
        let entry = self
            .parameters
            .shift_remove(id)
            .ok_or_else(|| OpenCadError::not_found(format!("parameter '{id}'")))?;
        self.edges
            .retain(|edge| edge.source != entry.id && edge.target != entry.id);
        Ok(entry)
    }

    /// All parameter IDs in deterministic sorted order.
    pub fn parameter_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.parameters.keys().cloned().collect();
        ids.sort();
        ids
    }

    pub fn set_expr(&mut self, id: &str, expr: impl Into<String>) -> Result<()> {
        let entry = self
            .parameters
            .get_mut(id)
            .ok_or_else(|| OpenCadError::not_found(format!("parameter '{id}'")))?;
        entry.expr = expr.into();
        entry.dirty = true;
        Ok(())
    }

    pub fn add_dependency(
        &mut self,
        source: impl Into<String>,
        target: impl Into<String>,
    ) -> Result<()> {
        let source = source.into();
        let target = target.into();
        if !self.parameters.contains_key(&source) {
            return Err(OpenCadError::validation(format!(
                "parameter source '{source}' not found"
            )));
        }
        self.edges.push(DependencyEdge::depends_on(source, target));
        Ok(())
    }

    pub fn evaluation_order(&self) -> Result<Vec<String>> {
        let ids: Vec<String> = self.parameters.keys().cloned().collect();
        topological_sort(&ids, &self.edges).map_err(OpenCadError::validation)
    }

    /// All dependency edges recorded in this graph.
    pub fn dependency_edges(&self) -> &[DependencyEdge] {
        &self.edges
    }

    pub fn mark_dirty(&mut self, param_id: &str) {
        let mut queue = vec![param_id.to_string()];
        let mut visited = indexmap::IndexSet::new();

        while let Some(current) = queue.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            if let Some(entry) = self.parameters.get_mut(&current) {
                entry.dirty = true;
            }
            for edge in &self.edges {
                if edge.source == current {
                    queue.push(edge.target.clone());
                }
            }
        }
    }

    pub fn sync_to_design_graph(&self, graph: &mut DesignGraph) -> Result<()> {
        for entry in self.parameters.values() {
            if graph.get_node(&entry.id).is_none() {
                let mut node = GraphNode::new(
                    entry.id.clone(),
                    GraphNodeKind::Parameter,
                    entry.name.clone(),
                );
                if let Some(role) = &entry.role {
                    node = node.with_role(role.clone());
                }
                graph.add_node(node)?;
            }
        }
        for edge in &self.edges {
            if graph
                .edges()
                .iter()
                .any(|e| e.source == edge.source && e.target == edge.target && e.kind == edge.kind)
            {
                continue;
            }
            graph.add_edge(edge.clone())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_parameter_drops_entry_and_touching_edges() {
        let mut graph = ParamGraph::new();
        graph
            .add_parameter(ParameterEntry::new("param:width", "width", "80 mm"))
            .unwrap();
        graph
            .add_parameter(ParameterEntry::new("param:height", "height", "width / 2"))
            .unwrap();
        graph
            .add_parameter(ParameterEntry::new("param:depth", "depth", "5 mm"))
            .unwrap();
        graph.add_dependency("param:width", "param:height").unwrap();

        let removed = graph.remove_parameter("param:height").unwrap();
        assert_eq!(removed.name, "height");
        assert!(graph.dependency_edges().is_empty());
        assert_eq!(
            graph
                .entries()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["param:width", "param:depth"]
        );
        assert_eq!(
            graph.find_by_name("depth").map(|entry| entry.id.as_str()),
            Some("param:depth")
        );
        assert!(graph.remove_parameter("param:height").is_err());
    }

    #[test]
    fn parameter_evaluation_order() {
        let mut graph = ParamGraph::new();
        graph
            .add_parameter(ParameterEntry::new("param:width", "width", "80 mm"))
            .unwrap();
        graph
            .add_parameter(ParameterEntry::new("param:height", "height", "width / 2"))
            .unwrap();
        graph.add_dependency("param:width", "param:height").unwrap();

        let order = graph.evaluation_order().expect("order");
        assert_eq!(order[0], "param:width");
        assert_eq!(order[1], "param:height");
    }

    #[test]
    fn dirty_propagation_through_param_chain() {
        let mut graph = ParamGraph::new();
        graph
            .add_parameter(ParameterEntry::new("param:width", "width", "80 mm"))
            .unwrap();
        graph
            .add_parameter(ParameterEntry::new("param:pitch", "pitch", "width - 20 mm"))
            .unwrap();
        graph.add_dependency("param:width", "param:pitch").unwrap();

        graph.mark_dirty("param:width");
        assert!(graph.get("param:width").unwrap().dirty);
        assert!(graph.get("param:pitch").unwrap().dirty);
    }
}
