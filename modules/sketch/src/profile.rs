use serde::{Deserialize, Serialize};

use opencad_core::{EntityId, Result};

use crate::entity::SketchEntity;

/// Classification of a detected profile loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    Closed,
    Open,
    SelfIntersecting,
}

/// A profile loop formed by connected sketch entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub entity_ids: Vec<EntityId>,
    pub kind: ProfileKind,
    /// Stable reference for extrude operations (e.g. `sketch:base/profile:outer`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_ref: Option<String>,
}

impl Profile {
    pub fn new(id: impl Into<String>, entity_ids: Vec<EntityId>, kind: ProfileKind) -> Self {
        Self {
            id: id.into(),
            entity_ids,
            kind,
            profile_ref: None,
        }
    }

    pub fn with_ref(mut self, profile_ref: impl Into<String>) -> Self {
        self.profile_ref = Some(profile_ref.into());
        self
    }

    pub fn is_closed(&self) -> bool {
        self.kind == ProfileKind::Closed
    }
}

/// Detect connected loops from lines and circles.
pub fn detect_profiles(entities: &[SketchEntity]) -> Result<Vec<Profile>> {
    let mut profiles = Vec::new();

    for entity in entities {
        if let SketchEntity::Circle(c) = entity {
            if !c.base.construction {
                profiles.push(
                    Profile::new(
                        format!("profile:circle:{}", c.base.id),
                        vec![c.base.id.clone()],
                        ProfileKind::Closed,
                    )
                    .with_ref(format!("profile:circle:{}", c.base.id.as_str())),
                );
            }
        }
    }

    // Lines and arcs with both endpoint points form loop segments
    // (ADR-021); both are traced through shared point IDs.
    let segments: Vec<ChainEdge> = entities
        .iter()
        .filter_map(|e| match e {
            SketchEntity::Line(l) if !l.base.construction => Some(ChainEdge {
                id: l.base.id.clone(),
                start: l.start.clone(),
                end: l.end.clone(),
                is_arc: false,
            }),
            SketchEntity::Arc(a) if !a.base.construction => Some(ChainEdge {
                id: a.base.id.clone(),
                start: a.start_point.clone()?,
                end: a.end_point.clone()?,
                is_arc: true,
            }),
            _ => None,
        })
        .collect();
    let lines: Vec<&ChainEdge> = segments.iter().collect();

    if lines.len() >= 2 {
        let mut used = vec![false; lines.len()];
        for start in 0..lines.len() {
            if used[start] {
                continue;
            }
            if let Some(chain) = trace_chain(start, &lines, &mut used) {
                let kind = classify_chain(&chain, &lines);
                if kind != ProfileKind::Open || chain.len() >= 2 {
                    profiles.push(Profile::new(
                        format!("profile:loop:{}", profiles.len()),
                        chain,
                        kind,
                    ));
                }
            }
        }
    }

    Ok(profiles)
}

/// A loop segment: a line, or an arc with both endpoint points.
struct ChainEdge {
    id: EntityId,
    start: EntityId,
    end: EntityId,
    is_arc: bool,
}

/// Fewest segments that close a loop: three lines, or two segments when one
/// is an arc (for example a half-circle and its chord).
fn closes_with(chain: &[EntityId], lines: &[&ChainEdge]) -> bool {
    chain.len() >= 3
        || (chain.len() == 2
            && chain
                .iter()
                .any(|id| lines.iter().any(|l| &l.id == id && l.is_arc)))
}

fn trace_chain(start: usize, lines: &[&ChainEdge], used: &mut [bool]) -> Option<Vec<EntityId>> {
    let start_line = lines[start];
    let mut chain = vec![start_line.id.clone()];
    let mut current_end = start_line.end.clone();
    used[start] = true;

    let target_start = start_line.start.clone();
    let max_steps = lines.len() + 1;

    for _ in 0..max_steps {
        if current_end == target_start && closes_with(&chain, lines) {
            return Some(chain);
        }

        let mut found = false;
        for (i, line) in lines.iter().enumerate() {
            if used[i] {
                continue;
            }
            if line.start == current_end {
                chain.push(line.id.clone());
                current_end = line.end.clone();
                used[i] = true;
                found = true;
                break;
            } else if line.end == current_end {
                chain.push(line.id.clone());
                current_end = line.start.clone();
                used[i] = true;
                found = true;
                break;
            }
        }

        if !found {
            if chain.len() >= 2 {
                return Some(chain);
            }
            unwind_used(&chain, lines, used);
            return None;
        }
    }

    unwind_used(&chain, lines, used);
    None
}

fn unwind_used(chain: &[EntityId], lines: &[&ChainEdge], used: &mut [bool]) {
    for (i, u) in used.iter_mut().enumerate() {
        if chain.iter().any(|id| id == &lines[i].id) {
            *u = false;
        }
    }
}

