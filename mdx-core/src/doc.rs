//! Document model with Rope-based text storage

use anyhow::{Context, Result};
use ropey::Rope;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::toc;

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
#[derive(Clone)]
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
    pub fn load(path: &Path) -> Result<Self> {
        // Canonicalize the path to get absolute path (needed for git integration)
        let abs_path = path.canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?;

        let content = fs::read_to_string(&abs_path)
            .with_context(|| format!("Failed to read file: {}", abs_path.display()))?;

        let rope = Rope::from_str(&content);
        let headings = toc::extract_headings(&rope);

        let metadata = fs::metadata(&abs_path).ok();
        let mtime = metadata.and_then(|m| m.modified().ok());

        // Initialize with empty diff gutter - will be computed asynchronously by worker thread
        #[cfg(feature = "git")]
        let diff_gutter = {
            let line_count = rope.len_lines();
            DiffGutter::empty(line_count)
        };

        // Initialize with empty images vector - will be extracted in Stage 2
        #[cfg(feature = "images")]
        let images = Vec::new();

        Ok(Self {
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
        })
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

        // Reset images vector - will be extracted in Stage 2
        #[cfg(feature = "images")]
        {
            self.images.clear();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_empty_file() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"")?;

        let doc = Document::load(file.path())?;
        assert_eq!(doc.line_count(), 1); // Empty file has 1 line in Rope
        assert_eq!(doc.headings.len(), 0);
        assert_eq!(doc.rev, 1);

        Ok(())
    }

    #[test]
    fn test_load_simple_file() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"# Heading\n\nSome text\n")?;

        let doc = Document::load(file.path())?;
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

        let mut doc = Document::load(file.path())?;
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

        let doc = Document::load(file.path())?;
        assert_eq!(doc.get_lines(0, 0), "Line 1");
        assert_eq!(doc.get_lines(1, 1), "Line 2");

        Ok(())
    }

    #[test]
    fn test_get_lines_range() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"Line 1\nLine 2\nLine 3\n")?;

        let doc = Document::load(file.path())?;
        assert_eq!(doc.get_lines(0, 2), "Line 1\nLine 2\nLine 3");
        assert_eq!(doc.get_lines(1, 2), "Line 2\nLine 3");

        Ok(())
    }

    #[test]
    fn test_get_lines_out_of_bounds() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"Line 1\nLine 2\n")?;

        let doc = Document::load(file.path())?;
        // Should clamp to valid range
        let result = doc.get_lines(0, 100);
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));

        Ok(())
    }
}
