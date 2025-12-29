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

    // Render help popup if active
    if app.show_help {
        render_help_popup(frame, app);
    }

    // Render TOC dialog if active
    if app.show_toc_dialog {
        render_toc_dialog(frame, app);
    }
}

fn render_markdown(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect, pane_id: usize) {
    use ratatui::text::Span;

    // Split area for breadcrumb and content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Breadcrumb
            Constraint::Min(1),    // Content
        ])
        .split(area);

    let breadcrumb_area = chunks[0];
    let content_area = chunks[1];

    // Render breadcrumb
    render_breadcrumb(frame, app, breadcrumb_area, pane_id);

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

    // If in raw mode, render plain text without markdown processing
    if pane.view.show_raw {
        render_raw_text(frame, app, content_area, pane_id, scroll, cursor, is_focused, selection_range, line_count);
        return;
    }

    // Get search query for highlighting (clone to avoid borrow issues)
    let search_query = if !app.search_query.is_empty() {
        Some(app.search_query.clone())
    } else {
        None
    };

    // Determine if we're in a code block at the scroll position
    // by quickly scanning lines before the viewport
    let mut in_code_block = false;
    let mut code_block_lang = String::new();
    for line_idx in 0..scroll.min(line_count) {
        let line_text: String = app.doc.rope.line(line_idx).chunks().collect();
        if line_text.trim_end().starts_with("```") {
            if !in_code_block {
                // Opening fence - extract language
                let lang = line_text.trim_end()
                    .strip_prefix("```")
                    .unwrap_or("")
                    .trim();
                code_block_lang = if lang.is_empty() {
                    "plain".to_string()
                } else {
                    lang.to_string()
                };
            }
            in_code_block = !in_code_block;
            if !in_code_block {
                code_block_lang.clear();
            }
        }
    }

    // Calculate left margin width for line numbers and gutter
    let line_num_width = format!("{}", line_count).len().max(3);
    let gutter_width = 2; // Git gutter or spacing
    let left_margin_width = (line_num_width + 1 + gutter_width) as u16; // +1 for space after line number

    // Build only visible lines
    let mut styled_lines: Vec<Line> = Vec::new();
    let mut is_table_row_flags: Vec<bool> = Vec::new();
    // Account for borders (top and bottom borders take 2 lines)
    let content_height = content_area.height.saturating_sub(2) as usize;
    let visible_end = (scroll + content_height).min(line_count);
    let mut is_first_code_line = false;

    let mut line_idx = scroll;
    while line_idx < visible_end {
        let mut line_spans: Vec<Span> = Vec::new();

        // Get line text first to check if it's a fence
        let line_text: String = if line_idx < line_count {
            app.doc.rope.line(line_idx).chunks().collect()
        } else {
            String::new()
        };

        // Remove trailing newline for styling
        let line_text = line_text.trim_end_matches('\n');

        // Table detection: header row followed by a separator row
        if !in_code_block && line_idx + 1 < line_count {
            let next_line: String = app.doc.rope.line(line_idx + 1).chunks().collect();
            let next_line = next_line.trim_end_matches('\n');
            if is_table_row(line_text) && is_table_separator_row(next_line) {
                let (table_lines, consumed) = render_table_block(
                    app,
                    content_area,
                    line_idx,
                    visible_end,
                    line_count,
                    line_num_width,
                    is_focused,
                    cursor,
                    selection_range,
                    left_margin_width,
                    search_query.as_deref(),
                );

                for line in table_lines {
                    styled_lines.push(line);
                    is_table_row_flags.push(true);
                }

                line_idx = line_idx.saturating_add(consumed);
                continue;
            }
        }

        // Check for image rendering
        #[cfg(feature = "images")]
        if !in_code_block {
            use mdx_core::config::ImageEnabled;
            let backend = crate::image_backend::select_backend(app.config.images.backend);
            let has_backend = !matches!(backend, mdx_core::config::ImageBackend::None);

            let should_render_images = match app.config.images.enabled {
                ImageEnabled::Always => true,
                ImageEnabled::Auto => has_backend,
                ImageEnabled::Never => false,
            };

            if should_render_images {
                // Check if there's an image on this line (clone to avoid borrow issues)
                let image_opt = app.doc.images.iter()
                    .find(|img| img.source_line == line_idx)
                    .cloned();

                if let Some(image) = image_opt {
                    let (image_lines, _consumed) = render_image(
                        app,
                        content_area,
                        line_idx,
                        &image,
                        line_num_width,
                        is_focused,
                        cursor,
                        selection_range,
                        left_margin_width,
                        backend,
                    );

                    for line in image_lines {
                        styled_lines.push(line);
                        is_table_row_flags.push(false);
                    }

                    line_idx += 1;
                    continue;
                }
            }
        }

        // Track if this is a table row (before styling splits the pipes)
        let is_table_row = line_text.contains('|');

        // Check for code block fence markers - skip rendering them
        if line_text.starts_with("```") {
            if !in_code_block {
                // Opening fence - extract language
                let lang = line_text.strip_prefix("```").unwrap_or("").trim();
                code_block_lang = if lang.is_empty() {
                    "plain".to_string()
                } else {
                    lang.to_string()
                };
                is_first_code_line = true;
            } else {
                // Closing fence - clear language
                code_block_lang.clear();
            }
            in_code_block = !in_code_block;
            // Skip this line entirely (don't render fence markers)
            line_idx += 1;
            continue;
        }

        // Add line number
        let line_num = format!("{:>width$} ", line_idx + 1, width = line_num_width);
        let line_num_color = if is_focused && line_idx == cursor {
            Color::White
        } else {
            Color::DarkGray
        };
        line_spans.push(Span::styled(line_num, Style::default().fg(line_num_color)));

        // Add diff gutter with vertical bars
        #[cfg(feature = "git")]
        if app.config.git.diff {
            use mdx_core::diff::DiffMark;
            let mark = app.doc.diff_gutter.get(line_idx);
            let gutter = match mark {
                DiffMark::None => "  ",
                DiffMark::Added => "│ ",
                DiffMark::Modified => "│ ",
                DiffMark::DeletedAfter(_) => "│ ",
            };
            let gutter_color = match mark {
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

        // Track if this is a code block line for background styling
        let is_code_block_line;

        if in_code_block {
            // Inside code block - render with syntax highlighting and different background
            line_spans.extend(render_code_line(line_text, &app.theme, search_query.as_deref()));
            is_code_block_line = true;
        } else {
            // Apply markdown styling to the line
            line_spans.extend(style_markdown_line(line_text, &app.theme, search_query.as_deref()));
            is_code_block_line = false;
        }

        // For code blocks, pad to full viewport width and add language label on first line
        if is_code_block_line {
            let line_visual_width: usize = line_spans.iter()
                .map(|span| span.content.chars().count())
                .sum();
            // Calculate available width (content_area width - borders)
            let available_width = content_area.width.saturating_sub(2) as usize;

            if is_first_code_line && !code_block_lang.is_empty() {
                // Add language label on the right side of the first line
                let lang_label = format!(" {} ", code_block_lang);
                let lang_width = lang_label.chars().count();
                let remaining_width = available_width.saturating_sub(line_visual_width);

                if remaining_width > lang_width {
                    // Add padding before the label
                    let padding_before = " ".repeat(remaining_width - lang_width);
                    line_spans.push(Span::styled(
                        padding_before,
                        Style::default().bg(Color::Rgb(40, 44, 52))
                    ));
                    // Add the language label
                    line_spans.push(Span::styled(
                        lang_label,
                        Style::default()
                            .fg(Color::Rgb(120, 120, 120))
                            .bg(Color::Rgb(40, 44, 52))
                    ));
                } else {
                    // Not enough space for label, just pad
                    let padding = " ".repeat(remaining_width);
                    line_spans.push(Span::styled(
                        padding,
                        Style::default().bg(Color::Rgb(40, 44, 52))
                    ));
                }
                is_first_code_line = false;
            } else if line_visual_width < available_width {
                // Regular code block line - just pad
                let padding = " ".repeat(available_width - line_visual_width);
                line_spans.push(Span::styled(
                    padding,
                    Style::default().bg(Color::Rgb(40, 44, 52))
                ));
            }
        }

        // Check if this line is selected or cursor
        let is_selected = if let Some((start, end)) = selection_range {
            line_idx >= start && line_idx <= end
        } else {
            false
        };

        let mut line = Line::from(line_spans);

        // Apply highlighting - priority order: selection > cursor > code block
        if is_focused && is_selected {
            line = line.style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::REVERSED));
        } else if is_focused && line_idx == cursor {
            line = line.style(Style::default().bg(app.theme.cursor_line_bg));
        } else if is_code_block_line {
            // Apply code block background to entire line
            line = line.style(Style::default().bg(Color::Rgb(40, 44, 52)));
        }

        styled_lines.push(line);
        is_table_row_flags.push(is_table_row);
        line_idx += 1;
    }

    // Add border to pane with focus highlight
    let border_style = if is_focused {
        Style::default().fg(app.theme.toc_active.bg.unwrap_or(Color::LightCyan))
    } else {
        Style::default().fg(app.theme.toc_border)
    };

    // Manual wrapping to indent continuation lines
    let available_width = content_area.width.saturating_sub(2) as usize; // -2 for borders
    let content_start = left_margin_width as usize;
    let content_width = available_width.saturating_sub(content_start);

    let mut wrapped_lines: Vec<Line> = Vec::new();
    let indent_str = " ".repeat(content_start);

    for (idx, line) in styled_lines.into_iter().enumerate() {
        // Check if this is a table row - if so, don't wrap it
        let is_table_row = is_table_row_flags.get(idx).copied().unwrap_or(false);

        if is_table_row {
            // Don't wrap table rows, just add them as-is
            wrapped_lines.push(line);
            continue;
        }

        // Calculate the visual width of the line
        let mut current_width = 0;
        let mut current_line_spans: Vec<Span> = Vec::new();
        let mut first_segment = true;

        for span in line.spans {
            let span_text = span.content.to_string();
            let span_width = span_text.chars().count();

            if current_width + span_width <= available_width {
                // Fits on current line
                current_line_spans.push(span);
                current_width += span_width;
            } else {
                // Need to wrap
                if !current_line_spans.is_empty() {
                    wrapped_lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                    current_width = 0;
                    first_segment = false;
                }

                // Add indentation for continuation lines
                if !first_segment {
                    current_line_spans.push(Span::raw(indent_str.clone()));
                    current_width = content_start;
                }

                // Word-aware wrapping within the span
                let mut remaining = span_text.as_str();
                while !remaining.is_empty() {
                    let available = if first_segment {
                        available_width - current_width
                    } else {
                        content_width
                    };

                    let remaining_len = remaining.chars().count();

                    if remaining_len <= available {
                        // Entire remaining text fits
                        current_line_spans.push(Span::styled(
                            remaining.to_string(),
                            span.style
                        ));
                        current_width += remaining_len;
                        break;
                    } else {
                        // Need to wrap - find word boundary
                        let mut split_at = 0;
                        let mut last_word_end = None;
                        let mut char_count = 0;

                        for (byte_idx, ch) in remaining.char_indices() {
                            if char_count >= available {
                                break;
                            }

                            // Track word boundaries (space, tab, or punctuation followed by space)
                            if ch.is_whitespace() {
                                last_word_end = Some(byte_idx);
                            }

                            split_at = byte_idx + ch.len_utf8();
                            char_count += 1;
                        }

                        // Prefer splitting at word boundary if we found one
                        let split_pos = if let Some(word_end) = last_word_end {
                            // Split at the word boundary, but skip the trailing whitespace
                            let after_space = remaining[word_end..].char_indices()
                                .skip_while(|(_, c)| c.is_whitespace())
                                .next()
                                .map(|(i, _)| word_end + i)
                                .unwrap_or(word_end);
                            (word_end, after_space)
                        } else {
                            // No word boundary found, fall back to character split
                            // But ensure we split at least one character
                            if split_at == 0 && !remaining.is_empty() {
                                let first_char_len = remaining.chars().next().unwrap().len_utf8();
                                split_at = first_char_len;
                            }
                            (split_at, split_at)
                        };

                        let (chunk, rest) = remaining.split_at(split_pos.0);
                        let rest = &rest[split_pos.1 - split_pos.0..];

                        if !chunk.is_empty() {
                            current_line_spans.push(Span::styled(
                                chunk.to_string(),
                                span.style
                            ));
                        }

                        wrapped_lines.push(Line::from(current_line_spans.clone()));
                        current_line_spans.clear();
                        current_line_spans.push(Span::raw(indent_str.clone()));
                        current_width = content_start;
                        remaining = rest;
                        first_segment = false;
                    }
                }
            }
        }

        if !current_line_spans.is_empty() {
            wrapped_lines.push(Line::from(current_line_spans));
        }
    }

    let paragraph = Paragraph::new(wrapped_lines)
        .block(Block::default().borders(Borders::ALL).border_style(border_style))
        .style(app.theme.base);

    frame.render_widget(paragraph, content_area);
}

