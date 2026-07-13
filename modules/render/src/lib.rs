//! Viewport rendering with wgpu.

pub mod animation;
pub mod camera;
pub mod face_catalog;
pub mod mesh;
pub mod overlay;
pub mod png;
pub mod presentation;
pub mod scene;
pub mod selection;
pub mod solid;
pub mod stroke_font;
pub mod viewport;
pub mod wgpu_renderer;

pub use animation::{render_orbit_gif, write_gif_frames, AnimationOptions, AnimationSummary};
pub use camera::{project_world_to_screen, OrbitCamera};
pub use face_catalog::{FaceCatalog, FaceGroup, FaceRole};
pub use mesh::RenderMesh;
pub use overlay::{
    build_sketch_overlay, label_depth_offset_for_bounds, PickableSketchLine, SketchOverlay,
};
pub use png::write_png;
pub use presentation::presentation_overlay;
pub use scene::{BoundingBox, RenderScene};
pub use selection::{
    face_group_boundary_edges, face_group_highlight_edges, triangle_world_positions, PickResult,
    SelectionCatalog, SelectionId,
};
pub use viewport::{
    run_viewport, run_viewport_with_callbacks, run_viewport_with_pick, ViewportCameraCallback,
    ViewportPickCallback,
};
pub use wgpu_renderer::{OffscreenRenderer, RenderImage, RenderOutput};
