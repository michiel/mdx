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
    use pulldown_cmark::{Parser, Event, Tag, TagEnd};
    use ratatui::text::Span;

    // Get the pane's view state
    let pane = match app.panes.panes.get(&pane_id) {
        Some(p) => p,
        None => return,
    };

    // Get markdown content from document
    let content: String = app.doc.rope.chunks().collect();

    let scroll = pane.view.scroll_line;
    let cursor = pane.view.cursor_line;
    let is_focused = app.panes.focused == pane_id;

    // Get selection range if in visual line mode
    let selection_range = if pane.view.mode == crate::app::Mode::VisualLine {
        pane.view.selection.as_ref().map(|sel| sel.range())
    } else {
        None
    };

    // Parse markdown and convert to styled lines
    let parser = Parser::new(&content);
    let mut lines: Vec<(Line, usize)> = Vec::new(); // (Line, source_line)
    let mut current_line_spans: Vec<Span> = Vec::new();
    let mut source_line = 0;
    let mut in_code_block = false;
    let mut in_bold = false;
    let mut in_italic = false;
    let mut in_heading = false;
    let mut heading_level = 0;

    // Add diff gutter for first line
    #[cfg(feature = "git")]
    let first_gutter = if app.config.git.diff {
        use mdx_core::diff::DiffMark;
        match app.doc.diff_gutter.get(0) {
            DiffMark::None => "  ",
            DiffMark::Added => "+ ",
            DiffMark::Modified => "~ ",
            DiffMark::DeletedAfter(_) => "▾ ",
        }
    } else {
        "  "
    };
    #[cfg(not(feature = "git"))]
    let first_gutter = "  ";

    current_line_spans.push(Span::raw(first_gutter));

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    in_heading = true;
                    heading_level = level as usize;
                }
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                }
                Tag::Strong => {
                    in_bold = true;
                }
                Tag::Emphasis => {
                    in_italic = true;
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    in_heading = false;
                    heading_level = 0;
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                }
                TagEnd::Strong => {
                    in_bold = false;
                }
                TagEnd::Emphasis => {
                    in_italic = false;
                }
                TagEnd::Paragraph => {
                    // Finish current line
                    if !current_line_spans.is_empty() {
                        lines.push((Line::from(current_line_spans.clone()), source_line));
                        current_line_spans.clear();
                        source_line += 1;

                        // Add gutter for next line
                        #[cfg(feature = "git")]
                        if app.config.git.diff {
                            use mdx_core::diff::DiffMark;
                            let gutter = match app.doc.diff_gutter.get(source_line) {
                                DiffMark::None => "  ",
                                DiffMark::Added => "+ ",
                                DiffMark::Modified => "~ ",
                                DiffMark::DeletedAfter(_) => "▾ ",
                            };
                            current_line_spans.push(Span::raw(gutter));
                        } else {
                            current_line_spans.push(Span::raw("  "));
                        }
                        #[cfg(not(feature = "git"))]
                        current_line_spans.push(Span::raw("  "));
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                let mut style = app.theme.base;

                if in_heading && heading_level > 0 && heading_level <= 6 {
                    style = app.theme.heading[heading_level - 1];
                } else if in_code_block {
                    style = app.theme.code;
                } else {
                    if in_bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if in_italic {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                }

                // Handle newlines in text
                for (i, line_text) in text.split('\n').enumerate() {
                    if i > 0 {
                        lines.push((Line::from(current_line_spans.clone()), source_line));
                        current_line_spans.clear();
                        source_line += 1;

                        // Add gutter
                        #[cfg(feature = "git")]
                        if app.config.git.diff {
                            use mdx_core::diff::DiffMark;
                            let gutter = match app.doc.diff_gutter.get(source_line) {
                                DiffMark::None => "  ",
                                DiffMark::Added => "+ ",
                                DiffMark::Modified => "~ ",
                                DiffMark::DeletedAfter(_) => "▾ ",
                            };
                            current_line_spans.push(Span::raw(gutter));
                        } else {
                            current_line_spans.push(Span::raw("  "));
                        }
                        #[cfg(not(feature = "git"))]
                        current_line_spans.push(Span::raw("  "));
                    }
                    if !line_text.is_empty() {
                        current_line_spans.push(Span::styled(line_text.to_string(), style));
                    }
                }
            }
            Event::Code(code) => {
                current_line_spans.push(Span::styled(code.to_string(), app.theme.code));
            }
            Event::SoftBreak | Event::HardBreak => {
                lines.push((Line::from(current_line_spans.clone()), source_line));
                current_line_spans.clear();
                source_line += 1;

                // Add gutter
                #[cfg(feature = "git")]
                if app.config.git.diff {
                    use mdx_core::diff::DiffMark;
                    let gutter = match app.doc.diff_gutter.get(source_line) {
                        DiffMark::None => "  ",
                        DiffMark::Added => "+ ",
                        DiffMark::Modified => "~ ",
                        DiffMark::DeletedAfter(_) => "▾ ",
                    };
                    current_line_spans.push(Span::raw(gutter));
                } else {
                    current_line_spans.push(Span::raw("  "));
                }
                #[cfg(not(feature = "git"))]
                current_line_spans.push(Span::raw("  "));
            }
            _ => {}
        }
    }

    // Add any remaining spans
    if !current_line_spans.is_empty() {
        lines.push((Line::from(current_line_spans), source_line));
    }

    // Apply cursor and selection highlighting, then scrolling
    let visible_lines: Vec<Line> = lines
        .into_iter()
        .map(|(line, src_line)| {
            // Check if this source line is in selection
            let is_selected = if let Some((start, end)) = selection_range {
                src_line >= start && src_line <= end
            } else {
                false
            };

            // Apply highlighting based on state
            if is_focused && is_selected {
                // Selected line in visual line mode
                line.style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::REVERSED))
            } else if is_focused && src_line == cursor {
                // Cursor line when focused
                line.style(Style::default().bg(app.theme.cursor_line_bg))
            } else {
                // Normal line
                line
            }
        })
        .skip(scroll)
        .take(area.height as usize)
        .collect();

    // Add border to pane with focus highlight
    let border_style = if is_focused {
        Style::default().fg(app.theme.toc_active.bg.unwrap_or(Color::LightCyan))
    } else {
        Style::default().fg(app.theme.toc_border)
    };

    let paragraph = Paragraph::new(visible_lines)
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