/// Render breadcrumb bar with heading hierarchy and git status
fn render_breadcrumb(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, pane_id: usize) {
    use ratatui::text::Span;

    let is_focused = app.panes.focused == pane_id;
    let breadcrumbs = app.get_breadcrumb_path(pane_id);

    if breadcrumbs.is_empty() {
        // No breadcrumb - just render empty line
        let empty_line = Line::from(vec![]);
        frame.render_widget(Paragraph::new(vec![empty_line]), area);
        return;
    }

    // Build breadcrumb spans
    let mut spans = Vec::new();

    // Limit breadcrumb to 50% of viewport width
    let max_breadcrumb_width = (area.width / 2) as usize;
    let mut current_width = 0;

    // Add breadcrumb items with separators
    for (idx, crumb) in breadcrumbs.iter().enumerate() {
        if idx > 0 {
            // Add separator
            let sep = " › ";
            if current_width + sep.len() >= max_breadcrumb_width {
                spans.push(Span::styled("…", Style::default().fg(Color::DarkGray)));
                break;
            }
            spans.push(Span::styled(sep, Style::default().fg(Color::DarkGray)));
            current_width += sep.len();
        }

        // Truncate crumb if needed
        let crumb_text = if current_width + crumb.len() > max_breadcrumb_width {
            let available = max_breadcrumb_width.saturating_sub(current_width).saturating_sub(1);
            if available > 3 {
                format!("{}…", &crumb.chars().take(available - 1).collect::<String>())
            } else {
                "…".to_string()
            }
        } else {
            crumb.clone()
        };

        current_width += crumb_text.len();

        // Style the breadcrumb
        let crumb_style = if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(crumb_text.clone(), crumb_style));

        if current_width >= max_breadcrumb_width {
            break;
        }
    }

    // Add git status indicator if available
    #[cfg(feature = "git")]
    if let Some(status) = app.get_git_status() {
        let (status_text, status_color) = match status {
            "new" => ("│ new", Color::Green),
            "modified" => ("│ modified", Color::Yellow),
            "deleted" => ("│ deleted", Color::Red),
            _ => ("│", Color::DarkGray),
        };

        // Add spacing before status
        let padding_width = area.width.saturating_sub(current_width as u16 + status_text.len() as u16 + 2);
        if padding_width > 0 {
            spans.push(Span::raw(" ".repeat(padding_width as usize)));
        }

        spans.push(Span::styled(status_text, Style::default().fg(status_color)));
    }

    let breadcrumb_line = Line::from(spans);
    frame.render_widget(Paragraph::new(vec![breadcrumb_line]), area);
}

