# Inline Image Rendering Implementation Plan

## Overview

This plan implements optional inline image rendering for Markdown images (`![alt](path)` and `![alt](url)`) in the mdx TUI. The implementation follows an incremental approach with testing at each stage, starting with basic infrastructure and progressing to full multi-backend support.

## Design Principles

1. **Enabled by default** - Images render automatically when terminal supports them
2. **Graceful degradation** - Falls back to text placeholders on unsupported terminals
3. **No breaking changes** - Existing functionality remains unchanged
4. **Performance first** - Caching and lazy loading to maintain fast scrolling
5. **Offline by default** - Remote URLs disabled unless explicitly enabled
6. **Responsive sizing** - Image dimensions adapt to pane size (50% height, 90% width max)

## Architecture Summary

```
Markdown Parse → Image Registry → Layout Planning → Terminal Rendering
     ↓                ↓                  ↓                  ↓
  Extract         Decode/Cache      Reserve Space      Emit Protocol
  Image Nodes     Pixel Buffers     in Layout          or Placeholder
```

## Stage 1: Configuration and Data Model

**Goal**: Add configuration support and extend data model to represent images.

**Status**: Not Started

### Tasks

#### 1.1: Add image configuration to mdx-core

**File**: `mdx-core/src/config.rs`

Add new configuration struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub enabled: ImageEnabled,
    pub backend: ImageBackend,
    pub max_width_percent: u8,   // Percentage of pane width (1-100)
    pub max_height_percent: u8,  // Percentage of pane height (1-100)
    pub allow_remote: bool,
    pub cache_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageEnabled {
    Auto,  // Enable if terminal supports it
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageBackend {
    Auto,
    Kitty,
    ITerm2,
    Sixel,
    None,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            enabled: ImageEnabled::Auto,
            backend: ImageBackend::Auto,
            max_width_percent: 90,   // 90% of pane width
            max_height_percent: 50,  // 50% of pane height
            allow_remote: false,
            cache_dir: None,
        }
    }
}
```

Add to main `Config` struct:

```rust
pub struct Config {
    // ... existing fields
    #[cfg(feature = "images")]
    pub images: ImageConfig,
}
```

**Tests**:
- Verify default configuration values (enabled: Auto, max_width: 90%, max_height: 50%)
- Test YAML serialisation/deserialisation for ImageEnabled enum
- Test configuration loading with and without images section
- Verify percentage values are clamped to 1-100 range

#### 1.2: Add image feature flag

**File**: `Cargo.toml` (all three crates)

Add feature flag:

```toml
[features]
default = []
git = ["dep:gix", "dep:similar"]
watch = ["dep:notify"]
images = ["dep:image", "dep:blake3"]
```

Add dependencies (initially to mdx-core):

```toml
[dependencies]
image = { version = "0.25", optional = true, default-features = false, features = ["png", "jpeg"] }
blake3 = { version = "1.5", optional = true }
```

**Tests**:
- Verify feature compiles with `cargo build --features images`
- Verify feature doesn't break default build

#### 1.3: Add image node data structure

**File**: `mdx-core/src/image.rs` (new file)

```rust
use std::path::PathBuf;

/// Represents an image in the Markdown document
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageNode {
    /// Image source (path or URL)
    pub src: String,
    /// Alt text
    pub alt: String,
    /// Optional title
    pub title: Option<String>,
    /// Source line number in document
    pub source_line: usize,
}

/// Image resolution result
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Local file path (absolute)
    Local(PathBuf),
    /// Remote URL
    Remote(String),
}

impl ImageNode {
    pub fn new(src: String, alt: String, source_line: usize) -> Self {
        Self {
            src,
            alt,
            title: None,
            source_line,
        }
    }

