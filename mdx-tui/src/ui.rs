//! UI rendering

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
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
    use ratatui::text::Span;

    // Get the pane's view state
    let pane = match app.panes.panes.get(&pane_id) {
        Some(p) => p,
        None => return,
    };

    let scroll = pane.view.scroll_line;
    let cursor = pane.view.cursor_line;
    let is_focused = app.panes.focused == pane_id;

    // Get selection range if in visual line mode
    let selection_range = if pane.view.mode == crate::app::Mode::VisualLine {
        pane.view.selection.as_ref().map(|sel| sel.range())
    } else {
        None
    };

    let line_count = app.doc.line_count();

    // Determine if we're in a code block at the scroll position
    // by quickly scanning lines before the viewport
    let mut in_code_block = false;
    for line_idx in 0..scroll.min(line_count) {
        let line_text: String = app.doc.rope.line(line_idx).chunks().collect();
        if line_text.trim_end().starts_with("```") {
            in_code_block = !in_code_block;
        }
    }

    // Build only visible lines
    let mut styled_lines: Vec<Line> = Vec::new();
    let visible_end = (scroll + area.height as usize).min(line_count);

    for line_idx in scroll..visible_end {
        let mut line_spans: Vec<Span> = Vec::new();

        // Add line number
        let line_num_width = format!("{}", line_count).len().max(3);
        let line_num = format!("{:>width$} ", line_idx + 1, width = line_num_width);
        line_spans.push(Span::styled(line_num, Style::default().fg(Color::DarkGray)));

        // Add diff gutter
        #[cfg(feature = "git")]
        if app.config.git.diff {
            use mdx_core::diff::DiffMark;
            let gutter = match app.doc.diff_gutter.get(line_idx) {
                DiffMark::None => "  ",
                DiffMark::Added => "+ ",
                DiffMark::Modified => "~ ",
                DiffMark::DeletedAfter(_) => "▾ ",
            };
            let gutter_color = match app.doc.diff_gutter.get(line_idx) {
                DiffMark::None => Color::DarkGray,
                DiffMark::Added => Color::Green,
                DiffMark::Modified => Color::Yellow,
                DiffMark::DeletedAfter(_) => Color::Red,
            };
            line_spans.push(Span::styled(gutter, Style::default().fg(gutter_color)));
        } else {
            line_spans.push(Span::raw("  "));
        }
        #[cfg(not(feature = "git"))]
        line_spans.push(Span::raw("  "));

        // Get line text
        let line_text: String = if line_idx < line_count {
            app.doc.rope.line(line_idx).chunks().collect()
        } else {
            String::new()
        };

        // Remove trailing newline for styling
        let line_text = line_text.trim_end_matches('\n');

        // Check for code block markers
        if line_text.starts_with("```") {
            in_code_block = !in_code_block;
            line_spans.push(Span::styled(line_text.to_string(), app.theme.code));
        } else if in_code_block {
            // Inside code block - render as code
            line_spans.push(Span::styled(line_text.to_string(), app.theme.code));
        } else {
            // Apply markdown styling to the line
            line_spans.extend(style_markdown_line(line_text, &app.theme));
        }

        // Check if this line is selected or cursor
        let is_selected = if let Some((start, end)) = selection_range {
            line_idx >= start && line_idx <= end
        } else {
            false
        };

        let mut line = Line::from(line_spans);

        // Apply highlighting
        if is_focused && is_selected {
            line = line.style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::REVERSED));
        } else if is_focused && line_idx == cursor {
            line = line.style(Style::default().bg(app.theme.cursor_line_bg));
        }

        styled_lines.push(line);
    }

    // Add border to pane with focus highlight
    let border_style = if is_focused {
        Style::default().fg(app.theme.toc_active.bg.unwrap_or(Color::LightCyan))
    } else {
        Style::default().fg(app.theme.toc_border)
    };

    let paragraph = Paragraph::new(styled_lines)
        .block(Block::default().borders(Borders::ALL).border_style(border_style))
        .style(app.theme.base)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Style a single line of markdown text
