//! Image decoding and caching for terminal rendering

#[cfg(feature = "images")]
use lru::LruCache;
#[cfg(feature = "images")]
use std::num::NonZeroUsize;
#[cfg(feature = "images")]
use std::path::Path;

#[cfg(feature = "images")]
/// Cache key for decoded images
#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct ImageCacheKey {
    /// Blake3 hash of file path + mtime
    pub hash: [u8; 32],
}

#[cfg(feature = "images")]
impl ImageCacheKey {
    /// Create cache key from file path
    pub fn from_path(path: &Path) -> Option<Self> {
        use std::fs;

        // Get file metadata for mtime
        let metadata = fs::metadata(path).ok()?;
        let mtime = metadata.modified().ok()?;

        // Create hash input: path + mtime
        let path_str = path.to_string_lossy();
        let mtime_secs = mtime.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();

        let mut hasher = blake3::Hasher::new();
        hasher.update(path_str.as_bytes());
        hasher.update(&mtime_secs.to_le_bytes());

        let hash = hasher.finalize();
        Some(Self {
            hash: *hash.as_bytes(),
        })
    }
}

#[cfg(feature = "images")]
/// Decoded image data ready for terminal rendering
#[derive(Clone)]
pub struct DecodedImage {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// RGBA pixel data
    pub data: Vec<u8>,
}

#[cfg(feature = "images")]
impl DecodedImage {
    /// Decode image from file path with size constraints
    pub fn from_path(path: &Path, max_width: u32, max_height: u32) -> anyhow::Result<Self> {
        use image::GenericImageView;

        // Load and decode image
        let img = image::open(path)?;
        let (orig_width, orig_height) = img.dimensions();

        // Calculate scaled dimensions maintaining aspect ratio
        let (width, height) = calculate_scaled_dimensions(
            orig_width,
            orig_height,
            max_width,
            max_height,
        );

        // Resize if needed
        let resized = if width != orig_width || height != orig_height {
            img.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // Convert to RGBA
        let rgba = resized.to_rgba8();
        let data = rgba.into_raw();

        Ok(Self {
            width,
            height,
            data,
        })
    }
}

#[cfg(feature = "images")]
/// Calculate scaled dimensions maintaining aspect ratio
fn calculate_scaled_dimensions(
    orig_width: u32,
    orig_height: u32,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    if orig_width <= max_width && orig_height <= max_height {
        return (orig_width, orig_height);
    }

    let width_ratio = max_width as f64 / orig_width as f64;
    let height_ratio = max_height as f64 / orig_height as f64;

    let scale = width_ratio.min(height_ratio);

    let new_width = (orig_width as f64 * scale).round() as u32;
    let new_height = (orig_height as f64 * scale).round() as u32;

    (new_width.max(1), new_height.max(1))
}

#[cfg(feature = "images")]
/// LRU cache for decoded images
pub struct ImageCache {
    cache: LruCache<ImageCacheKey, DecodedImage>,
}

#[cfg(feature = "images")]
impl ImageCache {
    /// Create new image cache with capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap()),
        }
    }

    /// Get cached image or decode and cache it
    pub fn get_or_decode(
        &mut self,
        path: &Path,
        max_width: u32,
        max_height: u32,
    ) -> anyhow::Result<DecodedImage> {
        // Generate cache key
        let key = ImageCacheKey::from_path(path)
            .ok_or_else(|| anyhow::anyhow!("Failed to generate cache key for {:?}", path))?;

        // Check cache first
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached.clone());
        }

        // Decode image
        let decoded = DecodedImage::from_path(path, max_width, max_height)?;

        // Store in cache
        self.cache.put(key, decoded.clone());

        Ok(decoded)
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

#[cfg(feature = "images")]
impl Default for ImageCache {
    fn default() -> Self {
        Self::new(32) // Default to 32 cached images
    }
}

#[cfg(all(test, feature = "images"))]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_image() -> NamedTempFile {
        use image::{ImageBuffer, Rgb};
        use std::io::Cursor;

        let mut file = tempfile::Builder::new()
            .suffix(".png")
            .tempfile()
            .unwrap();

        // Create a simple 100x100 red image
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(100, 100, |_x, _y| {
            Rgb([255, 0, 0])
        });

        // Encode as PNG
        let mut png_data = Vec::new();
        let mut cursor = Cursor::new(&mut png_data);
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        file.write_all(&png_data).unwrap();
        file.flush().unwrap();

        file
    }

    #[test]
    fn test_cache_key_from_path() {
        let file = create_test_image();
        let key = ImageCacheKey::from_path(file.path());
        assert!(key.is_some());
    }

    #[test]
    fn test_decode_image() {
        let file = create_test_image();
        let decoded = DecodedImage::from_path(file.path(), 100, 100);
        assert!(decoded.is_ok());

        let img = decoded.unwrap();
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);
        assert_eq!(img.data.len(), 100 * 100 * 4); // RGBA
    }

    #[test]
    fn test_decode_with_scaling() {
        let file = create_test_image();
        let decoded = DecodedImage::from_path(file.path(), 50, 50);
        assert!(decoded.is_ok());

        let img = decoded.unwrap();
        assert_eq!(img.width, 50);
        assert_eq!(img.height, 50);
    }

    #[test]
    fn test_scaled_dimensions() {
        // No scaling needed
        assert_eq!(calculate_scaled_dimensions(100, 100, 200, 200), (100, 100));

        // Width constrained
        assert_eq!(calculate_scaled_dimensions(200, 100, 100, 200), (100, 50));

        // Height constrained
        assert_eq!(calculate_scaled_dimensions(100, 200, 200, 100), (50, 100));

        // Both constrained, width is limiting
        assert_eq!(calculate_scaled_dimensions(200, 100, 50, 100), (50, 25));

        // Both constrained, height is limiting
        assert_eq!(calculate_scaled_dimensions(100, 200, 100, 50), (25, 50));
    }

    #[test]
    fn test_image_cache() {
        let file = create_test_image();
        let mut cache = ImageCache::new(2);

        // First access - should decode
        let img1 = cache.get_or_decode(file.path(), 100, 100);
        assert!(img1.is_ok());

        // Second access - should hit cache
        let img2 = cache.get_or_decode(file.path(), 100, 100);
        assert!(img2.is_ok());
    }

    #[test]
    fn test_cache_clear() {
        let file = create_test_image();
        let mut cache = ImageCache::new(2);

        cache.get_or_decode(file.path(), 100, 100).unwrap();
        assert_eq!(cache.cache.len(), 1);

        cache.clear();
        assert_eq!(cache.cache.len(), 0);
    }
}