    /// Resolve image source relative to document path
    pub fn resolve(&self, doc_path: &Path) -> Option<ImageSource> {
        // Implementation will determine if src is URL or local path
        // and resolve relative paths
        todo!()
    }
}
```

Add to `mdx-core/src/lib.rs`:

```rust
#[cfg(feature = "images")]
pub mod image;
```

**Tests**:
- Test ImageNode creation
- Test path resolution (relative and absolute)
- Test URL detection

#### 1.4: Extend Document to track images

**File**: `mdx-core/src/doc.rs`

Add field to Document:

```rust
pub struct Document {
    // ... existing fields
    #[cfg(feature = "images")]
    pub images: Vec<ImageNode>,
}
```

Update `Document::load()` to initialise empty vector.

**Tests**:
- Verify Document compiles with and without images feature
- Test Document::load() with images feature enabled

**Success Criteria**:
- Configuration loads from YAML with images section
- Feature flag compiles cleanly
- Data structures in place for image tracking
- All existing tests pass

---

## Stage 2: Markdown Parsing for Images

**Goal**: Extract image nodes from Markdown during document load.

**Status**: Not Started

### Tasks

#### 2.1: Add image extraction to parsing

**File**: `mdx-core/src/doc.rs`

Extend `Document::load()` to extract images from Markdown:

```rust
#[cfg(feature = "images")]
fn extract_images(rope: &Rope) -> Vec<ImageNode> {
    use pulldown_cmark::{Parser, Event, Tag, TagEnd};

    let text: String = rope.chunks().collect();
    let parser = Parser::new(&text);
    let mut images = Vec::new();
    let mut current_line = 0;

    for event in parser {
        match event {
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                // Extract image information
                // Track line number
            }
            Event::Text(text) if in_image => {
                // Capture alt text
            }
            _ => {}
        }
    }

    images
}
```

Call from `Document::load()`:

```rust
#[cfg(feature = "images")]
let images = extract_images(&rope);
```

**Tests**:
- Parse `![alt](path.png)` correctly
- Parse `![alt](path.png "title")` with title
- Parse `![](path.png)` with empty alt
- Track correct line numbers
- Handle multiple images in document
- Ignore images in code blocks

#### 2.2: Implement image source resolution

**File**: `mdx-core/src/image.rs`

Complete the `resolve()` method:

```rust
impl ImageNode {
    pub fn resolve(&self, doc_path: &Path) -> Option<ImageSource> {
        // Check if src is a URL
        if self.src.starts_with("http://") || self.src.starts_with("https://") {
            return Some(ImageSource::Remote(self.src.clone()));
        }

        // Resolve relative to document directory
        let doc_dir = doc_path.parent()?;
        let img_path = doc_dir.join(&self.src);

        // Canonicalise if it exists
        if let Ok(canonical) = img_path.canonicalize() {
            Some(ImageSource::Local(canonical))
        } else {
            None
        }
    }
}
```

**Tests**:
- Resolve relative paths (`./image.png`, `../images/test.png`)
- Resolve absolute paths
- Detect HTTP and HTTPS URLs
- Return None for non-existent files
- Handle edge cases (empty src, invalid paths)

**Success Criteria**:
- Images are extracted during document load
- Source paths resolve correctly
- Line numbers track accurately
- All tests pass

---

## Stage 3: Placeholder Rendering

**Goal**: Render text placeholders for images with reserved space in layout.

**Status**: Not Started

### Tasks

#### 3.1: Add placeholder rendering to UI

**File**: `mdx-tui/src/ui.rs`

When rendering document lines, detect image nodes and render placeholder:

```rust
// In render_document_content or similar function
#[cfg(feature = "images")]
if app.images_active {
    // Check if current line contains an image
    if let Some(image) = find_image_at_line(&app.doc.images, line_idx) {
        // Render placeholder
        let placeholder = format!("[Image: {}]",
            if image.alt.is_empty() { &image.src } else { &image.alt }
        );
        line_spans.push(Span::styled(
            placeholder,
            Style::default().fg(Color::Blue).add_modifier(Modifier::ITALIC)
        ));

        // Reserve vertical space (percentage of pane height)
        let pane_height = content_area.height;
        let image_height = calculate_placeholder_height(
            pane_height,
            app.config.images.max_height_percent,
        );
        for _ in 1..image_height {
            styled_lines.push(Line::from(""));
            line_idx += 1;
        }
        continue;
    }
}
```

Add helper functions:

```rust
#[cfg(feature = "images")]
fn find_image_at_line(images: &[ImageNode], line: usize) -> Option<&ImageNode> {
    images.iter().find(|img| img.source_line == line)
}

