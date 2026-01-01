//! Input handling and keybindings

use crate::app::{App, KeyPrefix, MouseState};
use crate::panes::{Direction, PaneId, SplitDir};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// Result of handling input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Continue,
    Quit,
    OpenEditor,
}

/// Handle a key event with viewport dimensions for scroll commands
pub fn handle_input(app: &mut App, key: KeyEvent, viewport_height: usize, viewport_width: usize) -> Result<Action> {
    // Clear status message on any keystroke (except pure modifiers)
    // This ensures messages don't persist indefinitely
    if !matches!(key.code, KeyCode::Modifier(_)) {
        app.clear_status_message();
    }

    // Handle close pane with 'q' - quit if last pane
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        }
    ) {
        // Try to close the focused pane
        let has_remaining_panes = app.panes.close_focused();
        if !has_remaining_panes {
            // Was the last pane, quit the app
            app.quit();
            return Ok(Action::Quit);
        }
        return Ok(Action::Continue);
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

    // Handle help dialog - close with Esc or ?
    if app.show_help {
        if matches!(
            key,
            KeyEvent {
                code: KeyCode::Esc,
                ..
            }
        ) || matches!(
            key,
            KeyEvent {
                code: KeyCode::Char('?'),
                ..
            }
        ) {
            app.toggle_help();
            return Ok(Action::Continue);
        }
        // Ignore all other keys when help is shown
        return Ok(Action::Continue);
    }

    // Handle options dialog
    if app.options_dialog.is_some() {
        match key {
            // Esc or Shift+O - close dialog without changes
            KeyEvent {
                code: KeyCode::Esc,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('O'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                app.close_options();
                return Ok(Action::Continue);
            }

            // Up/k - move up
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(ref mut dialog) = app.options_dialog {
                    dialog.move_up();
                }
                return Ok(Action::Continue);
            }

            // Down/j - move down
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(ref mut dialog) = app.options_dialog {
                    dialog.move_down();
                }
                return Ok(Action::Continue);
            }

            // Left/Right - toggle current option
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(ref mut dialog) = app.options_dialog {
                    dialog.toggle_current();
                }
                return Ok(Action::Continue);
            }

            // Enter - execute focused button action
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(ref dialog) = app.options_dialog {
                    match dialog.focused_button {
                        crate::options_dialog::DialogButton::Cancel => {
                            app.close_options();
                        }
                        crate::options_dialog::DialogButton::Ok => {
                            app.apply_options();
                        }
                        crate::options_dialog::DialogButton::Save => {
                            if let Err(e) = app.save_options() {
                                eprintln!("Failed to save options: {}", e);
                            }
                        }
                    }
                }
                return Ok(Action::Continue);
            }

            // Tab - cycle through buttons
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(ref mut dialog) = app.options_dialog {
                    dialog.next_button();
                }
                return Ok(Action::Continue);
            }

            // Shift+Tab - cycle backwards through buttons
            KeyEvent {
                code: KeyCode::BackTab,
                ..
            } => {
                if let Some(ref mut dialog) = app.options_dialog {
                    dialog.prev_button();
                }
                return Ok(Action::Continue);
            }

            // c - execute Cancel button
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.close_options();
                return Ok(Action::Continue);
            }

            // o - execute Ok button
            KeyEvent {
                code: KeyCode::Char('o'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.apply_options();
                return Ok(Action::Continue);
            }

            // s - execute Save button
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                match app.save_options() {
                    Ok(()) => {
                        app.set_success_message("Configuration saved successfully");
                    }
                    Err(e) => {
                        app.set_error_message(format!("Failed to save options: {}", e));
                    }
                }
                return Ok(Action::Continue);
            }

            _ => {
                // Ignore other keys
                return Ok(Action::Continue);
            }
        }
    }

    // Handle TOC dialog
    if app.show_toc_dialog {
        let dialog_height = viewport_height;

        match key {
            // j or Down - move down in TOC dialog
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.toc_dialog_move_down(dialog_height);
                return Ok(Action::Continue);
            }

            // k or Up - move up in TOC dialog
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.toc_dialog_move_up(dialog_height);
                return Ok(Action::Continue);
            }

            // Enter - jump to selected heading and close dialog
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                app.toc_dialog_jump_to_selected();
                return Ok(Action::Continue);
            }

            // Esc or T - close TOC dialog
            KeyEvent {
                code: KeyCode::Esc, ..
            }
            | KeyEvent {
                code: KeyCode::Char('T'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                app.toggle_toc_dialog();
                return Ok(Action::Continue);
            }

            _ => return Ok(Action::Continue),
        }
    }

    // Handle key prefix sequences
    if app.key_prefix == KeyPrefix::CtrlW {
        // Compute pane layouts for focus movement
        let pane_layouts = app.panes.compute_layout(ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        });

        match key {
            // ^w s - horizontal split
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.split_focused(SplitDir::Horizontal);
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w v - vertical split
            KeyEvent {
                code: KeyCode::Char('v'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.split_focused(SplitDir::Vertical);
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w ↑ - move focus up
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if !app.toc_focus {
                    app.panes.move_focus(Direction::Up, &pane_layouts);
                }
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w ↓ - move focus down
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if !app.toc_focus {
                    app.panes.move_focus(Direction::Down, &pane_layouts);
                }
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w ← - move focus left
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Handle TOC focus if visible
                if app.show_toc {
                    if app.config.toc.side == mdx_core::config::TocSide::Left {
                        if !app.toc_focus {
                            app.toc_focus = true;
                            app.key_prefix = KeyPrefix::None;
                            return Ok(Action::Continue);
                        }
                    } else if app.toc_focus {
                        app.toc_focus = false;
                        app.key_prefix = KeyPrefix::None;
                        return Ok(Action::Continue);
                    }
                }

                if !app.toc_focus {
                    app.panes.move_focus(Direction::Left, &pane_layouts);
                }
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w → - move focus right
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Handle TOC focus if visible
                if app.show_toc {
                    if app.config.toc.side == mdx_core::config::TocSide::Right {
                        if !app.toc_focus {
                            app.toc_focus = true;
                            app.key_prefix = KeyPrefix::None;
                            return Ok(Action::Continue);
                        }
                    } else if app.toc_focus {
                        app.toc_focus = false;
                        app.key_prefix = KeyPrefix::None;
                        return Ok(Action::Continue);
                    }
                }

                if !app.toc_focus {
                    app.panes.move_focus(Direction::Right, &pane_layouts);
                }
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w h - move focus left (vim-style)
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Handle TOC focus if visible
                if app.show_toc {
                    if app.config.toc.side == mdx_core::config::TocSide::Left {
                        if !app.toc_focus {
                            app.toc_focus = true;
                            app.key_prefix = KeyPrefix::None;
                            return Ok(Action::Continue);
                        }
                    } else if app.toc_focus {
                        app.toc_focus = false;
                        app.key_prefix = KeyPrefix::None;
                        return Ok(Action::Continue);
                    }
                }

                if !app.toc_focus {
                    app.panes.move_focus(Direction::Left, &pane_layouts);
                }
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w j - move focus down (vim-style)
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if !app.toc_focus {
                    app.panes.move_focus(Direction::Down, &pane_layouts);
                }
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w k - move focus up (vim-style)
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if !app.toc_focus {
                    app.panes.move_focus(Direction::Up, &pane_layouts);
                }
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // ^w l - move focus right (vim-style)
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Handle TOC focus if visible
                if app.show_toc {
                    if app.config.toc.side == mdx_core::config::TocSide::Right {
                        if !app.toc_focus {
                            app.toc_focus = true;
                            app.key_prefix = KeyPrefix::None;
                            return Ok(Action::Continue);
                        }
                    } else if app.toc_focus {
                        app.toc_focus = false;
                        app.key_prefix = KeyPrefix::None;
                        return Ok(Action::Continue);
                    }
                }

                if !app.toc_focus {
                    app.panes.move_focus(Direction::Right, &pane_layouts);
                }
                app.key_prefix = KeyPrefix::None;
                return Ok(Action::Continue);
            }

            // Any other key cancels the prefix
            _ => {
                app.key_prefix = KeyPrefix::None;
            }
        }
    }

    // ^w - enter prefix mode
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    ) {
        app.key_prefix = KeyPrefix::CtrlW;
        return Ok(Action::Continue);
    }

    // Ctrl+Arrow keys - move focus between panes and TOC
    let pane_layouts = app.panes.compute_layout(ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    });

    match key {
        KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            if !app.toc_focus {
                app.panes.move_focus(Direction::Up, &pane_layouts);
            }
            return Ok(Action::Continue);
        }

        KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            if !app.toc_focus {
                app.panes.move_focus(Direction::Down, &pane_layouts);
            }
            return Ok(Action::Continue);
        }

        KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            // Handle TOC focus if visible
            if app.show_toc {
                if app.config.toc.side == mdx_core::config::TocSide::Left {
                    // TOC is on left
                    if !app.toc_focus {
                        app.toc_focus = true;
                        return Ok(Action::Continue);
                    }
                } else {
                    // TOC is on right, unfocus it if focused
                    if app.toc_focus {
                        app.toc_focus = false;
                        return Ok(Action::Continue);
                    }
                }
            }

            if !app.toc_focus {
                app.panes.move_focus(Direction::Left, &pane_layouts);
            }
            return Ok(Action::Continue);
        }

        KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            // Handle TOC focus if visible
            if app.show_toc {
                if app.config.toc.side == mdx_core::config::TocSide::Right {
                    // TOC is on right
                    if !app.toc_focus {
                        app.toc_focus = true;
                        return Ok(Action::Continue);
                    }
                } else {
                    // TOC is on left, unfocus it if focused
                    if app.toc_focus {
                        app.toc_focus = false;
                        return Ok(Action::Continue);
                    }
                }
            }

            if !app.toc_focus {
                app.panes.move_focus(Direction::Right, &pane_layouts);
            }
            return Ok(Action::Continue);
        }

        _ => {}
    }

    // Handle search mode input
    if let Some(pane) = app.panes.focused_pane() {
        if pane.view.mode == crate::app::Mode::Search {
            match key {
                // Enter - execute search and exit search mode
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    app.exit_search_mode();
                    return Ok(Action::Continue);
                }

                // Esc - cancel search and exit search mode
                KeyEvent {
                    code: KeyCode::Esc,
                    ..
                } => {
                    app.clear_search();
                    app.exit_search_mode();
                    return Ok(Action::Continue);
                }

                // Backspace - remove last character
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    app.search_backspace();
                    return Ok(Action::Continue);
                }

                // Any printable character - add to search query
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                    ..
                } => {
                    app.search_add_char(c);
                    return Ok(Action::Continue);
                }

                _ => return Ok(Action::Continue),
            }
        }
    }

    // Handle TOC-specific keys when TOC is focused
    if app.toc_focus {
        // TOC viewport height is similar to main viewport
        let toc_height = viewport_height;

        match key {
            // j - move down in TOC
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.toc_move_down(toc_height);
                return Ok(Action::Continue);
            }

            // k - move up in TOC
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.toc_move_up(toc_height);
                return Ok(Action::Continue);
            }

            // Enter or l - jump to selected heading
            KeyEvent {
                code: KeyCode::Enter,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.toc_jump_to_selected();
                app.toc_focus = false; // Return focus to document
                return Ok(Action::Continue);
            }

            // t or Esc - close TOC
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                app.toggle_toc();
                return Ok(Action::Continue);
            }

            // Arrow keys - same as j/k in TOC
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.toc_move_down(toc_height);
                return Ok(Action::Continue);
            }

            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                app.toc_move_up(toc_height);
                return Ok(Action::Continue);
            }

            _ => {}
        }
    }

    // Shift+V - enter visual line mode
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('V'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        app.enter_visual_line_mode();
        return Ok(Action::Continue);
    }

    // Esc - exit visual line mode
    if matches!(key, KeyEvent { code: KeyCode::Esc, .. }) {
        app.exit_visual_line_mode();
        return Ok(Action::Continue);
    }

    // Y - yank in visual line mode
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('Y'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        match app.yank_selection() {
            Ok(_count) => {
                // Show feedback in status (would need message system for full implementation)
                // For now, just exit visual mode after yank
                app.exit_visual_line_mode();
            }
            Err(_e) => {
                // Silently fail - clipboard might not be available
                app.exit_visual_line_mode();
            }
        }
        return Ok(Action::Continue);
    }

    // / - enter search mode
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('/'),
            modifiers: KeyModifiers::NONE,
            ..
        }
    ) {
        app.enter_search_mode();
        return Ok(Action::Continue);
    }

    // n - next search match
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::NONE,
            ..
        }
    ) {
        app.next_search_match(viewport_height);
        return Ok(Action::Continue);
    }

    // N - previous search match
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('N'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        app.prev_search_match(viewport_height);
        return Ok(Action::Continue);
    }

    // t - toggle TOC sidebar
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('t'),
            modifiers: KeyModifiers::NONE,
            ..
        }
    ) {
        app.toggle_toc();
        return Ok(Action::Continue);
    }

    // T - open TOC dialog
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('T'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        app.toggle_toc_dialog();
        return Ok(Action::Continue);
    }

    // W - toggle security warnings pane
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('W'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        app.toggle_security_warnings();
        return Ok(Action::Continue);
    }

    // M - toggle theme
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('M'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        app.toggle_theme();
        return Ok(Action::Continue);
    }

    // ? - toggle help dialog
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('?'),
            ..
        }
    ) {
        app.toggle_help();
        return Ok(Action::Continue);
    }

    // O - open options dialog
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('O'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        app.open_options();
        return Ok(Action::Continue);
    }

    // e - open in editor
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::NONE,
            ..
        }
    ) {
        return Ok(Action::OpenEditor);
    }

    // r - toggle raw/rendered mode in active pane
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::NONE,
            ..
        }
    ) {
        if let Some(pane) = app.panes.focused_pane_mut() {
            pane.view.show_raw = !pane.view.show_raw;
        }
        return Ok(Action::Continue);
    }

    // R - reload document from disk
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('R'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        if let Err(e) = app.reload_document() {
            // Silently fail - would need message system for full implementation
            eprintln!("Failed to reload document: {}", e);
        }
        return Ok(Action::Continue);
    }

    // Navigation commands (when not in TOC)
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
            app.scroll_half_page_down(viewport_height, viewport_width);
            app.auto_scroll(viewport_height);
        }

        // Ctrl+u - half page up
        KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            app.scroll_half_page_up(viewport_height, viewport_width);
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

        // Arrow keys - same as j/k
        KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            app.move_cursor_down(1);
            app.auto_scroll(viewport_height);
        }

        KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            app.move_cursor_up(1);
            app.auto_scroll(viewport_height);
        }

        // PageDown/PageUp - scroll by full page
        KeyEvent {
            code: KeyCode::PageDown,
            ..
        } => {
            app.move_cursor_down(viewport_height);
            app.auto_scroll(viewport_height);
        }

        KeyEvent {
            code: KeyCode::PageUp,
            ..
        } => {
            app.move_cursor_up(viewport_height);
            app.auto_scroll(viewport_height);
        }

        // Space - same as PageDown
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            app.move_cursor_down(viewport_height);
            app.auto_scroll(viewport_height);
        }

        // Home/End - same as g/G
        KeyEvent {
            code: KeyCode::Home,
            ..
        } => {
            app.jump_to_line(0);
            app.auto_scroll(viewport_height);
        }

        KeyEvent {
            code: KeyCode::End,
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

/// Hit test result - what was clicked
#[derive(Debug, Clone, PartialEq)]
enum HitTarget {
    Pane(PaneId, Rect),
    Toc(Rect),
    SplitBorder { path: Vec<usize>, is_vertical: bool },
    None,
}

/// Handle mouse events
pub fn handle_mouse(
    app: &mut App,
    mouse: MouseEvent,
    viewport_height: usize,
    _viewport_width: usize,
) -> Result<()> {
    let MouseEvent { kind, column, row, .. } = mouse;

    // Get terminal size to compute layout
    // We need to account for the status bar at the bottom
    let term_width = crossterm::terminal::size()?.0;
    let term_height = crossterm::terminal::size()?.1;

    // Compute layout areas (must match ui.rs layout logic)
    let layout_info = compute_layout_info(app, term_width, term_height);

    match kind {
        MouseEventKind::Down(MouseButton::Left) => {
            handle_mouse_down(app, column, row, &layout_info, viewport_height)?;
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            handle_mouse_drag(app, column, row, &layout_info, viewport_height)?;
        }
        MouseEventKind::Up(MouseButton::Left) => {
            handle_mouse_up(app)?;
        }
        MouseEventKind::ScrollDown => {
            handle_scroll(app, column, row, &layout_info, viewport_height, 3)?;
        }
        MouseEventKind::ScrollUp => {
            handle_scroll(app, column, row, &layout_info, viewport_height, -3)?;
        }
        _ => {
            // Ignore other mouse events
        }
    }

    Ok(())
}

/// Layout information for hit testing
#[derive(Debug)]
struct LayoutInfo {
    toc_rect: Option<Rect>,
    pane_rects: std::collections::HashMap<PaneId, Rect>,
    content_offset_y: u16, // Offset for security warnings
}

/// Compute layout information matching ui.rs
fn compute_layout_info(app: &App, term_width: u16, term_height: u16) -> LayoutInfo {
    use ratatui::layout::{Constraint, Direction as LayoutDir, Layout};

    let base_layout = Layout::default()
        .direction(LayoutDir::Vertical)
        .constraints([
            Constraint::Min(1),      // Main content area
            Constraint::Length(1),   // Status bar
        ])
        .split(Rect::new(0, 0, term_width, term_height));

    let mut content_area = base_layout[0];
    let mut content_offset_y = 0;

    // Account for security warnings (matches ui.rs logic)
    if app.show_security_warnings && !app.security_warnings.is_empty() {
        let warnings_height = app.security_warnings.len().min(5) as u16 + 2; // +2 for borders
        content_area.y += warnings_height;
        content_area.height = content_area.height.saturating_sub(warnings_height);
        content_offset_y = warnings_height;
    }

    // Split TOC and panes area
    let (toc_rect, panes_area) = if app.show_toc {
        let toc_width = app.config.toc.width as u16;
        let chunks = Layout::default()
            .direction(LayoutDir::Horizontal)
            .constraints([
                Constraint::Length(toc_width),
                Constraint::Min(1),
            ])
            .split(content_area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, content_area)
    };

    // Compute pane layout
    let pane_rects = app.panes.compute_layout(panes_area);

    LayoutInfo {
        toc_rect,
        pane_rects,
        content_offset_y,
    }
}

/// Perform hit testing to determine what was clicked
fn hit_test(x: u16, y: u16, layout: &LayoutInfo) -> HitTarget {
    // Check TOC first
    if let Some(toc_rect) = layout.toc_rect {
        if x >= toc_rect.x
            && x < toc_rect.x + toc_rect.width
            && y >= toc_rect.y
            && y < toc_rect.y + toc_rect.height
        {
            return HitTarget::Toc(toc_rect);
        }
    }

    // Check panes
    for (pane_id, rect) in &layout.pane_rects {
        if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
            return HitTarget::Pane(*pane_id, *rect);
        }
    }

    // TODO: Check split borders (Stage 4)

    HitTarget::None
}

