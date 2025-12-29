//! MDX Core - Document model, parsing, and configuration
//!
//! This crate contains the core logic for mdx, independent of terminal UI concerns:
//! - Document model with Rope-based text storage
//! - Markdown parsing and TOC extraction
//! - Selection model
//! - Configuration management
//! - Git diff computation (optional feature)

pub mod config;
pub mod doc;
pub mod selection;
pub mod toc;

#[cfg(feature = "git")]
pub mod diff;
#[cfg(feature = "git")]
pub mod git;

#[cfg(feature = "images")]
pub mod image;

// Re-export commonly used types
pub use config::Config;
pub use doc::Document;
pub use selection::LineSelection;
