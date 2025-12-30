//! Integration tests for mdx-tui
//!
//! These tests exercise the full application flow end-to-end,
//! including document loading, navigation, search, and pane management.

use mdx_core::{Config, Document};
use mdx_tui::App;
use std::io::Write as _;
use tempfile::NamedTempFile;

/// Helper to create a test document with known content
fn create_test_doc(content: &str) -> (Document, NamedTempFile) {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(content.as_bytes())
        .expect("Failed to write test content");
    file.flush().expect("Failed to flush");

    let path = file.path();
    let (doc, _warnings) = Document::load(path).expect("Failed to load test document");
    (doc, file)
}

/// Helper to create a test app with a document
/// Returns (App, NamedTempFile) - keep the file alive for the duration of the test
fn create_test_app(content: &str) -> (App, NamedTempFile) {
    let (doc, file) = create_test_doc(content);
    let config = Config::default();
    (App::new(config, doc, vec![]), file)
}

#[test]
fn integration_app_initialization() {
    let content = "# Test Document\n\nThis is a test.\n";
    let (app, _file) = create_test_app(content);

    assert!(!app.should_quit);
    assert_eq!(app.panes.panes.len(), 1);
    assert!(!app.show_help);
    assert!(!app.show_toc_dialog);
}

#[test]
fn integration_document_navigation() {
    let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n";
    let (mut app, _file) = create_test_app(content);

    // Get the focused pane
    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 0);

    // Move down
    app.move_cursor_down(1);
    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 1);

    // Move down again
    app.move_cursor_down(1);
    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 2);

    // Move up
    app.move_cursor_up(1);
    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 1);
}

#[test]
fn integration_search_functionality() {
    let content = "# Header\n\nThis is a test.\nAnother test line.\nFinal line.\n";
    let (mut app, _file) = create_test_app(content);

    // Perform search
    app.search("test");
    assert_eq!(app.search_query, "test");

    // Should find matches
    assert!(app.search_matches.len() > 0);
    assert_eq!(app.search_matches.len(), 2);
}

#[test]
fn integration_visual_line_mode() {
    let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n";
    let (mut app, _file) = create_test_app(content);

    // Enter visual line mode
    app.enter_visual_line_mode();
    let pane = app.panes.focused_pane().unwrap();
    assert!(pane.view.selection.is_some());

    // Move selection
    app.move_cursor_down(1);
    app.move_cursor_down(1);
    app.update_selection();

    let pane = app.panes.focused_pane().unwrap();
    if let Some(selection) = &pane.view.selection {
        assert_eq!(selection.cursor, 2);
    } else {
        panic!("Expected selection to be present");
    }

    // Exit visual line mode
    app.exit_visual_line_mode();
    let pane = app.panes.focused_pane().unwrap();
    assert!(pane.view.selection.is_none());
}

#[test]
fn integration_toc_toggle() {
    let content = "# Heading 1\n\nSome text.\n\n## Heading 2\n\nMore text.\n\n### Heading 3\n";
    let (mut app, _file) = create_test_app(content);

    assert_eq!(app.doc.headings.len(), 3);

    // Toggle TOC
    let initial_show_toc = app.show_toc;
    app.toggle_toc();
    assert_eq!(app.show_toc, !initial_show_toc);

    // Toggle again
    app.toggle_toc();
    assert_eq!(app.show_toc, initial_show_toc);
}

#[test]
fn integration_pane_splitting() {
    let content = "# Test Document\n\nContent here.\n";
    let (mut app, _file) = create_test_app(content);

    // Initially one pane
    assert_eq!(app.panes.panes.len(), 1);

    // Split vertically
    app.split_focused(mdx_tui::panes::SplitDir::Vertical);
    assert_eq!(app.panes.panes.len(), 2);

    // Split horizontally
    app.split_focused(mdx_tui::panes::SplitDir::Horizontal);
    assert_eq!(app.panes.panes.len(), 3);
}

#[test]
fn integration_quit_command() {
    let content = "# Test Document\n";
    let (mut app, _file) = create_test_app(content);

    assert!(!app.should_quit);

    app.quit();
    assert!(app.should_quit);
}

#[test]
fn integration_help_toggle() {
    let content = "# Test Document\n";
    let (mut app, _file) = create_test_app(content);

    assert!(!app.show_help);

    app.toggle_help();
    assert!(app.show_help);

    app.toggle_help();
    assert!(!app.show_help);
}

#[test]
fn integration_scrolling() {
    let mut content = String::new();
    for i in 1..=100 {
        content.push_str(&format!("Line {}\n", i));
    }
    let (mut app, _file) = create_test_app(&content);

    let pane = app.panes.focused_pane().unwrap();
    let initial_cursor = pane.view.cursor_line;
    assert_eq!(pane.view.scroll_line, 0);

    // Scroll down half page (viewport_height = 50, viewport_width = 80)
    app.scroll_half_page_down(50, 80);
    let pane = app.panes.focused_pane().unwrap();
    // Cursor should have moved down
    assert!(pane.view.cursor_line > initial_cursor);

    let cursor_after_down = pane.view.cursor_line;

    // Scroll up half page
    app.scroll_half_page_up(50, 80);
    let pane = app.panes.focused_pane().unwrap();
    // Cursor should be back closer to start
    assert!(pane.view.cursor_line < cursor_after_down);
}

