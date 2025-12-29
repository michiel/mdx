//! Kitty graphics protocol implementation
//!
//! Implements the Kitty terminal graphics protocol for displaying images.
//! See: https://sw.kovidgoyal.net/kitty/graphics-protocol/

#[cfg(feature = "images")]
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
#[cfg(feature = "images")]
use std::io::Write;

#[cfg(feature = "images")]
/// Transmit image to Kitty terminal
///
/// This function sends an image using the Kitty graphics protocol.
/// The image is transmitted in chunks if needed.
pub fn transmit_image(
    image_data: &[u8],
    width: u32,
    height: u32,
    image_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut output = Vec::new();

    // Encode image data as base64
    let encoded = BASE64.encode(image_data);

    // Kitty protocol uses chunks of max 4096 bytes
    const CHUNK_SIZE: usize = 4096;
    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(CHUNK_SIZE)
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect();

    for (idx, chunk) in chunks.iter().enumerate() {
        let is_first = idx == 0;
        let is_last = idx == chunks.len() - 1;

        // Build control data
        let mut control = String::new();

        if is_first {
            // First chunk includes image metadata
            control.push_str(&format!("a=T,f=32,s={},v={},i={}", width, height, image_id));
        } else {
            // Continuation chunks just reference the image ID
            control.push_str(&format!("i={}", image_id));
        }

        // Add 'm' parameter for chunking
        if !is_last {
            control.push_str(",m=1"); // More chunks coming
        } else {
            control.push_str(",m=0"); // Last chunk
        }

        // Write graphics command
        // Format: ESC _G<control>;<payload>ESC \
        write!(output, "\x1b_G{};{}\x1b\\", control, chunk)?;
    }

    Ok(output)
}

#[cfg(feature = "images")]
/// Delete image from Kitty terminal
pub fn delete_image(image_id: u32) -> anyhow::Result<Vec<u8>> {
    let mut output = Vec::new();
    // Delete by image ID: a=d,d=I,i=<id>
    write!(output, "\x1b_Ga=d,d=I,i={}\x1b\\", image_id)?;
    Ok(output)
}

#[cfg(feature = "images")]
/// Display image at cursor position with specified rows/columns
pub fn display_image(
    image_id: u32,
    rows: u16,
    cols: u16,
) -> anyhow::Result<Vec<u8>> {
    let mut output = Vec::new();
    // Display command: a=p,i=<id>,r=<rows>,c=<cols>
    write!(output, "\x1b_Ga=p,i={},r={},c={}\x1b\\", image_id, rows, cols)?;
    Ok(output)
}

#[cfg(all(test, feature = "images"))]
mod tests {
    use super::*;

    #[test]
    fn test_transmit_small_image() {
        // Create a small 2x2 RGBA image (16 bytes)
        let image_data = vec![
            255, 0, 0, 255, // Red pixel
            0, 255, 0, 255, // Green pixel
            0, 0, 255, 255, // Blue pixel
            255, 255, 255, 255, // White pixel
        ];

        let result = transmit_image(&image_data, 2, 2, 1);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = String::from_utf8_lossy(&output);

        // Should contain Kitty graphics escape sequence
        assert!(output_str.contains("\x1b_G"));
        assert!(output_str.contains("a=T")); // Transmit action
        assert!(output_str.contains("f=32")); // RGBA format
        assert!(output_str.contains("s=2")); // Width
        assert!(output_str.contains("v=2")); // Height
        assert!(output_str.contains("i=1")); // Image ID
        assert!(output_str.contains("m=0")); // Single chunk
    }

    #[test]
    fn test_transmit_large_image_chunked() {
        // Create a large image that will require chunking
        // 4096 base64 chars = 3072 bytes of raw data
        // So 4000 bytes should definitely trigger chunking
        let image_data = vec![0u8; 4000];

        let result = transmit_image(&image_data, 100, 100, 2);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = String::from_utf8_lossy(&output);

        // Should contain multiple chunks
        assert!(output_str.contains("m=1")); // More chunks indicator
        assert!(output_str.contains("m=0")); // Last chunk indicator

        // Should have multiple escape sequences
        let escape_count = output_str.matches("\x1b_G").count();
        assert!(escape_count > 1);
    }

    #[test]
    fn test_delete_image() {
        let result = delete_image(42);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = String::from_utf8_lossy(&output);

        assert!(output_str.contains("\x1b_G"));
        assert!(output_str.contains("a=d")); // Delete action
        assert!(output_str.contains("d=I")); // Delete by ID
        assert!(output_str.contains("i=42")); // Image ID
    }

    #[test]
    fn test_display_image() {
        let result = display_image(5, 10, 20);
        assert!(result.is_ok());

        let output = result.unwrap();
        let output_str = String::from_utf8_lossy(&output);

        assert!(output_str.contains("\x1b_G"));
        assert!(output_str.contains("a=p")); // Display action
        assert!(output_str.contains("i=5")); // Image ID
        assert!(output_str.contains("r=10")); // Rows
        assert!(output_str.contains("c=20")); // Columns
    }

    #[test]
    fn test_single_pixel_image() {
        // Test a minimal valid image (1x1 RGBA = 4 bytes)
        let image_data = vec![255, 0, 0, 255]; // Single red pixel
        let result = transmit_image(&image_data, 1, 1, 1);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.is_empty());

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("s=1")); // Width 1
        assert!(output_str.contains("v=1")); // Height 1
    }
}
