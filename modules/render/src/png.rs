//! PNG export for offscreen viewport renders.

use std::path::Path;

use image::{ImageBuffer, RgbaImage};
use opencad_core::{OpenCadError, Result};

/// Encode RGBA8 pixels as PNG bytes in memory.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let image: RgbaImage =
        ImageBuffer::from_vec(width, height, rgba.to_vec()).ok_or_else(|| {
            OpenCadError::validation(format!("invalid RGBA buffer for {width}x{height} image"))
        })?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|err| OpenCadError::Other(format!("failed to encode PNG: {err}")))?;
    Ok(bytes.into_inner())
}

/// Write tightly packed RGBA8 pixels to a PNG file.
pub fn write_png(path: impl AsRef<Path>, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| OpenCadError::validation("image dimensions overflow"))?;
    if rgba.len() != expected {
        return Err(OpenCadError::validation(format!(
            "rgba buffer length {} does not match {width}x{height}",
            rgba.len()
        )));
    }

    let image: RgbaImage =
        ImageBuffer::from_vec(width, height, rgba.to_vec()).ok_or_else(|| {
            OpenCadError::validation(format!("invalid RGBA buffer for {width}x{height} image"))
        })?;

    image
        .save(path.as_ref())
        .map_err(|err| OpenCadError::Other(format!("failed to write PNG: {err}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_valid_png_header() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("preview.png");
        let rgba = vec![
            255_u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        write_png(&path, 2, 2, &rgba).expect("write png");

        let bytes = std::fs::read(&path).expect("read png");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(bytes.len() > 32);
    }
}
