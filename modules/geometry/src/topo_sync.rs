//! Semantic topology reference sync from tessellated B-Rep faces.

use opencad_core::{OpenCadError, Result, TopoRefId};

use crate::refs::{GeometricFingerprint, TopoRef};

/// Discovered B-Rep face from tessellation and feature inference.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceRefDiscovery {
    pub kernel_face_id: u64,
    pub role: String,
    pub normal_m: [f32; 3],
    pub centroid_m: [f32; 3],
    pub feature_id: Option<String>,
}

/// Discovered B-Rep edge from kernel topology inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeRefDiscovery {
    pub kernel_edge_id: u64,
    pub role: String,
    pub midpoint_m: [f32; 3],
    pub tangent_m: [f32; 3],
    pub length_m: f32,
    pub feature_id: Option<String>,
}

/// A face derivation pair from the kernel: `(post_id, src_id)`.
pub type FaceDerivation = (u64, u64);

pub fn kernel_topo_ref_id(kernel_face_id: u64) -> Result<TopoRefId> {
    TopoRefId::new(format!("ref:face:kernel_{kernel_face_id}"))
}

pub fn resolve_topo_ref_id(semantic_refs: &[TopoRef], kernel_face_id: u64) -> Option<String> {
    resolve_topo_ref_id_with_history(semantic_refs, kernel_face_id, &[])
}

pub fn resolve_topo_ref_id_with_history(
    semantic_refs: &[TopoRef],
    kernel_face_id: u64,
    history: &[FaceDerivation],
) -> Option<String> {
    if let Some(ref_id) = semantic_refs
        .iter()
        .find(|topo_ref| topo_ref.kernel_face_id() == Some(kernel_face_id))
        .map(|topo_ref| topo_ref.ref_id.as_str().to_string())
    {
        return Some(ref_id);
    }

    for (post_id, src_id) in history {
        if *post_id == kernel_face_id {
            if let Some(ref_id) = semantic_refs
                .iter()
                .find(|topo_ref| topo_ref.kernel_face_id() == Some(*src_id))
                .map(|topo_ref| topo_ref.ref_id.as_str().to_string())
            {
                return Some(ref_id);
            }
        }
    }

    kernel_topo_ref_id(kernel_face_id)
        .ok()
        .map(|id| id.as_str().to_string())
}

/// Resolve a persisted `ref:face:...` id to the current kernel face id for regeneration.
pub fn resolve_kernel_face_id_for_topo_ref(
    semantic_refs: &[TopoRef],
    face_history: &[FaceDerivation],
    ref_id: &str,
) -> Result<u64> {
    resolve_kernel_face_id_for_topo_ref_with_discoveries(semantic_refs, face_history, ref_id, None)
}

/// Resolve a topo ref id, optionally matching tessellated faces by geometric fingerprint.
pub fn resolve_kernel_face_id_for_topo_ref_with_discoveries(
    semantic_refs: &[TopoRef],
    face_history: &[FaceDerivation],
    ref_id: &str,
    discoveries: Option<&[FaceRefDiscovery]>,
) -> Result<u64> {
    let topo_ref = semantic_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
        .ok_or_else(|| OpenCadError::not_found(format!("topo ref '{ref_id}'")))?;

    if let Some(stored_id) = topo_ref.kernel_face_id() {
        let remap = build_src_to_post_map(face_history);
        return Ok(remap.get(&stored_id).copied().unwrap_or(stored_id));
    }

    if let Some(discoveries) = discoveries {
        if let Some(kernel_face_id) = match_face_discovery_for_topo_ref(topo_ref, discoveries) {
            return Ok(kernel_face_id);
        }
    }

    Err(OpenCadError::validation(format!(
        "topo ref '{ref_id}' has no kernel_face_id{}",
        if discoveries.is_some() {
            " and fingerprint fallback did not match a face"
        } else {
            "; run sync_topo_refs first or provide tessellated face discoveries"
        }
    )))
}