/// Handle mouse down event
fn handle_mouse_down(
    app: &mut App,
    x: u16,
    y: u16,
    layout: &LayoutInfo,
    _viewport_height: usize,
) -> Result<()> {
    let target = hit_test(x, y, layout);

    match target {
        HitTarget::Pane(pane_id, _rect) => {
            // Stage 2: Click to focus and potentially start selection
            app.mouse_state = MouseState::Selecting {
                pane_id,
                anchor_line: 0, // Will be computed in Stage 2
            };
        }
        HitTarget::Toc(_rect) => {
            // Stage 3: TOC click handling
            app.mouse_state = MouseState::Idle;
        }
        HitTarget::SplitBorder { path, .. } => {
            // Stage 4: Start resize
            app.mouse_state = MouseState::Resizing {
                split_path: path,
                start_ratio: 0.5, // Will be computed in Stage 4
                start_pos: (x, y),
            };
        }
        HitTarget::None => {
            app.mouse_state = MouseState::Idle;
        }
    }

    Ok(())
}

/// Handle mouse drag event
fn handle_mouse_drag(
    app: &mut App,
    _x: u16,
    _y: u16,
    _layout: &LayoutInfo,
    _viewport_height: usize,
) -> Result<()> {
    match &app.mouse_state {
        MouseState::Selecting { .. } => {
            // Stage 2: Update selection
        }
        MouseState::Resizing { .. } => {
            // Stage 4: Update split ratio
        }
        MouseState::Idle => {
            // Not dragging anything
        }
    }

    Ok(())
}