#[cfg(feature = "images")]
fn calculate_placeholder_height(pane_height: u16, max_percent: u8) -> usize {
    // Reserve space based on percentage of pane height (minimum 3 lines)
    let height = (pane_height as f32 * (max_percent as f32 / 100.0)) as usize;
    height.max(3)
}
```

**Tests**:
- Verify placeholder appears at correct line
- Verify space is reserved
- Verify scrolling still works smoothly
- Test with images disabled (no placeholders)

#### 3.2: Add runtime toggle for images

**File**: `mdx-tui/src/input.rs`

Add keybinding to toggle images:

```rust
// In handle_input or normal mode handling
#[cfg(feature = "images")]
KeyCode::Char('I') => {
    // Toggle runtime images flag
    app.images_active = !app.images_active;

    // Re-initialise or clear cache based on new state
    if app.images_active {
        app.image_cache = Some(ImageCache::new(&app.config.images));
    } else {
        app.image_cache = None;
    }

    Action::Continue
}
```

**File**: `mdx-tui/src/ui.rs`

Add status indicator when images are enabled:

```rust
// In status bar rendering
#[cfg(feature = "images")]
if app.images_active {
    status_parts.push(Span::styled(
        "[IMG]",
        Style::default().fg(Color::Green)
    ));
}
```

**Tests**:
- Verify 'I' key toggles images on/off
- Verify status indicator appears/disappears
- Verify layout updates correctly on toggle

**Success Criteria**:
- Placeholders render at correct locations
- Space is reserved in layout
- Runtime toggle works
- Status indicator shows image state
- Scrolling performance unaffected

---

## Stage 4: Image Decoding and Caching

**Goal**: Implement image loading, decoding, and caching infrastructure.

**Status**: Not Started

### Tasks

#### 4.1: Create image cache module

**File**: `mdx-tui/src/image_cache.rs` (new file)

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use blake3::Hash;

/// Cached decoded image data
#[derive(Clone)]
pub struct CachedImage {
    /// Original image dimensions
    pub width: u32,
    pub height: u32,
    /// RGBA pixel data
    pub pixels: Vec<u8>,
    /// Content hash for cache key
    pub hash: Hash,
}

/// Image cache with memory and optional disk backing
pub struct ImageCache {
    /// In-memory cache
    memory: HashMap<Hash, CachedImage>,
    /// Maximum cache size in bytes
    max_memory: usize,
    /// Current cache size in bytes
    current_memory: usize,
    /// Optional disk cache directory
    disk_cache: Option<PathBuf>,
}

impl ImageCache {
    pub fn new(config: &ImageConfig) -> Self {
        Self {
            memory: HashMap::new(),
            max_memory: 50 * 1024 * 1024, // 50MB default
            current_memory: 0,
            disk_cache: config.cache_dir.clone(),
        }
    }

    /// Load and cache an image
    pub fn load(&mut self, source: &ImageSource) -> Result<&CachedImage> {
        // Compute hash of source
        let hash = self.compute_hash(source)?;

        // Check memory cache
        if let Some(cached) = self.memory.get(&hash) {
            return Ok(cached);
        }

        // Check disk cache if enabled
        if let Some(from_disk) = self.load_from_disk(&hash)? {
            self.memory.insert(hash, from_disk);
            return Ok(self.memory.get(&hash).unwrap());
        }

        // Decode image
        let decoded = self.decode_image(source)?;

        // Store in disk cache if enabled
        if self.disk_cache.is_some() {
            self.save_to_disk(&hash, &decoded)?;
        }

        // Store in memory
        self.memory.insert(hash, decoded);
        Ok(self.memory.get(&hash).unwrap())
    }

    fn decode_image(&self, source: &ImageSource) -> Result<CachedImage> {
        match source {
            ImageSource::Local(path) => {
                let img = image::open(path)?;
                let rgba = img.to_rgba8();
                Ok(CachedImage {
                    width: rgba.width(),
                    height: rgba.height(),
                    pixels: rgba.into_raw(),
                    hash: self.compute_hash(source)?,
                })
            }
            ImageSource::Remote(_url) => {
                // Initially return error, implement in Stage 7
                Err(anyhow::anyhow!("Remote images not yet supported"))
            }
        }
    }

    fn compute_hash(&self, source: &ImageSource) -> Result<Hash> {
        match source {
            ImageSource::Local(path) => {
                let data = std::fs::read(path)?;
                Ok(blake3::hash(&data))
            }
            ImageSource::Remote(url) => {
                Ok(blake3::hash(url.as_bytes()))
            }
        }
    }

    fn load_from_disk(&self, hash: &Hash) -> Result<Option<CachedImage>> {
        // Implementation for disk cache loading
        Ok(None)
    }

    fn save_to_disk(&self, hash: &Hash, image: &CachedImage) -> Result<()> {
        // Implementation for disk cache saving
        Ok(())
    }

    /// Evict least recently used images if over memory limit
    fn evict_if_needed(&mut self) {
        // LRU eviction implementation
    }
}
```

