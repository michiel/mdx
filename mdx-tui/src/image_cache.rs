//! Image metadata reading for placeholders

#[cfg(feature = "images")]
use std::path::Path;

#[cfg(feature = "images")]
/// Image metadata (just dimensions)
#[derive(Clone, Debug)]
pub struct ImageMetadata {
    /// Image width in pixels
    pub width: usize,
    /// Image height in pixels
    pub height: usize,
}

#[cfg(feature = "images")]
impl ImageMetadata {
    /// Read image dimensions from file path
    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        let size = imagesize::size(path)?;

        Ok(Self {
            width: size.width,
            height: size.height,
        })
    }
}

#[cfg(all(test, feature = "images"))]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_png() -> NamedTempFile {
        let mut file = tempfile::Builder::new()
            .suffix(".png")
            .tempfile()
            .unwrap();

        // Minimal valid PNG (1x1 red pixel)
        let png_data = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1 dimensions
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
            0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41,
            0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
            0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D,
            0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
            0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        file.write_all(&png_data).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_read_metadata() {
        let file = create_test_png();
        let metadata = ImageMetadata::from_path(file.path());
        assert!(metadata.is_ok());

        let meta = metadata.unwrap();
        assert_eq!(meta.width, 1);
        assert_eq!(meta.height, 1);
    }
}