/// Handle mouse up event
fn handle_mouse_up(app: &mut App) -> Result<()> {
    match &app.mouse_state {
        MouseState::Selecting { .. } => {
            // Stage 2: Finalize selection
            app.mouse_state = MouseState::Idle;
        }
        MouseState::Resizing { .. } => {
            // Stage 4: Finalize resize
            app.mouse_state = MouseState::Idle;
        }
        MouseState::Idle => {
            // Nothing to do
        }
    }

    Ok(())
}

/// Handle scroll wheel event
fn handle_scroll(
    app: &mut App,
    x: u16,
    y: u16,
    layout: &LayoutInfo,
    _viewport_height: usize,
    delta: i32,
) -> Result<()> {
    let target = hit_test(x, y, layout);

    match target {
        HitTarget::Toc(rect) => {
            // Stage 3: Scroll TOC
            let _visible_rows = rect.height.saturating_sub(2) as usize; // -2 for borders
            // TODO: Adjust toc_scroll
        }
        HitTarget::Pane(pane_id, rect) => {
            // Stage 3: Scroll pane
            let _visible_lines = rect.height.saturating_sub(3) as usize; // -3 for borders + breadcrumb
            // TODO: Adjust pane's scroll_line
            let _ = pane_id; // Suppress unused warning for now
        }
        _ => {
            // Ignore scroll on other areas
        }
    }

    let _ = delta; // Suppress unused warning for now
    Ok(())
}