Add to `mdx-tui/src/lib.rs`:

```rust
#[cfg(feature = "images")]
pub mod image_cache;
```

**Tests**:
- Test image loading from local file
- Test hash computation consistency
- Test memory cache hit/miss
- Test cache eviction
- Test error handling for missing files

#### 4.2: Integrate cache with App state

**File**: `mdx-tui/src/app.rs`

Add cache to App:

```rust
pub struct App {
    // ... existing fields
    #[cfg(feature = "images")]
    image_cache: Option<ImageCache>,
}
```

Initialise in `App::new()`:

```rust
#[cfg(feature = "images")]
let (image_cache, images_active) = {
    // Detect terminal support
    let backend = detect_backend(config.images.backend);
    let has_support = backend != DetectedBackend::None;

    // Determine if images should be active
    let active = match config.images.enabled {
        ImageEnabled::Auto => has_support,
        ImageEnabled::Always => true,
        ImageEnabled::Never => false,
    };

    let cache = if active {
        Some(ImageCache::new(&config.images))
    } else {
        None
    };

    (cache, active)
};
```

Store in App struct:

```rust
pub struct App {
    // ... existing fields
    #[cfg(feature = "images")]
    image_cache: Option<ImageCache>,
    #[cfg(feature = "images")]
    images_active: bool,  // Runtime flag for whether images are currently enabled
}
```

**Tests**:
- Verify cache initialises when enabled=Always
- Verify cache initialises when enabled=Auto and terminal supports images
- Verify cache is None when enabled=Never
- Verify cache is None when enabled=Auto and terminal doesn't support images

**Success Criteria**:
- Images can be loaded and decoded
- Cache stores and retrieves images correctly
- Memory limits are enforced
- Hash-based deduplication works

---

## Stage 5: Terminal Backend Detection

**Goal**: Detect terminal capabilities and select appropriate rendering backend.

**Status**: Not Started

### Tasks

#### 5.1: Create backend detection module

**File**: `mdx-tui/src/image_backend.rs` (new file)

```rust
use crate::config::ImageBackend;

/// Detected terminal image capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedBackend {
    Kitty,
    ITerm2,
    Sixel,
    None,
}

/// Detect terminal image support
pub fn detect_backend(config_backend: ImageBackend) -> DetectedBackend {
    match config_backend {
        ImageBackend::Auto => auto_detect(),
        ImageBackend::Kitty => DetectedBackend::Kitty,
        ImageBackend::ITerm2 => DetectedBackend::ITerm2,
        ImageBackend::Sixel => DetectedBackend::Sixel,
        ImageBackend::None => DetectedBackend::None,
    }
}

fn auto_detect() -> DetectedBackend {
    // Check for Kitty
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return DetectedBackend::Kitty;
    }

    // Check for iTerm2
    if std::env::var("ITERM_SESSION_ID").is_ok() {
        return DetectedBackend::ITerm2;
    }

    // Check TERM for Sixel support
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("sixel") || term.contains("mlterm") {
            return DetectedBackend::Sixel;
        }
    }

    // No support detected
    DetectedBackend::None
}
```

**Tests**:
- Test detection with KITTY_WINDOW_ID set
- Test detection with ITERM_SESSION_ID set
- Test detection with TERM=xterm-256color-sixel
- Test fallback to None
- Test explicit backend override

**Success Criteria**:
- Backend detection works for common terminals
- Config override takes precedence
- Defaults to None when unsure

---

## Stage 6: Kitty Graphics Protocol Implementation

**Goal**: Implement basic image rendering using Kitty graphics protocol.

**Status**: Not Started

### Tasks

#### 6.1: Create Kitty protocol renderer

