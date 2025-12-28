//! Input handling and keybindings

use crate::app::{App, KeyPrefix};
use crate::panes::{Direction, SplitDir};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Result of handling input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Continue,
    Quit,
    OpenEditor,
}

/// Handle a key event with viewport height for scroll commands
pub fn handle_input(app: &mut App, key: KeyEvent, viewport_height: usize) -> Result<Action> {
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

            // T or Esc - close TOC
            KeyEvent {
                code: KeyCode::Char('T'),
                modifiers: KeyModifiers::SHIFT,
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

    // T - toggle TOC
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('T'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
    ) {
        app.toggle_toc();
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

    // r - reload document from disk
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::NONE,
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
            app.scroll_half_page_down(viewport_height);
            app.auto_scroll(viewport_height);
        }

        // Ctrl+u - half page up
        KeyEvent {
            code: KeyCode::Char('u'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => {
            app.scroll_half_page_up(viewport_height);
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
