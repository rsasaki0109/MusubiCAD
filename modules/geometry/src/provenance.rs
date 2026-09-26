//! Fail-closed semantic reference provenance (MCAD-P6-003).
//!
//! Every semantic [`TopoRef`] resolution against regenerated topology is
//! classified as `exact`, `derived`, `fingerprint`, `ambiguous`, or `missing`.
//! Equal-scoring candidates are reported as `ambiguous` instead of being chosen
//! by incidental kernel order, and a required reference that is ambiguous or
//! missing fails closed.

use std::collections::BTreeSet;

use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};

use crate::refs::{TopoRef, TopoRefKind, TopoRefTolerancePolicy};
use crate::topo_sync::{
    edge_match_candidates, face_match_candidates, EdgeRefDiscovery, FaceDerivation,
    FaceRefDiscovery,
};

/// Score distance treated as an exact tie during candidate classification.
const TIE_SCORE_EPSILON: f64 = 1e-9;

/// How a semantic reference was resolved against the regenerated topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceStatus {
    /// Stored kernel id is present unchanged in the regenerated body.
    Exact,
    /// Stored kernel id was remapped through derivation history to a current id.
    Derived,
    /// No usable stored id; matched by role and/or geometric fingerprint.
    Fingerprint,
    /// More than one candidate had an equal best score; no deterministic choice.
    Ambiguous,
    /// No candidate satisfied the reference.
    Missing,
}

impl ReferenceStatus {
    /// Whether the status means the reference could not be pinned to one face/edge.
    pub fn is_unresolved(self) -> bool {
        matches!(self, Self::Ambiguous | Self::Missing)
    }
}

/// One scored candidate considered for a reference resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateEvidence {
    pub kernel_id: u64,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Fail-closed provenance for one semantic reference resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceProvenance {
    pub ref_id: String,
    pub kind: TopoRefKind,
    pub status: ReferenceStatus,
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Kernel id selected for Exact/Derived/Fingerprint; absent when ambiguous/missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_kernel_id: Option<u64>,
    /// Number of distinct candidates considered after scoring.
    pub candidate_count: usize,
    /// Human-readable reason for the outcome.
    pub reason: String,
    /// Candidate kernel ids and scores that were considered (best first).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<CandidateEvidence>,
    /// When true, an ambiguous or missing result fails closed instead of returning.
    pub required: bool,
}

impl ReferenceProvenance {
    /// Assert the reference resolved to a single current face/edge.
    pub fn assert_resolved(&self) -> Result<()> {
        if self.resolved_kernel_id.is_some() {
            return Ok(());
        }
        Err(OpenCadError::validation(format!(
            "required reference '{}' ({:?}) is {:?}: {}",
            self.ref_id, self.kind, self.status, self.reason
        )))
    }
}

/// Outcome of a provenance-aware resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceResolution {
    pub provenance: ReferenceProvenance,
    /// Present only for Exact/Derived/Fingerprint.
    pub resolved_kernel_id: Option<u64>,
}

fn finish(resolution: ReferenceResolution, required: bool) -> Result<ReferenceResolution> {
    if required {
        resolution.provenance.assert_resolved()?;
    }
    Ok(resolution)
}

fn provenance_from(
    topo_ref: &TopoRef,
    status: ReferenceStatus,
    resolved_kernel_id: Option<u64>,
    candidates: &[CandidateEvidence],
    reason: impl Into<String>,
    required: bool,
) -> ReferenceProvenance {
    ReferenceProvenance {
        ref_id: topo_ref.ref_id.as_str().to_string(),
        kind: topo_ref.kind,
        status,
        created_by: topo_ref.semantic.created_by.clone(),
        role: topo_ref.semantic.role.clone(),
        resolved_kernel_id,
        candidate_count: candidates.len(),
        reason: reason.into(),
        candidates: candidates.to_vec(),
        required,
    }
}

