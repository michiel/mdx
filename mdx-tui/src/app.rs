//! Application state

use mdx_core::{Config, Document};

/// Main application state
pub struct App {
    pub config: Config,
    pub doc: Document,
    pub should_quit: bool,
}

impl App {
    /// Create a new application instance with a document
    pub fn new(config: Config, doc: Document) -> Self {
        Self {
            config,
            doc,
            should_quit: false,
        }
    }

    /// Handle quit request
    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}
