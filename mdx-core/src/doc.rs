//! Document model with Rope-based text storage

use anyhow::{Context, Result};
use ropey::Rope;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::security::SecurityEvent;
use crate::toc;

/// Maximum file size that can be loaded (10MB)
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum number of headings allowed in a document
const MAX_HEADINGS: usize = 1000;

/// Maximum number of images allowed in a document
const MAX_IMAGES: usize = 100;

#[cfg(feature = "git")]
use crate::diff::DiffGutter;

#[cfg(feature = "images")]
use crate::image::ImageNode;

/// A heading in the markdown document
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
    pub anchor: String,
}

/// The main document structure
#[derive(Clone, Debug)]
pub struct Document {
    pub path: PathBuf,
    pub rope: Rope,
    pub headings: Vec<Heading>,
    pub loaded_mtime: Option<SystemTime>,
    pub disk_mtime: Option<SystemTime>,
    pub dirty_on_disk: bool,
    pub rev: u64,
    #[cfg(feature = "git")]
    pub diff_gutter: DiffGutter,
    #[cfg(feature = "images")]
    pub images: Vec<ImageNode>,
}

impl Document {
    /// Load a document from a file path
    /// Returns (Document, Vec<SecurityEvent>) where events track security warnings
    pub fn load(path: &Path) -> Result<(Self, Vec<SecurityEvent>)> {
        let mut warnings = Vec::new();

        // Canonicalize the path to get absolute path (needed for git integration)
        let abs_path = path.canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?;

        // Check file size before reading
        let metadata = fs::metadata(&abs_path)
            .with_context(|| format!("Failed to read file metadata: {}", abs_path.display()))?;

        let file_size = metadata.len();
        if file_size > MAX_FILE_SIZE {
            anyhow::bail!(
                "File exceeds maximum size of 10MB ({} bytes)",
                file_size
            );
        }

        // Warn if approaching size limit (>80%)
        if file_size > MAX_FILE_SIZE * 8 / 10 {
            warnings.push(SecurityEvent::warning(
                format!("Large file: {} bytes", file_size),
                "document"
            ));
        }

        let content = fs::read_to_string(&abs_path)
            .with_context(|| format!("Failed to read file: {}", abs_path.display()))?;

        let rope = Rope::from_str(&content);
        let headings = toc::extract_headings(&rope);

        // Check heading count limit
        if headings.len() > MAX_HEADINGS {
            anyhow::bail!(
                "Document has too many headings ({}, max is {})",
                headings.len(),
                MAX_HEADINGS
            );
        }

        // Warn if approaching heading limit (>80%)
        if headings.len() > MAX_HEADINGS * 8 / 10 {
            warnings.push(SecurityEvent::warning(
                format!("Many headings: {}", headings.len()),
                "document"
            ));
        }

        let mtime = metadata.modified().ok();

        // Initialize with empty diff gutter - will be computed asynchronously by worker thread
        #[cfg(feature = "git")]
        let diff_gutter = {
            let line_count = rope.len_lines();
            DiffGutter::empty(line_count)
        };

        // Extract images from Markdown
        #[cfg(feature = "images")]
        let images = extract_images(&rope);

        // Check image count limit
        #[cfg(feature = "images")]
        if images.len() > MAX_IMAGES {
            anyhow::bail!(
                "Document has too many images ({}, max is {})",
                images.len(),
                MAX_IMAGES
            );
        }

        // Warn if approaching image limit (>80%)
        #[cfg(feature = "images")]
        if images.len() > MAX_IMAGES * 8 / 10 {
            warnings.push(SecurityEvent::warning(
                format!("Many images: {}", images.len()),
                "document"
            ));
        }

        let doc = Self {
            path: abs_path,
            rope,
            headings,
            loaded_mtime: mtime,
            disk_mtime: mtime,
            dirty_on_disk: false,
            rev: 1,
            #[cfg(feature = "git")]
            diff_gutter,
            #[cfg(feature = "images")]
            images,
        };

        Ok((doc, warnings))
    }

