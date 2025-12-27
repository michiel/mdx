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
            Constraint::Min(1),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    // Render markdown content
    render_markdown(frame, app, chunks[0]);

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
                        .fg(Color::Black)
                        .bg(Color::LightYellow)
                )
            } else {
                Line::from(text.to_string())
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, area);
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

    let status_text = format!(
        " mdx  {}  {} lines  {} headings  {}:{}/{}  [{}]",
        filename, line_count, heading_count, filename, current_line, line_count, mode_str
    );

    let status = Paragraph::new(Line::from(vec![Span::styled(
        status_text,
        Style::default()
            .fg(Color::Black)
            .bg(Color::LightBlue)
            .add_modifier(Modifier::BOLD),
    )]));

    frame.render_widget(status, area);
}
