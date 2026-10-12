//! Deterministic millimetre drawing DXF (ADR-046).
use crate::{
    build_sheet_segments, layout_angular_dimension, layout_linear_dimension, layout_title_block,
    LineVisibility, RadialLayout, SectionLayout, Sheet, ViewMesh,
};
use opencad_core::{OpenCadError, Result};

const MAX_ENTITIES: usize = 200_000;
const MAX_COORDINATE_MM: f64 = 1e9;

/// Export a regenerated single sheet in right-handed, millimetre paper coordinates.
/// Dimension labels are derived text, not editable DXF DIMENSION entities.
pub fn render_sheet_dxf(
    sheet: &Sheet,
    meshes: &[ViewMesh],
    radial: &[RadialLayout],
    sections: &[SectionLayout],
) -> Result<String> {
    // Share the existing layout admission checks, including missing derived layouts.
    crate::render_sheet_svg_with_layouts(sheet, meshes, radial, sections)?;
    if ![sheet.width_m, sheet.height_m]
        .iter()
        .all(|v| v.is_finite() && *v > 0.)
    {
        return Err(OpenCadError::validation(
            "DXF paper size must be finite and positive",
        ));
    }
    let mut d = Dxf::new(sheet)?;
    d.rectangle("PAPER", [0., 0.], [sheet.width_m, sheet.height_m])?;
    for s in build_sheet_segments(sheet, meshes)? {
        d.line(
            match s.visibility {
                LineVisibility::Visible => "VISIBLE",
                LineVisibility::Hidden => "HIDDEN",
            },
            s.start_m,
            s.end_m,
        )?;
    }
    for dim in &sheet.dimensions {
        let view = sheet
            .views
            .iter()
            .find(|v| v.id == dim.view_id)
            .ok_or_else(|| OpenCadError::validation("DXF dimension view missing"))?;
        let l = layout_linear_dimension(dim, view)?;
        for (a, b) in [
            (l.witness_start_m, l.line_start_m),
            (l.witness_end_m, l.line_end_m),
            (l.line_start_m, l.line_end_m),
        ] {
            d.line("DIMENSIONS", a, b)?;
        }
        d.text(
            "DIMENSIONS",
            [l.text_position_m[0], l.text_position_m[1] + 0.001],
            0.003,
            &l.label,
            true,
        )?;
    }
    for dim in &sheet.angular_dimensions {
        let view = sheet
            .views
            .iter()
            .find(|v| v.id == dim.view_id)
            .ok_or_else(|| OpenCadError::validation("DXF angular view missing"))?;
        let l = layout_angular_dimension(dim, view)?;
        d.line("DIMENSIONS", l.vertex_m, l.arc_start_m)?;
        d.line("DIMENSIONS", l.vertex_m, l.arc_end_m)?;
        let angle = |p: [f64; 2]| {
            (p[1] - l.vertex_m[1])
                .atan2(p[0] - l.vertex_m[0])
                .to_degrees()
                .rem_euclid(360.)
        };
        let (a, b) = if l.clockwise {
            (l.arc_end_m, l.arc_start_m)
        } else {
            (l.arc_start_m, l.arc_end_m)
        };
        d.arc(l.vertex_m, l.radius_m, angle(a), angle(b))?;
        d.text("DIMENSIONS", l.text_position_m, 0.003, &l.label, true)?;
    }
    for l in radial {
        d.line("DIMENSIONS", l.line_start_m, l.circle_point_m)?;
        d.line("DIMENSIONS", l.circle_point_m, l.text_position_m)?;
        d.text(
            "DIMENSIONS",
            [l.text_position_m[0], l.text_position_m[1] + 0.001],
            0.003,
            &l.label,
            false,
        )?;
    }
    for s in sections {
        for points in &s.loops_on_sheet_m {
            d.polyline("SECTIONS", points)?;
        }
        for (a, b) in hatch_segments(s, sheet.height_m)? {
            d.line("HATCH", a, b)?;
        }
    }
    for v in sheet.views.iter().filter(|v| v.detail.is_some()) {
        let parent = crate::validate_detail_parent(v, sheet)?;
        let detail = v
            .detail
            .as_ref()
            .ok_or_else(|| OpenCadError::validation("DXF detail missing"))?;
        let a = std::array::from_fn(|i| {
            parent.origin_on_sheet_m[i] + detail.region_min_m[i] * parent.scale
        });
        let b = std::array::from_fn(|i| {
            parent.origin_on_sheet_m[i] + detail.region_max_m[i] * parent.scale
        });
        d.rectangle("DETAILS", a, b)?;
        let h =
            (detail.region_max_m[1] - detail.region_min_m[1]) * parent.scale * detail.magnification;
        d.text(
            "DETAILS",
            [
                v.origin_on_sheet_m[0],
                v.origin_on_sheet_m[1] + h / 2. + 0.004,
            ],
            0.003,
            &format!("DETAIL {:.2}×", detail.magnification),
            true,
        )?;
    }
    if let Some(block) = &sheet.title_block {
        for c in layout_title_block(sheet, block)? {
            d.rectangle(
                "TITLE",
                c.origin_m,
                [c.origin_m[0] + c.width_m, c.origin_m[1] + c.height_m],
            )?;
            d.text(
                "TITLE",
                [c.origin_m[0] + 0.002, c.origin_m[1] + c.height_m - 0.003],
                0.002,
                c.label,
                false,
            )?;
            d.text(
                "TITLE",
                [c.origin_m[0] + 0.002, c.origin_m[1] + c.height_m - 0.009],
                0.003,
                &c.value,
                false,
            )?;
        }
    }
    d.pair(0, "ENDSEC");
    d.pair(0, "EOF");
    Ok(d.data)
}

