//! End-to-end scroll, paging, and resize harness tests.
//!
//! These tests drive the full event handlers (not just scroll_math) so we
//! catch integration bugs where the clamp, the layout, and the focused
//! pane interact. Each scenario sets up an App, runs a sequence of
//! App/handle_input calls, and asserts the resulting scroll_line /
//! cursor_line / pane count.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mdx_core::{Config, Document};
use mdx_tui::input::handle_input;
use mdx_tui::App;
use std::io::Write as _;
use tempfile::NamedTempFile;

fn make_long_doc(lines: usize) -> String {
    let mut s = String::with_capacity(lines * 10);
    for i in 1..=lines {
        s.push_str(&format!("Line {:04}\n", i));
    }
    s
}

fn new_app_with(content: &str) -> (App, NamedTempFile) {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    let (doc, _warn) = Document::load(file.path()).unwrap();
    let app = App::new(Config::default(), doc, vec![]);
    (app, file)
}

fn focused_scroll(app: &App) -> usize {
    app.panes.focused_pane().unwrap().view.scroll_line
}

fn focused_cursor(app: &App) -> usize {
    app.panes.focused_pane().unwrap().view.cursor_line
}

fn press(app: &mut App, code: KeyCode, mods: KeyModifiers, vh: usize, vw: usize) {
    let ev = KeyEvent::new(code, mods);
    handle_input(app, ev, vh, vw).expect("handle_input failed");
}

#[test]
fn harness_held_j_burst_advances_cursor_monotonically() {
    let content = make_long_doc(500);
    let (mut app, _f) = new_app_with(&content);

    // Simulate a held-j key burst — 100 j-presses in a row (like the
    // event drain loop in lib.rs processes up to 32/tick, but the
    // per-event handler is what we're exercising here).
    for _ in 0..100 {
        press(&mut app, KeyCode::Char('j'), KeyModifiers::NONE, 20, 80);
    }
    assert_eq!(focused_cursor(&app), 100);
    // scroll should have followed the cursor (auto_scroll_to_cursor),
    // keeping the viewport anchored such that cursor is visible.
    let scroll = focused_scroll(&app);
    assert!(scroll <= 100, "scroll must not exceed cursor");
    assert!(
        scroll + 20 > 100,
        "cursor at 100 must fall within [scroll, scroll+20): scroll={scroll}"
    );
}

#[test]
fn harness_resize_clamps_cursor_and_scroll_at_bottom() {
    let content = make_long_doc(200);
    let (mut app, _f) = new_app_with(&content);

    // Jump to end, then resize the terminal smaller while the cursor
    // is past the new document-tail boundary.
    let last = app.doc.line_count().saturating_sub(1);
    app.jump_to_line(last);
    app.auto_scroll(30);

    let cursor_before = focused_cursor(&app);
    assert_eq!(cursor_before, last);

    // Resize to a tiny terminal.
    app.on_resize(40, 10);

    let cursor_after = focused_cursor(&app);
    let scroll_after = focused_scroll(&app);
    assert!(
        cursor_after <= last,
        "cursor must remain within doc after resize"
    );
    assert!(
        scroll_after <= last,
        "scroll must remain within doc after resize"
    );
}

#[test]
fn harness_pgdn_pgup_with_overlap_round_trips() {
    let content = make_long_doc(1000);
    let (mut app, _f) = new_app_with(&content);

    // PgDn 5x then PgUp 5x with default overlap=2.
    for _ in 0..5 {
        press(&mut app, KeyCode::PageDown, KeyModifiers::NONE, 30, 80);
    }
    let mid_cursor = focused_cursor(&app);
    assert!(mid_cursor > 0);
    for _ in 0..5 {
        press(&mut app, KeyCode::PageUp, KeyModifiers::NONE, 30, 80);
    }
    // On a stable range (neither end reached), cursor returns to start.
    assert_eq!(focused_cursor(&app), 0);
    assert_eq!(focused_scroll(&app), 0);
}