/// Render raw text without markdown processing
fn render_raw_text(
    frame: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    _pane_id: usize,
    scroll: usize,
    cursor: usize,
    is_focused: bool,
    selection_range: Option<(usize, usize)>,
    line_count: usize,
) {
    use ratatui::text::Span;

    // Calculate left margin width for line numbers and gutter
    let line_num_width = format!("{}", line_count).len().max(3);
    let _gutter_width = 2; // Git gutter or spacing

    // Build only visible lines
    let mut lines: Vec<Line> = Vec::new();
    let content_height = area.height.saturating_sub(2) as usize;
    let visible_end = (scroll + content_height).min(line_count);

    for line_idx in scroll..visible_end {
        let mut line_spans: Vec<Span> = Vec::new();

        // Get line text
        let line_text: String = if line_idx < line_count {
            app.doc.rope.line(line_idx).chunks().collect()
        } else {
            String::new()
        };

        // Remove trailing newline
        let line_text = line_text.trim_end_matches('\n');

        // Add line number
        let line_num = format!("{:>width$} ", line_idx + 1, width = line_num_width);
        let line_num_color = if is_focused && line_idx == cursor {
            Color::White
        } else {
            Color::DarkGray
        };
        line_spans.push(Span::styled(line_num, Style::default().fg(line_num_color)));

        // Add diff gutter with vertical bars
        #[cfg(feature = "git")]
        if app.config.git.diff {
            use mdx_core::diff::DiffMark;
            let gutter = match app.doc.diff_gutter.get(line_idx) {
                DiffMark::None => "  ",
                DiffMark::Added => "│ ",
                DiffMark::Modified => "│ ",
                DiffMark::DeletedAfter(_) => "│ ",
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

        // Add raw text content
        line_spans.push(Span::styled(
            line_text.to_string(),
            app.theme.base,
        ));

        // Check if this line is selected or cursor
        let is_selected = if let Some((start, end)) = selection_range {
            line_idx >= start && line_idx <= end
        } else {
            false
        };

        let mut line = Line::from(line_spans);

        // Apply highlighting - priority order: selection > cursor
        if is_focused && is_selected {
            line = line.style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::REVERSED));
        } else if is_focused && line_idx == cursor {
            line = line.style(Style::default().bg(app.theme.cursor_line_bg));
        }

        lines.push(line);
    }

    // Create border style
    let border_style = if is_focused {
        Style::default().fg(app.theme.toc_active.bg.unwrap_or(Color::LightCyan))
    } else {
        Style::default().fg(app.theme.toc_border)
    };

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).border_style(border_style).title(" Raw "))
        .style(app.theme.base);

    frame.render_widget(paragraph, area);
}

