//! Input handling and keybindings

use crate::app::App;
use crossterm::event::KeyEvent;

/// Result of handling input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Continue,
    Quit,
}

/// Handle a key event
pub fn handle_input(_app: &mut App, _key: KeyEvent) -> anyhow::Result<Action> {
    // TODO: Implementation in Stage 4
    Ok(Action::Continue)
}