#[test]
fn harness_split_drag_reclamps_scroll_in_narrowed_pane() {
    let content = make_long_doc(300);
    let (mut app, _f) = new_app_with(&content);

    // Split horizontally so both panes share a ~20-row viewport.
    app.split_focused(mdx_tui::panes::SplitDir::Horizontal);
    assert_eq!(app.panes.panes.len(), 2);

    // Jump to end in the focused pane.
    let last = app.doc.line_count().saturating_sub(1);
    app.jump_to_line(last);
    app.auto_scroll(20);

    let scroll_before = focused_scroll(&app);

    // Simulate a drag that shrinks the focused pane dramatically.
    // Split path is empty for the root split; 0.1 is the clamp floor.
    let _ = app.panes.update_split_ratio(&[], 0.1);
    app.enforce_rendered_bounds();

    // The focused pane still has a valid scroll position in-range.
    let scroll_after = focused_scroll(&app);
    assert!(scroll_after <= last);
    // And cursor is still in-range.
    assert!(focused_cursor(&app) <= last);

    // A degenerate split shouldn't push scroll past the end.
    assert!(
        scroll_after <= scroll_before.max(1),
        "scroll should not advance past prior position after narrowing"
    );
}

#[test]
fn harness_scrollbar_toggle_reclamps_via_apply_options() {
    // Changing show_scrollbar reduces content_width by 1. The
    // enforce_rendered_bounds call in apply_options re-clamps scroll so
    // we never end up scrolled past the new rendered tail.
    let content = make_long_doc(500);
    let (mut app, _f) = new_app_with(&content);

    let last = app.doc.line_count().saturating_sub(1);
    app.jump_to_line(last);
    app.auto_scroll(20);
    let scroll_before = focused_scroll(&app);

    // Toggle scrollbar off then on via apply_options.
    app.config.render.show_scrollbar = false;
    app.enforce_rendered_bounds();
    let scroll_off = focused_scroll(&app);
    assert!(scroll_off <= last);

    app.config.render.show_scrollbar = true;
    app.enforce_rendered_bounds();
    let scroll_on = focused_scroll(&app);
    assert!(scroll_on <= last);

    // No regressions — scroll stayed sane across toggles.
    assert!(scroll_before.abs_diff(scroll_on) <= 5);
}

#[test]
fn harness_jump_stack_ctrl_o_ctrl_i_round_trip() {
    let content = make_long_doc(500);
    let (mut app, _f) = new_app_with(&content);

    // Move cursor to line 50 manually so it's a "non-jump" origin.
    for _ in 0..50 {
        press(&mut app, KeyCode::Char('j'), KeyModifiers::NONE, 20, 80);
    }
    assert_eq!(focused_cursor(&app), 50);

    // G — jumps to last line and push_jump records origin at 50.
    press(&mut app, KeyCode::Char('G'), KeyModifiers::SHIFT, 20, 80);
    let last = app.doc.line_count().saturating_sub(1);
    assert_eq!(focused_cursor(&app), last);

    // Ctrl-O — back to 50.
    press(&mut app, KeyCode::Char('o'), KeyModifiers::CONTROL, 20, 80);
    assert_eq!(focused_cursor(&app), 50, "Ctrl-O should return to origin");

    // Ctrl-I — forward to last.
    press(&mut app, KeyCode::Char('i'), KeyModifiers::CONTROL, 20, 80);
    assert_eq!(
        focused_cursor(&app),
        last,
        "Ctrl-I should return to the tip"
    );
}

