//! UI rendering

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Draw the UI
pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // Main content area (potentially split)
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    let pane_area = if app.show_toc {
        let toc_width = app.config.toc.width as u16;
        let main_chunks = if app.config.toc.side == mdx_core::config::TocSide::Left {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(toc_width), // TOC
                    Constraint::Min(1),            // Panes area
                ])
                .split(chunks[0])
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(1),            // Panes area
                    Constraint::Length(toc_width), // TOC
                ])
                .split(chunks[0])
        };

        // Render TOC based on position
        if app.config.toc.side == mdx_core::config::TocSide::Left {
            render_toc(frame, app, main_chunks[0]);
            main_chunks[1]
        } else {
            render_toc(frame, app, main_chunks[1]);
            main_chunks[0]
        }
    } else {
        chunks[0]
    };

    // Compute layout for all panes and render them
    let pane_layouts = app.panes.compute_layout(pane_area);
    for (pane_id, rect) in pane_layouts.iter() {
        render_markdown(frame, app, *rect, *pane_id);
    }

    // Render status bar
    render_status_bar(frame, app, chunks[1]);
}

fn render_markdown(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, pane_id: usize) {
    // Get the pane's view state
    let pane = match app.panes.panes.get(&pane_id) {
        Some(p) => p,
        None => return,
    };

    // Get markdown content from document
    let content: String = app.doc.rope.chunks().collect();

    // Convert to lines with cursor and selection highlighting
    let scroll = pane.view.scroll_line;
    let cursor = pane.view.cursor_line;
    let is_focused = app.panes.focused == pane_id;

    // Get selection range if in visual line mode
    let selection_range = if pane.view.mode == crate::app::Mode::VisualLine {
        pane.view.selection.as_ref().map(|sel| sel.range())
    } else {
        None
    };

    let lines: Vec<Line> = content
        .lines()
        .enumerate()
        .skip(scroll)
        .take(area.height as usize)
        .map(|(idx, text)| {
            // Get diff mark for this line
            #[cfg(feature = "git")]
            let gutter = if app.config.git.diff {
                use mdx_core::diff::DiffMark;
                match app.doc.diff_gutter.get(idx) {
                    DiffMark::None => "  ",
                    DiffMark::Added => "+ ",
                    DiffMark::Modified => "~ ",
                    DiffMark::DeletedAfter(_) => "▾ ",
                }
            } else {
                "  "
            };
            #[cfg(not(feature = "git"))]
            let gutter = "  ";

            let line_text = format!("{}{}", gutter, text);

            // Check if line is in selection
            let is_selected = if let Some((start, end)) = selection_range {
                idx >= start && idx <= end
            } else {
                false
            };

            // Highlight selected lines
            if is_focused && is_selected {
                Line::from(line_text).style(
                    Style::default()
                        .fg(app.theme.base.fg.unwrap_or(Color::White))
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::REVERSED)
                )
            }
            // Highlight cursor line (only if focused and not selected)
            else if is_focused && idx == cursor && !is_selected {
                Line::from(line_text).style(
                    Style::default()
                        .fg(app.theme.base.fg.unwrap_or(Color::White))
                        .bg(app.theme.cursor_line_bg)
                )
            } else {
                Line::from(line_text).style(app.theme.base)
            }
        })
        .collect();

    // Add border to pane with focus highlight
    let border_style = if is_focused {
        Style::default().fg(app.theme.toc_active.bg.unwrap_or(Color::LightCyan))
    } else {
        Style::default().fg(app.theme.toc_border)
    };

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).border_style(border_style))
        .style(app.theme.base);

    frame.render_widget(paragraph, area);
}

fn render_toc(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // Get current heading index to highlight
    let current_heading = app.current_heading_index();

    // Build TOC lines with indentation based on heading level
    let toc_lines: Vec<Line> = app
        .doc
        .headings
        .iter()
        .enumerate()
        .map(|(idx, heading)| {
            // Indent based on level (2 spaces per level, starting from level 1)
            let indent = "  ".repeat((heading.level as usize).saturating_sub(1));
            let text = format!("{}{}", indent, heading.text);

            // Highlight selected or current heading
            if app.toc_focus && idx == app.toc_selected {
                // Selected in TOC focus mode
                Line::from(text).style(app.theme.toc_active)
            } else if !app.toc_focus && Some(idx) == current_heading {
                // Current heading when not focused
                Line::from(text).style(
                    Style::default()
                        .fg(app.theme.toc_active.bg.unwrap_or(Color::Cyan))
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::from(text).style(app.theme.base)
            }
        })
        .collect();

    // Create title based on focus state
    let title = if app.toc_focus {
        "TOC [focused]"
    } else {
        "TOC"
    };

    let border_style = if app.toc_focus {
        Style::default().fg(app.theme.toc_active.bg.unwrap_or(Color::LightCyan))
    } else {
        Style::default().fg(app.theme.toc_border)
    };

    let toc_widget = Paragraph::new(toc_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .style(app.theme.base);

    frame.render_widget(toc_widget, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let filename = app
        .doc
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("untitled");

    let line_count = app.doc.line_count();
    let heading_count = app.doc.headings.len();

    let (current_line, mode_str, selection_count) = if let Some(pane) = app.panes.focused_pane() {
        let line = pane.view.cursor_line + 1; // 1-based for display
        let (mode, sel_count) = match pane.view.mode {
            crate::app::Mode::Normal => ("NORMAL", None),
            crate::app::Mode::VisualLine => {
                let count = pane.view.selection.as_ref().map(|sel| {
                    let (start, end) = sel.range();
                    end - start + 1
                });
                ("V-LINE", count)
            }
        };
        (line, mode, sel_count)
    } else {
        (1, "NORMAL", None)
    };

    let toc_indicator = if app.show_toc {
        if app.toc_focus {
            " [TOC*]"
        } else {
            " [TOC]"
        }
    } else {
        ""
    };

    let theme_str = match app.theme_variant {
        mdx_core::config::ThemeVariant::Dark => "DARK",
        mdx_core::config::ThemeVariant::Light => "LIGHT",
    };

    let prefix_str = match app.key_prefix {
        crate::app::KeyPrefix::None => "",
        crate::app::KeyPrefix::CtrlW => "  ^W-",
    };

    let selection_str = if let Some(count) = selection_count {
        format!(" ({} lines)", count)
    } else {
        String::new()
    };

    #[cfg(feature = "watch")]
    let watch_str = if app.watcher.is_some() {
        if app.doc.dirty_on_disk {
            "  [DIRTY]"
        } else {
            "  [WATCH]"
        }
    } else {
        ""
    };
    #[cfg(not(feature = "watch"))]
    let watch_str = "";

    let status_text = format!(
        " mdx  {}  {} lines  {} headings  {}:{}/{}  [{}{}]{}  [{}]{}{}",
        filename, line_count, heading_count, filename, current_line, line_count, mode_str, selection_str, toc_indicator, theme_str, prefix_str, watch_str
    );

    let status = Paragraph::new(Line::from(vec![Span::styled(
        status_text,
        Style::default()
            .fg(app.theme.status_bar_fg)
            .bg(app.theme.status_bar_bg)
            .add_modifier(Modifier::BOLD),
    )]));

    frame.render_widget(status, area);
}
