//! Interactive viewport launcher for the desktop shell.

use std::sync::{Arc, Mutex};

use opencad_core::Result;
use opencad_render::{
    run_viewport_with_callbacks, OrbitCamera, PickResult, ViewportCameraCallback,
    ViewportPickCallback,
};
use serde::{Deserialize, Serialize};

use crate::pick::{
    build_pick_summary, highlight_segments_for_camera, PickOptions, PickSummary, PickTarget,
    ScreenSegment,
};
use crate::preview::{render_preview_png, CameraState, ViewData, PREVIEW_HEIGHT, PREVIEW_WIDTH};

/// Preview image and highlight overlay synced from the 3D viewport camera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewSynced {
    pub png_base64: String,
    pub camera: CameraState,
    pub highlight_segments_px: Vec<ScreenSegment>,
}

pub fn run_document_viewport<F>(data: ViewData, title: &str, on_pick: Option<F>) -> Result<()>
where
    F: Fn(PickSummary) + Send + 'static,
{
    run_document_viewport_with_sync(data, title, on_pick, None::<fn(PreviewSynced)>)
}

pub fn run_document_viewport_with_sync<F, G>(
    data: ViewData,
    title: &str,
    on_pick: Option<F>,
    on_camera_sync: Option<G>,
) -> Result<()>
where
    F: Fn(PickSummary) + Send + 'static,
    G: Fn(PreviewSynced) + Send + 'static,
{
    let overlay_empty = data.overlay.is_empty();
    let scene = data.scene.clone();
    let overlay = data.overlay.clone();
    let feature_nodes = data.feature_nodes.clone();
    let semantic_refs = data.semantic_refs.clone();
    let face_history = data.face_history.clone();
    let last_selection: Arc<Mutex<Option<PickTarget>>> = Arc::new(Mutex::new(None));

    let pick_callback = on_pick.map(|handler| {
        let scene = scene.clone();
        let overlay = overlay.clone();
        let feature_nodes = feature_nodes.clone();
        let semantic_refs = semantic_refs.clone();
        let face_history = face_history.clone();
        let last_selection = last_selection.clone();
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
                *last_selection.lock().expect("selection lock") = match &summary.selection {
                    PickTarget::None => None,
                    selection => Some(selection.clone()),
                };
                handler(summary);
            },
        ) as ViewportPickCallback
    });

    let camera_callback = on_camera_sync.map(|handler| {
        let data = data.clone();
        let scene = scene.clone();
        let last_selection = last_selection.clone();
        Box::new(move |camera: OrbitCamera| {
            let camera_state = CameraState::from(camera);
            let Ok(png_base64) = render_preview_png(&data, Some(camera_state)) else {
                return;
            };
            let highlight_segments_px = last_selection
                .lock()
                .expect("selection lock")
                .as_ref()
                .map(|selection| {
                    highlight_segments_for_camera(
                        &scene,
                        selection,
                        &camera_state,
                        PREVIEW_WIDTH,
                        PREVIEW_HEIGHT,
                    )
                })
                .unwrap_or_default();
            handler(PreviewSynced {
                png_base64,
                camera: camera_state,
                highlight_segments_px,
            });
        }) as ViewportCameraCallback
    });

    let overlay_ref = if overlay_empty {
        None
    } else {
        Some(&data.overlay)
    };
    run_viewport_with_callbacks(
        &data.scene,
        overlay_ref,
        title,
        pick_callback,
        camera_callback,
    )
}
