//! Sixel graphics protocol implementation
//!
//! Implements the Sixel graphics protocol for displaying images.
//! See: https://en.wikipedia.org/wiki/Sixel

#[cfg(feature = "images")]
use std::io::Write;

#[cfg(feature = "images")]
/// Convert RGBA image data to Sixel format
///
/// This is a basic Sixel encoder that converts RGBA pixels to Sixel format.
/// Sixel uses a palette-based encoding with run-length compression.
pub fn encode_sixel(
    rgba_data: &[u8],
    width: u32,
    height: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut output = Vec::new();

    // Start Sixel sequence: DCS q
    write!(output, "\x1bPq")?;

    // Set aspect ratio (1:1 for square pixels)
    write!(output, "\"1;1;{};{}", width, height)?;

    // Build a simple 256-color palette
    // For simplicity, we'll use a limited palette approach
    let palette = build_palette(rgba_data, width, height);

    // Write palette definitions
    for (idx, color) in palette.iter().enumerate() {
        if idx >= 256 {
            break; // Sixel supports up to 256 colors
        }
        // Define color: #<index>;2;<r>;<g>;<b>
        write!(
            output,
            "#{};2;{};{};{}",
            idx,
            (color.0 as u16 * 100) / 255,
            (color.1 as u16 * 100) / 255,
            (color.2 as u16 * 100) / 255
        )?;
    }

    // Convert image to sixel data
    // Process image in strips of 6 pixels height
    for strip_y in (0..height).step_by(6) {
        let strip_height = 6.min(height - strip_y);

        // For each color in palette
        for (color_idx, _color) in palette.iter().enumerate().take(256) {
            write!(output, "#{}", color_idx)?;

            // Encode this strip for this color
            let mut x = 0;
            while x < width {
                let mut sixel_char = 0u8;

                // Build sixel character from 6 vertical pixels
                for bit in 0..strip_height {
                    let y = strip_y + bit;
                    let pixel_idx = ((y * width + x) * 4) as usize;

                    if pixel_idx + 3 < rgba_data.len() {
                        let r = rgba_data[pixel_idx];
                        let g = rgba_data[pixel_idx + 1];
                        let b = rgba_data[pixel_idx + 2];
                        let a = rgba_data[pixel_idx + 3];

                        // Check if this pixel matches the current color
                        if a > 127 && palette_match(r, g, b, &palette[color_idx]) {
                            sixel_char |= 1 << bit;
                        }
                    }
                }

                // Output sixel character (offset by 63)
                if sixel_char != 0 {
                    write!(output, "{}", (sixel_char + 63) as char)?;
                } else {
                    write!(output, "?")?; // '?' = 63, represents empty
                }

                x += 1;
            }

            // Carriage return after each color
            write!(output, "$")?;
        }

        // Line feed after each strip
        write!(output, "-")?;
    }

    // End Sixel sequence: ST
    write!(output, "\x1b\\")?;

    Ok(output)
}

#[cfg(feature = "images")]
/// Build a simple palette from the image
fn build_palette(rgba_data: &[u8], width: u32, height: u32) -> Vec<(u8, u8, u8)> {
    use std::collections::HashMap;

    let mut color_counts: HashMap<(u8, u8, u8), u32> = HashMap::new();

    // Count color occurrences (quantized to reduce palette size)
    for y in 0..height {
        for x in 0..width {
            let pixel_idx = ((y * width + x) * 4) as usize;
            if pixel_idx + 3 < rgba_data.len() {
                let r = rgba_data[pixel_idx];
                let g = rgba_data[pixel_idx + 1];
                let b = rgba_data[pixel_idx + 2];
                let a = rgba_data[pixel_idx + 3];

                // Skip transparent pixels
                if a < 128 {
                    continue;
                }

                // Quantize to 6-bit per channel (64 levels)
                let qr = (r >> 2) << 2;
                let qg = (g >> 2) << 2;
                let qb = (b >> 2) << 2;

                *color_counts.entry((qr, qg, qb)).or_insert(0) += 1;
            }
        }
    }

    // Sort by frequency and take top 256
    let mut colors: Vec<_> = color_counts.into_iter().collect();
    colors.sort_by(|a, b| b.1.cmp(&a.1));

    colors
        .into_iter()
        .take(256)
        .map(|(color, _count)| color)
        .collect()
}

#[cfg(feature = "images")]
/// Check if RGB color matches palette color (with tolerance)
fn palette_match(r: u8, g: u8, b: u8, palette_color: &(u8, u8, u8)) -> bool {
    let threshold = 8; // Tolerance threshold
    let dr = (r as i16 - palette_color.0 as i16).abs();
    let dg = (g as i16 - palette_color.1 as i16).abs();
    let db = (b as i16 - palette_color.2 as i16).abs();

    dr < threshold && dg < threshold && db < threshold
}

#[cfg(all(test, feature = "images"))]
mod tests {
    use super::*;

    #[test]
    fn test_encode_single_pixel() {
        // Single red pixel
        let rgba_data = vec![255, 0, 0, 255];

        let result = encode_sixel(&rgba_data, 1, 1);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = String::from_utf8_lossy(&output);

        // Should start with DCS q
        assert!(output_str.starts_with("\x1bPq"));
        // Should end with ST
        assert!(output_str.ends_with("\x1b\\"));
    }

    #[test]
    fn test_encode_small_image() {
        // 2x2 image with different colors
        let rgba_data = vec![
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
            0, 0, 255, 255, // Blue
            255, 255, 255, 255, // White
        ];

        let result = encode_sixel(&rgba_data, 2, 2);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.is_empty());

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("\x1bPq")); // Start
        assert!(output_str.contains("\"1;1;2;2")); // Aspect ratio and dimensions
    }

    #[test]
    fn test_build_palette() {
        // Image with 3 distinct colors
        let rgba_data = vec![
            255, 0, 0, 255, // Red
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
            0, 0, 255, 255, // Blue
        ];

        let palette = build_palette(&rgba_data, 2, 2);

        // Should extract at least 3 colors
        assert!(palette.len() >= 3);

        // Most frequent should be red (appears twice)
        // Note: quantization might group similar colors
    }

    #[test]
    fn test_palette_match() {
        let palette_color = (128, 128, 128);

        // Exact match
        assert!(palette_match(128, 128, 128, &palette_color));

        // Close match (within threshold)
        assert!(palette_match(130, 130, 130, &palette_color));

        // No match (outside threshold)
        assert!(!palette_match(200, 200, 200, &palette_color));
    }

    #[test]
    fn test_encode_transparent_pixel() {
        // Fully transparent pixel
        let rgba_data = vec![255, 0, 0, 0];

        let result = encode_sixel(&rgba_data, 1, 1);
        assert!(result.is_ok());

        // Should still produce valid output
        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_encode_vertical_strip() {
        // 1x6 vertical strip (one sixel character worth)
        let rgba_data = vec![
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
            0, 0, 255, 255, // Blue
            255, 255, 0, 255, // Yellow
            255, 0, 255, 255, // Magenta
            0, 255, 255, 255, // Cyan
        ];

        let result = encode_sixel(&rgba_data, 1, 6);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = String::from_utf8_lossy(&output);

        // Should have dimension 1x6
        assert!(output_str.contains("\"1;1;1;6"));
    }
}