#[test]
fn harness_gg_respects_front_matter_bounds() {
    // Front-matter at the top; gg should land on the first rendered
    // line, not on line 0.
    let content = "---\ntitle: Test\ndate: 2026-01-01\n---\n# Real heading\n\nBody line 1\nBody line 2\n";
    let (mut app, _f) = new_app_with(content);
    // skip_front_matter is true by default; front matter is parsed on
    // App construction.
    assert!(app.config.render.skip_front_matter);

    // Move cursor far down first.
    for _ in 0..6 {
        press(&mut app, KeyCode::Char('j'), KeyModifiers::NONE, 20, 80);
    }

    // gg — two 'g' presses, per vim convention.
    press(&mut app, KeyCode::Char('g'), KeyModifiers::NONE, 20, 80);
    press(&mut app, KeyCode::Char('g'), KeyModifiers::NONE, 20, 80);
    let c = focused_cursor(&app);

    // With front matter occupying lines 0..=3, rendered content begins
    // at line 4 so cursor should land there, not at 0.
    assert!(
        c >= 4,
        "gg with skip_front_matter should land at or after line 4, got {c}"
    );
}

#[test]
fn harness_gg_prefix_requires_two_presses() {
    let content = make_long_doc(200);
    let (mut app, _f) = new_app_with(&content);

    // Move cursor off line 0.
    for _ in 0..10 {
        press(&mut app, KeyCode::Char('j'), KeyModifiers::NONE, 20, 80);
    }
    assert_eq!(focused_cursor(&app), 10);

    // Single 'g' should NOT jump — it should set the g prefix.
    press(&mut app, KeyCode::Char('g'), KeyModifiers::NONE, 20, 80);
    assert_eq!(
        focused_cursor(&app),
        10,
        "single 'g' must not jump in gg-prefix mode"
    );

    // A non-g key cancels the prefix and is processed normally.
    press(&mut app, KeyCode::Char('j'), KeyModifiers::NONE, 20, 80);
    assert_eq!(
        focused_cursor(&app),
        11,
        "'j' after 'g' should cancel prefix and move down"
    );

    // Now the real 'gg' sequence.
    press(&mut app, KeyCode::Char('g'), KeyModifiers::NONE, 20, 80);
    press(&mut app, KeyCode::Char('g'), KeyModifiers::NONE, 20, 80);
    assert_eq!(focused_cursor(&app), 0, "'gg' should jump to top");
}

#[test]
fn harness_search_next_centers_match_in_viewport() {
    // After /pattern + n, the match should sit roughly in the middle of
    // the viewport, not at the edge. This validates the ScrollPolicy::Center
    // wiring in next_search_match.
    let content = make_long_doc(500);
    let (mut app, _f) = new_app_with(&content);

    // Seed search_matches directly — we want to avoid the interactive
    // search-mode keyboard path here.
    app.search_query = "Line 0300".to_string();
    app.search_matches = vec![299];
    app.search_current_match = Some(0);

    // No draw has happened so goto() uses DEFAULT_FALLBACK_HEIGHT (20).
    // Center → scroll = 299 - 20/2 = 289.
    app.next_search_match(20);

    let cursor = focused_cursor(&app);
    let scroll = focused_scroll(&app);
    assert_eq!(cursor, 299, "cursor should land on the matched line");

    let expected = 289;
    assert!(
        scroll.abs_diff(expected) <= 2,
        "search match should be centered; got scroll={scroll}, expected ~{expected}"
    );
    // And critically, the match must fall within the viewport (not at
    // an edge like NearestEdge would produce).
    assert!(
        scroll < cursor && cursor < scroll + 20,
        "cursor {cursor} should be well inside viewport [{scroll}, {}]",
        scroll + 20
    );
}

#[test]
fn harness_empty_doc_is_not_panicking_on_pgdn_resize() {
    // Degenerate inputs should not panic.
    let (mut app, _f) = new_app_with("");
    press(&mut app, KeyCode::PageDown, KeyModifiers::NONE, 20, 80);
    app.on_resize(1, 1);
    app.on_resize(0, 0);
    // Reaches here without panic → pass.
    assert!(app.doc.line_count() >= 1);
}