**File**: `mdx-tui/src/image_render.rs` (new file)

```rust
use crate::image_cache::CachedImage;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Render image using Kitty graphics protocol
pub fn render_kitty_image(
    image: &CachedImage,
    x: u16,
    y: u16,
    cell_width: u16,
    cell_height: u16,
) -> Result<String> {
    // Encode image data as base64
    let mut output = String::new();

    // Kitty graphics protocol escape sequence
    // Format: \x1b_Ga=T,f=32,s=<width>,v=<height>,m=1;<base64_data>\x1b\\

    output.push_str("\x1b_G");
    output.push_str("a=T,"); // Action: transmit
    output.push_str("f=32,"); // Format: RGBA
    output.push_str(&format!("s={},", image.width));
    output.push_str(&format!("v={},", image.height));
    output.push_str(&format!("c={},", cell_width));
    output.push_str(&format!("r={},", cell_height));
    output.push_str("m=1;"); // More data follows

    // Encode pixel data
    let encoded = BASE64.encode(&image.pixels);
    output.push_str(&encoded);
    output.push_str("\x1b\\");

    Ok(output)
}

/// Calculate terminal cell dimensions for image based on pane size
pub fn calculate_cell_dimensions(
    image_width: u32,
    image_height: u32,
    pane_width: u16,
    pane_height: u16,
    max_width_percent: u8,
    max_height_percent: u8,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) -> (u16, u16) {
    // Calculate max cells based on pane size and percentages
    let max_width = (pane_width as f32 * (max_width_percent as f32 / 100.0)) as u16;
    let max_height = (pane_height as f32 * (max_height_percent as f32 / 100.0)) as u16;

    // Calculate cells needed preserving aspect ratio
    let aspect = image_width as f32 / image_height as f32;

    let mut width_cells = (image_width as f32 / cell_pixel_width as f32).ceil() as u16;
    let mut height_cells = (image_height as f32 / cell_pixel_height as f32).ceil() as u16;

    // Clamp to max dimensions (percentage of pane)
    if width_cells > max_width {
        width_cells = max_width;
        height_cells = (width_cells as f32 / aspect).ceil() as u16;
    }

    if height_cells > max_height {
        height_cells = max_height;
        width_cells = (height_cells as f32 * aspect).ceil() as u16;
    }

    (width_cells, height_cells)
}
```

**Tests**:
- Test escape sequence generation
- Test cell dimension calculation with pane size
- Test percentage-based max dimension calculation (90% width, 50% height)
- Test aspect ratio preservation
- Test clamping to percentage-based limits
- Test with various pane sizes (small, medium, large)

#### 6.2: Integrate rendering with UI

**File**: `mdx-tui/src/ui.rs`

Replace placeholder rendering with actual image rendering:

```rust
#[cfg(feature = "images")]
if app.images_active && backend != DetectedBackend::None {
    if let Some(image) = find_image_at_line(&app.doc.images, line_idx) {
        // Resolve image source
        if let Some(source) = image.resolve(&app.doc.path) {
            // Load from cache
            if let Ok(cached) = app.image_cache.as_mut().unwrap().load(&source) {
                // Get current pane dimensions
                let pane_width = content_area.width;
                let pane_height = content_area.height;

                // Calculate dimensions based on pane size and percentages
                let (width_cells, height_cells) = calculate_cell_dimensions(
                    cached.width,
                    cached.height,
                    pane_width,
                    pane_height,
                    app.config.images.max_width_percent,
                    app.config.images.max_height_percent,
                    10, // Cell pixel width (estimate, should detect)
                    20, // Cell pixel height (estimate, should detect)
                );

                // Render based on backend
                match backend {
                    DetectedBackend::Kitty => {
                        let escape = render_kitty_image(cached, 0, line_idx as u16, width_cells, height_cells)?;
                        // Write escape sequence to terminal
                        write!(terminal.backend_mut(), "{}", escape)?;
                    }
                    _ => {
                        // Fallback to placeholder
                        render_placeholder(image);
                    }
                }

                // Reserve space in layout
                for _ in 0..height_cells {
                    styled_lines.push(Line::from(""));
                }
                line_idx += height_cells as usize;
                continue;
            }
        }
        // If loading failed, render placeholder
        render_placeholder(image);
    }
}
```

