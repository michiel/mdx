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

    // Split main area if TOC is shown
    if app.show_toc {
        let toc_width = app.config.toc.width as u16;
        let main_chunks = if app.config.toc.side == mdx_core::config::TocSide::Left {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(toc_width), // TOC
                    Constraint::Min(1),            // Markdown
                ])
                .split(chunks[0])
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(1),            // Markdown
                    Constraint::Length(toc_width), // TOC
                ])
                .split(chunks[0])
        };

        // Render based on TOC position
        if app.config.toc.side == mdx_core::config::TocSide::Left {
            render_toc(frame, app, main_chunks[0]);
            render_markdown(frame, app, main_chunks[1]);
        } else {
            render_markdown(frame, app, main_chunks[0]);
            render_toc(frame, app, main_chunks[1]);
        }
    } else {
        // No TOC, full width markdown
        render_markdown(frame, app, chunks[0]);
    }

    // Render status bar
    render_status_bar(frame, app, chunks[1]);
}

fn render_markdown(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // Get markdown content from document
    let content: String = app.doc.rope.chunks().collect();

    // Convert to lines with cursor highlighting
    let scroll = app.view.scroll_line;
    let cursor = app.view.cursor_line;

    let lines: Vec<Line> = content
        .lines()
        .enumerate()
        .skip(scroll)
        .take(area.height as usize)
        .map(|(idx, text)| {
            // Highlight cursor line
            if idx == cursor {
                Line::from(text.to_string()).style(
                    Style::default()
                        .fg(app.theme.base.fg.unwrap_or(Color::White))
                        .bg(app.theme.cursor_line_bg)
                )
            } else {
                Line::from(text.to_string()).style(app.theme.base)
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
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
    let current_line = app.view.cursor_line + 1; // 1-based for display
    let mode_str = match app.view.mode {
        crate::app::Mode::Normal => "NORMAL",
        crate::app::Mode::VisualLine => "V-LINE",
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

    let status_text = format!(
        " mdx  {}  {} lines  {} headings  {}:{}/{}  [{}]{}  [{}]",
        filename, line_count, heading_count, filename, current_line, line_count, mode_str, toc_indicator, theme_str
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
