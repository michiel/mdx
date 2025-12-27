//! Application state

use mdx_core::{Config, Document, LineSelection};

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    VisualLine,
}

/// View state for a document viewport
#[derive(Debug, Clone)]
pub struct ViewState {
    pub scroll_line: usize,
    pub cursor_line: usize,
    pub mode: Mode,
    pub selection: Option<LineSelection>,
}

impl ViewState {
    /// Create a new view state at the top of the document
    pub fn new() -> Self {
        Self {
            scroll_line: 0,
            cursor_line: 0,
            mode: Mode::Normal,
            selection: None,
        }
    }
}

/// Main application state
pub struct App {
    pub config: Config,
    pub doc: Document,
    pub view: ViewState,
    pub should_quit: bool,
}

impl App {
    /// Create a new application instance with a document
    pub fn new(config: Config, doc: Document) -> Self {
        Self {
            config,
            doc,
            view: ViewState::new(),
            should_quit: false,
        }
    }

    /// Handle quit request
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Move cursor down by n lines
    pub fn move_cursor_down(&mut self, n: usize) {
        let max_line = self.doc.line_count().saturating_sub(1);
        self.view.cursor_line = (self.view.cursor_line + n).min(max_line);
    }

    /// Move cursor up by n lines
    pub fn move_cursor_up(&mut self, n: usize) {
        self.view.cursor_line = self.view.cursor_line.saturating_sub(n);
    }

    /// Jump to specific line
    pub fn jump_to_line(&mut self, line: usize) {
        let max_line = self.doc.line_count().saturating_sub(1);
        self.view.cursor_line = line.min(max_line);
    }

    /// Scroll down by half viewport height
    pub fn scroll_half_page_down(&mut self, viewport_height: usize) {
        let half_page = viewport_height / 2;
        self.move_cursor_down(half_page);
    }

    /// Scroll up by half viewport height
    pub fn scroll_half_page_up(&mut self, viewport_height: usize) {
        let half_page = viewport_height / 2;
        self.move_cursor_up(half_page);
    }

    /// Auto-scroll viewport to keep cursor visible
    pub fn auto_scroll(&mut self, viewport_height: usize) {
        let cursor = self.view.cursor_line;
        let scroll = self.view.scroll_line;

        // Cursor above viewport - scroll up
        if cursor < scroll {
            self.view.scroll_line = cursor;
        }
        // Cursor below viewport - scroll down
        else if cursor >= scroll + viewport_height {
            self.view.scroll_line = cursor.saturating_sub(viewport_height - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdx_core::Document;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_doc(lines: usize) -> Document {
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..lines {
            if i > 0 {
                writeln!(file).unwrap();
            }
            write!(file, "Line {}", i + 1).unwrap();
        }
        file.flush().unwrap();
        Document::load(file.path()).unwrap()
    }

    #[test]
    fn test_move_cursor_down() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        assert_eq!(app.view.cursor_line, 0);
        app.move_cursor_down(1);
        assert_eq!(app.view.cursor_line, 1);
        app.move_cursor_down(3);
        assert_eq!(app.view.cursor_line, 4);
    }

    #[test]
    fn test_move_cursor_down_bounded() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        // Try to move beyond last line
        app.move_cursor_down(100);
        assert_eq!(app.view.cursor_line, 9); // 0-indexed, so line 9 is the last
    }

    #[test]
    fn test_move_cursor_up() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        app.view.cursor_line = 5;
        app.move_cursor_up(1);
        assert_eq!(app.view.cursor_line, 4);
        app.move_cursor_up(3);
        assert_eq!(app.view.cursor_line, 1);
    }

    #[test]
    fn test_move_cursor_up_bounded() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        app.view.cursor_line = 2;
        // Try to move before first line
        app.move_cursor_up(100);
        assert_eq!(app.view.cursor_line, 0);
    }

    #[test]
    fn test_jump_to_line() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        app.jump_to_line(5);
        assert_eq!(app.view.cursor_line, 5);

        app.jump_to_line(0);
        assert_eq!(app.view.cursor_line, 0);

        app.jump_to_line(9);
        assert_eq!(app.view.cursor_line, 9);

        // Beyond bounds
        app.jump_to_line(100);
        assert_eq!(app.view.cursor_line, 9);
    }

    #[test]
    fn test_scroll_half_page() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc);

        let viewport_height = 20;

        // Half page down (10 lines)
        app.scroll_half_page_down(viewport_height);
        assert_eq!(app.view.cursor_line, 10);

        // Half page up
        app.scroll_half_page_up(viewport_height);
        assert_eq!(app.view.cursor_line, 0);
    }

    #[test]
    fn test_auto_scroll_down() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc);
        let viewport_height = 10;

        // Move cursor to line 15 (beyond viewport of 10 lines)
        app.view.cursor_line = 15;
        app.auto_scroll(viewport_height);

        // Scroll should adjust so cursor is at bottom of viewport
        assert_eq!(app.view.scroll_line, 6); // 15 - 9 = 6
    }

    #[test]
    fn test_auto_scroll_up() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc);
        let viewport_height = 10;

        // Start scrolled down
        app.view.scroll_line = 20;
        app.view.cursor_line = 15; // Above current scroll

        app.auto_scroll(viewport_height);

        // Scroll should move up to show cursor
        assert_eq!(app.view.scroll_line, 15);
    }

    #[test]
    fn test_navigation_with_empty_doc() {
        let config = Config::default();
        let doc = create_test_doc(0);
        let mut app = App::new(config, doc);

        // Should handle empty doc gracefully
        app.move_cursor_down(1);
        assert_eq!(app.view.cursor_line, 0);

        app.move_cursor_up(1);
        assert_eq!(app.view.cursor_line, 0);
    }

    #[test]
    fn test_navigation_with_single_line() {
        let config = Config::default();
        let doc = create_test_doc(1);
        let mut app = App::new(config, doc);

        app.move_cursor_down(1);
        assert_eq!(app.view.cursor_line, 0); // Can't move beyond line 0

        app.move_cursor_up(1);
        assert_eq!(app.view.cursor_line, 0);
    }
}
