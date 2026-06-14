//! Interactive viewport launcher for the desktop shell.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

const CAMERA_SYNC_DEBOUNCE_MS: u64 = 120;

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
    run_document_viewport_with_sync::<F, fn(PreviewSynced), fn(), fn()>(data, title, on_pick, None)
}

pub fn run_document_viewport_with_sync<F, G, H, I>(
    data: ViewData,
    title: &str,
    on_pick: Option<F>,
    on_camera_sync: Option<(H, G, I)>,
) -> Result<()>
where
    F: Fn(PickSummary) + Send + 'static,
    G: Fn(PreviewSynced) + Send + 'static,
    H: Fn() + Send + 'static,
    I: Fn() + Send + 'static,
{
    let overlay_empty = data.overlay.is_empty();
    let scene = data.scene.clone();
    let last_selection: Arc<Mutex<Option<PickTarget>>> = Arc::new(Mutex::new(None));
    let view_data = data.clone();

    let pick_callback = on_pick.map(|handler| {
        let view_data = view_data.clone();
        let last_selection = last_selection.clone();
        Box::new(
            move |x: f64, y: f64, width: u32, height: u32, pick: PickResult| {
                let options = PickOptions {
                    x,
                    y,
                    width,
                    height,
                };
                let summary = build_pick_summary(&view_data, pick, &options);
                *last_selection.lock().expect("selection lock") = match &summary.selection {
                    PickTarget::None => None,
                    selection => Some(selection.clone()),
                };
                handler(summary);
            },
        ) as ViewportPickCallback
    });

    let camera_callback = on_camera_sync.map(|(on_syncing, on_synced, on_aborted)| {
        spawn_camera_sync_sender(
            data.clone(),
            scene,
            last_selection,
            on_syncing,
            on_synced,
            on_aborted,
        )
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

fn spawn_camera_sync_sender<G, H, I>(
    data: ViewData,
    scene: opencad_render::RenderScene,
    last_selection: Arc<Mutex<Option<PickTarget>>>,
    on_syncing: H,
    on_synced: G,
    on_aborted: I,
) -> ViewportCameraCallback
where
    G: Fn(PreviewSynced) + Send + 'static,
    H: Fn() + Send + 'static,
    I: Fn() + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<OrbitCamera>();
    thread::spawn(move || {
        let debounce = Duration::from_millis(CAMERA_SYNC_DEBOUNCE_MS);
        while let Ok(mut latest) = rx.recv() {
            on_syncing();
            while let Ok(next) = rx.recv_timeout(debounce) {
                latest = next;
            }
            let camera_state = CameraState::from(latest);
            let Ok(png_base64) = render_preview_png(&data, Some(camera_state)) else {
                on_aborted();
                continue;
            };
            let highlight_segments_px = last_selection
                .lock()
                .expect("selection lock")
                .as_ref()
                .map(|selection| {
                    highlight_segments_for_camera(
                        &scene,
                        &data.overlay,
                        selection,
                        &camera_state,
                        PREVIEW_WIDTH,
                        PREVIEW_HEIGHT,
                    )
                })
                .unwrap_or_default();
            on_synced(PreviewSynced {
                png_base64,
                camera: camera_state,
                highlight_segments_px,
            });
        }
    });

    Box::new(move |camera: OrbitCamera| {
        let _ = tx.send(camera);
    })
}
