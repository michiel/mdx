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
pub mod panes;
pub mod render;
pub mod terminal;
pub mod theme;
pub mod ui;

// These will be added in later stages
// pub mod toc;
// pub mod editor;
// #[cfg(feature = "watch")]
// pub mod watcher;
// #[cfg(feature = "git")]
// pub mod diff_worker;

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyEventKind};
use std::time::Duration;

// Re-export main types
pub use app::App;
pub use event::AppEvent;

/// Run the TUI application
pub fn run(mut app: App) -> Result<()> {
    let mut terminal = terminal::init().context("Failed to initialize terminal")?;

    // Main event loop
    let result = run_loop(&mut terminal, &mut app);

    // Always restore terminal, even if run_loop fails
    terminal::restore().context("Failed to restore terminal")?;

    result
}

fn run_loop(terminal: &mut terminal::Tui, app: &mut App) -> Result<()> {
    loop {
        // Get terminal size for viewport calculations
        let viewport_height = terminal.size()?.height.saturating_sub(1) as usize; // -1 for status bar

        // Draw UI
        terminal
            .draw(|frame| ui::draw(frame, app))
            .context("Failed to draw frame")?;

        // Check if we should quit
        if app.should_quit {
            break;
        }

        // Poll for events with timeout
        if crossterm::event::poll(Duration::from_millis(100)).context("Failed to poll events")? {
            if let Event::Key(key) = crossterm::event::read().context("Failed to read event")? {
                // Only handle key press events, ignore release
                if key.kind == KeyEventKind::Press {
                    input::handle_input(app, key, viewport_height)?;
                }
            }
        }
    }

    Ok(())
}