**Tests**:
- Manual test in Kitty terminal with PNG
- Manual test in Kitty with JPEG
- Verify fallback to placeholder on decode failure
- Verify layout stability

**Success Criteria**:
- Images render in Kitty terminal
- Aspect ratio preserved
- Size limits respected
- Scrolling still smooth
- Fallback works correctly

---

## Stage 7: Additional Backends (Optional)

**Goal**: Add support for iTerm2 and Sixel protocols.

**Status**: Not Started

### Tasks

#### 7.1: Implement iTerm2 inline images

**File**: `mdx-tui/src/image_render.rs`

Add iTerm2 renderer:

```rust
/// Render image using iTerm2 inline images protocol
pub fn render_iterm2_image(
    image: &CachedImage,
    width_cells: u16,
    height_cells: u16,
) -> Result<String> {
    // iTerm2 protocol: \x1b]1337;File=inline=1;width=<width>;height=<height>:<base64>\x07

    let mut output = String::new();
    output.push_str("\x1b]1337;File=inline=1;");
    output.push_str(&format!("width={};", width_cells));
    output.push_str(&format!("height={};", height_cells));
    output.push_str(":");

    // Encode as PNG and base64
    let png_data = encode_as_png(image)?;
    let encoded = BASE64.encode(&png_data);
    output.push_str(&encoded);
    output.push_str("\x07");

    Ok(output)
}

fn encode_as_png(image: &CachedImage) -> Result<Vec<u8>> {
    // Use image crate to encode as PNG
    let img = image::RgbaImage::from_raw(
        image.width,
        image.height,
        image.pixels.clone(),
    ).ok_or_else(|| anyhow::anyhow!("Failed to create image"))?;

    let mut png_data = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
    encoder.write_image(
        &img,
        image.width,
        image.height,
        image::ColorType::Rgba8,
    )?;

    Ok(png_data)
}
```

#### 7.2: Implement Sixel protocol (optional)

This can be deferred or implemented as a lower priority. Sixel support requires more complex encoding and is less common.

**Success Criteria**:
- iTerm2 images render correctly on macOS
- PNG encoding works
- Sixel implementation (if included) works in compatible terminals

---

## Stage 8: Remote Image Support (Optional)

**Goal**: Add support for loading images from HTTP/HTTPS URLs.

**Status**: Not Started

### Tasks

#### 8.1: Add HTTP client for image fetching

Add dependency:

```toml
[dependencies]
reqwest = { version = "0.11", optional = true, default-features = false, features = ["blocking", "rustls-tls"] }
```

Update feature:

```toml
[features]
images-remote = ["images", "dep:reqwest"]
```

#### 8.2: Implement remote image loading

**File**: `mdx-tui/src/image_cache.rs`

Add remote fetching:

```rust
fn fetch_remote(&self, url: &str) -> Result<Vec<u8>> {
    // Size limit check
    const MAX_SIZE: u64 = 10 * 1024 * 1024; // 10MB

    let response = reqwest::blocking::get(url)?;

    // Check content length
    if let Some(len) = response.content_length() {
        if len > MAX_SIZE {
            return Err(anyhow::anyhow!("Image too large"));
        }
    }

    // Download with size limit
    let bytes = response.bytes()?.to_vec();
    if bytes.len() as u64 > MAX_SIZE {
        return Err(anyhow::anyhow!("Image too large"));
    }

    Ok(bytes)
}
```

Update `decode_image()` to handle remote:

```rust
ImageSource::Remote(url) => {
    if !config.allow_remote {
        return Err(anyhow::anyhow!("Remote images disabled"));
    }

    let data = self.fetch_remote(url)?;
    let img = image::load_from_memory(&data)?;
    // ... rest of decoding
}
```

**Tests**:
- Test remote fetch with allow_remote enabled
- Test rejection when allow_remote disabled
- Test size limit enforcement
- Test timeout handling

**Success Criteria**:
- Remote images load when enabled
- Size limits enforced
- Errors handled gracefully
- Respects allow_remote config

---

## Testing Strategy

### Unit Tests

Each module should have comprehensive unit tests:

- `mdx-core/src/image.rs`: Image node parsing and resolution
- `mdx-tui/src/image_cache.rs`: Cache operations, eviction, disk I/O
- `mdx-tui/src/image_render.rs`: Dimension calculations, escape sequence generation
- `mdx-tui/src/image_backend.rs`: Backend detection logic

