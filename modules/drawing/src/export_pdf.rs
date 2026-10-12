//! Deterministic multi-page vector PDF without external resources (ADR-045).
use crate::{RadialLayout, SectionLayout, Sheet, ViewMesh};
use opencad_core::{OpenCadError, Result};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
use std::collections::BTreeMap;
const FONT: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
pub const PDF_POINTS_PER_M: f64 = 1000. * 72. / 25.4;
pub struct PdfPageInput<'a> {
    pub sheet: &'a Sheet,
    pub meshes: &'a [ViewMesh],
    pub radial: &'a [RadialLayout],
    pub sections: &'a [SectionLayout],
}
/// Render validated, regenerated page inputs; never reads fonts/files or the clock.
pub fn render_drawing_pdf(pages: &[PdfPageInput<'_>]) -> Result<Vec<u8>> {
    if pages.is_empty() || pages.len() > 128 {
        return Err(invalid("PDF requires 1..128 drawing sheets"));
    }
    let attempted_image = std::sync::atomic::AtomicBool::new(false);
    let mut options = svg2pdf::usvg::Options {
        dpi: 72.,
        font_family: "DejaVu Sans".into(),
        ..Default::default()
    };
    options.fontdb_mut().load_font_data(FONT.to_vec());
    options.fontdb_mut().set_sans_serif_family("DejaVu Sans");
    options.image_href_resolver = svg2pdf::usvg::ImageHrefResolver {
        resolve_data: Box::new(|_, _, _| {
            attempted_image.store(true, std::sync::atomic::Ordering::Relaxed);
            None
        }),
        resolve_string: Box::new(|_, _| {
            attempted_image.store(true, std::sync::atomic::Ordering::Relaxed);
            None
        }),
    };
    let mut pdf = Pdf::new();
    let mut next = Ref::new(1);
    let catalog = next.bump();
    let page_tree = next.bump();
    let mut page_refs = Vec::new();
    pdf.catalog(catalog).pages(page_tree);
    for input in pages {
        validate_scene_numbers(input)?;
        let svg = crate::render_sheet_svg_with_layouts(
            input.sheet,
            input.meshes,
            input.radial,
            input.sections,
        )?;
        let width_mm = input.sheet.width_m * 1000.;
        let height_mm = input.sheet.height_m * 1000.;
        let expected = [
            input.sheet.width_m * PDF_POINTS_PER_M,
            input.sheet.height_m * PDF_POINTS_PER_M,
        ];
        if !expected
            .iter()
            .all(|v| v.is_finite() && *v > 0. && *v <= 1e7)
        {
            return Err(invalid(
                "PDF sheet point dimensions are invalid or exceed the supported range",
            ));
        }
        let (_, body) = svg
            .split_once('\n')
            .ok_or_else(|| invalid("generated SVG has no root header"))?;
        let precise=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width_mm:.9} {height_mm:.9}\" width=\"{width_mm:.9}mm\" height=\"{height_mm:.9}mm\">\n{body}");
        let tree = svg2pdf::usvg::Tree::from_str(&precise, &options)
            .map_err(|e| invalid(&format!("PDF SVG parse: {e}")))?;
        if attempted_image.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(invalid("PDF images or external resources are unsupported"));
        }
        let width = tree.size().width();
        let height = tree.size().height();
        if [width as f64, height as f64]
            .into_iter()
            .zip(expected)
            .any(|(a, b)| (a - b).abs() > 1e-3)
        {
            return Err(invalid(
                "PDF paper dimensions do not match authored sheet metres",
            ));
        }
        let (chunk, object) = svg2pdf::to_chunk(
            &tree,
            svg2pdf::ConversionOptions {
                pdfa: true,
                ..Default::default()
            },
        )
        .map_err(|e| invalid(&format!("PDF conversion: {e}")))?;
        // Reserving room before bumping keeps all signed reference IDs in range.
        if chunk.refs().len() > 1_000_000
            || next.get() as usize + chunk.refs().len() + 2 > 1_000_000
        {
            return Err(invalid("PDF exceeds its object budget"));
        }
        let mut mapping = BTreeMap::new();
        let chunk = chunk.renumber(|old| *mapping.entry(old.get()).or_insert_with(|| next.bump()));
        let object = *mapping
            .get(&object.get())
            .ok_or_else(|| invalid("PDF form object was not renumbered"))?;
        pdf.extend(&chunk);
        let page = next.bump();
        let stream = next.bump();
        page_refs.push(page);
        let mut writer = pdf.page(page);
        writer
            .media_box(Rect::new(0., 0., width, height))
            .parent(page_tree)
            .contents(stream);
        writer.resources().x_objects().pair(Name(b"Sheet"), object);
        writer.finish();
        let mut content = Content::new();
        content
            .transform([width, 0., 0., height, 0., 0.])
            .x_object(Name(b"Sheet"));
        pdf.stream(stream, &content.finish());
    }
    pdf.pages(page_tree)
        .kids(page_refs)
        .count(pages.len() as i32);
    Ok(pdf.finish())
}
fn invalid(message: &str) -> OpenCadError {
    OpenCadError::validation(message)
}

