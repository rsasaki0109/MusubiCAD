//! `musubicad view` and `musubicad screenshot` commands.

use opencad_core::Result;
use opencad_render::{run_viewport, OffscreenRenderer};

use crate::mesh::{load_view_data, PREVIEW_HEIGHT, PREVIEW_WIDTH};

pub fn view_document(input: &str) -> Result<()> {
    let data = load_view_data(input)?;
    let overlay = if data.overlay.is_empty() {
        None
    } else {
        Some(&data.overlay)
    };
    run_viewport(&data.scene, overlay, &data.name)
}

pub fn screenshot_document(input: &str, output: &str) -> Result<()> {
    let data = load_view_data(input)?;
    let renderer = OffscreenRenderer::new()?;
    let overlay = if data.overlay.is_empty() {
        None
    } else {
        Some(&data.overlay)
    };
    renderer.render_scene_png(&data.scene, overlay, PREVIEW_WIDTH, PREVIEW_HEIGHT, output)?;
    println!("screenshot: {output}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::write_bracket_fixture_at;
    use tempfile::tempdir;

    #[test]
    fn loads_scene_and_overlay_for_viewport() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);

        let data = load_view_data(path.to_str().expect("path")).expect("view data");
        assert!(data.scene.triangle_count() > 0);
        assert!(!data.overlay.lines.is_empty());
    }

    #[test]
    fn screenshot_writes_png_with_overlay() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("bracket.ocad.d");
        write_bracket_fixture_at(&path);
        let png_path = dir.path().join("preview.png");

        screenshot_document(
            path.to_str().expect("path"),
            png_path.to_str().expect("png"),
        )
        .expect("screenshot");
        let bytes = std::fs::read(&png_path).expect("read png");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}