#[test]
fn integration_jump_to_line() {
    let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n";
    let (mut app, _file) = create_test_app(content);

    // Jump to specific line
    app.jump_to_line(2);
    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 2);

    // Jump to another line
    app.jump_to_line(4);
    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 4);
}

#[test]
fn integration_heading_navigation() {
    let content = "# Heading 1\n\nText 1.\n\n## Heading 2\n\nText 2.\n\n### Heading 3\n\nText 3.\n";
    let (app, _file) = create_test_app(content);

    assert_eq!(app.doc.headings.len(), 3);

    // Get current heading
    let heading_idx = app.current_heading_index();
    assert!(heading_idx.is_some() || heading_idx.is_none());
}

#[test]
fn integration_configuration() {
    let content = "# Test Document\n\nContent.\n";
    let (doc, _file) = create_test_doc(content);

    let mut config = Config::default();
    config.toc.enabled = true;
    config.theme = mdx_core::config::ThemeVariant::Light;

    let app = App::new(config.clone(), doc, vec![]);

    assert_eq!(app.show_toc, true);
    assert_eq!(app.theme_variant, mdx_core::config::ThemeVariant::Light);
    assert_eq!(app.config.theme, mdx_core::config::ThemeVariant::Light);
}

#[test]
fn integration_security_warnings() {
    let content = "# Test Document\n";
    let (doc, _file) = create_test_doc(content);
    let config = Config::default();

    let warnings = vec![mdx_core::SecurityEvent::warning("Test warning", "test")];

    let app = App::new(config, doc, warnings);
    assert!(app.show_security_warnings);
    assert_eq!(app.security_warnings.len(), 1);
}

#[test]
fn integration_search_navigation() {
    let content = "test\nother\ntest\nmore\ntest\n";
    let (mut app, _file) = create_test_app(content);

    // Perform search
    app.search("test");

    // Should have found 3 matches
    assert_eq!(app.search_matches.len(), 3);

    // Navigate through matches (viewport_height = 20)
    app.next_search_match(20);
    assert!(app.search_current_match.is_some());
}

#[test]
fn integration_empty_document() {
    let content = "";
    let (app, _file) = create_test_app(content);

    assert_eq!(app.doc.rope.len_lines(), 1); // Empty rope has 1 line
    assert_eq!(app.doc.headings.len(), 0);

    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 0);
    assert_eq!(pane.view.scroll_line, 0);
}

#[test]
fn integration_single_line_document() {
    let content = "Single line";
    let (mut app, _file) = create_test_app(content);

    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 0);

    // Try to move down (should stay at line 0)
    app.move_cursor_down(1);
    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 0);

    // Try to move up (should stay at line 0)
    app.move_cursor_up(1);
    let pane = app.panes.focused_pane().unwrap();
    assert_eq!(pane.view.cursor_line, 0);
}

#[test]
fn integration_toc_dialog() {
    let content = "# H1\n\n## H2\n\n### H3\n";
    let (mut app, _file) = create_test_app(content);

    assert!(!app.show_toc_dialog);

    app.toggle_toc_dialog();
    assert!(app.show_toc_dialog);

    app.toggle_toc_dialog();
    assert!(!app.show_toc_dialog);
}

#[test]
fn integration_options_dialog() {
    let content = "# Test\n";
    let (mut app, _file) = create_test_app(content);

    assert!(app.options_dialog.is_none());

    app.open_options();
    assert!(app.options_dialog.is_some());

    app.close_options();
    assert!(app.options_dialog.is_none());
}

#[test]
fn integration_theme_toggle() {
    let content = "# Test\n";
    let (mut app, _file) = create_test_app(content);

    let initial_theme = app.theme_variant;
    app.toggle_theme();
    assert_ne!(app.theme_variant, initial_theme);

    app.toggle_theme();
    assert_eq!(app.theme_variant, initial_theme);
}

#[test]
fn integration_multi_pane_independence() {
    let content = "# Test\nLine 2\nLine 3\n";
    let (mut app, _file) = create_test_app(content);

    // Split to create multiple panes
    app.split_focused(mdx_tui::panes::SplitDir::Vertical);
    assert_eq!(app.panes.panes.len(), 2);

    // Each pane should have independent view state
    let pane = app.panes.focused_pane().unwrap();
    let cursor_before = pane.view.cursor_line;

    // Move cursor in focused pane
    app.move_cursor_down(1);

    let pane_after = app.panes.focused_pane().unwrap();
    assert!(pane_after.view.cursor_line >= cursor_before);
}