fn style_markdown_line(line: &str, theme: &crate::theme::Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    // Check for horizontal rule
    let trimmed = line.trim();
    if (trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3)
        || (trimmed.chars().all(|c| c == '*') && trimmed.len() >= 3)
        || (trimmed.chars().all(|c| c == '_') && trimmed.len() >= 3)
    {
        spans.push(Span::styled(
            line.to_string(),
            Style::default().fg(Color::DarkGray),
        ));
        return spans;
    }

    // Check for table row (contains |)
    if line.contains('|') && (line.trim_start().starts_with('|') || line.contains(" | ")) {
        // Simple table rendering - split by | and style each cell
        let parts: Vec<&str> = line.split('|').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    "|".to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
            // Check if this is a separator row (contains only -, :, and spaces)
            let is_separator = part.trim().chars().all(|c| c == '-' || c == ':' || c == ' ');
            if is_separator && !part.trim().is_empty() {
                spans.push(Span::styled(
                    part.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            } else {
                // Style content within cell
                spans.extend(style_inline_markdown(part, theme.base, theme.code));
            }
        }
        return spans;
    }

    // Check for list item (unordered: -, *, +)
    let list_pattern = if let Some(rest) = line.trim_start().strip_prefix("- ") {
        Some(("- ", rest, line.len() - line.trim_start().len()))
    } else if let Some(rest) = line.trim_start().strip_prefix("* ") {
        Some(("* ", rest, line.len() - line.trim_start().len()))
    } else if let Some(rest) = line.trim_start().strip_prefix("+ ") {
        Some(("+ ", rest, line.len() - line.trim_start().len()))
    } else {
        // Check for ordered list (number followed by . or ))
        let trimmed_start = line.trim_start();
        if let Some(pos) = trimmed_start.find(|c| c == '.' || c == ')') {
            let prefix = &trimmed_start[..pos];
            if prefix.chars().all(|c| c.is_ascii_digit()) && trimmed_start.len() > pos + 1 {
                let rest = &trimmed_start[pos + 2..]; // Skip ". " or ") "
                let marker = &trimmed_start[..pos + 2];
                Some((marker, rest, line.len() - line.trim_start().len()))
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some((marker, content, indent)) = list_pattern {
        // Add indentation
        if indent > 0 {
            spans.push(Span::raw(" ".repeat(indent)));
        }
        // Add list marker with special color
        spans.push(Span::styled(
            marker.to_string(),
            Style::default().fg(Color::Yellow),
        ));
        // Style the rest as inline markdown
        spans.extend(style_inline_markdown(content, theme.base, theme.code));
        return spans;
    }

    // Check for heading
    let (is_heading, heading_level, content) = if let Some(stripped) = line.strip_prefix("# ") {
        (true, 1, stripped)
    } else if let Some(stripped) = line.strip_prefix("## ") {
        (true, 2, stripped)
    } else if let Some(stripped) = line.strip_prefix("### ") {
        (true, 3, stripped)
    } else if let Some(stripped) = line.strip_prefix("#### ") {
        (true, 4, stripped)
    } else if let Some(stripped) = line.strip_prefix("##### ") {
        (true, 5, stripped)
    } else if let Some(stripped) = line.strip_prefix("###### ") {
        (true, 6, stripped)
    } else {
        (false, 0, line)
    };

    // If it's a heading, show the ## prefix with heading style
    if is_heading {
        let prefix = &line[..(line.len() - content.len())];
        spans.push(Span::styled(
            prefix.to_string(),
            theme.heading[heading_level - 1],
        ));
    }

    // For headings or regular text, parse inline markdown
    let base_style = if is_heading && heading_level > 0 && heading_level <= 6 {
        theme.heading[heading_level - 1]
    } else {
        theme.base
    };

    spans.extend(style_inline_markdown(content, base_style, theme.code));

    // If we didn't get any spans, just return the raw text
    if spans.is_empty() {
        spans.push(Span::styled(line.to_string(), theme.base));
    }

    spans
}

/// Style inline markdown (bold, italic, code) within text
fn style_inline_markdown(text: &str, base_style: Style, code_style: Style) -> Vec<Span<'static>> {
    use pulldown_cmark::{Parser, Event, Tag, TagEnd};

    let mut spans = Vec::new();
    let parser = Parser::new(text);
    let mut in_bold = false;
    let mut in_italic = false;

    for event in parser {
        match event {
            Event::Start(Tag::Strong) => {
                in_bold = true;
            }
            Event::End(TagEnd::Strong) => {
                in_bold = false;
            }
            Event::Start(Tag::Emphasis) => {
                in_italic = true;
            }
            Event::End(TagEnd::Emphasis) => {
                in_italic = false;
            }
            Event::Text(content) => {
                let mut style = base_style;
                if in_bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if in_italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                spans.push(Span::styled(content.to_string(), style));
            }
            Event::Code(code) => {
                spans.push(Span::styled(code.to_string(), code_style));
            }
            _ => {}
        }
    }

    spans
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
            crate::app::Mode::Search => ("SEARCH", None),
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

    let search_str = if !app.search_query.is_empty() {
        if let Some(current_idx) = app.search_current_match {
            format!(
                "  /{} ({}/{})",
                app.search_query,
                current_idx + 1,
                app.search_matches.len()
            )
        } else {
            format!("  /{} (no matches)", app.search_query)
        }
    } else {
        String::new()
    };

    let status_text = format!(
        " mdx  {}  {} lines  {} headings  {}:{}/{}  [{}{}]{}  [{}]{}{}{}",
        filename, line_count, heading_count, filename, current_line, line_count, mode_str, selection_str, toc_indicator, theme_str, prefix_str, watch_str, search_str
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
