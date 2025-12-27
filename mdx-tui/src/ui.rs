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

    // For now, render as simple paragraph (will use tui-markdown in enhancement)
    let lines: Vec<Line> = content.lines().map(|l| Line::from(l.to_string())).collect();

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

    let status_text = format!(
        " mdx  {}  {} lines  {} headings  [press 'q' to quit]",
        filename, line_count, heading_count
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
