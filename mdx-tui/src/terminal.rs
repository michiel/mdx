//! Terminal initialization and management

use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

/// Initialize the terminal
pub fn init() -> Result<Tui> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend).context("Failed to create terminal")?;

    Ok(terminal)
}

/// Restore the terminal to its original state
pub fn restore() -> Result<()> {
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
        .context("Failed to leave alternate screen")?;
    Ok(())
}