/// Classify scored candidates into a fingerprint pick or an ambiguity.
///
/// Returns `(status, resolved_kernel_id)`:
/// - `Some((Fingerprint, id))` when exactly one distinct candidate has the best score;
/// - `Some((Ambiguous, 0))` when two or more distinct candidates tie for best;
/// - `None` when there are no candidates.
fn classify_candidates(candidates: &[CandidateEvidence]) -> Option<(ReferenceStatus, u64)> {
    if candidates.is_empty() {
        return None;
    }
    let best_score = candidates
        .iter()
        .map(|candidate| candidate.score)
        .fold(f64::NEG_INFINITY, f64::max);
    let top_ids: BTreeSet<u64> = candidates
        .iter()
        .filter(|candidate| (candidate.score - best_score).abs() <= TIE_SCORE_EPSILON)
        .map(|candidate| candidate.kernel_id)
        .collect();
    if top_ids.len() > 1 {
        return Some((ReferenceStatus::Ambiguous, 0));
    }
    Some((
        ReferenceStatus::Fingerprint,
        *top_ids.iter().next().expect("non-empty top ids"),
    ))
}

fn resolve_fingerprint(
    topo_ref: &TopoRef,
    candidates: Vec<CandidateEvidence>,
    required: bool,
) -> Result<ReferenceResolution> {
    let Some((status, kernel_id)) = classify_candidates(&candidates) else {
        let provenance = provenance_from(
            topo_ref,
            ReferenceStatus::Missing,
            None,
            &candidates,
            "no face or edge discovery satisfies the reference",
            required,
        );
        return finish(
            ReferenceResolution {
                provenance,
                resolved_kernel_id: None,
            },
            required,
        );
    };
    if status == ReferenceStatus::Ambiguous {
        let best_score = candidates
            .iter()
            .map(|candidate| candidate.score)
            .fold(f64::NEG_INFINITY, f64::max);
        let tied = candidates
            .iter()
            .filter(|candidate| (candidate.score - best_score).abs() <= TIE_SCORE_EPSILON)
            .count();
        let reason =
            format!("{tied} candidates tie with equal best score; refusing an arbitrary pick");
        let provenance = provenance_from(topo_ref, status, None, &candidates, reason, required);
        return finish(
            ReferenceResolution {
                provenance,
                resolved_kernel_id: None,
            },
            required,
        );
    }
    let provenance = provenance_from(
        topo_ref,
        ReferenceStatus::Fingerprint,
        Some(kernel_id),
        &candidates,
        "matched by role and/or geometric fingerprint",
        required,
    );
    Ok(ReferenceResolution {
        provenance,
        resolved_kernel_id: Some(kernel_id),
    })
}

/// Resolve a face reference with fail-closed provenance.
pub fn resolve_face_ref_with_provenance(
    semantic_refs: &[TopoRef],
    face_history: &[FaceDerivation],
    ref_id: &str,
    discoveries: Option<&[FaceRefDiscovery]>,
    policy: TopoRefTolerancePolicy,
    required: bool,
) -> Result<ReferenceResolution> {
    policy.validate()?;
    let Some(topo_ref) = semantic_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
    else {
        let provenance = ReferenceProvenance {
            ref_id: ref_id.to_string(),
            kind: TopoRefKind::Face,
            status: ReferenceStatus::Missing,
            created_by: String::new(),
            role: None,
            resolved_kernel_id: None,
            candidate_count: 0,
            reason: "reference is not present in the document".into(),
            candidates: Vec::new(),
            required,
        };
        return finish(
            ReferenceResolution {
                provenance,
                resolved_kernel_id: None,
            },
            required,
        );
    };

    let candidates = discoveries
        .map(|discoveries| face_match_candidates(topo_ref, discoveries, policy))
        .unwrap_or_default();

    if let Some(stored_id) = topo_ref.kernel_face_id() {
        // Stored IDs are final-body indices (ADR-018) and are not remapped
        // through per-operation history, so nothing is reported as derived.
        let _ = face_history;
        let resolved_id = stored_id;
        let derived = false;
        let verified = discoveries
            .map(|discoveries| {
                discoveries.iter().any(|d| {
                    d.kernel_face_id == resolved_id
                        && crate::topo_sync::role_agrees(topo_ref, &d.role)
                })
            })
            .unwrap_or(true);
        if verified {
            let (status, reason) = if derived {
                (
                    ReferenceStatus::Derived,
                    format!(
                        "stored kernel face {stored_id} remapped through derivation history to current face {resolved_id}"
                    ),
                )
            } else {
                (
                    ReferenceStatus::Exact,
                    format!(
                        "stored kernel face {resolved_id}{}",
                        if discoveries.is_some() {
                            " present in the regenerated body"
                        } else {
                            " accepted without discovery verification"
                        }
                    ),
                )
            };
            let provenance = provenance_from(
                topo_ref,
                status,
                Some(resolved_id),
                &candidates,
                reason,
                required,
            );
            return Ok(ReferenceResolution {
                provenance,
                resolved_kernel_id: Some(resolved_id),
            });
        }
    }

    resolve_fingerprint(topo_ref, candidates, required)
}

