//! Event types and event loop

use crossterm::event::KeyEvent;
use std::path::PathBuf;

/// Application events
#[derive(Debug)]
pub enum AppEvent {
    /// User input
    Input(KeyEvent),
    /// Periodic tick for animations/debounce
    Tick,
    /// File changed on disk
    #[cfg(feature = "watch")]
    FileChanged(PathBuf),
    /// Diff computation completed
    #[cfg(feature = "git")]
    DiffReady(u64, mdx_core::diff::DiffGutter),
}
