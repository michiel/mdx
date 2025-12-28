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

    // If in raw mode, render plain text without markdown processing
    if pane.view.show_raw {
        render_raw_text(frame, app, area, pane_id, scroll, cursor, is_focused, selection_range, line_count);
        return;
    }

    // Get search query for highlighting
    let search_query = if !app.search_query.is_empty() {
        Some(app.search_query.as_str())
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
    // Account for borders (top and bottom borders take 2 lines)
    let content_height = area.height.saturating_sub(2) as usize;
    let visible_end = (scroll + content_height).min(line_count);
    let mut is_first_code_line = false;

    for line_idx in scroll..visible_end {
        let mut line_spans: Vec<Span> = Vec::new();

        // Get line text first to check if it's a fence
        let line_text: String = if line_idx < line_count {
            app.doc.rope.line(line_idx).chunks().collect()
        } else {
            String::new()
        };

        // Remove trailing newline for styling
        let line_text = line_text.trim_end_matches('\n');

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

        // Track if this is a code block line for background styling
        let is_code_block_line;

        if in_code_block {
            // Inside code block - render with syntax highlighting and different background
            line_spans.extend(render_code_line(line_text, &app.theme, search_query));
            is_code_block_line = true;
        } else {
            // Apply markdown styling to the line
            line_spans.extend(style_markdown_line(line_text, &app.theme, search_query));
            is_code_block_line = false;
        }

        // For code blocks, pad to full viewport width and add language label on first line
        if is_code_block_line {
            let line_visual_width: usize = line_spans.iter()
                .map(|span| span.content.chars().count())
                .sum();
            // Calculate available width (area width - borders)
            let available_width = area.width.saturating_sub(2) as usize;

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
    }

    // Add border to pane with focus highlight
    let border_style = if is_focused {
        Style::default().fg(app.theme.toc_active.bg.unwrap_or(Color::LightCyan))
    } else {
        Style::default().fg(app.theme.toc_border)
    };

    // Manual wrapping to indent continuation lines
    let available_width = area.width.saturating_sub(2) as usize; // -2 for borders
    let content_start = left_margin_width as usize;
    let content_width = available_width.saturating_sub(content_start);

    let mut wrapped_lines: Vec<Line> = Vec::new();
    let indent_str = " ".repeat(content_start);

    for line in styled_lines {
        // Check if this is a table row - if so, don't wrap it
        let is_table_row = line.spans.iter().any(|span| {
            span.content.contains('|')
        });

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

    frame.render_widget(paragraph, area);
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
