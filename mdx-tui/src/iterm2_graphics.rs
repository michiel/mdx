//! iTerm2 inline images protocol implementation
//!
//! Implements the iTerm2 inline images protocol for displaying images.
//! See: https://iterm2.com/documentation-images.html

#[cfg(feature = "images")]
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
#[cfg(feature = "images")]
use std::io::Write;

#[cfg(feature = "images")]
/// Transmit and display image to iTerm2 terminal
///
/// iTerm2 uses a simpler protocol than Kitty - the entire image is transmitted
/// inline with display parameters.
pub fn display_image(
    image_data: &[u8],
    width_cells: u16,
    height_cells: u16,
) -> anyhow::Result<Vec<u8>> {
    let mut output = Vec::new();

    // Encode image data as base64
    let encoded = BASE64.encode(image_data);

    // iTerm2 protocol format:
    // ESC ] 1337 ; File=[arguments]:base64data ^G
    // Arguments: inline=1,width=<cells>,height=<cells>,preserveAspectRatio=1

    write!(
        output,
        "\x1b]1337;File=inline=1,width={},height={},preserveAspectRatio=1:{}\x07",
        width_cells,
        height_cells,
        encoded
    )?;

    Ok(output)
}

#[cfg(feature = "images")]
/// Convert PNG image data to iTerm2 format
///
/// iTerm2 expects PNG data, so this is a simple pass-through with encoding.
pub fn transmit_png(
    png_data: &[u8],
    width_cells: u16,
    height_cells: u16,
) -> anyhow::Result<Vec<u8>> {
    display_image(png_data, width_cells, height_cells)
}

#[cfg(feature = "images")]
/// Encode RGBA data as PNG for iTerm2
pub fn encode_rgba_as_png(
    rgba_data: &[u8],
    width: u32,
    height: u32,
) -> anyhow::Result<Vec<u8>> {
    use image::{ImageBuffer, RgbaImage};
    use std::io::Cursor;

    // Create image buffer from RGBA data
    let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba_data.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer from RGBA data"))?;

    // Encode as PNG
    let mut png_data = Vec::new();
    let mut cursor = Cursor::new(&mut png_data);
    img.write_to(&mut cursor, image::ImageFormat::Png)?;

    Ok(png_data)
}

#[cfg(all(test, feature = "images"))]
mod tests {
    use super::*;

    #[test]
    fn test_display_image() {
        // Create a small test image (just a few bytes)
        let image_data = vec![137, 80, 78, 71]; // PNG header

        let result = display_image(&image_data, 10, 5);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = String::from_utf8_lossy(&output);

        // Should contain iTerm2 inline image escape sequence
        assert!(output_str.contains("\x1b]1337"));
        assert!(output_str.contains("File="));
        assert!(output_str.contains("inline=1"));
        assert!(output_str.contains("width=10"));
        assert!(output_str.contains("height=5"));
        assert!(output_str.contains("preserveAspectRatio=1"));
        assert!(output_str.ends_with("\x07"));
    }

    #[test]
    fn test_transmit_png() {
        let png_data = vec![137, 80, 78, 71, 13, 10, 26, 10]; // PNG signature

        let result = transmit_png(&png_data, 20, 10);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_encode_rgba_as_png() {
        // Create a 2x2 RGBA image
        let rgba_data = vec![
            255, 0, 0, 255, // Red pixel
            0, 255, 0, 255, // Green pixel
            0, 0, 255, 255, // Blue pixel
            255, 255, 255, 255, // White pixel
        ];

        let result = encode_rgba_as_png(&rgba_data, 2, 2);
        assert!(result.is_ok());

        let png_data = result.unwrap();
        // PNG should start with PNG signature
        assert!(png_data.starts_with(&[137, 80, 78, 71]));
    }

    #[test]
    fn test_encode_single_pixel() {
        let rgba_data = vec![255, 0, 0, 255]; // Single red pixel

        let result = encode_rgba_as_png(&rgba_data, 1, 1);
        assert!(result.is_ok());

        let png_data = result.unwrap();
        assert!(!png_data.is_empty());
        assert!(png_data.starts_with(&[137, 80, 78, 71]));
    }

    #[test]
    fn test_display_empty_image() {
        let image_data = vec![];

        let result = display_image(&image_data, 1, 1);
        assert!(result.is_ok());

        // Should still produce valid escape sequence
        let output = result.unwrap();
        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("\x1b]1337"));
    }

    #[test]
    fn test_encode_invalid_dimensions() {
        // RGBA data that doesn't match dimensions
        let rgba_data = vec![255, 0, 0, 255]; // 1 pixel worth of data

        // Try to create a 2x2 image (should fail)
        let result = encode_rgba_as_png(&rgba_data, 2, 2);
        assert!(result.is_err());
    }
}