/// Render a code block line with syntax highlighting
fn render_code_line(text: &str, theme: &crate::theme::Theme, search_query: Option<&str>) -> Vec<Span<'static>> {
    // Code block background color
    let code_bg = Color::Rgb(40, 44, 52); // Slightly darker background for code

    // Define syntax highlighting colors
    let keyword_color = Color::Rgb(198, 120, 221); // Purple for keywords
    let string_color = Color::Rgb(152, 195, 121); // Green for strings
    let comment_color = Color::Rgb(92, 99, 112);  // Gray for comments
    let number_color = Color::Rgb(209, 154, 102); // Orange for numbers
    let function_color = Color::Rgb(97, 175, 239); // Blue for functions

    // Common keywords across languages
    let keywords = [
        "fn", "func", "function", "def", "class", "struct", "enum", "impl", "trait",
        "let", "const", "var", "mut", "pub", "priv", "private", "public", "protected",
        "if", "else", "match", "switch", "case", "for", "while", "loop", "break", "continue",
        "return", "yield", "async", "await", "import", "export", "from", "use",
        "type", "interface", "extends", "implements", "new", "this", "self", "super",
    ];

    let mut spans = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();

    while i < chars.len() {
        // Check for comments
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            // Single-line comment - rest of line
            let comment: String = chars[i..].iter().collect();
            spans.push(Span::styled(
                comment,
                Style::default().fg(comment_color).bg(code_bg),
            ));
            break;
        }

        // Check for strings
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 2; // Skip escaped character
                } else {
                    i += 1;
                }
            }
            if i < chars.len() {
                i += 1; // Include closing quote
            }
            let string: String = chars[start..i].iter().collect();
            spans.push(Span::styled(
                string,
                Style::default().fg(string_color).bg(code_bg),
            ));
            continue;
        }

        // Check for numbers
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_') {
                i += 1;
            }
            let number: String = chars[start..i].iter().collect();
            spans.push(Span::styled(
                number,
                Style::default().fg(number_color).bg(code_bg),
            ));
            continue;
        }

        // Check for keywords and identifiers
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();

            // Check if it's a keyword
            if keywords.contains(&word.as_str()) {
                spans.push(Span::styled(
                    word,
                    Style::default().fg(keyword_color).bg(code_bg).add_modifier(Modifier::BOLD),
                ));
            } else {
                // Regular identifier - check if followed by '(' for function
                let is_function = i < chars.len() && chars[i] == '(';
                let color = if is_function { function_color } else { theme.code.fg.unwrap_or(Color::White) };
                spans.push(Span::styled(
                    word,
                    Style::default().fg(color).bg(code_bg),
                ));
            }
            continue;
        }

        // Regular character
        let ch = chars[i].to_string();
        spans.push(Span::styled(
            ch,
            Style::default().fg(theme.code.fg.unwrap_or(Color::White)).bg(code_bg),
        ));
        i += 1;
    }

    // TODO: Apply search highlighting on top of syntax highlighting
    // This is complex as it requires re-parsing with highlight preservation
    let _ = search_query;

    spans
}

/// Highlight text matches within a string
fn highlight_text_matches(text: &str, query: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();

    let mut last_end = 0;

    for (idx, _) in lower_text.match_indices(&lower_query) {
        // Add text before match
        if idx > last_end {
            spans.push(Span::styled(text[last_end..idx].to_string(), base_style));
        }

        // Add highlighted match
        spans.push(Span::styled(
            text[idx..idx + query.len()].to_string(),
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));

        last_end = idx + query.len();
    }

    // Add remaining text
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    // If no matches, return original text
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }

    spans
}

fn is_table_row(line: &str) -> bool {
    !line.trim().is_empty() && line.contains('|')
}

fn split_table_cells(line: &str) -> Vec<String> {
    let mut trimmed = line.trim();
    if trimmed.starts_with('|') {
        trimmed = &trimmed[1..];
    }
    if trimmed.ends_with('|') && trimmed.len() > 1 {
        trimmed = &trimmed[..trimmed.len() - 1];
    }
    trimmed
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn is_table_separator_row(line: &str) -> bool {
    if !is_table_row(line) {
        return false;
    }

    let cells = split_table_cells(line);
    if cells.is_empty() {
        return false;
    }

    for cell in cells {
        let trimmed = cell.trim();
        if trimmed.is_empty() {
            return false;
        }
        if !trimmed.chars().all(|c| c == '-' || c == ':' || c == ' ') {
            return false;
        }
        let dash_count = trimmed.chars().filter(|c| *c == '-').count();
        if dash_count < 3 {
            return false;
        }
    }

    true
}

fn wrap_cell_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_len = 0;

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if current_len == 0 {
            current.push_str(word);
            current_len = word_len;
        } else if current_len + 1 + word_len <= width {
            current.push(' ');
            current.push_str(word);
            current_len += 1 + word_len;
        } else {
            lines.push(current);
            current = word.to_string();
            current_len = word_len;
        }
    }

    if current.is_empty() {
        lines.push(String::new());
    } else {
        lines.push(current);
    }

    let mut wrapped: Vec<String> = Vec::new();
    for line in lines {
        let mut start = 0;
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            wrapped.push(String::new());
            continue;
        }
        while start < chars.len() {
            let end = (start + width).min(chars.len());
            wrapped.push(chars[start..end].iter().collect());
            start = end;
        }
    }

    wrapped
}

fn compute_table_widths(rows: &[Vec<String>], content_width: usize) -> Vec<usize> {
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return Vec::new();
    }

    let mut widths = vec![0usize; col_count];
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            let width = cell.chars().count();
            if width > widths[idx] {
                widths[idx] = width;
            }
        }
    }

    let separator_overhead = 1 + col_count * 3;
    let available = content_width.saturating_sub(separator_overhead);
    if available == 0 {
        return vec![1; col_count];
    }

    let min_width = if available < col_count * 3 { 1 } else { 3 };
    for width in widths.iter_mut() {
        if *width < min_width {
            *width = min_width;
        }
    }

    let mut total: usize = widths.iter().sum();
    if total > available {
        while total > available {
            let mut max_idx = None;
            let mut max_width = 0;
            for (idx, width) in widths.iter().enumerate() {
                if *width > max_width && *width > min_width {
                    max_width = *width;
                    max_idx = Some(idx);
                }
            }

            if let Some(idx) = max_idx {
                widths[idx] -= 1;
                total -= 1;
            } else {
                break;
            }
        }
    }

    widths
}

fn build_table_separator_cell(width: usize, raw: &str) -> String {
    if width == 0 {
        return String::new();
    }

    let trimmed = raw.trim();
    let mut cell: Vec<char> = vec!['-'; width];
    if trimmed.starts_with(':') {
        cell[0] = ':';
    }
    if trimmed.ends_with(':') {
        cell[width - 1] = ':';
    }
    cell.iter().collect()
}

