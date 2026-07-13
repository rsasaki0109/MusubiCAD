//! Presentation overlays for deterministic showcase renders.

use std::collections::{BTreeMap, BTreeSet};

use crate::overlay::{OverlayLine, SketchOverlay};
use crate::scene::RenderScene;

const EDGE_QUANTIZATION_PER_M: f32 = 1_000_000.0;

/// Combine an optional sketch overlay with a floor grid and deduplicated mesh edges.
pub fn presentation_overlay(scene: &RenderScene, source: Option<&SketchOverlay>) -> SketchOverlay {
    let mut overlay = source.cloned().unwrap_or_default();
    overlay.lines.extend(floor_grid(scene));
    overlay.lines.extend(mesh_edges(scene));
    overlay
}

fn floor_grid(scene: &RenderScene) -> Vec<OverlayLine> {
    let center = scene.bounds.center();
    let diagonal = scene.bounds.diagonal().max(0.01);
    let half_extent = diagonal * 0.75;
    let floor_y = scene.bounds.min[1] - diagonal * 0.04;
    let divisions = 10;
    let step = half_extent * 2.0 / divisions as f32;
    let mut lines = Vec::with_capacity((divisions + 1) * 2);
    for index in 0..=divisions {
        let offset = -half_extent + index as f32 * step;
        lines.push(line(
            [center[0] + offset, floor_y, center[2] - half_extent],
            [center[0] + offset, floor_y, center[2] + half_extent],
            true,
        ));
        lines.push(line(
            [center[0] - half_extent, floor_y, center[2] + offset],
            [center[0] + half_extent, floor_y, center[2] + offset],
            true,
        ));
    }
    lines
}

fn mesh_edges(scene: &RenderScene) -> Vec<OverlayLine> {
    let mut edges: BTreeMap<([i32; 3], [i32; 3]), EdgeRecord> = BTreeMap::new();
    for mesh in &scene.meshes {
        for (triangle_index, triangle) in mesh.indices.chunks_exact(3).enumerate() {
            let face_id = mesh.triangle_face_ids.get(triangle_index).copied();
            for (start_index, end_index) in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                let Some(start) = mesh.positions.get(start_index as usize).copied() else {
                    continue;
                };
                let Some(end) = mesh.positions.get(end_index as usize).copied() else {
                    continue;
                };
                let key = edge_key(start, end);
                let record = edges.entry(key).or_insert_with(|| EdgeRecord {
                    start,
                    end,
                    adjacent_triangles: 0,
                    face_ids: BTreeSet::new(),
                });
                record.adjacent_triangles += 1;
                if let Some(face_id) = face_id {
                    record.face_ids.insert(face_id);
                }
            }
        }
    }
    edges
        .into_values()
        .filter(|edge| edge.adjacent_triangles == 1 || edge.face_ids.len() > 1)
        .map(|edge| line(edge.start, edge.end, false))
        .collect()
}

struct EdgeRecord {
    start: [f32; 3],
    end: [f32; 3],
    adjacent_triangles: usize,
    face_ids: BTreeSet<u64>,
}

fn edge_key(start: [f32; 3], end: [f32; 3]) -> ([i32; 3], [i32; 3]) {
    let start = quantize(start);
    let end = quantize(end);
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

fn quantize(point: [f32; 3]) -> [i32; 3] {
    point.map(|value| (value * EDGE_QUANTIZATION_PER_M).round() as i32)
}

fn line(start: [f32; 3], end: [f32; 3], construction: bool) -> OverlayLine {
    OverlayLine {
        start,
        end,
        construction,
        sketch_id: None,
        entity_id: None,
        segment_index: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_geometry::MeshSet;

    #[test]
    fn presentation_overlay_contains_grid_and_edges() {
        let scene = RenderScene::from_mesh_set(&MeshSet::box_prism(0.08, 0.001)).expect("scene");
        let overlay = presentation_overlay(&scene, None);
        assert!(overlay.lines.len() > 22, "grid plus model edges expected");
        assert!(overlay.lines.iter().any(|line| !line.construction));
    }
}