/// Resolve an edge reference with fail-closed provenance.
pub fn resolve_edge_ref_with_provenance(
    semantic_refs: &[TopoRef],
    ref_id: &str,
    discoveries: Option<&[EdgeRefDiscovery]>,
    policy: TopoRefTolerancePolicy,
    required: bool,
) -> Result<ReferenceResolution> {
    policy.validate()?;
    let Some(topo_ref) = semantic_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
    else {
        let provenance = ReferenceProvenance {
            ref_id: ref_id.to_string(),
            kind: TopoRefKind::Edge,
            status: ReferenceStatus::Missing,
            created_by: String::new(),
            role: None,
            resolved_kernel_id: None,
            candidate_count: 0,
            reason: "reference is not present in the document".into(),
            candidates: Vec::new(),
            required,
        };
        return finish(
            ReferenceResolution {
                provenance,
                resolved_kernel_id: None,
            },
            required,
        );
    };

    let candidates = discoveries
        .map(|discoveries| edge_match_candidates(topo_ref, discoveries, policy))
        .unwrap_or_default();

    if let Some(stored_id) = topo_ref.kernel_edge_id() {
        let verified = discoveries
            .map(|discoveries| {
                discoveries.iter().any(|d| {
                    d.kernel_edge_id == stored_id
                        && crate::topo_sync::role_agrees(topo_ref, &d.role)
                })
            })
            .unwrap_or(true);
        if verified {
            let provenance = provenance_from(
                topo_ref,
                ReferenceStatus::Exact,
                Some(stored_id),
                &candidates,
                format!(
                    "stored kernel edge {stored_id}{}",
                    if discoveries.is_some() {
                        " present in the regenerated body"
                    } else {
                        " accepted without discovery verification"
                    }
                ),
                required,
            );
            return Ok(ReferenceResolution {
                provenance,
                resolved_kernel_id: Some(stored_id),
            });
        }
    }

    resolve_fingerprint(topo_ref, candidates, required)
}

