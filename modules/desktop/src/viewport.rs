//! Interactive viewport launcher for the desktop shell.

use opencad_core::Result;
use opencad_render::{run_viewport_with_pick, PickResult, ViewportPickCallback};

use crate::pick::{build_pick_summary, PickOptions, PickSummary};
use crate::preview::ViewData;

pub fn run_document_viewport<F>(data: ViewData, title: &str, on_pick: Option<F>) -> Result<()>
where
    F: Fn(PickSummary) + Send + 'static,
{
    let overlay_empty = data.overlay.is_empty();
    let scene = data.scene.clone();
    let overlay = data.overlay.clone();
    let feature_nodes = data.feature_nodes.clone();
    let semantic_refs = data.semantic_refs.clone();
    let face_history = data.face_history.clone();

    let pick_callback = on_pick.map(|handler| {
        let scene = scene.clone();
        let overlay = overlay.clone();
        let feature_nodes = feature_nodes.clone();
        let semantic_refs = semantic_refs.clone();
        let face_history = face_history.clone();
        Box::new(
            move |x: f64, y: f64, width: u32, height: u32, pick: PickResult| {
                let options = PickOptions {
                    x,
                    y,
                    width,
                    height,
                };
                let summary = build_pick_summary(
                    &scene,
                    &overlay,
                    pick,
                    &options,
                    Some(&feature_nodes),
                    &semantic_refs,
                    &face_history,
                );
                handler(summary);
            },
        ) as ViewportPickCallback
    });

    let overlay_ref = if overlay_empty {
        None
    } else {
        Some(&data.overlay)
    };
    run_viewport_with_pick(&data.scene, overlay_ref, title, pick_callback)
}
