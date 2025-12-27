//! Input handling and keybindings

use crate::app::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Result of handling input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Continue,
    Quit,
}

/// Handle a key event with viewport height for scroll commands
pub fn handle_input(app: &mut App, key: KeyEvent, viewport_height: usize) -> Result<Action> {
    // Handle quit with 'q'
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        }
    ) {
        app.quit();
        return Ok(Action::Quit);
    }

    // Handle Ctrl+C
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    ) {
        app.quit();
        return Ok(Action::Quit);
    }

    // Navigation commands
    match key {
        // j - move down
        KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            app.move_cursor_down(1);
            app.auto_scroll(viewport_height);
        }

        // k - move up
        KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            app.move_cursor_up(1);
            app.auto_scroll(viewport_height);
        }

        // Ctrl+d - half page down
        KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            app.scroll_half_page_down(viewport_height);
            app.auto_scroll(viewport_height);
        }

        // Ctrl+u - half page up
        KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            app.scroll_half_page_up(viewport_height);
            app.auto_scroll(viewport_height);
        }

        // g - prefix for gg (go to top)
        KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            // For now, implement gg as single 'g' (proper prefix state in later enhancement)
            app.jump_to_line(0);
            app.auto_scroll(viewport_height);
        }

        // G - go to bottom
        KeyEvent {
            code: KeyCode::Char('G'),
            modifiers: KeyModifiers::SHIFT,
            ..
        } => {
            let last_line = app.doc.line_count().saturating_sub(1);
            app.jump_to_line(last_line);
            app.auto_scroll(viewport_height);
        }

        _ => {}
    }

    Ok(Action::Continue)
}
