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

pub use dimension::{
    layout_linear_dimension, resolve_linear_dimension, DimensionLayout, LinearDimension,
    LinearMeasurement, DIMENSION_LENGTH_TOLERANCE_M,
};
pub use export_svg::{render_sheet_svg, validate_svg};
pub use hidden_line::{
    classify_hidden_lines, ClassifiedEdge, LineVisibility, HIDDEN_LINE_BARYCENTRIC_TOLERANCE,
    HIDDEN_LINE_DEPTH_TOLERANCE_M, HIDDEN_LINE_PARAMETER_TOLERANCE,
    HIDDEN_LINE_PROJECTION_TOLERANCE_M,
};
pub use model::DrawingModel;
pub use projection::ProjectionKind;
pub use reference::ModelReference;
pub use render::{build_sheet_segments, project_mesh_wireframe, SheetSegment, ViewMesh};
pub use sheet::{Sheet, A4_HEIGHT_M, A4_WIDTH_M};
pub use view::DrawingView;

pub mod angular;
pub use angular::{
    layout_angular_dimension, resolve_angular_dimension, AngularDimension, AngularLayout,
    AngularMeasurement, RayEnd,
};

pub mod radial;
pub use radial::{
    layout_radial_dimension, resolve_radial_dimension, RadialDimension, RadialKind, RadialLayout,
    ResolvedRadialDimension,
};

pub use export_svg::render_sheet_svg_with_radial_layouts;

pub mod section;
pub use section::{layout_section, SectionLayout, SectionView};

pub use export_svg::render_sheet_svg_with_layouts;

pub mod detail;
pub use detail::{clip_detail_segment, layout_detail_segments, validate_detail_parent, DetailView};

pub mod title_block;
pub use title_block::{
    layout_title_block, TitleBlock, TitleBlockCell, TITLE_BLOCK_HEIGHT_M, TITLE_BLOCK_MARGIN_M,
    TITLE_BLOCK_WIDTH_M,
};

pub mod export_pdf;
pub use export_pdf::{render_drawing_pdf, PdfPageInput, PDF_POINTS_PER_M};
