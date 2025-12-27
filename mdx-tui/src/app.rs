//! Application state

use mdx_core::{config::ThemeVariant, Config, Document, LineSelection};
use crate::panes::PaneManager;
use crate::theme::Theme;

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    VisualLine,
}

/// Key prefix state for multi-key sequences
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPrefix {
    None,
    CtrlW,
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
    pub panes: PaneManager,
    pub theme: Theme,
    pub theme_variant: ThemeVariant,
    pub show_toc: bool,
    pub toc_focus: bool,
    pub toc_selected: usize,
    pub key_prefix: KeyPrefix,
    pub should_quit: bool,
}

impl App {
    /// Create a new application instance with a document
    pub fn new(config: Config, doc: Document) -> Self {
        let show_toc = config.toc.enabled;
        let theme_variant = config.theme;
        let theme = Theme::for_variant(theme_variant);
        let panes = PaneManager::new(0); // Single pane for single document
        Self {
            config,
            doc,
            panes,
            theme,
            theme_variant,
            show_toc,
            toc_focus: false,
            toc_selected: 0,
            key_prefix: KeyPrefix::None,
            should_quit: false,
        }
    }

    /// Handle quit request
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Move cursor down by n lines
    pub fn move_cursor_down(&mut self, n: usize) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            let max_line = self.doc.line_count().saturating_sub(1);
            pane.view.cursor_line = (pane.view.cursor_line + n).min(max_line);
        }
    }

    /// Move cursor up by n lines
    pub fn move_cursor_up(&mut self, n: usize) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            pane.view.cursor_line = pane.view.cursor_line.saturating_sub(n);
        }
    }

    /// Jump to specific line
    pub fn jump_to_line(&mut self, line: usize) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            let max_line = self.doc.line_count().saturating_sub(1);
            pane.view.cursor_line = line.min(max_line);
        }
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
        if let Some(pane) = self.panes.focused_pane_mut() {
            let cursor = pane.view.cursor_line;
            let scroll = pane.view.scroll_line;

            // Cursor above viewport - scroll up
            if cursor < scroll {
                pane.view.scroll_line = cursor;
            }
            // Cursor below viewport - scroll down
            else if cursor >= scroll + viewport_height {
                pane.view.scroll_line = cursor.saturating_sub(viewport_height - 1);
            }
        }
    }

    /// Toggle between dark and light themes
    pub fn toggle_theme(&mut self) {
        self.theme_variant = match self.theme_variant {
            ThemeVariant::Dark => ThemeVariant::Light,
            ThemeVariant::Light => ThemeVariant::Dark,
        };
        self.theme = Theme::for_variant(self.theme_variant);
    }

    /// Toggle TOC visibility and focus
    pub fn toggle_toc(&mut self) {
        if self.show_toc {
            // If already shown, hide it
            self.show_toc = false;
            self.toc_focus = false;
        } else {
            // Show and focus TOC
            self.show_toc = true;
            self.toc_focus = true;
        }
    }

    /// Move TOC selection down
    pub fn toc_move_down(&mut self) {
        if !self.doc.headings.is_empty() {
            self.toc_selected = (self.toc_selected + 1).min(self.doc.headings.len() - 1);
        }
    }

    /// Move TOC selection up
    pub fn toc_move_up(&mut self) {
        self.toc_selected = self.toc_selected.saturating_sub(1);
    }

    /// Jump to the selected heading in TOC
    pub fn toc_jump_to_selected(&mut self, viewport_height: usize) {
        if let Some(heading) = self.doc.headings.get(self.toc_selected) {
            self.jump_to_line(heading.line);
            self.auto_scroll(viewport_height);
        }
    }

    /// Get the index of the current heading based on cursor position
    pub fn current_heading_index(&self) -> Option<usize> {
        if self.doc.headings.is_empty() {
            return None;
        }

        let cursor_line = self.panes.focused_pane()?.view.cursor_line;

        // Find the last heading that's at or before the cursor
        for (i, heading) in self.doc.headings.iter().enumerate().rev() {
            if heading.line <= cursor_line {
                return Some(i);
            }
        }

        None
    }

    /// Split the focused pane
    pub fn split_focused(&mut self, dir: crate::panes::SplitDir) {
        self.panes.split_focused(dir, 0); // doc_id is 0 for single document
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

        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
        app.move_cursor_down(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 1);
        app.move_cursor_down(3);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 4);
    }

    #[test]
    fn test_move_cursor_down_bounded() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        // Try to move beyond last line
        app.move_cursor_down(100);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 9); // 0-indexed, so line 9 is the last
    }

    #[test]
    fn test_move_cursor_up() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        app.panes.focused_pane_mut().unwrap().view.cursor_line = 5;
        app.move_cursor_up(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 4);
        app.move_cursor_up(3);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 1);
    }

    #[test]
    fn test_move_cursor_up_bounded() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        app.panes.focused_pane_mut().unwrap().view.cursor_line = 2;
        // Try to move before first line
        app.move_cursor_up(100);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
    }

    #[test]
    fn test_jump_to_line() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        app.jump_to_line(5);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 5);

        app.jump_to_line(0);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);

        app.jump_to_line(9);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 9);

        // Beyond bounds
        app.jump_to_line(100);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 9);
    }

    #[test]
    fn test_scroll_half_page() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc);

        let viewport_height = 20;

        // Half page down (10 lines)
        app.scroll_half_page_down(viewport_height);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 10);

        // Half page up
        app.scroll_half_page_up(viewport_height);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
    }

    #[test]
    fn test_auto_scroll_down() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc);
        let viewport_height = 10;

        // Move cursor to line 15 (beyond viewport of 10 lines)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 15;
        app.auto_scroll(viewport_height);

        // Scroll should adjust so cursor is at bottom of viewport
        assert_eq!(app.panes.focused_pane_mut().unwrap().view.scroll_line, 6); // 15 - 9 = 6
    }

    #[test]
    fn test_auto_scroll_up() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc);
        let viewport_height = 10;

        // Start scrolled down
        app.panes.focused_pane_mut().unwrap().view.scroll_line = 20;
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 15; // Above current scroll

        app.auto_scroll(viewport_height);

        // Scroll should move up to show cursor
        assert_eq!(app.panes.focused_pane_mut().unwrap().view.scroll_line, 15);
    }

    #[test]
    fn test_navigation_with_empty_doc() {
        let config = Config::default();
        let doc = create_test_doc(0);
        let mut app = App::new(config, doc);

        // Should handle empty doc gracefully
        app.move_cursor_down(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);

        app.move_cursor_up(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
    }

    #[test]
    fn test_navigation_with_single_line() {
        let config = Config::default();
        let doc = create_test_doc(1);
        let mut app = App::new(config, doc);

        app.move_cursor_down(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0); // Can't move beyond line 0

        app.move_cursor_up(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
    }

    #[test]
    fn test_toggle_toc() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc);

        // Initially shown (from config default)
        assert!(app.show_toc);
        assert!(!app.toc_focus);

        // Toggle - should hide
        app.toggle_toc();
        assert!(!app.show_toc);
        assert!(!app.toc_focus);

        // Toggle again - should show and focus
        app.toggle_toc();
        assert!(app.show_toc);
        assert!(app.toc_focus);
    }

    #[test]
    fn test_toc_navigation() {
        let config = Config::default();
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "# Heading 1\nSome text\n## Heading 2\nMore text\n### Heading 3"
        )
        .unwrap();
        file.flush().unwrap();
        let doc = Document::load(file.path()).unwrap();
        let mut app = App::new(config, doc);

        assert_eq!(app.toc_selected, 0);

        // Move down in TOC
        app.toc_move_down();
        assert_eq!(app.toc_selected, 1);

        app.toc_move_down();
        assert_eq!(app.toc_selected, 2);

        // Try to move beyond last heading
        app.toc_move_down();
        assert_eq!(app.toc_selected, 2); // Should stay at 2

        // Move up
        app.toc_move_up();
        assert_eq!(app.toc_selected, 1);

        app.toc_move_up();
        assert_eq!(app.toc_selected, 0);

        // Try to move above first heading
        app.toc_move_up();
        assert_eq!(app.toc_selected, 0); // Should stay at 0
    }

    #[test]
    fn test_toc_jump_to_heading() {
        let config = Config::default();
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "# Heading 1\nSome text\n## Heading 2\nMore text\n### Heading 3"
        )
        .unwrap();
        file.flush().unwrap();
        let doc = Document::load(file.path()).unwrap();
        let mut app = App::new(config, doc);

        // Jump to second heading
        app.toc_selected = 1;
        app.toc_jump_to_selected(10);

        // Heading 2 should be at line 2 (0-indexed)
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 2);
    }

    #[test]
    fn test_current_heading_index() {
        let config = Config::default();
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "# Heading 1\ntext\ntext\n## Heading 2\ntext\n### Heading 3\ntext"
        )
        .unwrap();
        file.flush().unwrap();
        let doc = Document::load(file.path()).unwrap();
        let mut app = App::new(config, doc);

        // At line 0 - should be heading 0
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 0;
        assert_eq!(app.current_heading_index(), Some(0));

        // At line 2 (still under heading 1)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 2;
        assert_eq!(app.current_heading_index(), Some(0));

        // At line 3 (heading 2)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 3;
        assert_eq!(app.current_heading_index(), Some(1));

        // At line 5 (heading 3)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 5;
        assert_eq!(app.current_heading_index(), Some(2));
    }
}
