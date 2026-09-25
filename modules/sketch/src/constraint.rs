use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use opencad_core::{ConstraintId, EntityId, Expression};

/// Geometric or dimensional constraint in a sketch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Constraint {
    Coincident {
        id: ConstraintId,
        a: EntityRef,
        b: EntityRef,
    },
    Horizontal {
        id: ConstraintId,
        line: EntityId,
    },
    Vertical {
        id: ConstraintId,
        line: EntityId,
    },
    Parallel {
        id: ConstraintId,
        line_a: EntityId,
        line_b: EntityId,
    },
    Perpendicular {
        id: ConstraintId,
        line_a: EntityId,
        line_b: EntityId,
    },
    Distance {
        id: ConstraintId,
        target: DistanceTarget,
        expr: Expression,
    },
    Radius {
        id: ConstraintId,
        target: EntityId,
        expr: Expression,
    },
    Diameter {
        id: ConstraintId,
        target: EntityId,
        expr: Expression,
    },
    Equal {
        id: ConstraintId,
        a: EqualTarget,
        b: EqualTarget,
    },
}

/// Reference to a point, line, circle, or a sub-element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EntityRef {
    Entity(EntityId),
    PointOnLine { line: EntityId, end: LineEnd },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnd {
    Start,
    End,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DistanceTarget {
    PointToPoint {
        a: EntityId,
        b: EntityId,
    },
    LineLength {
        line: EntityId,
    },
    /// Dimension on a rectangle edge (e.g. `ent:rect_1.width`).
    RectangleDimension {
        rectangle: EntityId,
        edge: RectangleEdge,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RectangleEdge {
    Width,
    Height,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EqualTarget {
    /// Length of a line entity.
    LineLength(EntityId),
    /// Radius of a circle or arc entity.
    Radius(EntityId),
}

/// Canonical wire representation for an [`EqualTarget`].
///
/// The old untagged representation serialized both variants as a bare entity
/// ID string. Since both variants contain the same `EntityId` shape, a radius
/// target was deserialized as `LineLength` after a round trip. Canonical files
/// now carry the target kind explicitly while the custom deserializer below
/// still accepts old bare strings as line-length targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EqualTargetObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<EntityId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    radius: Option<EntityId>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum EqualTargetWire {
    Legacy(EntityId),
    Object(EqualTargetObject),
}

impl Serialize for EqualTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let object = match self {
            Self::LineLength(id) => EqualTargetObject {
                line: Some(id.clone()),
                radius: None,
            },
            Self::Radius(id) => EqualTargetObject {
                line: None,
                radius: Some(id.clone()),
            },
        };
        object.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EqualTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match EqualTargetWire::deserialize(deserializer)? {
            // Bare strings are the pre-disambiguation representation. They
            // were only able to represent line-length targets, so preserve
            // that interpretation for backward compatibility.
            EqualTargetWire::Legacy(id) => Ok(Self::LineLength(id)),
            EqualTargetWire::Object(EqualTargetObject { line, radius }) => match (line, radius) {
                (Some(id), None) => Ok(Self::LineLength(id)),
                (None, Some(id)) => Ok(Self::Radius(id)),
                (Some(_), Some(_)) => Err(de::Error::custom(
                    "equal target must contain exactly one of 'line' or 'radius'",
                )),
                (None, None) => Err(de::Error::custom(
                    "equal target must contain exactly one of 'line' or 'radius'",
                )),
            },
        }
    }
}

impl Constraint {
    pub fn id(&self) -> &ConstraintId {
        match self {
            Self::Coincident { id, .. }
            | Self::Horizontal { id, .. }
            | Self::Vertical { id, .. }
            | Self::Parallel { id, .. }
            | Self::Perpendicular { id, .. }
            | Self::Distance { id, .. }
            | Self::Radius { id, .. }
            | Self::Diameter { id, .. }
            | Self::Equal { id, .. } => id,
        }
    }

    /// Every sketch entity this constraint refers to, in field order.
    pub fn entity_refs(&self) -> Vec<&EntityId> {
        fn entity_ref(value: &EntityRef) -> &EntityId {
            match value {
                EntityRef::Entity(id) | EntityRef::PointOnLine { line: id, .. } => id,
            }
        }
        fn equal_target(value: &EqualTarget) -> &EntityId {
            match value {
                EqualTarget::LineLength(id) | EqualTarget::Radius(id) => id,
            }
        }
        match self {
            Self::Coincident { a, b, .. } => vec![entity_ref(a), entity_ref(b)],
            Self::Horizontal { line, .. } | Self::Vertical { line, .. } => vec![line],
            Self::Parallel { line_a, line_b, .. } | Self::Perpendicular { line_a, line_b, .. } => {
                vec![line_a, line_b]
            }
            Self::Distance { target, .. } => match target {
                DistanceTarget::PointToPoint { a, b } => vec![a, b],
                DistanceTarget::LineLength { line } => vec![line],
                DistanceTarget::RectangleDimension { rectangle, .. } => vec![rectangle],
            },
            Self::Radius { target, .. } | Self::Diameter { target, .. } => vec![target],
            Self::Equal { a, b, .. } => vec![equal_target(a), equal_target(b)],
        }
    }

    /// The parametric expression driving this constraint, if any.
    pub fn expression(&self) -> Option<&Expression> {
        match self {
            Self::Distance { expr, .. }
            | Self::Radius { expr, .. }
            | Self::Diameter { expr, .. } => Some(expr),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cid(id: &str) -> ConstraintId {
        ConstraintId::new(id).expect("valid id")
    }

    fn eid(id: &str) -> EntityId {
        EntityId::new(id).expect("valid id")
    }

    #[test]
    fn entity_refs_and_expression_cover_every_target_shape() {
        let coincident = Constraint::Coincident {
            id: cid("con:c"),
            a: EntityRef::Entity(eid("ent:p")),
            b: EntityRef::PointOnLine {
                line: eid("ent:l"),
                end: LineEnd::End,
            },
        };
        assert_eq!(coincident.entity_refs(), vec![&eid("ent:p"), &eid("ent:l")]);
        assert!(coincident.expression().is_none());

        let rectangle_width = Constraint::Distance {
            id: cid("con:w"),
            target: DistanceTarget::RectangleDimension {
                rectangle: eid("ent:rect"),
                edge: RectangleEdge::Width,
            },
            expr: Expression::new("width").expect("expr"),
        };
        assert_eq!(rectangle_width.entity_refs(), vec![&eid("ent:rect")]);
        assert_eq!(
            rectangle_width.expression().map(Expression::as_str),
            Some("width")
        );

        let equal = Constraint::Equal {
            id: cid("con:eq"),
            a: EqualTarget::LineLength(eid("ent:a")),
            b: EqualTarget::Radius(eid("ent:b")),
        };
        assert_eq!(equal.entity_refs(), vec![&eid("ent:a"), &eid("ent:b")]);
    }

    #[test]
    fn coincident_constraint_round_trip() {
        let c = Constraint::Coincident {
            id: cid("con:coincident_1"),
            a: EntityRef::Entity(eid("ent:pt_1")),
            b: EntityRef::Entity(eid("ent:pt_2")),
        };
        let json = serde_json::to_string(&c).expect("serialize");
        let restored: Constraint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(c, restored);
    }

    #[test]
    fn horizontal_constraint_round_trip() {
        let c = Constraint::Horizontal {
            id: cid("con:horiz_1"),
            line: eid("ent:line_1"),
        };
        round_trip(&c);
    }

    #[test]
    fn vertical_constraint_round_trip() {
        let c = Constraint::Vertical {
            id: cid("con:vert_1"),
            line: eid("ent:line_1"),
        };
        round_trip(&c);
    }

    #[test]
    fn parallel_constraint_round_trip() {
        let c = Constraint::Parallel {
            id: cid("con:parallel_1"),
            line_a: eid("ent:line_1"),
            line_b: eid("ent:line_2"),
        };
        round_trip(&c);
    }

    #[test]
    fn perpendicular_constraint_round_trip() {
        let c = Constraint::Perpendicular {
            id: cid("con:perp_1"),
            line_a: eid("ent:line_1"),
            line_b: eid("ent:line_2"),
        };
        round_trip(&c);
    }

    #[test]
    fn distance_constraint_round_trip() {
        let c = Constraint::Distance {
            id: cid("con:dist_1"),
            target: DistanceTarget::LineLength {
                line: eid("ent:line_1"),
            },
            expr: Expression::new("80 mm").expect("expr"),
        };
        round_trip(&c);
    }

    #[test]
    fn radius_constraint_round_trip() {
        let c = Constraint::Radius {
            id: cid("con:radius_1"),
            target: eid("ent:circle_1"),
            expr: Expression::new("10 mm").expect("expr"),
        };
        round_trip(&c);
    }

    #[test]
    fn diameter_constraint_round_trip() {
        let c = Constraint::Diameter {
            id: cid("con:diam_1"),
            target: eid("ent:circle_1"),
            expr: Expression::new("20 mm").expect("expr"),
        };
        round_trip(&c);
    }

    #[test]
    fn equal_constraint_round_trip() {
        let c = Constraint::Equal {
            id: cid("con:equal_1"),
            a: EqualTarget::LineLength(eid("ent:line_1")),
            b: EqualTarget::LineLength(eid("ent:line_2")),
        };
        round_trip(&c);
    }

    #[test]
    fn equal_line_target_uses_explicit_canonical_object() {
        let target = EqualTarget::LineLength(eid("ent:line_1"));
        let json = serde_json::to_string(&target).expect("serialize");
        assert_eq!(json, r#"{"line":"ent:line_1"}"#);
        let restored: EqualTarget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(target, restored);
    }

    #[test]
    fn equal_radius_target_round_trips_without_changing_kind() {
        let target = EqualTarget::Radius(eid("ent:circle_1"));
        let json = serde_json::to_string(&target).expect("serialize");
        assert_eq!(json, r#"{"radius":"ent:circle_1"}"#);
        let restored: EqualTarget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(target, restored);
    }

    #[test]
    fn legacy_equal_target_string_is_a_line_length() {
        let restored: EqualTarget =
            serde_json::from_str(r#""ent:legacy_line""#).expect("deserialize legacy target");
        assert_eq!(restored, EqualTarget::LineLength(eid("ent:legacy_line")));
    }

    #[test]
    fn legacy_equal_constraint_strings_are_line_lengths() {
        let restored: Constraint = serde_json::from_str(
            r#"{
                "type":"equal",
                "id":"con:legacy_equal",
                "a":"ent:line_a",
                "b":"ent:line_b"
            }"#,
        )
        .expect("deserialize legacy constraint");
        assert!(matches!(
            restored,
            Constraint::Equal {
                a: EqualTarget::LineLength(_),
                b: EqualTarget::LineLength(_),
                ..
            }
        ));
    }

    #[test]
    fn equal_target_rejects_ambiguous_object() {
        let error =
            serde_json::from_str::<EqualTarget>(r#"{"line":"ent:line_1","radius":"ent:circle_1"}"#)
                .expect_err("ambiguous target");
        assert!(error.to_string().contains("exactly one"));
    }

    fn round_trip(c: &Constraint) {
        let json = serde_json::to_string(c).expect("serialize");
        let restored: Constraint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*c, restored);
    }
}
