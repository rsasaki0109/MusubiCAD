//! Drawing system (Phase 4).

pub mod dimension;
pub mod export_svg;
pub mod hidden_line;
pub mod model;
pub mod projection;
pub mod reference;
pub mod render;
pub mod sheet;
pub mod view;

pub use export_svg::{render_sheet_svg, validate_svg};
pub use model::DrawingModel;
pub use projection::ProjectionKind;
pub use reference::ModelReference;
pub use render::{build_sheet_segments, project_mesh_wireframe, SheetSegment, ViewMesh};
pub use sheet::{Sheet, A4_HEIGHT_M, A4_WIDTH_M};
pub use view::DrawingView;