fn spans_visual_width(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

fn render_table_block(
    app: &App,
    area: ratatui::layout::Rect,
    start_idx: usize,
    visible_end: usize,
    line_count: usize,
    line_num_width: usize,
    is_focused: bool,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
    left_margin_width: u16,
    search_query: Option<&str>,
) -> (Vec<Line<'static>>, usize) {
    let mut table_rows: Vec<(usize, String)> = Vec::new();
    let mut idx = start_idx;
    while idx < line_count {
        let line_text: String = app.doc.rope.line(idx).chunks().collect();
        let line_text = line_text.trim_end_matches('\n').to_string();
        if !is_table_row(&line_text) {
            break;
        }
        table_rows.push((idx, line_text));
        idx += 1;
    }

    let table_rows_len = table_rows.len();
    let consumed = visible_end.saturating_sub(start_idx).min(table_rows_len);

    let mut cell_rows: Vec<Vec<String>> = Vec::new();
    for (_, row_text) in &table_rows {
        cell_rows.push(split_table_cells(row_text));
    }

    let content_width = area.width.saturating_sub(2) as usize;
    let content_width = content_width.saturating_sub(left_margin_width as usize);
    let widths = compute_table_widths(&cell_rows, content_width);

    let mut rendered: Vec<Line> = Vec::new();
    let indent_str = " ".repeat(left_margin_width as usize);

    for (row_idx, (source_idx, row_text)) in table_rows.iter().enumerate().take(consumed) {
        let cells = split_table_cells(row_text);
        let is_separator = row_idx == 1 && is_table_separator_row(row_text);

        let mut padded_cells = cells.clone();
        while padded_cells.len() < widths.len() {
            padded_cells.push(String::new());
        }

        let mut wrapped_cells: Vec<Vec<String>> = Vec::new();
        if !is_separator {
            for (cell, width) in padded_cells.iter().zip(widths.iter()) {
                wrapped_cells.push(wrap_cell_text(cell, *width));
            }
        }

        let row_height = if is_separator {
            1
        } else {
            wrapped_cells.iter().map(|c| c.len()).max().unwrap_or(1)
        };

        for line_offset in 0..row_height {
            let mut line_spans: Vec<Span> = Vec::new();

            if line_offset == 0 {
                let line_num = format!("{:>width$} ", source_idx + 1, width = line_num_width);
                let line_num_color = if is_focused && *source_idx == cursor {
                    Color::White
                } else {
                    Color::DarkGray
                };
                line_spans.push(Span::styled(line_num, Style::default().fg(line_num_color)));

                #[cfg(feature = "git")]
                if app.config.git.diff {
                    use mdx_core::diff::DiffMark;
                    let gutter = match app.doc.diff_gutter.get(*source_idx) {
                        DiffMark::None => "  ",
                        DiffMark::Added => "│ ",
                        DiffMark::Modified => "│ ",
                        DiffMark::DeletedAfter(_) => "│ ",
                    };
                    let gutter_color = match app.doc.diff_gutter.get(*source_idx) {
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
            } else {
                line_spans.push(Span::raw(indent_str.clone()));
            }

            line_spans.push(Span::styled("|".to_string(), Style::default().fg(Color::Cyan)));

            for (col_idx, width) in widths.iter().enumerate() {
                line_spans.push(Span::raw(" ".to_string()));

                if is_separator {
                    let cell_text = build_table_separator_cell(*width, &padded_cells[col_idx]);
                    line_spans.push(Span::styled(
                        cell_text,
                        Style::default().fg(Color::DarkGray),
                    ));
                } else {
                    let cell_line = wrapped_cells[col_idx].get(line_offset).map(String::as_str).unwrap_or("");
                    let mut cell_spans = style_inline_markdown(
                        cell_line,
                        app.theme.base,
                        app.theme.code,
                        search_query.as_deref(),
                    );

                    let cell_width = spans_visual_width(&cell_spans);
                    if cell_width < *width {
                        let padding = " ".repeat(*width - cell_width);
                        cell_spans.push(Span::styled(padding, app.theme.base));
                    }
                    line_spans.extend(cell_spans);
                }

                line_spans.push(Span::raw(" ".to_string()));
                line_spans.push(Span::styled("|".to_string(), Style::default().fg(Color::Cyan)));
            }

            let mut line = Line::from(line_spans);

            let is_selected = if let Some((start, end)) = selection_range {
                *source_idx >= start && *source_idx <= end
            } else {
                false
            };

            if is_focused && is_selected {
                line = line.style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::REVERSED));
            } else if is_focused && *source_idx == cursor {
                line = line.style(Style::default().bg(app.theme.cursor_line_bg));
            }

            rendered.push(line);
        }
    }

    (rendered, consumed)
}

/// Style a single line of markdown text
fn style_markdown_line(line: &str, theme: &crate::theme::Theme, search_query: Option<&str>) -> Vec<Span<'static>> {
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
    if line.contains('|') {
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
                spans.extend(style_inline_markdown(part, theme.base, theme.code, search_query));
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
        spans.extend(style_inline_markdown(content, theme.base, theme.code, search_query));
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

    spans.extend(style_inline_markdown(content, base_style, theme.code, search_query));

    // If we didn't get any spans, just return the raw text
    if spans.is_empty() {
        if let Some(query) = search_query {
            spans.extend(highlight_text_matches(line, query, theme.base));
        } else {
            spans.push(Span::styled(line.to_string(), theme.base));
        }
    }

    spans
}

/// Style inline markdown (bold, italic, code) within text
fn style_inline_markdown(text: &str, base_style: Style, code_style: Style, search_query: Option<&str>) -> Vec<Span<'static>> {
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
                    // Make bold text bright yellow for better visibility
                    style = Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::BOLD);
                }
                if in_italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }

                // Apply search highlighting if query present
                if let Some(query) = search_query {
                    spans.extend(highlight_text_matches(&content, query, style));
                } else {
                    spans.push(Span::styled(content.to_string(), style));
                }
            }
            Event::Code(code) => {
                // Apply search highlighting to code if query present
                if let Some(query) = search_query {
                    spans.extend(highlight_text_matches(&code, query, code_style));
                } else {
                    spans.push(Span::styled(code.to_string(), code_style));
                }
            }
            _ => {}
        }
    }

    spans
}

