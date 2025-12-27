//! MDX TUI - Terminal user interface components
//!
//! This crate contains all ratatui/crossterm integration:
//! - App state management
//! - Event loop and input handling
//! - Rendering (markdown, TOC, status bar)
//! - Pane management and splits
//! - Theme system

pub mod app;
pub mod event;
pub mod input;
pub mod render;
pub mod theme;
pub mod ui;

// These will be added in later stages
// pub mod panes;
// pub mod toc;
// pub mod editor;
// #[cfg(feature = "watch")]
// pub mod watcher;
// #[cfg(feature = "git")]
// pub mod diff_worker;

// Re-export main types
pub use app::App;
pub use event::AppEvent;
