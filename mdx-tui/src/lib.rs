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
pub mod options_dialog;
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
#[cfg(feature = "images")]
pub mod image_cache;

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
        let term_size = terminal.size()?;
        // -1 for status bar, -2 for pane borders (top and bottom)
        let viewport_height = term_size.height.saturating_sub(3) as usize;
        // -2 for pane borders (left and right)
        let viewport_width = term_size.width.saturating_sub(2) as usize;

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
            let event = crossterm::event::read().context("Failed to read event")?;
            match event {
                Event::Key(key) => {
                    // Only handle key press events, ignore release
                    if key.kind == KeyEventKind::Press {
                        let action = input::handle_input(app, key, viewport_height, viewport_width)?;

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
                                    app.set_error_message(format!("Editor error: {}", e));
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
                Event::Mouse(mouse_event) => {
                    input::handle_mouse(app, mouse_event, viewport_height, viewport_width)?;
                }
                _ => {
                    // Ignore other events (resize, focus, etc.)
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
            }
        }
    }

    Ok(())
}