### Integration Tests

Test full pipeline:

1. Load document with images
2. Images extracted and resolved
3. Cache loads and stores images
4. Rendering produces correct output
5. Toggle functionality works

### Manual Testing

Test in real terminals:

- Kitty (Linux/macOS)
- iTerm2 (macOS)
- Alacritty (fallback to placeholder)
- Terminal.app (fallback to placeholder)
- tmux/screen (verify no corruption)

### Performance Testing

- Load document with 50+ images
- Verify scrolling remains smooth
- Monitor memory usage
- Test cache eviction under memory pressure

---

## Configuration Examples

### Default (auto-enabled, responsive sizing)

```yaml
# Images are enabled automatically if terminal supports them
# Uses default 90% width, 50% height of pane
images:
  enabled: auto
  backend: auto
```

### Always enabled with custom percentages

```yaml
images:
  enabled: always
  backend: auto
  max_width: 80        # 80% of pane width
  max_height: 60       # 60% of pane height
```

### Disabled

```yaml
images:
  enabled: never
```

### With remote images and caching

```yaml
images:
  enabled: auto
  backend: kitty
  max_width: 90        # 90% of pane width
  max_height: 50       # 50% of pane height
  allow_remote: true
  cache_dir: "~/.cache/mdx/images"
```

---

## Open Questions and Decisions

### Q1: Inline vs block images?

**Decision**: Start with block-level only (images on their own line). Inline images within paragraphs can be added later if needed.

### Q2: Images in selection and yank?

**Decision**: Skip images during visual line selection. Only yank the markdown source `![alt](path)`.

### Q3: Images in lists and blockquotes?

**Decision**: Respect indentation level of the containing block. Indent image content by the same amount.

### Q4: Automatic SSH detection?

**Decision**: Don't auto-disable over SSH. Users can disable via config or runtime toggle if needed.

### Q5: Cell pixel size detection?

**Decision**: Start with hardcoded estimates (10x20 pixels per cell). Add actual detection via terminal queries in a later iteration if needed.

---

## Dependencies Summary

New dependencies required:

```toml
# mdx-core
image = { version = "0.25", optional = true, default-features = false, features = ["png", "jpeg"] }
blake3 = { version = "1.5", optional = true }

# mdx-tui (if remote images enabled)
reqwest = { version = "0.11", optional = true, default-features = false, features = ["blocking", "rustls-tls"] }
```

Additional features for more formats (optional):

```toml
image = { features = ["png", "jpeg", "webp", "gif"] }
```

---

## Implementation Order

1. Stage 1: Configuration and data model (1-2 days)
2. Stage 2: Markdown parsing (1 day)
3. Stage 3: Placeholder rendering (1 day)
4. Stage 4: Image decoding and caching (2-3 days)
5. Stage 5: Backend detection (0.5 days)
6. Stage 6: Kitty implementation (2-3 days)
7. Stage 7: iTerm2 implementation (1-2 days, optional)
8. Stage 8: Remote images (1-2 days, optional)

**Total estimate**: 9-14 days for core functionality (Stages 1-6), with additional time for optional backends and remote support.

---

## Success Criteria

The feature is complete when:

1. Images are automatically enabled on supported terminals (Kitty, iTerm2, Sixel)
2. Images gracefully fall back to placeholders on unsupported terminals
3. Image dimensions respect pane size (90% width max, 50% height max)
4. Images resize responsively when pane is resized
5. Configuration allows override (always/never/auto)
6. Runtime toggle (I key) works
7. Performance remains acceptable with many images
8. Memory usage stays within limits via caching
9. All tests pass
10. Documentation updated

---

## Documentation Updates Needed

- README.md: Add images feature to feature list
- Configuration section: Document images config options
- Keybindings: Document 'I' toggle key
- Build instructions: Note images feature flag

---

## Future Enhancements

Beyond initial implementation:

- Inline images within paragraphs
- Image dimension hints from Markdown attributes
- Animated GIF support (first frame only initially)
- WebP and AVIF format support
- Automatic terminal capability probing
- Image preview on hover/focus
- Jump to image keybinding
- Image gallery view
