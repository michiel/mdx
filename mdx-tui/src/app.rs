//! Application state

use mdx_core::Config;

/// Main application state
pub struct App {
    pub config: Config,
    // TODO: Add more fields in Stage 3
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}
