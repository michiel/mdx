//! Document model with Rope-based text storage

use anyhow::Result;
use ropey::Rope;
use std::path::PathBuf;
use std::time::SystemTime;

/// A heading in the markdown document
#[derive(Clone, Debug)]
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
}

impl Document {
    /// Load a document from a file path
    pub fn load(_path: &std::path::Path) -> Result<Self> {
        // TODO: Implementation in Stage 1
        unimplemented!("Document::load will be implemented in Stage 1")
    }

    /// Reload the document from disk
    pub fn reload(&mut self) -> Result<()> {
        // TODO: Implementation in Stage 1
        unimplemented!("Document::reload will be implemented in Stage 1")
    }

    /// Get the number of lines in the document
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Extract lines for yank operations
    pub fn get_lines(&self, _start: usize, _end_inclusive: usize) -> String {
        // TODO: Implementation in Stage 1
        unimplemented!("Document::get_lines will be implemented in Stage 1")
    }
}
