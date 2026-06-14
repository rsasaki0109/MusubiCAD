//! `opencad pick` — headless viewport selection query.

pub use opencad_desktop::{
    pick_document, PickOptions, PickSummary, PickTarget,
};

pub fn print_summary(summary: &PickSummary) {
    println!("pick_x: {}", summary.x);
    println!("pick_y: {}", summary.y);
    println!("viewport: {}x{}", summary.width, summary.height);
    println!("overlay_lines: {}", summary.overlay_line_count);
    println!("triangles: {}", summary.triangle_count);
    match &summary.selection {
        PickTarget::None => println!("selection: none"),
        PickTarget::SketchLine {
            line_index,
            sketch_id,
            entity_id,
            construction,
            ..
        } => {
            println!("selection: sketch_line");
            println!("line_index: {line_index}");
            if let Some(sketch_id) = sketch_id {
                println!("sketch_id: {sketch_id}");
            }
            if let Some(entity_id) = entity_id {
                println!("entity_id: {entity_id}");
            }
            println!("construction: {construction}");
        }
        PickTarget::SolidTriangle {
            triangle_index,
            face_role,
            kernel_face_id,
            inferred_feature_id,
            inferred_topo_ref_id,
            ..
        } => {
            println!("selection: solid_triangle");
            println!("triangle_index: {triangle_index}");
            if let Some(role) = face_role {
                println!("face_role: {role}");
            }
            if let Some(kernel_face_id) = kernel_face_id {
                println!("kernel_face_id: {kernel_face_id}");
            }
            if let Some(feature_id) = inferred_feature_id {
                println!("inferred_feature_id: {feature_id}");
            }
            if let Some(topo_ref_id) = inferred_topo_ref_id {
                println!("inferred_topo_ref_id: {topo_ref_id}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_desktop::{
        build_pick_summary, pick_document, PickOptions, PickTarget, ViewData,
    };
    use crate::mesh::write_bracket_fixture_at;
    use opencad_feature::{apply_parameters, bracket_base_plate};
    use opencad_graph::{bracket_parameters, evaluate_param_graph};
    use opencad_render::{build_sketch_overlay, PickResult, RenderScene};
    use opencad_sketch::Sketch;
    use tempfile::tempdir;

    #[test]
    fn pick_center_hits_solid_triangle() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let summary =
            pick_document(path.to_str().expect("path"), &PickOptions::default()).expect("pick");
        assert!(summary.triangle_count > 0);
        assert!(summary.overlay_line_count > 0);
        assert!(matches!(
            summary.selection,
            PickTarget::SolidTriangle {
                face_role: Some(_),
                inferred_feature_id: Some(_),
                ..
            } | PickTarget::SketchLine { .. }
        ));
    }

    #[test]
    fn pick_summary_maps_line_index_to_entity_id() {
        let mut model = bracket_base_plate().expect("model");
        let params = bracket_parameters();
        apply_parameters(&mut model, &params).expect("apply");
        let values = evaluate_param_graph(&params).expect("eval");
        let sketches: Vec<Sketch> = model.sketches.values().cloned().collect();
        let overlay = build_sketch_overlay(&sketches, &values).expect("overlay");
        let line_index = overlay
            .lines
            .iter()
            .position(|line| line.entity_id.as_deref() == Some("ent:e0"))
            .expect("ent:e0 overlay line");
        let scene = RenderScene::from_mesh_set(&opencad_geometry::MeshSet::box_prism(0.08, 0.006))
            .expect("scene");
        let data = ViewData {
            scene,
            overlay,
            name: String::new(),
            feature_nodes: Vec::new(),
            semantic_refs: Vec::new(),
            face_history: Vec::new(),
            parameter_ids: vec!["param:width".into(), "param:height".into()],
        };
        let summary = build_pick_summary(
            &data,
            PickResult::SketchLine(line_index),
            &PickOptions::default(),
        );
        let PickTarget::SketchLine {
            sketch_id,
            entity_id,
            entity_kind,
            segment_index,
            ..
        } = summary.selection
        else {
            panic!("expected sketch line");
        };
        assert_eq!(sketch_id.as_deref(), Some("sketch:base"));
        assert_eq!(entity_id.as_deref(), Some("ent:e0"));
        assert_eq!(entity_kind.as_deref(), Some("line"));
        assert!(segment_index.is_none());
    }

    #[test]
    fn pick_corner_returns_none() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let summary = pick_document(
            path.to_str().expect("path"),
            &PickOptions {
                x: 0.0,
                y: 0.0,
                ..PickOptions::default()
            },
        )
        .expect("pick");
        assert!(matches!(summary.selection, PickTarget::None));
    }
}
