//! MDX TUI - Terminal user interface components
//!
//! This crate contains all ratatui/crossterm integration:
//! - App state management
//! - Event loop and input handling
//! - Rendering (markdown, TOC, status bar)
//! - Pane management and splits
//! - Theme system

pub mod app;
pub mod editor;
pub mod event;
pub mod input;
pub mod panes;
pub mod render;
pub mod terminal;
pub mod theme;
pub mod ui;

// These will be added in later stages
// pub mod toc;
#[cfg(feature = "watch")]
pub mod watcher;
#[cfg(feature = "git")]
pub mod diff_worker;

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
        // -1 for status bar, -2 for pane borders (top and bottom)
        let viewport_height = terminal.size()?.height.saturating_sub(3) as usize;

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
                    let action = input::handle_input(app, key, viewport_height)?;

                    // Handle special actions
                    match action {
                        input::Action::OpenEditor => {
                            // Suspend terminal
                            terminal::restore().context("Failed to restore terminal for editor")?;

                            // Launch editor
                            let editor_result = app.open_in_editor();

                            // Restore terminal
                            *terminal = terminal::init().context("Failed to reinitialize terminal after editor")?;

                            // Handle editor errors (after terminal is restored)
                            if let Err(e) = editor_result {
                                // TODO: Show error in status bar
                                // For now, just continue silently
                                eprintln!("Editor error: {}", e);
                            }
                        }
                        input::Action::Quit => {
                            // Quit already handled by should_quit flag
                        }
                        input::Action::Continue => {
                            // Nothing to do
                        }
                    }
                }
            }
        }

        // Check for file changes (with debouncing)
        #[cfg(feature = "watch")]
        {
            if let Some(ref mut watcher) = app.watcher {
                if watcher.check_changed(250) {
                    // File changed on disk after debounce period
                    if app.config.watch.auto_reload {
                        // Auto reload
                        if let Err(e) = app.reload_document() {
                            eprintln!("Failed to reload document: {}", e);
                        }
                    } else {
                        // Just mark as dirty
                        app.doc.dirty_on_disk = true;
                    }
                }
            }
        }

        // Check for diff results from worker
        #[cfg(feature = "git")]
        {
            if let Some(result) = app.diff_worker.try_recv_result() {
                // Check if result matches current document revision
                if result.doc_id == 0 && result.rev == app.doc.rev {
                    // Apply the diff gutter
                    app.doc.diff_gutter = result.gutter;
                }
                // Otherwise discard stale result
            }
        }
    }

    Ok(())
}
