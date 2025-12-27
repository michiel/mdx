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

/// Handle a key event
pub fn handle_input(app: &mut App, key: KeyEvent) -> Result<Action> {
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

    Ok(Action::Continue)
}
