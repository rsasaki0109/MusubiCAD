//! 2D sketch data model: entities, constraints, profiles, and solve state.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use opencad_core::{ConstraintId, EntityId, Expression, OpenCadError, Result, SketchId};

pub mod constraint;
pub mod dimension;
pub mod entity;
pub mod profile;
pub mod solve;
pub mod solve_state;
pub mod workplane;

pub use constraint::{Constraint, DistanceTarget, EntityRef, EqualTarget, LineEnd, RectangleEdge};
pub use dimension::Dimension;
pub use entity::{
    expand_rectangle, ArcEntity, CircleEntity, Coord, EntityBase, LineEntity, PointEntity,
    RectangleEntity, SketchEntity,
};
pub use profile::{assign_profile_refs, detect_profiles, Profile, ProfileKind};
pub use solve::{parse_angle_expr, parse_length_expr, solve_sketch};
pub use solve_state::SolveState;
pub use workplane::{GlobalPlane, Workplane};

/// A 2D sketch on a workplane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    pub id: SketchId,
    pub name: String,
    pub workplane: Workplane,
    pub entities: Vec<SketchEntity>,
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub dimensions: Vec<Dimension>,
    #[serde(default)]
    pub solve_state: SolveState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<Profile>,
}

impl Sketch {
    pub fn new(id: SketchId, name: impl Into<String>, workplane: Workplane) -> Self {
        Self {
            id,
            name: name.into(),
            workplane,
            entities: Vec::new(),
            constraints: Vec::new(),
            dimensions: Vec::new(),
            solve_state: SolveState::Unknown,
            profiles: Vec::new(),
        }
    }

    pub fn add_entity(&mut self, entity: SketchEntity) -> Result<()> {
        if self.find_entity(entity.id().as_str()).is_some() {
            return Err(OpenCadError::validation(format!(
                "entity '{}' already exists",
                entity.id()
            )));
        }
        self.entities.push(entity);
        Ok(())
    }

    pub fn add_constraint(&mut self, constraint: Constraint) -> Result<()> {
        if self.constraints.iter().any(|c| c.id() == constraint.id()) {
            return Err(OpenCadError::validation(format!(
                "constraint '{}' already exists",
                constraint.id()
            )));
        }
        self.constraints.push(constraint);
        Ok(())
    }

    pub fn find_entity(&self, id: &str) -> Option<&SketchEntity> {
        self.entities.iter().find(|e| e.id().as_str() == id)
    }

    pub fn entity_index(&self) -> IndexMap<String, &SketchEntity> {
        self.entities
            .iter()
            .map(|e| (e.id().as_str().to_string(), e))
            .collect()
    }

    pub fn construction_entities(&self) -> Vec<&SketchEntity> {
        self.entities
            .iter()
            .filter(|e| e.is_construction())
            .collect()
    }

    pub fn update_profiles(&mut self) -> Result<()> {
        let mut profiles = detect_profiles(&self.entities)?;
        assign_profile_refs(self.id.as_str(), &mut profiles);
        self.profiles = profiles;
        Ok(())
    }
}

/// Build the bracket base sketch from the architecture sample.
pub fn bracket_base_sketch() -> Result<Sketch> {
    let mut sketch = Sketch::new(
        SketchId::new("sketch:base")?,
        "Base Sketch",
        Workplane::xy(),
    );

    sketch.add_entity(SketchEntity::Rectangle(RectangleEntity {
        base: EntityBase {
            id: EntityId::new("ent:rect_1")?,
            construction: false,
        },
        origin: [Coord::expr("-width/2")?, Coord::expr("-30 mm")?],
        size: [Coord::expr("width")?, Coord::expr("60 mm")?],
        corner_ids: vec![
            EntityId::new("ent:rect_1_c0")?,
            EntityId::new("ent:rect_1_c1")?,
            EntityId::new("ent:rect_1_c2")?,
            EntityId::new("ent:rect_1_c3")?,
        ],
        edge_ids: vec![
            EntityId::new("ent:rect_1_e0")?,
            EntityId::new("ent:rect_1_e1")?,
            EntityId::new("ent:rect_1_e2")?,
            EntityId::new("ent:rect_1_e3")?,
        ],
    }))?;

    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new("con:rect_width")?,
        target: DistanceTarget::RectangleDimension {
            rectangle: EntityId::new("ent:rect_1")?,
            edge: RectangleEdge::Width,
        },
        expr: Expression::new("width")?,
    })?;

    Ok(sketch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sketch_round_trip() {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:test").expect("id"),
            "Test",
            Workplane::xy(),
        );
        sketch
            .add_entity(SketchEntity::Point(PointEntity {
                base: EntityBase {
                    id: EntityId::new("ent:pt_1").expect("id"),
                    construction: false,
                },
                x: Coord::literal(0.0),
                y: Coord::literal(0.0),
            }))
            .expect("add entity");

        let json = serde_json::to_string_pretty(&sketch).expect("serialize");
        let restored: Sketch = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sketch, restored);
    }

    #[test]
    fn bracket_base_sketch_has_rectangle_and_constraint() {
        let sketch = bracket_base_sketch().expect("sketch");
        assert_eq!(sketch.entities.len(), 1);
        assert_eq!(sketch.constraints.len(), 1);
        assert!(matches!(sketch.entities[0], SketchEntity::Rectangle(_)));
    }

    #[test]
    fn duplicate_entity_is_rejected() {
        let mut sketch = Sketch::new(
            SketchId::new("sketch:test").expect("id"),
            "Test",
            Workplane::xy(),
        );
        let point = SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new("ent:pt_1").expect("id"),
                construction: false,
            },
            x: Coord::literal(0.0),
            y: Coord::literal(0.0),
        });
        sketch.add_entity(point.clone()).expect("first");
        assert!(sketch.add_entity(point).is_err());
    }
}