struct Dxf {
    data: String,
    entities: usize,
    next_handle: u32,
}
impl Dxf {
    fn new(sheet: &Sheet) -> Result<Self> {
        let mut d = Self {
            data: String::new(),
            entities: 0,
            next_handle: 1,
        };
        d.pair(0, "SECTION");
        d.pair(2, "HEADER");
        d.pair(9, "$ACADVER");
        d.pair(1, "AC1027");
        d.pair(9, "$INSUNITS");
        d.pair(70, 4);
        d.pair(9, "$MEASUREMENT");
        d.pair(70, 1);
        d.pair(9, "$EXTMIN");
        d.point(10, [0., 0.])?;
        d.pair(9, "$EXTMAX");
        d.point(10, [sheet.width_m, sheet.height_m])?;
        d.pair(0, "ENDSEC");
        d.pair(0, "SECTION");
        d.pair(2, "TABLES");
        d.pair(0, "TABLE");
        d.pair(2, "LTYPE");
        d.handle();
        d.pair(100, "AcDbSymbolTable");
        d.pair(70, 2);
        for (name, segments) in [("CONTINUOUS", &[][..]), ("DASHED", &[2., -1.][..])] {
            d.pair(0, "LTYPE");
            d.handle();
            d.pair(100, "AcDbSymbolTableRecord");
            d.pair(100, "AcDbLinetypeTableRecord");
            d.pair(2, name);
            d.pair(70, 0);
            d.pair(3, name);
            d.pair(72, 65);
            d.pair(73, segments.len());
            d.pair(40, if segments.is_empty() { 0. } else { 3. });
            for n in segments {
                d.pair(49, n);
                d.pair(74, 0);
            }
        }
        d.pair(0, "ENDTAB");
        d.pair(0, "TABLE");
        d.pair(2, "LAYER");
        d.handle();
        d.pair(100, "AcDbSymbolTable");
        d.pair(70, 9);
        for (name, color, line) in [
            ("0", 7, "CONTINUOUS"),
            ("PAPER", 8, "CONTINUOUS"),
            ("VISIBLE", 7, "CONTINUOUS"),
            ("HIDDEN", 8, "DASHED"),
            ("DIMENSIONS", 5, "CONTINUOUS"),
            ("SECTIONS", 7, "CONTINUOUS"),
            ("HATCH", 8, "CONTINUOUS"),
            ("DETAILS", 5, "DASHED"),
            ("TITLE", 7, "CONTINUOUS"),
        ] {
            d.pair(0, "LAYER");
            d.handle();
            d.pair(100, "AcDbSymbolTableRecord");
            d.pair(100, "AcDbLayerTableRecord");
            d.pair(2, name);
            d.pair(70, 0);
            d.pair(62, color);
            d.pair(6, line);
        }
        d.pair(0, "ENDTAB");
        d.pair(0, "TABLE");
        d.pair(2, "STYLE");
        d.handle();
        d.pair(100, "AcDbSymbolTable");
        d.pair(70, 1);
        d.pair(0, "STYLE");
        d.handle();
        d.pair(100, "AcDbSymbolTableRecord");
        d.pair(100, "AcDbTextStyleTableRecord");
        d.pair(2, "Standard");
        d.pair(70, 0);
        d.pair(40, 0.);
        d.pair(41, 1.);
        d.pair(50, 0.);
        d.pair(71, 0);
        d.pair(42, 3.);
        d.pair(3, "txt");
        d.pair(4, "");
        d.pair(0, "ENDTAB");
        d.pair(0, "ENDSEC");
        d.pair(0, "SECTION");
        d.pair(2, "ENTITIES");
        Ok(d)
    }
    fn handle(&mut self) {
        self.pair(5, format!("{:X}", self.next_handle));
        self.next_handle += 1;
    }
    fn pair(&mut self, code: i32, value: impl std::fmt::Display) {
        self.data.push_str(&format!("{code}\n{value}\n"));
    }
    fn number(&mut self, code: i32, v: f64) -> Result<()> {
        if !v.is_finite() || v.abs() > MAX_COORDINATE_MM {
            return Err(OpenCadError::validation(
                "DXF value exceeds finite ±1e9 mm/degrees bound",
            ));
        }
        self.pair(
            code,
            format!("{:.9}", if v.abs() < 0.5e-9 { 0. } else { v }),
        );
        Ok(())
    }
    fn point(&mut self, code: i32, p: [f64; 2]) -> Result<()> {
        self.number(code, p[0] * 1000.)?;
        self.number(code + 10, p[1] * 1000.)?;
        self.number(code + 20, 0.)
    }
    fn entity(&mut self, kind: &str, layer: &str, subclass: &str) -> Result<()> {
        self.entities += 1;
        if self.entities > MAX_ENTITIES {
            return Err(OpenCadError::validation("DXF exceeds 200000 entity limit"));
        }
        self.pair(0, kind);
        self.handle();
        self.pair(100, "AcDbEntity");
        self.pair(8, layer);
        self.pair(100, subclass);
        Ok(())
    }
    fn line(&mut self, layer: &str, a: [f64; 2], b: [f64; 2]) -> Result<()> {
        self.entity("LINE", layer, "AcDbLine")?;
        self.point(10, a)?;
        self.point(11, b)
    }
    fn rectangle(&mut self, layer: &str, a: [f64; 2], b: [f64; 2]) -> Result<()> {
        self.polyline(layer, &[a, [b[0], a[1]], b, [a[0], b[1]]])
    }
    fn polyline(&mut self, layer: &str, p: &[[f64; 2]]) -> Result<()> {
        if p.len() < 3 {
            return Err(OpenCadError::validation(
                "DXF contour must have at least three vertices",
            ));
        }
        self.entity("LWPOLYLINE", layer, "AcDbPolyline")?;
        self.pair(90, p.len());
        self.pair(70, 1);
        for point in p {
            self.number(10, point[0] * 1000.)?;
            self.number(20, point[1] * 1000.)?;
        }
        Ok(())
    }
    fn arc(&mut self, c: [f64; 2], r: f64, start: f64, end: f64) -> Result<()> {
        if r <= 0. {
            return Err(OpenCadError::validation("DXF arc radius must be positive"));
        }
        self.entity("ARC", "DIMENSIONS", "AcDbCircle")?;
        self.point(10, c)?;
        self.number(40, r * 1000.)?;
        self.pair(100, "AcDbArc");
        self.number(50, start)?;
        self.number(51, end)
    }
    fn text(&mut self, layer: &str, p: [f64; 2], height: f64, s: &str, center: bool) -> Result<()> {
        if s.chars().any(char::is_control)
            || s.contains("%%")
            || s.contains('\\')
            || s.contains('^')
        {
            return Err(OpenCadError::validation(
                "DXF text must be a single line without %% codes, backslashes or caret codes",
            ));
        }
        if s.is_empty() {
            return Ok(());
        }
        self.entity("TEXT", layer, "AcDbText")?;
        self.point(10, p)?;
        self.number(40, height * 1000.)?;
        // Refuse interpreted control syntax rather than changing authored labels.
        self.pair(1, s);
        self.pair(7, "Standard");
        if center {
            self.pair(72, 1);
            self.point(11, p)?;
        }
        self.pair(100, "AcDbText");
        self.pair(73, 0);
        Ok(())
    }
}