/// Resolve a persisted `ref:edge:...` id to the current kernel edge id for regeneration.
pub fn resolve_kernel_edge_id_for_topo_ref(
    semantic_refs: &[TopoRef],
    ref_id: &str,
    discoveries: Option<&[EdgeRefDiscovery]>,
) -> Result<u64> {
    let topo_ref = semantic_refs
        .iter()
        .find(|topo_ref| topo_ref.ref_id.as_str() == ref_id)
        .ok_or_else(|| OpenCadError::not_found(format!("topo ref '{ref_id}'")))?;

    if let Some(stored_id) = topo_ref.kernel_edge_id() {
        return Ok(stored_id);
    }

    if let Some(discoveries) = discoveries {
        if let Some(kernel_edge_id) = match_edge_discovery_for_topo_ref(topo_ref, discoveries) {
            return Ok(kernel_edge_id);
        }
    }

    Err(OpenCadError::validation(format!(
        "topo ref '{ref_id}' has no kernel_edge_id{}",
        if discoveries.is_some() {
            " and fingerprint fallback did not match an edge"
        } else {
            "; provide edge discoveries during regeneration"
        }
    )))
}

/// Match a discovered edge to a persisted topo ref using role and midpoint/tangent hints.
pub fn match_edge_discovery_for_topo_ref(
    topo_ref: &TopoRef,
    discoveries: &[EdgeRefDiscovery],
) -> Option<u64> {
    discoveries
        .iter()
        .filter(|discovery| edge_discovery_matches_topo_ref(topo_ref, discovery))
        .max_by(|left, right| {
            edge_fingerprint_match_score(topo_ref, left)
                .partial_cmp(&edge_fingerprint_match_score(topo_ref, right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .filter(|discovery| edge_fingerprint_match_score(topo_ref, discovery) > 0.0)
        .map(|discovery| discovery.kernel_edge_id)
}

fn edge_discovery_matches_topo_ref(topo_ref: &TopoRef, discovery: &EdgeRefDiscovery) -> bool {
    let created_by = topo_ref.semantic.created_by.as_str();
    let role_matches = topo_ref
        .semantic
        .role
        .as_deref()
        .map(|role| discovery.role == role)
        .unwrap_or(true);
    if !role_matches {
        return false;
    }

    if discovery.feature_id.as_deref() == Some(created_by) {
        return true;
    }

    topo_ref.geometric_fingerprint.is_none()
        || topo_ref
            .geometric_fingerprint
            .as_ref()
            .and_then(|fingerprint| fingerprint.bbox_hint.as_ref())
            .is_some()
}

fn edge_fingerprint_match_score(topo_ref: &TopoRef, discovery: &EdgeRefDiscovery) -> f64 {
    let mut score = 0.0;
    if topo_ref
        .semantic
        .role
        .as_deref()
        .is_some_and(|role| discovery.role == role)
    {
        score += 2.0;
    }
    if topo_ref.semantic.created_by == discovery.feature_id.as_deref().unwrap_or("") {
        score += 1.0;
    }
    score += discovery.length_m as f64;
    if let Some(fingerprint) = topo_ref.geometric_fingerprint.as_ref() {
        if let Some(hint) = fingerprint.bbox_hint.as_ref() {
            let midpoint_hint = hint[0];
            let tangent_hint = hint[1];
            let midpoint_dist = [
                discovery.midpoint_m[0] as f64 - midpoint_hint[0],
                discovery.midpoint_m[1] as f64 - midpoint_hint[1],
                discovery.midpoint_m[2] as f64 - midpoint_hint[2],
            ];
            let midpoint_len = (midpoint_dist[0] * midpoint_dist[0]
                + midpoint_dist[1] * midpoint_dist[1]
                + midpoint_dist[2] * midpoint_dist[2])
                .sqrt();
            if midpoint_len < 0.002 {
                score += 3.0 - midpoint_len * 1000.0;
            }
            let tangent_dot = discovery.tangent_m[0] as f64 * tangent_hint[0]
                + discovery.tangent_m[1] as f64 * tangent_hint[1]
                + discovery.tangent_m[2] as f64 * tangent_hint[2];
            if tangent_dot.abs() > 0.99 {
                score += 2.0;
            }
        }
    }
    score
}

/// Match a tessellated face to a persisted topo ref using role, feature, and normal hints.
pub fn match_face_discovery_for_topo_ref(
    topo_ref: &TopoRef,
    discoveries: &[FaceRefDiscovery],
) -> Option<u64> {
    discoveries
        .iter()
        .filter(|discovery| discovery_matches_topo_ref(topo_ref, discovery))
        .max_by(|left, right| {
            fingerprint_match_score(topo_ref, left)
                .partial_cmp(&fingerprint_match_score(topo_ref, right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .filter(|discovery| fingerprint_match_score(topo_ref, discovery) > 0.0)
        .map(|discovery| discovery.kernel_face_id)
}

fn discovery_matches_topo_ref(topo_ref: &TopoRef, discovery: &FaceRefDiscovery) -> bool {
    let created_by = topo_ref.semantic.created_by.as_str();
    let role_matches = topo_ref
        .semantic
        .role
        .as_deref()
        .map(|role| discovery.role == role)
        .unwrap_or(true);
    if !role_matches {
        return false;
    }

    if discovery.feature_id.as_deref() == Some(created_by) {
        return true;
    }

    topo_ref
        .geometric_fingerprint
        .as_ref()
        .map(|fingerprint| {
            fingerprint
                .adjacent_feature_ids
                .iter()
                .any(|feature_id| discovery.feature_id.as_deref() == Some(feature_id.as_str()))
        })
        .unwrap_or(false)
}

fn fingerprint_match_score(topo_ref: &TopoRef, discovery: &FaceRefDiscovery) -> f64 {
    let mut score = 0.0;
    if discovery.feature_id.as_deref() == Some(topo_ref.semantic.created_by.as_str()) {
        score += 2.0;
    }
    if topo_ref.semantic.role.as_deref() == Some(discovery.role.as_str()) {
        score += 2.0;
    }
    score += normal_alignment_score(topo_ref.semantic.normal_hint, discovery.normal_m) * 3.0;
    if topo_ref.kernel_face_id() == Some(discovery.kernel_face_id) {
        score += 5.0;
    }
    score
}

fn normal_alignment_score(normal_hint: Option<[f64; 3]>, normal_m: [f32; 3]) -> f64 {
    let Some(hint) = normal_hint else {
        return 0.0;
    };
    let hx = hint[0] as f32;
    let hy = hint[1] as f32;
    let hz = hint[2] as f32;
    let hint_len = (hx * hx + hy * hy + hz * hz).sqrt();
    let normal_len =
        (normal_m[0] * normal_m[0] + normal_m[1] * normal_m[1] + normal_m[2] * normal_m[2]).sqrt();
    if hint_len < 1e-9 || normal_len < 1e-9 {
        return 0.0;
    }
    let dot = (hx * normal_m[0] + hy * normal_m[1] + hz * normal_m[2]) / (hint_len * normal_len);
    f64::from(dot.abs())
}

/// Map source face ids to their latest post ids using kernel derivation history.
pub fn build_src_to_post_map(history: &[FaceDerivation]) -> std::collections::HashMap<u64, u64> {
    let mut map = std::collections::HashMap::new();
    for (post_id, src_id) in history {
        map.insert(*src_id, *post_id);
    }

    let mut changed = true;
    while changed {
        changed = false;
        for (src_id, post_id) in map.clone() {
            if let Some(next) = map.get(&post_id).copied() {
                if map.get(&src_id) != Some(&next) {
                    map.insert(src_id, next);
                    changed = true;
                }
            }
        }
    }

    map
}

/// Concatenate per-step `[post_id, src_id]` pairs from chronological kernel ops.
pub fn compose_face_derivation_histories(segments: &[&[FaceDerivation]]) -> Vec<FaceDerivation> {
    let mut composed = Vec::new();
    for segment in segments {
        composed.extend_from_slice(segment);
    }
    composed
}

/// Update persisted `kernel_face_id` values using `[post_id, src_id]` history pairs.
pub fn rebind_kernel_face_ids(semantic_refs: &mut [TopoRef], history: &[FaceDerivation]) {
    if history.is_empty() {
        return;
    }

    let remap = build_src_to_post_map(history);
    for topo_ref in semantic_refs.iter_mut() {
        let Some(stored_id) = topo_ref.kernel_face_id() else {
            continue;
        };
        let Some(new_id) = remap.get(&stored_id).copied() else {
            continue;
        };
        if new_id == stored_id {
            continue;
        }
        if let Some(fingerprint) = topo_ref.geometric_fingerprint.as_mut() {
            fingerprint.kernel_face_id = Some(new_id);
        }
    }
}

/// Rebind stored ids via history, then merge discovered faces into semantic refs.
pub fn sync_semantic_refs_with_history(
    existing: &[TopoRef],
    history: &[FaceDerivation],
    discoveries: &[FaceRefDiscovery],
) -> Vec<TopoRef> {
    let mut refs = existing.to_vec();
    rebind_kernel_face_ids(&mut refs, history);
    sync_semantic_refs(&refs, discoveries)
}

/// Merge discovered kernel faces into persisted semantic references.
pub fn sync_semantic_refs(existing: &[TopoRef], discoveries: &[FaceRefDiscovery]) -> Vec<TopoRef> {
    let mut refs: Vec<TopoRef> = existing.to_vec();

    for discovery in discoveries {
        if let Some(index) = refs
            .iter()
            .position(|topo_ref| topo_ref.kernel_face_id() == Some(discovery.kernel_face_id))
        {
            refs[index].geometric_fingerprint = Some(fingerprint_from(discovery));
            if discovery.feature_id.is_some() {
                refs[index].semantic.created_by = discovery.feature_id.clone().unwrap_or_default();
            }
            if !discovery.role.is_empty() {
                refs[index].semantic.role = Some(discovery.role.clone());
            }
            continue;
        }

        if let Some(feature_id) = discovery.feature_id.as_deref() {
            let candidate_indexes: Vec<usize> = refs
                .iter()
                .enumerate()
                .filter(|(_, topo_ref)| {
                    topo_ref.semantic.created_by == feature_id
                        && topo_ref.semantic.role.as_deref() == Some(discovery.role.as_str())
                })
                .map(|(index, _)| index)
                .collect();
            if let Some(index) = candidate_indexes
                .iter()
                .copied()
                .find(|&index| refs[index].kernel_face_id().is_none())
            {
                refs[index].geometric_fingerprint = Some(fingerprint_from(discovery));
                refs[index].semantic.normal_hint = Some([
                    discovery.normal_m[0] as f64,
                    discovery.normal_m[1] as f64,
                    discovery.normal_m[2] as f64,
                ]);
                continue;
            }
            if let Some(index) = candidate_indexes.into_iter().max_by(|left, right| {
                fingerprint_match_score(&refs[*left], discovery)
                    .partial_cmp(&fingerprint_match_score(&refs[*right], discovery))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                refs[index].geometric_fingerprint = Some(fingerprint_from(discovery));
                refs[index].semantic.normal_hint = Some([
                    discovery.normal_m[0] as f64,
                    discovery.normal_m[1] as f64,
                    discovery.normal_m[2] as f64,
                ]);
                continue;
            }
        }

        let ref_id = kernel_topo_ref_id(discovery.kernel_face_id).unwrap_or_else(|_| {
            TopoRefId::new(format!("ref:face:discovered_{}", discovery.kernel_face_id))
                .expect("fallback topo ref id")
        });
        let created_by = discovery
            .feature_id
            .clone()
            .unwrap_or_else(|| "feature:active_body".into());
        refs.push(TopoRef::kernel_face(
            ref_id,
            created_by,
            discovery.role.clone(),
            discovery.kernel_face_id,
            discovery.normal_m,
        ));
    }

    refs.sort_by(|left, right| left.ref_id.as_str().cmp(right.ref_id.as_str()));
    refs
}

pub fn assign_face_ref_to_refs(
    semantic_refs: &mut Vec<TopoRef>,
    kernel_face_id: u64,
    ref_id: TopoRefId,
    created_by: impl Into<String>,
    role: impl Into<String>,
    normal_m: [f32; 3],
) -> Result<()> {
    if let Some(index) = semantic_refs
        .iter()
        .position(|topo_ref| topo_ref.kernel_face_id() == Some(kernel_face_id))
    {
        semantic_refs[index].ref_id = ref_id;
        semantic_refs[index].semantic.created_by = created_by.into();
        semantic_refs[index].semantic.role = Some(role.into());
        semantic_refs[index].geometric_fingerprint = Some(GeometricFingerprint {
            surface_type: "brep_face".into(),
            kernel_face_id: Some(kernel_face_id),
            kernel_edge_id: None,
            area_range: None,
            bbox_hint: None,
            adjacent_feature_ids: Vec::new(),
        });
        semantic_refs[index].semantic.normal_hint =
            Some([normal_m[0] as f64, normal_m[1] as f64, normal_m[2] as f64]);
        return Ok(());
    }

    semantic_refs.push(TopoRef::kernel_face(
        ref_id,
        created_by,
        role,
        kernel_face_id,
        normal_m,
    ));
    semantic_refs.sort_by(|left, right| left.ref_id.as_str().cmp(right.ref_id.as_str()));
    Ok(())
}

/// Assign or update a named face ref, with optional kernel face id.
pub fn assign_named_face_ref(
    semantic_refs: &mut Vec<TopoRef>,
    ref_id: TopoRefId,
    created_by: impl Into<String>,
    role: impl Into<String>,
    kernel_face_id: Option<u64>,
    normal_m: [f32; 3],
) -> Result<()> {
    if let Some(kernel_face_id) = kernel_face_id.filter(|&id| id != 0) {
        return assign_face_ref_to_refs(
            semantic_refs,
            kernel_face_id,
            ref_id,
            created_by,
            role,
            normal_m,
        );
    }

    if let Some(index) = semantic_refs
        .iter()
        .position(|topo_ref| topo_ref.ref_id.as_str() == ref_id.as_str())
    {
        semantic_refs[index].semantic.created_by = created_by.into();
        semantic_refs[index].semantic.role = Some(role.into());
        semantic_refs[index].semantic.normal_hint =
            Some([normal_m[0] as f64, normal_m[1] as f64, normal_m[2] as f64]);
        return Ok(());
    }

    let mut topo_ref = TopoRef::face(ref_id, created_by, role);
    topo_ref.semantic.normal_hint =
        Some([normal_m[0] as f64, normal_m[1] as f64, normal_m[2] as f64]);
    semantic_refs.push(topo_ref);
    semantic_refs.sort_by(|left, right| left.ref_id.as_str().cmp(right.ref_id.as_str()));
    Ok(())
}

pub fn validate_kernel_face_on_mesh(
    mesh: &crate::tessellation::MeshSet,
    kernel_face_id: u64,
) -> Result<()> {
    if !mesh.has_triangle_face_ids() {
        return Err(OpenCadError::validation(
            "mesh has no triangle_face_ids; tessellate with OCCT first",
        ));
    }
    if mesh.triangle_face_ids.contains(&kernel_face_id) {
        Ok(())
    } else {
        Err(OpenCadError::not_found(format!(
            "kernel face '{kernel_face_id}' not found in tessellated body"
        )))
    }
}

fn fingerprint_from(discovery: &FaceRefDiscovery) -> GeometricFingerprint {
    GeometricFingerprint {
        surface_type: "brep_face".into(),
        kernel_face_id: Some(discovery.kernel_face_id),
        kernel_edge_id: None,
        area_range: None,
        bbox_hint: Some([
            [
                discovery.centroid_m[0] as f64,
                discovery.centroid_m[1] as f64,
                discovery.centroid_m[2] as f64,
            ],
            [
                discovery.centroid_m[0] as f64,
                discovery.centroid_m[1] as f64,
                discovery.centroid_m[2] as f64,
            ],
        ]),
        adjacent_feature_ids: discovery.feature_id.clone().into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_creates_kernel_face_refs() {
        let refs = sync_semantic_refs(
            &[],
            &[FaceRefDiscovery {
                kernel_face_id: 42,
                role: "top".into(),
                normal_m: [0.0, 0.0, 1.0],
                centroid_m: [0.0, 0.0, 0.006],
                feature_id: Some("feature:extrude_base".into()),
            }],
        );
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kernel_face_id(), Some(42));
        assert_eq!(refs[0].ref_id.as_str(), "ref:face:kernel_42");
    }

    #[test]
    fn sync_updates_existing_role_matched_ref() {
        let existing = vec![TopoRef::face(
            TopoRefId::new("ref:face:extrude_base_top").expect("id"),
            "feature:extrude_base",
            "top",
        )];
        let refs = sync_semantic_refs(
            &existing,
            &[FaceRefDiscovery {
                kernel_face_id: 99,
                role: "top".into(),
                normal_m: [0.0, 0.0, 1.0],
                centroid_m: [0.0, 0.0, 0.006],
                feature_id: Some("feature:extrude_base".into()),
            }],
        );
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kernel_face_id(), Some(99));
        assert_eq!(refs[0].ref_id.as_str(), "ref:face:extrude_base_top");
    }

    #[test]
    fn resolve_prefers_persisted_ref_id() {
        let refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:custom_top").expect("id"),
            "feature:extrude_base",
            "top",
            42,
            [0.0, 0.0, 1.0],
        )];
        assert_eq!(
            resolve_topo_ref_id(&refs, 42).as_deref(),
            Some("ref:face:custom_top")
        );
    }

    #[test]
    fn rebind_updates_stale_kernel_face_id() {
        let mut refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:custom_top").expect("id"),
            "feature:fillet_top",
            "top",
            100,
            [0.0, 0.0, 1.0],
        )];
        rebind_kernel_face_ids(&mut refs, &[(200, 100)]);
        assert_eq!(refs[0].kernel_face_id(), Some(200));
    }

    #[test]
    fn resolve_uses_history_when_picking_post_id() {
        let refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:custom_top").expect("id"),
            "feature:fillet_top",
            "top",
            100,
            [0.0, 0.0, 1.0],
        )];
        assert_eq!(
            resolve_topo_ref_id_with_history(&refs, 200, &[(200, 100)]).as_deref(),
            Some("ref:face:custom_top")
        );
    }

    #[test]
    fn compose_chains_boolean_then_fillet() {
        let boolean = [(10, 1), (20, 2)];
        let fillet = [(100, 10), (200, 20)];
        let composed = compose_face_derivation_histories(&[&boolean, &fillet]);
        let map = build_src_to_post_map(&composed);
        assert_eq!(map.get(&1), Some(&100));
        assert_eq!(map.get(&2), Some(&200));
        assert_eq!(composed.len(), 4);
    }

    #[test]
    fn sync_with_history_rebinds_before_role_match() {
        let existing = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:fillet_top",
            "top",
            100,
            [0.0, 0.0, 1.0],
        )];
        let refs = sync_semantic_refs_with_history(
            &existing,
            &[(200, 100)],
            &[FaceRefDiscovery {
                kernel_face_id: 200,
                role: "top".into(),
                normal_m: [0.0, 0.0, 1.0],
                centroid_m: [0.0, 0.0, 0.006],
                feature_id: Some("feature:fillet_top".into()),
            }],
        );
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kernel_face_id(), Some(200));
        assert_eq!(refs[0].ref_id.as_str(), "ref:face:bracket_top");
    }

    #[test]
    fn resolve_kernel_face_id_remaps_through_history() {
        use opencad_core::TopoRefId;

        let refs = vec![TopoRef::kernel_face(
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
            10,
            [0.0, 0.0, 1.0],
        )];
        let history = vec![(20, 10), (30, 20)];
        let resolved = resolve_kernel_face_id_for_topo_ref(&refs, &history, "ref:face:bracket_top")
            .expect("resolve");
        assert_eq!(resolved, 30);
    }

    #[test]
    fn fingerprint_fallback_resolves_named_ref_without_kernel_face_id() {
        use opencad_core::TopoRefId;

        let refs = vec![TopoRef::face(
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
        )];
        let discoveries = vec![FaceRefDiscovery {
            kernel_face_id: 77,
            role: "top".into(),
            normal_m: [0.0, 0.0, 1.0],
            centroid_m: [0.0, 0.0, 0.006],
            feature_id: Some("feature:extrude_base".into()),
        }];
        let resolved = resolve_kernel_face_id_for_topo_ref_with_discoveries(
            &refs,
            &[],
            "ref:face:bracket_top",
            Some(&discoveries),
        )
        .expect("resolve");
        assert_eq!(resolved, 77);
    }

    #[test]
    fn fingerprint_match_prefers_normal_aligned_discovery() {
        let topo_ref = TopoRef::face(
            TopoRefId::new("ref:face:bracket_top").expect("id"),
            "feature:extrude_base",
            "top",
        );
        let discoveries = vec![
            FaceRefDiscovery {
                kernel_face_id: 10,
                role: "top".into(),
                normal_m: [0.0, 1.0, 0.0],
                centroid_m: [0.0, 0.0, 0.006],
                feature_id: Some("feature:extrude_base".into()),
            },
            FaceRefDiscovery {
                kernel_face_id: 11,
                role: "top".into(),
                normal_m: [0.0, 0.0, 1.0],
                centroid_m: [0.0, 0.0, 0.006],
                feature_id: Some("feature:extrude_base".into()),
            },
        ];
        let mut topo_ref = topo_ref;
        topo_ref.semantic.normal_hint = Some([0.0, 0.0, 1.0]);
        assert_eq!(
            match_face_discovery_for_topo_ref(&topo_ref, &discoveries),
            Some(11)
        );
    }
}