    /// Reload the document from disk
    pub fn reload(&mut self) -> Result<()> {
        let content = fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to reload file: {}", self.path.display()))?;

        self.rope = Rope::from_str(&content);
        self.headings = toc::extract_headings(&self.rope);

        let metadata = fs::metadata(&self.path).ok();
        let mtime = metadata.and_then(|m| m.modified().ok());

        self.loaded_mtime = mtime;
        self.disk_mtime = mtime;
        self.dirty_on_disk = false;
        self.rev += 1;

        // Reset diff gutter to empty - will be computed asynchronously by worker thread
        #[cfg(feature = "git")]
        {
            let line_count = self.rope.len_lines();
            self.diff_gutter = DiffGutter::empty(line_count);
        }

        // Re-extract images from Markdown
        #[cfg(feature = "images")]
        {
            self.images = extract_images(&self.rope);
        }

        Ok(())
    }

    /// Get the number of lines in the document
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Extract lines for yank operations (inclusive range)
    pub fn get_lines(&self, start: usize, end_inclusive: usize) -> String {
        let line_count = self.line_count();

        // Clamp to valid range
        let start = start.min(line_count.saturating_sub(1));
        let end = end_inclusive.min(line_count.saturating_sub(1));

        if start > end {
            return String::new();
        }

        // Extract lines
        let mut result = String::new();
        for line_idx in start..=end {
            if line_idx < line_count {
                let line = self.rope.line(line_idx);
                for chunk in line.chunks() {
                    result.push_str(chunk);
                }
            }
        }

        // Remove trailing newline if present (yank should give clean text)
        if result.ends_with('\n') {
            result.pop();
        }

        result
    }
}