fn classify_chain(entity_ids: &[EntityId], lines: &[&ChainEdge]) -> ProfileKind {
    let mut point_visit_count: indexmap::IndexMap<String, usize> = indexmap::IndexMap::new();

    for line_id in entity_ids {
        let Some(line) = lines.iter().find(|l| &l.id == line_id) else {
            continue;
        };
        *point_visit_count
            .entry(line.start.as_str().to_string())
            .or_insert(0) += 1;
        *point_visit_count
            .entry(line.end.as_str().to_string())
            .or_insert(0) += 1;
    }

    let start = lines
        .iter()
        .find(|l| entity_ids.first() == Some(&l.id))
        .map(|l| l.start.as_str().to_string());

    let end = entity_ids
        .last()
        .and_then(|id| lines.iter().find(|l| &l.id == id))
        .map(|l| l.end.as_str().to_string());

    if let (Some(s), Some(e)) = (start, end) {
        if s == e && closes_with(entity_ids, lines) {
            if point_visit_count.values().any(|&c| c > 2) {
                return ProfileKind::SelfIntersecting;
            }
            return ProfileKind::Closed;
        }
    }

    if point_visit_count.values().any(|&c| c > 2) {
        return ProfileKind::SelfIntersecting;
    }

    ProfileKind::Open
}

/// Assign stable profile refs for the outermost closed profile in a sketch.
pub fn assign_profile_refs(sketch_id: &str, profiles: &mut [Profile]) {
    let outer = profiles
        .iter()
        .position(|p| p.kind == ProfileKind::Closed)
        .or_else(|| profiles.iter().position(|p| p.is_closed()));

    if let Some(idx) = outer {
        profiles[idx].profile_ref = Some(format!("{sketch_id}/profile:outer"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{CircleEntity, Coord, EntityBase, LineEntity};

    fn base(id: &str) -> EntityBase {
        EntityBase {
            id: EntityId::new(id).expect("id"),
            construction: false,
        }
    }

    fn half_disc(with_endpoints: bool) -> Vec<SketchEntity> {
        let id = |value: &str| EntityId::new(value).expect("id");
        vec![
            SketchEntity::Line(LineEntity {
                base: base("ent:chord"),
                start: id("ent:a"),
                end: id("ent:b"),
            }),
            SketchEntity::Arc(crate::entity::ArcEntity {
                base: base("ent:arc"),
                center: id("ent:c"),
                radius: Coord::literal(0.01),
                start_angle: Coord::literal(0.0),
                end_angle: Coord::literal(std::f64::consts::PI),
                start_point: with_endpoints.then(|| id("ent:b")),
                end_point: with_endpoints.then(|| id("ent:a")),
            }),
        ]
    }

    /// ADR-021: an arc with endpoint points closes a loop, even with only
    /// a chord (two segments); without endpoints it cannot join a loop.
    #[test]
    fn arcs_with_endpoints_close_loops() {
        let profiles = detect_profiles(&half_disc(true)).expect("profiles");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].kind, ProfileKind::Closed);
        assert_eq!(profiles[0].entity_ids.len(), 2);

        let profiles = detect_profiles(&half_disc(false)).expect("profiles");
        assert!(profiles.iter().all(|profile| !profile.is_closed()));
    }

    fn ent(id: &str) -> EntityId {
        EntityId::new(id).expect("valid id")
    }

    fn line(id: &str, start: &str, end: &str) -> SketchEntity {
        SketchEntity::Line(LineEntity {
            base: EntityBase {
                id: ent(id),
                construction: false,
            },
            start: ent(start),
            end: ent(end),
        })
    }

    #[test]
    fn detects_rectangular_closed_profile() {
        let entities = vec![
            line("ent:e0", "ent:c0", "ent:c1"),
            line("ent:e1", "ent:c1", "ent:c2"),
            line("ent:e2", "ent:c2", "ent:c3"),
            line("ent:e3", "ent:c3", "ent:c0"),
        ];
        let profiles = detect_profiles(&entities).expect("detect");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].kind, ProfileKind::Closed);
        assert_eq!(profiles[0].entity_ids.len(), 4);
    }

    #[test]
    fn detects_open_chain() {
        let entities = vec![
            line("ent:e0", "ent:c0", "ent:c1"),
            line("ent:e1", "ent:c1", "ent:c2"),
        ];
        let profiles = detect_profiles(&entities).expect("detect");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].kind, ProfileKind::Open);
    }

    #[test]
    fn detects_circle_profile() {
        let entities = vec![SketchEntity::Circle(CircleEntity {
            base: EntityBase {
                id: ent("ent:circle_1"),
                construction: false,
            },
            center: ent("ent:pt_center"),
            radius: Coord::literal(5.0),
        })];
        let profiles = detect_profiles(&entities).expect("detect");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].kind, ProfileKind::Closed);
    }

    #[test]
    fn profile_round_trip() {
        let profile = Profile::new(
            "profile:0",
            vec![ent("ent:e0"), ent("ent:e1")],
            ProfileKind::Closed,
        )
        .with_ref("sketch:base/profile:outer");
        let json = serde_json::to_string(&profile).expect("serialize");
        let restored: Profile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(profile, restored);
    }

    #[test]
    fn assigns_outer_profile_ref() {
        let mut profiles = vec![
            Profile::new("profile:0", vec![ent("ent:e0")], ProfileKind::Open),
            Profile::new(
                "profile:1",
                vec![ent("ent:e1"), ent("ent:e2")],
                ProfileKind::Closed,
            ),
        ];
        assign_profile_refs("sketch:base", &mut profiles);
        assert_eq!(
            profiles[1].profile_ref.as_deref(),
            Some("sketch:base/profile:outer")
        );
    }
}