/// Classify every document reference against the final discoveries (observability).
///
/// Uses `required = false` so the report records `ambiguous` and `missing`
/// outcomes instead of failing regeneration; the review or commit layer decides
/// whether to block on them.
pub fn resolve_all_reference_provenance(
    semantic_refs: &[TopoRef],
    face_history: &[FaceDerivation],
    face_discoveries: Option<&[FaceRefDiscovery]>,
    edge_discoveries: Option<&[EdgeRefDiscovery]>,
    policy: TopoRefTolerancePolicy,
) -> Vec<ReferenceProvenance> {
    let mut provenances = Vec::with_capacity(semantic_refs.len());
    for topo_ref in semantic_refs {
        let resolution = match topo_ref.kind {
            TopoRefKind::Face => resolve_face_ref_with_provenance(
                semantic_refs,
                face_history,
                topo_ref.ref_id.as_str(),
                face_discoveries,
                policy,
                false,
            ),
            TopoRefKind::Edge => resolve_edge_ref_with_provenance(
                semantic_refs,
                topo_ref.ref_id.as_str(),
                edge_discoveries,
                policy,
                false,
            ),
            TopoRefKind::Vertex => Ok(ReferenceResolution {
                provenance: ReferenceProvenance {
                    ref_id: topo_ref.ref_id.as_str().to_string(),
                    kind: TopoRefKind::Vertex,
                    status: ReferenceStatus::Missing,
                    created_by: topo_ref.semantic.created_by.clone(),
                    role: topo_ref.semantic.role.clone(),
                    resolved_kernel_id: None,
                    candidate_count: 0,
                    reason: "vertex references are not resolved by the current kernel".into(),
                    candidates: Vec::new(),
                    required: false,
                },
                resolved_kernel_id: None,
            }),
        };
        if let Ok(resolution) = resolution {
            provenances.push(resolution.provenance);
        }
    }
    provenances.sort_by(|left, right| left.ref_id.cmp(&right.ref_id));
    provenances
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs::TopoRef;
    use opencad_core::TopoRefId;

    fn face_discovery(kernel_face_id: u64, role: &str, feature: &str) -> FaceRefDiscovery {
        FaceRefDiscovery {
            kernel_face_id,
            role: role.into(),
            normal_m: [0.0, 0.0, 1.0],
            centroid_m: [0.0, 0.0, 0.006],
            feature_id: Some(feature.into()),
        }
    }

    fn edge_discovery(kernel_edge_id: u64, role: &str, feature: &str) -> EdgeRefDiscovery {
        EdgeRefDiscovery {
            kernel_edge_id,
            role: role.into(),
            midpoint_m: [0.0, 0.0, 0.006],
            tangent_m: [1.0, 0.0, 0.0],
            length_m: 0.01,
            feature_id: Some(feature.into()),
        }
    }

    #[test]
    fn exact_face_is_classified_without_fallback() {
        let refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:top").expect("id"),
            "feature:extrude_base",
            "top",
            42,
            [0.0, 0.0, 1.0],
        )];
        let discoveries = [face_discovery(42, "top", "feature:extrude_base")];
        let resolution = resolve_face_ref_with_provenance(
            &refs,
            &[],
            "ref:face:top",
            Some(&discoveries),
            TopoRefTolerancePolicy::default(),
            true,
        )
        .expect("exact resolution");
        assert_eq!(resolution.provenance.status, ReferenceStatus::Exact);
        assert_eq!(resolution.resolved_kernel_id, Some(42));
        assert_eq!(resolution.provenance.candidate_count, 1);
    }

    #[test]
    /// ADR-018: stored IDs are final-body indices and are not remapped
    /// through per-operation history; a stale ID falls back to fingerprints.
    fn history_does_not_remap_a_stored_face_id() {
        let refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:top").expect("id"),
            "feature:fillet_top",
            "top",
            100,
            [0.0, 0.0, 1.0],
        )];
        let history = [(200, 100)];
        let discoveries = [face_discovery(200, "top", "feature:fillet_top")];
        let resolution = resolve_face_ref_with_provenance(
            &refs,
            &history,
            "ref:face:top",
            Some(&discoveries),
            TopoRefTolerancePolicy::default(),
            true,
        )
        .expect("fingerprint resolution");
        assert_eq!(resolution.provenance.status, ReferenceStatus::Fingerprint);
        assert_eq!(resolution.resolved_kernel_id, Some(200));
    }

    #[test]
    fn fingerprint_face_is_classified_without_stored_id() {
        let refs = vec![TopoRef::face(
            TopoRefId::new("ref:face:top").expect("id"),
            "feature:extrude_base",
            "top",
        )];
        let discoveries = [face_discovery(77, "top", "feature:extrude_base")];
        let resolution = resolve_face_ref_with_provenance(
            &refs,
            &[],
            "ref:face:top",
            Some(&discoveries),
            TopoRefTolerancePolicy::default(),
            true,
        )
        .expect("fingerprint resolution");
        assert_eq!(resolution.provenance.status, ReferenceStatus::Fingerprint);
        assert_eq!(resolution.resolved_kernel_id, Some(77));
    }

    #[test]
    fn ambiguous_face_is_reported_and_blocks_when_required() {
        let refs = vec![TopoRef::face(
            TopoRefId::new("ref:face:top").expect("id"),
            "feature:extrude_base",
            "top",
        )];
        let discoveries = [
            face_discovery(10, "top", "feature:extrude_base"),
            face_discovery(11, "top", "feature:extrude_base"),
        ];
        let resolution = resolve_face_ref_with_provenance(
            &refs,
            &[],
            "ref:face:top",
            Some(&discoveries),
            TopoRefTolerancePolicy::default(),
            false,
        )
        .expect("non-required ambiguous resolution");
        assert_eq!(resolution.provenance.status, ReferenceStatus::Ambiguous);
        assert_eq!(resolution.resolved_kernel_id, None);
        assert_eq!(resolution.provenance.candidate_count, 2);

        let required = resolve_face_ref_with_provenance(
            &refs,
            &[],
            "ref:face:top",
            Some(&discoveries),
            TopoRefTolerancePolicy::default(),
            true,
        );
        assert!(
            required.is_err(),
            "ambiguous required reference must fail closed"
        );
        assert!(required
            .unwrap_err()
            .to_string()
            .to_lowercase()
            .contains("ambiguous"));
    }

    #[test]
    fn ambiguity_is_independent_of_input_order() {
        let refs = vec![TopoRef::face(
            TopoRefId::new("ref:face:top").expect("id"),
            "feature:extrude_base",
            "top",
        )];
        let first = face_discovery(10, "top", "feature:extrude_base");
        let second = face_discovery(11, "top", "feature:extrude_base");
        for discoveries in [
            vec![first.clone(), second.clone()],
            vec![second.clone(), first.clone()],
        ] {
            let resolution = resolve_face_ref_with_provenance(
                &refs,
                &[],
                "ref:face:top",
                Some(&discoveries),
                TopoRefTolerancePolicy::default(),
                false,
            )
            .expect("non-required resolution");
            assert_eq!(resolution.provenance.status, ReferenceStatus::Ambiguous);
            assert_eq!(resolution.resolved_kernel_id, None);
        }
    }

    #[test]
    fn missing_face_is_classified_and_blocks_when_required() {
        let refs = vec![TopoRef::face(
            TopoRefId::new("ref:face:top").expect("id"),
            "feature:extrude_base",
            "top",
        )];
        let discoveries = [face_discovery(77, "bottom", "feature:extrude_base")];
        let resolution = resolve_face_ref_with_provenance(
            &refs,
            &[],
            "ref:face:top",
            Some(&discoveries),
            TopoRefTolerancePolicy::default(),
            false,
        )
        .expect("non-required missing resolution");
        assert_eq!(resolution.provenance.status, ReferenceStatus::Missing);
        assert_eq!(resolution.resolved_kernel_id, None);

        let required = resolve_face_ref_with_provenance(
            &refs,
            &[],
            "ref:face:top",
            Some(&discoveries),
            TopoRefTolerancePolicy::default(),
            true,
        );
        assert!(
            required.is_err(),
            "missing required reference must fail closed"
        );
    }

    #[test]
    fn edge_fingerprint_resolution_classifies_exact_and_missing() {
        let exact = vec![TopoRef::kernel_edge(
            TopoRefId::new("ref:edge:rim").expect("id"),
            "feature:extrude_base",
            "rim",
            12,
            [0.0, 0.0, 0.006],
            [1.0, 0.0, 0.0],
        )];
        let discoveries = [edge_discovery(12, "rim", "feature:extrude_base")];
        let resolution = resolve_edge_ref_with_provenance(
            &exact,
            "ref:edge:rim",
            Some(&discoveries),
            TopoRefTolerancePolicy::default(),
            true,
        )
        .expect("exact edge");
        assert_eq!(resolution.provenance.status, ReferenceStatus::Exact);

        let tied = vec![TopoRef::edge(
            TopoRefId::new("ref:edge:rim").expect("id"),
            "feature:extrude_base",
            "rim",
        )];
        let ambiguous = [
            edge_discovery(1, "rim", "feature:extrude_base"),
            edge_discovery(2, "rim", "feature:extrude_base"),
        ];
        let resolution = resolve_edge_ref_with_provenance(
            &tied,
            "ref:edge:rim",
            Some(&ambiguous),
            TopoRefTolerancePolicy::default(),
            false,
        )
        .expect("non-required ambiguous edge");
        assert_eq!(resolution.provenance.status, ReferenceStatus::Ambiguous);
    }

    #[test]
    fn all_reference_provenance_is_sorted_and_observability_only() {
        let refs = vec![
            TopoRef::face(
                TopoRefId::new("ref:face:top").expect("id"),
                "feature:extrude_base",
                "top",
            ),
            TopoRef::kernel_face(
                TopoRefId::new("ref:face:mount").expect("id"),
                "feature:extrude_base",
                "mount",
                42,
                [0.0, 0.0, 1.0],
            ),
        ];
        let discoveries = [
            face_discovery(42, "mount", "feature:extrude_base"),
            face_discovery(77, "top", "feature:extrude_base"),
        ];
        let provenances = resolve_all_reference_provenance(
            &refs,
            &[],
            Some(&discoveries),
            None,
            TopoRefTolerancePolicy::default(),
        );
        assert_eq!(provenances.len(), 2);
        assert_eq!(provenances[0].ref_id, "ref:face:mount");
        assert_eq!(provenances[0].status, ReferenceStatus::Exact);
        assert_eq!(provenances[1].ref_id, "ref:face:top");
        assert_eq!(provenances[1].status, ReferenceStatus::Fingerprint);
        assert!(provenances.iter().all(|provenance| !provenance.required));
    }
}
