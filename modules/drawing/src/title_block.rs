//! Explicit authored title-block metadata and deterministic layout (ADR-044).
use crate::Sheet;
use opencad_core::{OpenCadError, Result};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TitleBlock {
    pub title: String,
    pub drawing_number: String,
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_note: Option<String>,
}
impl TitleBlock {
    pub fn validate(&self) -> Result<()> {
        for (name, value, max, required) in [
            ("title", Some(self.title.as_str()), 50, true),
            (
                "drawing_number",
                Some(self.drawing_number.as_str()),
                24,
                true,
            ),
            ("revision", Some(self.revision.as_str()), 8, true),
            ("date", self.date.as_deref(), 16, false),
            ("author", self.author.as_deref(), 24, false),
            ("material", self.material.as_deref(), 24, false),
            ("scale_note", self.scale_note.as_deref(), 50, false),
        ] {
            if let Some(value) = value {
                if (required && value.trim().is_empty())
                    || value.chars().count() > max
                    || value
                        .chars()
                        .any(|c| c.is_control() || c == '\u{fffe}' || c == '\u{ffff}')
                {
                    return Err(OpenCadError::validation(format!("title block {name} must be a single XML-compatible line of at most {max} characters{}",if required {" and nonempty"} else {""})));
                }
            }
        }
        Ok(())
    }
}
pub const TITLE_BLOCK_WIDTH_M: f64 = 0.180;
pub const TITLE_BLOCK_HEIGHT_M: f64 = 0.048;
pub const TITLE_BLOCK_MARGIN_M: f64 = 0.005;
#[derive(Debug, Clone, PartialEq)]
pub struct TitleBlockCell {
    pub label: &'static str,
    pub value: String,
    pub origin_m: [f64; 2],
    pub width_m: f64,
    pub height_m: f64,
}
pub fn layout_title_block(sheet: &Sheet, block: &TitleBlock) -> Result<Vec<TitleBlockCell>> {
    block.validate()?;
    if !sheet.width_m.is_finite()
        || !sheet.height_m.is_finite()
        || sheet.width_m < TITLE_BLOCK_WIDTH_M + 2. * TITLE_BLOCK_MARGIN_M
        || sheet.height_m < TITLE_BLOCK_HEIGHT_M + 2. * TITLE_BLOCK_MARGIN_M
        || !(sheet.width_m * 1000.).is_finite()
        || !(sheet.height_m * 1000.).is_finite()
    {
        return Err(OpenCadError::validation(
            "sheet cannot fit the 180 × 48 mm title block at 5 mm margins",
        ));
    }
    let x = sheet.width_m - TITLE_BLOCK_MARGIN_M - TITLE_BLOCK_WIDTH_M;
    let y = TITLE_BLOCK_MARGIN_M;
    let h = 0.012;
    let fields = [
        ("TITLE", block.title.as_str(), 0., 3., 0.180),
        (
            "DRAWING NUMBER",
            block.drawing_number.as_str(),
            0.,
            2.,
            0.090,
        ),
        ("REV", block.revision.as_str(), 0.090, 2., 0.030),
        (
            "DATE",
            block.date.as_deref().unwrap_or(""),
            0.120,
            2.,
            0.060,
        ),
        (
            "AUTHOR",
            block.author.as_deref().unwrap_or(""),
            0.,
            1.,
            0.090,
        ),
        (
            "MATERIAL",
            block.material.as_deref().unwrap_or(""),
            0.090,
            1.,
            0.090,
        ),
        (
            "SCALE NOTE",
            block.scale_note.as_deref().unwrap_or(""),
            0.,
            0.,
            0.180,
        ),
    ];
    Ok(fields
        .into_iter()
        .map(|(label, value, dx, row, width)| TitleBlockCell {
            label,
            value: value.into(),
            origin_m: [x + dx, y + row * h],
            width_m: width,
            height_m: h,
        })
        .collect())
}
pub(crate) fn escape_svg_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencad_core::SheetId;
    fn block() -> TitleBlock {
        TitleBlock {
            title: "Plate <A> & B".into(),
            drawing_number: "MUSUBI-001".into(),
            revision: "A".into(),
            date: None,
            author: None,
            material: None,
            scale_note: None,
        }
    }
    #[test]
    fn analytic_a4_frame_cells_and_round_trip() {
        let sheet = Sheet::a4_portrait(SheetId::new("sheet:a4").expect("id"), "A4");
        let cells = layout_title_block(&sheet, &block()).expect("layout");
        assert_eq!(cells.len(), 7);
        let left = cells
            .iter()
            .map(|c| c.origin_m[0])
            .fold(f64::INFINITY, f64::min);
        let right = cells
            .iter()
            .map(|c| c.origin_m[0] + c.width_m)
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = cells
            .iter()
            .map(|c| c.origin_m[1])
            .fold(f64::INFINITY, f64::min);
        let top = cells
            .iter()
            .map(|c| c.origin_m[1] + c.height_m)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((left - 0.025).abs() < 1e-12);
        assert!((right - 0.205).abs() < 1e-12);
        assert!((bottom - 0.005).abs() < 1e-12);
        assert!((top - 0.053).abs() < 1e-12);
        assert!(((right - left) * (top - bottom) - 0.00864).abs() < 1e-12);
        assert_eq!(
            block(),
            serde_json::from_str::<TitleBlock>(&serde_json::to_string(&block()).expect("json"))
                .expect("round trip")
        );
        assert_eq!(
            escape_svg_text("<A> & \"B\" 'C'"),
            "&lt;A&gt; &amp; &quot;B&quot; &apos;C&apos;"
        );
    }
    #[test]
    fn control_overlong_empty_and_small_sheet_values_fail_closed() {
        for title in ["".into(), "\n".into(), "A".repeat(51), "bad\u{ffff}".into()] {
            let mut bad = block();
            bad.title = title;
            assert!(bad.validate().is_err());
        }
        let mut sheet = Sheet::a4_portrait(SheetId::new("sheet:a4").expect("id"), "A4");
        sheet.width_m = 0.18;
        assert!(layout_title_block(&sheet, &block()).is_err());
        sheet.width_m = f64::INFINITY;
        assert!(layout_title_block(&sheet, &block()).is_err());
    }
}