/// Extract images from Markdown text
#[cfg(feature = "images")]
fn extract_images(rope: &Rope) -> Vec<ImageNode> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let text: String = rope.chunks().collect();
    let parser = Parser::new(&text);
    let parser_with_offsets = parser.into_offset_iter();

    let mut images = Vec::new();
    let mut in_image = false;
    let mut current_alt = String::new();

    for (event, range) in parser_with_offsets {
        match event {
            Event::Start(Tag::Image { link_type: _, ref dest_url, ref title, id: _ }) => {
                // Start of image tag
                in_image = true;
                current_alt.clear();

                // Find line number for this byte offset
                let byte_offset = range.start.min(rope.len_bytes().saturating_sub(1));
                let current_line = rope.byte_to_line(byte_offset);

                // Create image node (will update alt text in Text event)
                let mut img = ImageNode::new(
                    dest_url.to_string(),
                    String::new(),
                    current_line,
                );

                if !title.is_empty() {
                    img.title = Some(title.to_string());
                }

                // Store temporarily (will be updated with alt text)
                images.push(img);
            }
            Event::Text(ref text) if in_image => {
                // Capture alt text
                current_alt.push_str(text);
            }
            Event::End(TagEnd::Image) => {
                // End of image tag - update the last image with alt text
                if let Some(last_img) = images.last_mut() {
                    last_img.alt = current_alt.clone();
                }
                in_image = false;
                current_alt.clear();
            }
            _ => {}
        }
    }

    images
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_empty_file() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"")?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.line_count(), 1); // Empty file has 1 line in Rope
        assert_eq!(doc.headings.len(), 0);
        assert_eq!(doc.rev, 1);

        Ok(())
    }

    #[test]
    fn test_load_simple_file() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"# Heading\n\nSome text\n")?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.line_count(), 4);
        assert_eq!(doc.headings.len(), 1);
        assert_eq!(doc.headings[0].level, 1);
        assert_eq!(doc.headings[0].text, "Heading");

        Ok(())
    }

    #[test]
    fn test_reload_increments_revision() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"Initial content\n")?;
        file.flush()?;

        let (mut doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.rev, 1);

        // Modify file
        file.write_all(b"New content\n")?;
        file.flush()?;

        doc.reload()?;
        assert_eq!(doc.rev, 2);

        Ok(())
    }

    #[test]
    fn test_get_lines_single() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"Line 1\nLine 2\nLine 3\n")?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.get_lines(0, 0), "Line 1");
        assert_eq!(doc.get_lines(1, 1), "Line 2");

        Ok(())
    }

    #[test]
    fn test_get_lines_range() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"Line 1\nLine 2\nLine 3\n")?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.get_lines(0, 2), "Line 1\nLine 2\nLine 3");
        assert_eq!(doc.get_lines(1, 2), "Line 2\nLine 3");

        Ok(())
    }

    #[test]
    fn test_get_lines_out_of_bounds() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"Line 1\nLine 2\n")?;

        let (doc, _warnings) = Document::load(file.path())?;
        // Should clamp to valid range
        let result = doc.get_lines(0, 100);
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));

        Ok(())
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_extract_images_basic() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"# Heading\n\n![alt text](image.png)\n\nSome text\n")?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.images.len(), 1);
        assert_eq!(doc.images[0].src, "image.png");
        assert_eq!(doc.images[0].alt, "alt text");
        assert_eq!(doc.images[0].title, None);
        assert_eq!(doc.images[0].source_line, 2);

        Ok(())
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_extract_images_with_title() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"![alt text](image.png \"Image Title\")\n")?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.images.len(), 1);
        assert_eq!(doc.images[0].src, "image.png");
        assert_eq!(doc.images[0].alt, "alt text");
        assert_eq!(doc.images[0].title, Some("Image Title".to_string()));

        Ok(())
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_extract_images_empty_alt() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"![](image.png)\n")?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.images.len(), 1);
        assert_eq!(doc.images[0].src, "image.png");
        assert_eq!(doc.images[0].alt, "");

        Ok(())
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_extract_images_line_numbers() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(
            b"Line 1\nLine 2\n![first](a.png)\nLine 4\nLine 5\n![second](b.png)\nLine 7\n",
        )?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.images.len(), 2);
        assert_eq!(doc.images[0].source_line, 2);
        assert_eq!(doc.images[1].source_line, 5);

        Ok(())
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_extract_images_multiple() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(
            b"# Document\n\n![first](a.png)\n\nSome text\n\n![second](b.png)\n\n![third](c.png)\n",
        )?;

        let (doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.images.len(), 3);
        assert_eq!(doc.images[0].src, "a.png");
        assert_eq!(doc.images[0].alt, "first");
        assert_eq!(doc.images[1].src, "b.png");
        assert_eq!(doc.images[1].alt, "second");
        assert_eq!(doc.images[2].src, "c.png");
        assert_eq!(doc.images[2].alt, "third");

        Ok(())
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_extract_images_in_code_blocks() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(
            b"![real image](real.png)\n\n```markdown\n![fake image](fake.png)\n```\n\n![another real](real2.png)\n",
        )?;

        let (doc, _warnings) = Document::load(file.path())?;
        // Code block images should be ignored by pulldown_cmark
        assert_eq!(doc.images.len(), 2);
        assert_eq!(doc.images[0].src, "real.png");
        assert_eq!(doc.images[1].src, "real2.png");

        Ok(())
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_extract_images_reload_updates() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"![initial](initial.png)\n")?;
        file.flush()?;

        let (mut doc, _warnings) = Document::load(file.path())?;
        assert_eq!(doc.images.len(), 1);
        assert_eq!(doc.images[0].src, "initial.png");

        // Modify file with different images
        std::fs::write(file.path(), b"![new](new.png)\n![another](another.png)\n")?;

        doc.reload()?;
        assert_eq!(doc.images.len(), 2);
        assert_eq!(doc.images[0].src, "new.png");
        assert_eq!(doc.images[1].src, "another.png");

        Ok(())
    }

    #[test]
    fn test_document_size_limit() {
        use std::io::Write;
        let mut file = NamedTempFile::new().unwrap();

        // Create a file larger than MAX_FILE_SIZE (10MB)
        let large_content = "x".repeat(11 * 1024 * 1024); // 11MB
        file.write_all(large_content.as_bytes()).unwrap();
        file.flush().unwrap();

        // Attempt to load should fail
        let result = Document::load(file.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("exceeds maximum size"));
    }
}