fn validate_scene_numbers(input: &PdfPageInput<'_>) -> Result<()> {
    let finite = |points: &[[f64; 2]]| points.iter().flatten().all(|v| (v * 1000.).is_finite());
    for segment in crate::build_sheet_segments(input.sheet, input.meshes)? {
        if !finite(&[segment.start_m, segment.end_m]) {
            return Err(invalid("PDF geometry overflows sheet millimetres"));
        }
    }
    for dimension in &input.sheet.dimensions {
        let view = input
            .sheet
            .views
            .iter()
            .find(|v| v.id == dimension.view_id)
            .ok_or_else(|| invalid("PDF dimension view missing"))?;
        let layout = crate::layout_linear_dimension(dimension, view)?;
        if !finite(&[
            layout.witness_start_m,
            layout.witness_end_m,
            layout.line_start_m,
            layout.line_end_m,
            layout.text_position_m,
        ]) {
            return Err(invalid("PDF linear dimension overflows sheet millimetres"));
        }
    }
    for dimension in &input.sheet.angular_dimensions {
        let view = input
            .sheet
            .views
            .iter()
            .find(|v| v.id == dimension.view_id)
            .ok_or_else(|| invalid("PDF angular view missing"))?;
        let layout = crate::layout_angular_dimension(dimension, view)?;
        if !finite(&[
            layout.vertex_m,
            layout.arc_start_m,
            layout.arc_end_m,
            layout.text_position_m,
        ]) || !(layout.radius_m * 1000.).is_finite()
        {
            return Err(invalid("PDF angular dimension overflows sheet millimetres"));
        }
    }
    for layout in input.radial {
        if !finite(&[
            layout.line_start_m,
            layout.circle_point_m,
            layout.text_position_m,
        ]) {
            return Err(invalid("PDF radial layout overflows sheet millimetres"));
        }
    }
    for layout in input.sections {
        if layout.loops_on_sheet_m.iter().any(|p| !finite(p))
            || !(layout.hatch_spacing_m * 1000. * 2_f64.sqrt()).is_finite()
        {
            return Err(invalid("PDF section layout overflows sheet millimetres"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DrawingView, ModelReference, ProjectionKind, TitleBlock};
    use opencad_core::{DocumentId, SheetId, ViewId};
    use opencad_geometry::{GeometryKernel, KernelBody, MockGeometryKernel, TessellationSettings};
    fn sheet(id: &str) -> Sheet {
        let mut s = Sheet::a4_portrait(SheetId::new(id).expect("id"), "Paper");
        s.views.push(DrawingView::new(
            ViewId::new("view:front").expect("id"),
            "Front",
            ModelReference::new("plate.ocad.d", DocumentId::new("doc:plate").expect("id")),
            ProjectionKind::Front,
            0.5,
            [0.05, 0.1],
        ));
        s
    }
    pub fn media_boxes(bytes: &[u8]) -> Vec<[f64; 2]> {
        String::from_utf8_lossy(bytes)
            .split("/MediaBox [")
            .skip(1)
            .map(|s| {
                let values: Vec<f64> = s
                    .split(']')
                    .next()
                    .expect("box")
                    .split_whitespace()
                    .map(|v| v.parse().expect("number"))
                    .collect();
                [values[2], values[3]]
            })
            .collect()
    }
    #[test]
    fn mock_multi_page_sizes_precision_volume_and_determinism() {
        let mock = MockGeometryKernel::new();
        let body = KernelBody(80);
        let mesh = mock
            .tessellate(&body, &TessellationSettings::default())
            .expect("mesh");
        let meshes = [ViewMesh {
            view_id: ViewId::new("view:front").expect("id"),
            mesh_set: mesh,
        }];
        let a = sheet("sheet:a");
        let mut b = sheet("sheet:b");
        b.width_m = 0.297;
        b.height_m = 0.210;
        let mut c = sheet("sheet:c");
        c.width_m = 0.2101234;
        c.height_m = 0.2974321;
        let sheets = [a, b, c];
        let pages: Vec<_> = sheets
            .iter()
            .map(|sheet| PdfPageInput {
                sheet,
                meshes: &meshes,
                radial: &[],
                sections: &[],
            })
            .collect();
        let bytes = render_drawing_pdf(&pages).expect("PDF");
        assert!(bytes.starts_with(b"%PDF-1.7"));
        assert_eq!(bytes, render_drawing_pdf(&pages).expect("deterministic"));
        let boxes = media_boxes(&bytes);
        assert_eq!(boxes.len(), 3);
        for (actual, sheet) in boxes.iter().zip(&sheets) {
            assert!((actual[0] - sheet.width_m * PDF_POINTS_PER_M).abs() < 1e-3);
            assert!((actual[1] - sheet.height_m * PDF_POINTS_PER_M).abs() < 1e-3);
        }
        assert!(
            (mock.mass_properties(&body, 1.).expect("volume").volume_m3 - 0.08_f64.powi(3)).abs()
                < 1e-12
        );
    }
    #[test]
    fn unsupported_glyph_and_numeric_overflow_fail_closed() {
        let mock = MockGeometryKernel::new();
        let meshes = [ViewMesh {
            view_id: ViewId::new("view:front").expect("id"),
            mesh_set: mock
                .tessellate(&KernelBody(80), &TessellationSettings::default())
                .expect("mesh"),
        }];
        let mut s = sheet("sheet:a");
        s.title_block = Some(TitleBlock {
            title: "日本語".into(),
            drawing_number: "001".into(),
            revision: "A".into(),
            date: None,
            author: None,
            material: None,
            scale_note: None,
        });
        assert!(render_drawing_pdf(&[PdfPageInput {
            sheet: &s,
            meshes: &meshes,
            radial: &[],
            sections: &[]
        }])
        .is_err());
        s.title_block = None;
        s.views[0].scale = 1e308;
        assert!(render_drawing_pdf(&[PdfPageInput {
            sheet: &s,
            meshes: &meshes,
            radial: &[],
            sections: &[]
        }])
        .is_err());
        assert!(render_drawing_pdf(&[]).is_err());
    }
}