/// Exact line/polygon intersections with half-open crossings and even-odd pairing.
/// Phase matches SVG's global 45-degree hatch: x-y = n*pitch*sqrt(2)-sheet_height.
fn hatch_segments(s: &SectionLayout, height: f64) -> Result<Vec<([f64; 2], [f64; 2])>> {
    let pitch = s.hatch_spacing_m * 2_f64.sqrt();
    let points: Vec<_> = s.loops_on_sheet_m.iter().flatten().collect();
    if points.is_empty()
        || !pitch.is_finite()
        || pitch <= 1e-6
        || s.loops_on_sheet_m.iter().any(|p| p.len() < 3)
        || points.iter().any(|p| {
            p.iter()
                .any(|v| !v.is_finite() || v.abs() * 1000. > MAX_COORDINATE_MM)
        })
    {
        return Err(OpenCadError::validation(
            "DXF hatch requires finite closed contours and positive pitch",
        ));
    }
    let min = points
        .iter()
        .map(|p| p[0] - p[1])
        .fold(f64::INFINITY, f64::min);
    let max = points
        .iter()
        .map(|p| p[0] - p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let first = ((min + height) / pitch).ceil();
    let last = ((max + height) / pitch).floor();
    let count = (last - first + 1.).max(0.);
    if !first.is_finite()
        || !last.is_finite()
        || first.abs() > 1e12
        || last.abs() > 1e12
        || count > 100_000.
        || count * points.len() as f64 > 10_000_000.
    {
        return Err(OpenCadError::validation(
            "DXF hatch exceeds line/intersection budget",
        ));
    }
    let mut result = Vec::new();
    for index in 0..count as usize {
        let c = (first + index as f64) * pitch - height;
        let mut hits = Vec::new();
        for polygon in &s.loops_on_sheet_m {
            for i in 0..polygon.len() {
                let a = polygon[i];
                let b = polygon[(i + 1) % polygon.len()];
                let da = a[0] - a[1];
                let db = b[0] - b[1];
                if (da <= c && c < db) || (db <= c && c < da) {
                    let t = (c - da) / (db - da);
                    hits.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
                }
            }
        }
        hits.sort_by(|a, b| a[0].total_cmp(&b[0]));
        if hits.len() % 2 != 0 {
            return Err(OpenCadError::validation(
                "DXF hatch has odd contour crossings",
            ));
        }
        for pair in hits.chunks_exact(2) {
            if (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]) > 1e-12 {
                result.push((pair[0], pair[1]));
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::{DocumentId, SheetId, ViewId};
    use opencad_geometry::{GeometryKernel, KernelBody, MockGeometryKernel, TessellationSettings};
    #[test]
    fn mock_analytic_paper_geometry_volume_and_determinism() {
        let kernel = MockGeometryKernel::new();
        let body = KernelBody(80);
        let mut sheet = Sheet::a4_portrait(SheetId::new("sheet:a4").expect("id"), "A4");
        let id = ViewId::new("view:front").expect("id");
        sheet.views.push(crate::DrawingView::new(
            id.clone(),
            "Front",
            crate::ModelReference::new("part.ocad.d", DocumentId::new("doc:part").expect("id")),
            crate::ProjectionKind::Front,
            0.5,
            [0.05, 0.1],
        ));
        let meshes = [ViewMesh {
            view_id: id,
            mesh_set: kernel
                .tessellate(&body, &TessellationSettings::default())
                .expect("mesh"),
        }];
        let data = render_sheet_dxf(&sheet, &meshes, &[], &[]).expect("DXF");
        assert_eq!(
            data,
            render_sheet_dxf(&sheet, &meshes, &[], &[]).expect("determinism")
        );
        assert!(data.contains("9\n$INSUNITS\n70\n4\n"));
        assert!(data.contains("10\n210.000000000\n20\n297.000000000\n"));
        assert!(data.contains("10\n50.000000000\n20\n100.000000000\n"));
        // 80 mm square at 1:2 becomes 40 mm in paper coordinates.
        assert!(data.contains("90.000000000"));
        assert!(
            (kernel.mass_properties(&body, 1.).expect("volume").volume_m3 - 0.08_f64.powi(3)).abs()
                < 1e-12
        );
        sheet.width_m = f64::INFINITY;
        assert!(render_sheet_dxf(&sheet, &meshes, &[], &[]).is_err());
    }
    #[test]
    fn analytic_even_odd_hatch_leaves_hole_clear_and_preserves_phase() {
        let s = SectionLayout {
            view_id: ViewId::new("view:section").expect("id"),
            hatch_spacing_m: 0.01 / 2_f64.sqrt(),
            loops_on_sheet_m: vec![
                vec![[0., 0.], [0.08, 0.], [0.08, 0.06], [0., 0.06]],
                vec![[0.02, 0.02], [0.04, 0.02], [0.04, 0.04], [0.02, 0.04]],
            ],
        };
        let lines = hatch_segments(&s, 0.1).expect("hatch");
        let diagonal: Vec<_> = lines
            .iter()
            .filter(|(a, _)| (a[0] - a[1]).abs() < 1e-12)
            .collect();
        assert_eq!(diagonal.len(), 2);
        let length: f64 = diagonal
            .iter()
            .map(|(a, b)| (b[0] - a[0]).hypot(b[1] - a[1]))
            .sum();
        assert!((length - 0.04 * 2_f64.sqrt()).abs() < 1e-12);
        for (a, b) in lines {
            assert!(((a[0] - a[1]) - (b[0] - b[1])).abs() < 1e-12);
            let phase = (a[0] - a[1] + 0.1) / 0.01;
            assert!((phase - phase.round()).abs() < 1e-9);
            let m = [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.];
            assert!(
                !(m[0] > 0.02 + 1e-12
                    && m[0] < 0.04 - 1e-12
                    && m[1] > 0.02 + 1e-12
                    && m[1] < 0.04 - 1e-12)
            );
        }
        let mut huge = s.clone();
        huge.hatch_spacing_m = 1.1e-6;
        for point in huge.loops_on_sheet_m.iter_mut().flatten() {
            point[0] *= 10.;
            point[1] *= 10.;
        }
        assert!(hatch_segments(&huge, 0.1).is_err());
    }
    #[test]
    fn text_and_entity_budgets_fail_without_interpreting_authored_symbols() {
        let s = Sheet::a4_portrait(SheetId::new("sheet:a4").expect("id"), "A4");
        let mut d = Dxf::new(&s).expect("writer");
        d.text("TITLE", [0., 0.], 0.003, "日本語 ⌀20.00 mm 90° 50%", false)
            .expect("UTF8");
        for text in ["%%c", "a\\U+0041", "^J", "new\nline"] {
            assert!(d.text("TITLE", [0., 0.], 0.003, text, false).is_err());
        }
        d.entities = MAX_ENTITIES;
        assert!(d.line("VISIBLE", [0., 0.], [0.1, 0.]).is_err());
        assert!(d.line("VISIBLE", [f64::NAN, 0.], [0., 0.]).is_err());
    }
    #[test]
    fn clockwise_and_counterclockwise_rays_export_the_same_minor_arc() {
        let mut s = Sheet::a4_portrait(SheetId::new("sheet:a4").expect("id"), "A4");
        let view_id = ViewId::new("view:front").expect("id");
        s.views.push(crate::DrawingView::new(
            view_id.clone(),
            "Front",
            crate::ModelReference::new("part.ocad.d", DocumentId::new("doc:part").expect("id")),
            crate::ProjectionKind::Front,
            0.5,
            [0.05, 0.1],
        ));
        let meshes = [ViewMesh {
            view_id: view_id.clone(),
            mesh_set: opencad_geometry::MeshSet::box_prism(0.08, 0.006),
        }];
        for (a, b) in [
            ([0.04, 0., 0.], [0., 0.03, 0.]),
            ([0., 0.03, 0.], [0.04, 0., 0.]),
        ] {
            s.angular_dimensions = vec![crate::AngularDimension {
                id: opencad_core::DimensionId::new("dim:angle").expect("id"),
                view_id: view_id.clone(),
                measurement: crate::AngularMeasurement::Points {
                    vertex_model_m: [0., 0., 0.],
                    first_model_m: a,
                    second_model_m: b,
                },
                arc_radius_m: 0.01,
            }];
            let data = render_sheet_dxf(&s, &meshes, &[], &[]).expect("DXF");
            assert!(data
                .contains("40\n10.000000000\n100\nAcDbArc\n50\n0.000000000\n51\n90.000000000\n"));
            assert!(data.contains("1\n90.00°\n"));
        }
    }
}