fn render_toc(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    // Get current heading index to highlight
    let current_heading = app.current_heading_index();

    // Calculate visible TOC height (account for borders)
    let toc_height = area.height.saturating_sub(2) as usize;
    let scroll = app.toc_scroll;

    // Build visible TOC lines with indentation based on heading level
    let toc_lines: Vec<Line> = app
        .doc
        .headings
        .iter()
        .enumerate()
        .skip(scroll)
        .take(toc_height)
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
    // Check if we're in search mode
    let in_search_mode = if let Some(pane) = app.panes.focused_pane() {
        pane.view.mode == crate::app::Mode::Search
    } else {
        false
    };

    // In search mode, show search input
    if in_search_mode {
        let search_prompt = if !app.search_matches.is_empty() {
            if let Some(current_idx) = app.search_current_match {
                format!("/{} [{}/{}] ", app.search_query, current_idx + 1, app.search_matches.len())
            } else {
                format!("/{} ", app.search_query)
            }
        } else if !app.search_query.is_empty() {
            format!("/{} [no matches] ", app.search_query)
        } else {
            "/".to_string()
        };

        let status = Paragraph::new(Line::from(vec![Span::styled(
            search_prompt,
            Style::default()
                .fg(app.theme.status_bar_fg)
                .bg(app.theme.status_bar_bg)
                .add_modifier(Modifier::BOLD),
        )]));

        frame.render_widget(status, area);
        return;
    }

    // Normal status bar
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

fn render_help_popup(frame: &mut Frame, _app: &App) {
    use ratatui::widgets::{Clear, Paragraph};

    // Create a centered popup area
    let area = frame.area();
    let popup_width = 70.min(area.width.saturating_sub(4));
    let popup_height = 33.min(area.height.saturating_sub(4));

    let popup_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(popup_width)) / 2,
        y: (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    // Help text content
    let help_lines = vec![
        Line::from(vec![Span::styled(
            "MDX - Keyboard Commands",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Navigation", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  j/k, ↓/↑          Move cursor down/up"),
        Line::from("  Ctrl+d/u          Scroll half page down/up"),
        Line::from("  Space, PgDn       Scroll full page down"),
        Line::from("  PgUp              Scroll full page up"),
        Line::from("  g, Home           Go to top"),
        Line::from("  G, End            Go to bottom"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Search", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  /                 Start search"),
        Line::from("  n                 Next match"),
        Line::from("  N                 Previous match"),
        Line::from("  Esc               Cancel search"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Visual Mode", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  V                 Enter visual line mode"),
        Line::from("  Y                 Yank (copy) selected lines"),
        Line::from("  Esc               Exit visual mode"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Panes", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  Ctrl+w s          Split horizontally"),
        Line::from("  Ctrl+w v          Split vertically"),
        Line::from("  Ctrl+w hjkl/↑↓←→  Move focus between panes"),
        Line::from("  Ctrl+↑↓←→         Move focus between panes"),
        Line::from(""),
        Line::from(vec![
            Span::styled("Other", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from("  t                 Toggle TOC sidebar"),
        Line::from("  T                 Open TOC dialog (full screen)"),
        Line::from("  M                 Toggle theme (dark/light)"),
        Line::from("  e                 Open in $EDITOR"),
        Line::from("  r                 Toggle raw/rendered mode"),
        Line::from("  R                 Reload document"),
        Line::from("  ?                 Toggle this help"),
        Line::from("  q                 Close pane (quit if last)"),
        Line::from("  Ctrl+C            Force quit"),
    ];

    // Clear the background
    frame.render_widget(Clear, popup_area);

    // Render the popup
    let popup = Paragraph::new(help_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Help - Press ? or Esc to close ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .style(Style::default().bg(Color::Rgb(30, 34, 42)));

    frame.render_widget(popup, popup_area);
}

fn render_toc_dialog(frame: &mut Frame, app: &App) {
    use ratatui::widgets::Clear;

    // Create a full-screen popup area with small margins
    let area = frame.area();
    let popup_width = area.width.saturating_sub(4);
    let popup_height = area.height.saturating_sub(4);

    let popup_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(popup_width)) / 2,
        y: (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    // Calculate visible TOC height (account for borders and title)
    let toc_height = popup_height.saturating_sub(2) as usize;
    let scroll = app.toc_dialog_scroll;

    // Build visible TOC lines with indentation based on heading level
    let toc_lines: Vec<Line> = app
        .doc
        .headings
        .iter()
        .enumerate()
        .skip(scroll)
        .take(toc_height)
        .map(|(idx, heading)| {
            // Indent based on level (2 spaces per level, starting from level 1)
            let indent = "  ".repeat((heading.level as usize).saturating_sub(1));
            let text = format!("{}{}", indent, heading.text);

            // Highlight selected item
            if idx == app.toc_dialog_selected {
                Line::from(text).style(
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::from(text).style(Style::default().fg(Color::White))
            }
        })
        .collect();

    // Clear the background
    frame.render_widget(Clear, popup_area);

    // Render the TOC dialog
    let popup = Paragraph::new(toc_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Table of Contents - j/k to navigate, Enter to jump, T/Esc to close ")
                .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        )
        .style(Style::default().bg(Color::Rgb(30, 34, 42)));

    frame.render_widget(popup, popup_area);
}

/// Render image (actual image or placeholder)
#[cfg(feature = "images")]
fn render_image(
    app: &mut App,
    content_area: ratatui::layout::Rect,
    source_line: usize,
    image: &mdx_core::image::ImageNode,
    line_num_width: usize,
    is_focused: bool,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
    left_margin_width: u16,
    backend: mdx_core::config::ImageBackend,
) -> (Vec<Line<'static>>, usize) {
    // Try to resolve and load the image
    let image_result = try_load_image(app, image, content_area);

    match image_result {
        Ok(Some(decoded)) => {
            // Successfully loaded - render based on backend
            render_decoded_image(
                decoded,
                content_area,
                source_line,
                image,
                line_num_width,
                is_focused,
                cursor,
                selection_range,
                left_margin_width,
                backend,
            )
        }
        _ => {
            // Failed to load or unsupported - show placeholder
            render_image_placeholder(
                app,
                content_area,
                source_line,
                image,
                line_num_width,
                is_focused,
                cursor,
                selection_range,
                left_margin_width,
            )
        }
    }
}

/// Try to load an image from cache or disk/URL
#[cfg(feature = "images")]
fn try_load_image(
    app: &mut App,
    image: &mdx_core::image::ImageNode,
    content_area: ratatui::layout::Rect,
) -> anyhow::Result<Option<crate::image_cache::DecodedImage>> {
    use mdx_core::image::ImageSource;

    // Resolve image source
    let source = image.resolve(&app.doc.path);

    let source = match source {
        Some(s) => s,
        None => return Ok(None),
    };

    // Calculate max dimensions based on terminal size
    let content_width = content_area.width.saturating_sub(2) as u32;
    let content_height = content_area.height.saturating_sub(2) as u32;
    let max_width = (content_width * app.config.images.max_width_percent as u32) / 100;
    let max_height = (content_height * app.config.images.max_height_percent as u32) / 100;

    // Load from cache based on source type
    let decoded = match source {
        ImageSource::Local(path) => {
            app.image_cache.get_or_decode(&path, max_width, max_height)?
        }
        ImageSource::Remote(url) => {
            if !app.config.images.allow_remote {
                return Ok(None);
            }
            app.image_cache.get_or_fetch(&url, max_width, max_height)?
        }
    };

    Ok(Some(decoded))
}

/// Render a decoded image using the appropriate backend
#[cfg(feature = "images")]
fn render_decoded_image(
    decoded: crate::image_cache::DecodedImage,
    content_area: ratatui::layout::Rect,
    source_line: usize,
    image: &mdx_core::image::ImageNode,
    line_num_width: usize,
    is_focused: bool,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
    left_margin_width: u16,
    backend: mdx_core::config::ImageBackend,
) -> (Vec<Line<'static>>, usize) {
    // For now, terminal graphics protocols don't work well through ratatui
    // because ratatui processes all content as text and escapes control sequences.
    //
    // To properly support inline images, we would need to:
    // 1. Write escape sequences directly to stdout outside of ratatui
    // 2. Coordinate cursor positioning with ratatui's rendering
    // 3. Handle terminal state management carefully
    //
    // This is complex and error-prone. For now, show a nice placeholder
    // with information about the loaded image.

    let aspect_ratio = decoded.height as f32 / decoded.width as f32;
    let max_width = content_area.width.saturating_sub(left_margin_width).saturating_sub(5) as u32;
    let width_cells = (decoded.width / 10).min(max_width) as u16;
    let height_cells = ((width_cells as f32 * aspect_ratio) / 2.0).ceil() as u16;
    let content_height = content_area.height.saturating_sub(2) as u16;
    let height_cells = height_cells.min(content_height).max(1);

    // Show informative placeholder with image details
    render_image_info_placeholder(
        image,
        &decoded,
        height_cells as usize,
        backend,
        source_line,
        line_num_width,
        is_focused,
        cursor,
        selection_range,
        left_margin_width,
    )
}

/// Render image using Kitty graphics protocol
#[cfg(feature = "images")]
fn render_kitty_image(
    decoded: &crate::image_cache::DecodedImage,
    _width_cells: u16,
    height_cells: u16,
    image_id: usize,
) -> anyhow::Result<Vec<Line<'static>>> {
    use std::io::Write;

    // Generate unique image ID based on line number
    let id = image_id as u32;

    // Transmit image data
    let transmit_seq = crate::kitty_graphics::transmit_image(
        &decoded.data,
        decoded.width,
        decoded.height,
        id,
    )?;

    // Display image
    let display_seq = crate::kitty_graphics::display_image(
        id,
        height_cells,
        decoded.width as u16,
    )?;

    // Combine sequences
    let mut combined = Vec::new();
    combined.write_all(&transmit_seq)?;
    combined.write_all(&display_seq)?;

    // Convert to string and create a single line with escape sequences
    let escape_str = String::from_utf8_lossy(&combined).to_string();

    let mut lines = Vec::new();
    lines.push(Line::from(Span::raw(escape_str)));

    // Add empty lines for vertical spacing
    for _ in 1..height_cells {
        lines.push(Line::from(Span::raw("")));
    }

    Ok(lines)
}

/// Render image using iTerm2 inline images protocol
#[cfg(feature = "images")]
fn render_iterm2_image(
    decoded: &crate::image_cache::DecodedImage,
    width_cells: u16,
    height_cells: u16,
) -> anyhow::Result<Vec<Line<'static>>> {
    // First encode RGBA as PNG
    let png_data = crate::iterm2_graphics::encode_rgba_as_png(
        &decoded.data,
        decoded.width,
        decoded.height,
    )?;

    // Display image
    let display_seq = crate::iterm2_graphics::display_image(
        &png_data,
        width_cells,
        height_cells,
    )?;

    let escape_str = String::from_utf8_lossy(&display_seq).to_string();

    let mut lines = Vec::new();
    lines.push(Line::from(Span::raw(escape_str)));

    // Add empty lines for vertical spacing
    for _ in 1..height_cells {
        lines.push(Line::from(Span::raw("")));
    }

    Ok(lines)
}

/// Render image using Sixel graphics protocol
#[cfg(feature = "images")]
fn render_sixel_image(
    decoded: &crate::image_cache::DecodedImage,
    _width_cells: u16,
    height_cells: u16,
) -> anyhow::Result<Vec<Line<'static>>> {
    // Encode as Sixel
    let sixel_data = crate::sixel_graphics::encode_sixel(
        &decoded.data,
        decoded.width,
        decoded.height,
    )?;

    let escape_str = String::from_utf8_lossy(&sixel_data).to_string();

    let mut lines = Vec::new();
    lines.push(Line::from(Span::raw(escape_str)));

    // Add empty lines for vertical spacing
    for _ in 1..height_cells {
        lines.push(Line::from(Span::raw("")));
    }

    Ok(lines)
}

/// Render placeholder with image information
#[cfg(feature = "images")]
fn render_image_info_placeholder(
    image: &mdx_core::image::ImageNode,
    decoded: &crate::image_cache::DecodedImage,
    height: usize,
    backend: mdx_core::config::ImageBackend,
    _source_line: usize,
    _line_num_width: usize,
    _is_focused: bool,
    _cursor: usize,
    _selection_range: Option<(usize, usize)>,
    _left_margin_width: u16,
) -> (Vec<Line<'static>>, usize) {
    let mut lines = Vec::new();

    let alt_text = if image.alt.is_empty() {
        "Image"
    } else {
        &image.alt
    };

    // Format image information
    let backend_name = match backend {
        mdx_core::config::ImageBackend::Kitty => "Kitty",
        mdx_core::config::ImageBackend::ITerm2 => "iTerm2",
        mdx_core::config::ImageBackend::Sixel => "Sixel",
        _ => "None",
    };

    let info_text = format!(
        "🖼  {} | {}x{} | {}",
        alt_text,
        decoded.width,
        decoded.height,
        backend_name
    );

    // Show informative placeholder for calculated height
    let display_height = height.max(3);
    for i in 0..display_height {
        if i == display_height / 2 {
            // Center line with info
            lines.push(Line::from(Span::styled(
                info_text.clone(),
                Style::default()
                    .fg(Color::Rgb(100, 200, 255))
                    .bg(Color::Rgb(30, 40, 50))
                    .add_modifier(Modifier::BOLD)
            )));
        } else if i == 0 || i == display_height - 1 {
            // Border lines
            let border = "─".repeat(info_text.len().min(60));
            lines.push(Line::from(Span::styled(
                border,
                Style::default().fg(Color::Rgb(60, 80, 100)).bg(Color::Rgb(20, 25, 30))
            )));
        } else {
            // Empty placeholder line
            lines.push(Line::from(Span::styled(
                " ".repeat(info_text.len().min(60)),
                Style::default().bg(Color::Rgb(20, 25, 30))
            )));
        }
    }

    (lines, 1)
}

/// Simple placeholder rendering for error cases
#[cfg(feature = "images")]
fn render_image_placeholder_simple(
    image: &mdx_core::image::ImageNode,
    height: usize,
    _source_line: usize,
    _line_num_width: usize,
    _is_focused: bool,
    _cursor: usize,
    _selection_range: Option<(usize, usize)>,
    _left_margin_width: u16,
) -> (Vec<Line<'static>>, usize) {
    let mut lines = Vec::new();

    let alt_text = if image.alt.is_empty() {
        "Image"
    } else {
        &image.alt
    };

    // Show placeholder for calculated height
    for i in 0..height.max(1) {
        if i == height / 2 {
            // Center line with text
            let text = format!("[Image: {}]", alt_text);
            lines.push(Line::from(Span::styled(
                text,
                Style::default().fg(Color::Cyan).bg(Color::Rgb(50, 50, 50))
            )));
        } else {
            // Empty placeholder line
            lines.push(Line::from(Span::styled(
                " ".repeat(30),
                Style::default().bg(Color::Rgb(50, 50, 50))
            )));
        }
    }

    (lines, 1)
}

/// Render image placeholder when image cannot be loaded
#[cfg(feature = "images")]
fn render_image_placeholder(
    app: &App,
    content_area: ratatui::layout::Rect,
    source_line: usize,
    image: &mdx_core::image::ImageNode,
    line_num_width: usize,
    is_focused: bool,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
    left_margin_width: u16,
) -> (Vec<Line<'static>>, usize) {
    let mut rendered: Vec<Line> = Vec::new();

    // Calculate placeholder height based on config
    // Default to 10 lines, but respect configured percentages
    let content_height = content_area.height.saturating_sub(2) as usize;
    let content_width = content_area.width.saturating_sub(2) as usize;
    let max_height_from_config = (content_height * app.config.images.max_height_percent as usize) / 100;
    let max_width_from_config = (content_width * app.config.images.max_width_percent as usize) / 100;

    // Assume a 2:1 width-to-height ratio for terminal cells (cells are typically taller than wide)
    // So if we have width W available, we can show height H = W/2
    let max_height_from_width = max_width_from_config / 2;

    // Take minimum of both constraints
    let placeholder_height = max_height_from_config.min(max_height_from_width).max(3).min(20);

    // Create placeholder text
    let alt_text = if image.alt.is_empty() {
        "Image"
    } else {
        &image.alt
    };
    let placeholder_text = format!("[Image: {}]", alt_text);

    // Render lines
    for offset in 0..placeholder_height {
        let mut line_spans: Vec<Span> = Vec::new();

        // Only show line number and gutter on first line
        if offset == 0 {
            let line_num = format!("{:>width$} ", source_line + 1, width = line_num_width);
            let line_num_color = if is_focused && source_line == cursor {
                Color::White
            } else {
                Color::DarkGray
            };
            line_spans.push(Span::styled(line_num, Style::default().fg(line_num_color)));

            #[cfg(feature = "git")]
            if app.config.git.diff {
                use mdx_core::diff::DiffMark;
                let gutter = match app.doc.diff_gutter.get(source_line) {
                    DiffMark::None => "  ",
                    DiffMark::Added => "│ ",
                    DiffMark::Modified => "│ ",
                    DiffMark::DeletedAfter(_) => "│ ",
                };
                let gutter_color = match app.doc.diff_gutter.get(source_line) {
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
        } else {
            // Continuation lines - just indent
            line_spans.push(Span::raw(" ".repeat(left_margin_width as usize)));
        }

        // Add placeholder content
        if offset == placeholder_height / 2 {
            // Center line - show placeholder text
            let available_width = content_width.saturating_sub(left_margin_width as usize);
            let text_width = placeholder_text.chars().count();

            if text_width < available_width {
                let padding_left = (available_width - text_width) / 2;
                let padding_right = available_width - text_width - padding_left;

                line_spans.push(Span::styled(
                    " ".repeat(padding_left),
                    Style::default().bg(Color::Rgb(50, 50, 50))
                ));
                line_spans.push(Span::styled(
                    placeholder_text.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .bg(Color::Rgb(50, 50, 50))
                        .add_modifier(Modifier::BOLD)
                ));
                line_spans.push(Span::styled(
                    " ".repeat(padding_right),
                    Style::default().bg(Color::Rgb(50, 50, 50))
                ));
            } else {
                // Text too long, truncate
                let truncated = format!("{}…", placeholder_text.chars().take(available_width - 1).collect::<String>());
                line_spans.push(Span::styled(
                    truncated,
                    Style::default()
                        .fg(Color::Cyan)
                        .bg(Color::Rgb(50, 50, 50))
                        .add_modifier(Modifier::BOLD)
                ));
            }
        } else {
            // Empty placeholder line
            let available_width = content_width.saturating_sub(left_margin_width as usize);
            line_spans.push(Span::styled(
                " ".repeat(available_width),
                Style::default().bg(Color::Rgb(50, 50, 50))
            ));
        }

        let mut line = Line::from(line_spans);

        // Apply highlighting if this is the cursor or selected line
        let is_selected = if let Some((start, end)) = selection_range {
            source_line >= start && source_line <= end
        } else {
            false
        };

        if is_focused && is_selected {
            line = line.style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::REVERSED));
        } else if is_focused && source_line == cursor {
            line = line.style(Style::default().bg(app.theme.cursor_line_bg));
        }

        rendered.push(line);
    }

    (rendered, 1) // Consumed 1 source line
}
