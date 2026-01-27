//! UI rendering

use crate::app::App;
use crate::collapse::{self, CollapseRange};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

/// Draw the UI
pub fn draw(frame: &mut Frame, app: &mut App) {
    // Create base layout with optional security warnings pane
    let base_chunks = if !app.security_warnings.is_empty() && app.show_security_warnings {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // Main content area (TOC + panes)
                Constraint::Length(4), // Security warnings pane
                Constraint::Length(1), // Status bar
            ])
            .split(frame.area());

        // Render security warnings pane
        render_security_warnings(frame, chunks[1], &app.security_warnings, &app.theme);

        [chunks[0], chunks[2]] // Return [content_area, status_area]
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // Main content area
                Constraint::Length(1), // Status bar
            ])
            .split(frame.area());

        [chunks[0], chunks[1]] // Return [content_area, status_area]
    };

    let pane_area = if app.show_toc {
        let toc_width = app.config.toc.width as u16;
        let main_chunks = if app.config.toc.side == mdx_core::config::TocSide::Left {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(toc_width), // TOC
                    Constraint::Min(1),            // Panes area
                ])
                .split(base_chunks[0])
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(1),            // Panes area
                    Constraint::Length(toc_width), // TOC
                ])
                .split(base_chunks[0])
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
        base_chunks[0]
    };

    // Compute layout for all panes and render them
    let pane_layouts = app.panes.compute_layout(pane_area);
    app.update_layout_context(&pane_layouts);
    for (pane_id, rect) in pane_layouts.iter() {
        render_markdown(frame, app, *rect, *pane_id);
    }

    // Render status bar
    render_status_bar(frame, app, base_chunks[1]);

    // Render help popup if active
    if app.show_help {
        render_help_popup(frame, app);
    }

    // Render options dialog if active
    if app.options_dialog.is_some() {
        render_options_dialog(frame, app);
    }

    // Render TOC dialog if active
    if app.show_toc_dialog {
        render_toc_dialog(frame, app);
    }

    if app.command_output.is_some() {
        render_command_output(frame, app);
    }
}

fn sanitize_for_terminal(input: &str) -> String {
    input
        .chars()
        .filter(|&c| {
            // Allow newline, tab, and printable characters (including UTF-8)
            // Exclude C0 and C1 control characters except \n and \t
            c == '\n' || c == '\t' || (c >= ' ' && c != '\x7f' && (c < '\u{80}' || c > '\u{9f}'))
        })
        .collect()
}

/// Render a collapsed block summary line
///
/// Returns a styled Line showing the collapse indicator, heading text, and line count
fn render_collapsed_summary(
    range: &CollapseRange,
    line_num_width: usize,
    theme: &crate::theme::Theme,
    is_focused: bool,
    is_cursor: bool,
    content_width: usize,
) -> Line<'static> {
    let mut spans = Vec::new();

    // Add line number
    let line_num = format!("{:>width$} ", range.start + 1, width = line_num_width);
    let line_num_color = if is_focused && is_cursor {
        Color::White
    } else {
        Color::DarkGray
    };
    spans.push(Span::styled(line_num, Style::default().fg(line_num_color)));

    // Add gutter spacing (2 chars for diff gutter)
    spans.push(Span::raw("  "));

    // Add collapse indicator (▶)
    spans.push(Span::styled(
        "▶ ",
        Style::default().fg(theme.collapsed_indicator_fg),
    ));

    // Add heading marks based on level
    if let Some(level) = range.level {
        let marks = "#".repeat(level as usize);
        let heading_style = theme
            .heading
            .get(level as usize - 1)
            .copied()
            .unwrap_or(theme.base);
        spans.push(Span::styled(format!("{} ", marks), heading_style));
    }

    // Add heading text (truncated)
    let heading_style = if let Some(level) = range.level {
        theme
            .heading
            .get(level as usize - 1)
            .copied()
            .unwrap_or(theme.base)
    } else {
        theme.base
    };
    spans.push(Span::styled(range.text.clone(), heading_style));

    // Add line count
    let count_text = format!(" ({} lines)", range.line_count);
    spans.push(Span::styled(
        count_text,
        Style::default().fg(Color::DarkGray),
    ));

    // Calculate current width and pad to content width
    let current_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();

    if current_width < content_width {
        let padding = " ".repeat(content_width - current_width);
        spans.push(Span::styled(
            padding,
            Style::default().bg(theme.collapsed_block_bg),
        ));
    }

    // Apply background color to all spans
    let spans: Vec<Span> = spans
        .into_iter()
        .map(|mut span| {
            if is_focused && is_cursor {
                span.style = span.style.bg(theme.cursor_line_bg);
            } else {
                span.style = span.style.bg(theme.collapsed_block_bg);
            }
            span
        })
        .collect();

    Line::from(spans)
}

/// Render security warnings pane
fn render_security_warnings(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    warnings: &[mdx_core::SecurityEvent],
    theme: &crate::theme::Theme,
) {
    // Build warning items - show most recent first, limit to 100
    let items: Vec<ListItem> = warnings
        .iter()
        .rev()
        .take(100)
        .map(|w| {
            let color = match w.level {
                mdx_core::SecurityEventLevel::Error => Color::Red,
                mdx_core::SecurityEventLevel::Warning => Color::Yellow,
                mdx_core::SecurityEventLevel::Info => Color::Cyan,
            };
            let text = format!("[{}] {}", w.source, w.message);
            ListItem::new(text).style(Style::default().fg(color))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .title(" Security Warnings (W to toggle) "),
        )
        .style(theme.base);

    frame.render_widget(list, area);
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
    let mut content_area = chunks[1];

    // Split content area for scrollbar if enabled and document is larger than viewport
    let doc_line_count = app.doc.line_count();
    let viewport_height = content_area.height.saturating_sub(2) as usize; // Account for borders
    let show_scrollbar = app.config.render.show_scrollbar && doc_line_count > viewport_height;

    let scrollbar_area = if show_scrollbar {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(1),    // Main content
                Constraint::Length(1), // Scrollbar
            ])
            .split(content_area);
        content_area = chunks[0];
        Some(chunks[1])
    } else {
        None
    };

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
        render_raw_text(
            frame,
            app,
            content_area,
            pane_id,
            scroll,
            cursor,
            is_focused,
            selection_range,
            line_count,
        );
        // Render scrollbar if enabled
        if let Some(scrollbar_area) = scrollbar_area {
            render_scrollbar(
                frame,
                app,
                scrollbar_area,
                pane_id,
                doc_line_count,
                viewport_height,
            );
        }
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
    let mut code_block_indent = 0; // Track indentation of code block for list items
    for line_idx in 0..scroll.min(line_count) {
        let line_text: String = app.doc.rope.line(line_idx).chunks().collect();
        let trimmed = line_text.trim_end();
        let trimmed_start = trimmed.trim_start();
        if trimmed_start.starts_with("```") {
            if !in_code_block {
                // Opening fence - extract language and indentation
                let indent = trimmed.len() - trimmed_start.len();
                code_block_indent = indent;
                let lang = trimmed_start.strip_prefix("```").unwrap_or("").trim();
                code_block_lang = if lang.is_empty() {
                    "plain".to_string()
                } else {
                    lang.to_string()
                };
            }
            in_code_block = !in_code_block;
            if !in_code_block {
                code_block_lang.clear();
                code_block_indent = 0;
            }
        }
    }

    // Calculate left margin width for line numbers and gutter
    let line_num_width = format!("{}", line_count).len().max(3);
    let gutter_width = 2; // Git gutter or spacing
    let left_margin_width = (line_num_width + 1 + gutter_width) as u16; // +1 for space after line number

    // Compute collapsed ranges for this pane
    let collapsed_ranges =
        collapse::compute_all_collapsed_ranges(&pane.view.collapsed_headings, &app.doc);

    // Build only visible lines
    let mut styled_lines: Vec<Line> = Vec::new();
    let mut is_table_row_flags: Vec<bool> = Vec::new();
    let mut list_item_indents: Vec<Option<usize>> = Vec::new(); // Track list item continuation indent
                                                                // Account for borders (top and bottom borders take 2 lines)
    let content_height = content_area.height.saturating_sub(2) as usize;
    let mut visible_end = (scroll + content_height).min(line_count);
    let mut is_first_code_line = false;

    let mut line_idx = scroll;
    while line_idx < visible_end {
        // Check if this line is the start of a collapsed range
        if let Some(range) = collapse::find_range_at_line(&collapsed_ranges, line_idx) {
            // Render collapsed summary
            let content_width = content_area.width.saturating_sub(2) as usize;
            let is_cursor = is_focused && cursor == line_idx;
            let summary_line = render_collapsed_summary(
                range,
                line_num_width,
                &app.theme,
                is_focused,
                is_cursor,
                content_width,
            );

            styled_lines.push(summary_line);
            is_table_row_flags.push(false);
            list_item_indents.push(None);

            // Skip to the end of the collapsed range
            let skipped_lines = range.end.saturating_sub(range.start);
            if skipped_lines > 0 && visible_end < line_count {
                visible_end = (visible_end + skipped_lines).min(line_count);
            }
            line_idx = range.end + 1;
            continue;
        }

        // Check if this line is inside a collapsed range (but not the start)
        if collapse::find_range_containing_line(&collapsed_ranges, line_idx).is_some() {
            // Skip this line - it's hidden inside a collapsed block
            line_idx += 1;
            // Expand visible_end to compensate for skipped line
            if visible_end < line_count {
                visible_end += 1;
            }
            continue;
        }
        let mut line_spans: Vec<Span> = Vec::new();

        // Get line text first to check if it's a fence
        let line_text: String = if line_idx < line_count {
            app.doc.rope.line(line_idx).chunks().collect()
        } else {
            String::new()
        };

        // Remove trailing newline for styling
        let line_text = sanitize_for_terminal(line_text.trim_end_matches('\n'));

        // Table detection: header row followed by a separator row
        if !in_code_block && line_idx + 1 < line_count {
            let next_line: String = app.doc.rope.line(line_idx + 1).chunks().collect();
            let next_line = sanitize_for_terminal(next_line.trim_end_matches('\n'));
            if is_table_row(&line_text) && is_table_separator_row(&next_line) {
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
                    list_item_indents.push(None); // Tables are not list items
                }

                line_idx = line_idx.saturating_add(consumed);
                continue;
            }
        }

        // Check for image rendering
        #[cfg(feature = "images")]
        if !in_code_block && app.config.images.enabled && !app.config.security.safe_mode {
            // Check if there's an image on this line (clone to avoid borrow issues)
            let image_opt = app
                .doc
                .images
                .iter()
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
                );

                for line in image_lines {
                    styled_lines.push(line);
                    is_table_row_flags.push(false);
                    list_item_indents.push(None); // Images are not list items
                }

                line_idx += 1;
                continue;
            }
        }

        // Track if this is a table row (before styling splits the pipes)
        let is_table_row = line_text.contains('|');

        // Check for code block fence markers (including indented ones) - skip rendering them
        let trimmed = line_text.trim_end();
        let trimmed_start = trimmed.trim_start();
        if trimmed_start.starts_with("```") {
            if !in_code_block {
                // Opening fence - extract language and indentation
                let indent = trimmed.len() - trimmed_start.len();
                code_block_indent = indent;
                let lang = trimmed_start.strip_prefix("```").unwrap_or("").trim();
                code_block_lang = if lang.is_empty() {
                    "plain".to_string()
                } else {
                    lang.to_string()
                };
                is_first_code_line = true;
            } else {
                // Closing fence - clear language
                code_block_lang.clear();
                code_block_indent = 0;
            }
            in_code_block = !in_code_block;
            // Skip this line entirely (don't render fence markers).
            // Expand visible range so skipped fences don't leave empty space.
            if visible_end < line_count {
                visible_end += 1;
            }
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
            // For indented code blocks (in list items), preserve the indentation
            if code_block_indent > 0 {
                // Add the indentation as plain text
                let indent_str = " ".repeat(code_block_indent);
                line_spans.push(Span::raw(indent_str));

                // Render the code content (strip the indent from the line)
                let code_content = if line_text.len() >= code_block_indent {
                    &line_text[code_block_indent..]
                } else {
                    &line_text
                };
                line_spans.extend(render_code_line(
                    code_content,
                    &app.theme,
                    search_query.as_deref(),
                ));
            } else {
                line_spans.extend(render_code_line(
                    &line_text,
                    &app.theme,
                    search_query.as_deref(),
                ));
            }
            is_code_block_line = true;
        } else {
            // Apply markdown styling to the line
            line_spans.extend(style_markdown_line(
                &line_text,
                &app.theme,
                &app.config.render,
                search_query.as_deref(),
            ));
            is_code_block_line = false;
        }

        // For code blocks, pad to full viewport width and add language label on first line
        if is_code_block_line {
            let line_visual_width: usize = line_spans
                .iter()
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
                        Style::default().bg(app.theme.code_block_bg),
                    ));
                    // Add the language label
                    line_spans.push(Span::styled(
                        lang_label,
                        Style::default()
                            .fg(Color::Rgb(120, 120, 120))
                            .bg(app.theme.code_block_bg),
                    ));
                } else {
                    // Not enough space for label, just pad
                    let padding = " ".repeat(remaining_width);
                    line_spans.push(Span::styled(
                        padding,
                        Style::default().bg(app.theme.code_block_bg),
                    ));
                }
                is_first_code_line = false;
            } else if line_visual_width < available_width {
                // Regular code block line - just pad
                let padding = " ".repeat(available_width - line_visual_width);
                line_spans.push(Span::styled(
                    padding,
                    Style::default().bg(app.theme.code_block_bg),
                ));
            }
        }

        // Check if this line is selected or cursor
        let is_selected = if let Some((start, end)) = selection_range {
            line_idx >= start && line_idx <= end
        } else {
            false
        };

        // Apply highlighting directly to spans - priority order: selection > cursor > code block
        if is_focused && is_selected {
            // Visual line selection: apply cyan background to each span
            line_spans = line_spans
                .into_iter()
                .map(|mut span| {
                    let new_style = span.style.bg(Color::Cyan).fg(Color::Black);
                    span.style = new_style;
                    span
                })
                .collect();
        } else if is_focused && line_idx == cursor {
            // Cursor line: apply cursor background to each span
            line_spans = line_spans
                .into_iter()
                .map(|mut span| {
                    let new_style = span.style.bg(app.theme.cursor_line_bg);
                    span.style = new_style;
                    span
                })
                .collect();
        } else if is_code_block_line {
            // Code block: apply code block background to each span (if not already styled)
            line_spans = line_spans
                .into_iter()
                .map(|mut span| {
                    if span.style.bg.is_none() {
                        let new_style = span.style.bg(app.theme.code_block_bg);
                        span.style = new_style;
                    }
                    span
                })
                .collect();
        }

        let line = Line::from(line_spans);

        // Detect if this is a list item and calculate continuation indent
        let list_indent = if !in_code_block {
            detect_list_item_indent(&line_text)
        } else {
            None
        };

        styled_lines.push(line);
        is_table_row_flags.push(is_table_row);
        list_item_indents.push(list_indent);
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

    for (idx, line) in styled_lines.into_iter().enumerate() {
        // Check if this is a table row - if so, don't wrap it
        let is_table_row = is_table_row_flags.get(idx).copied().unwrap_or(false);

        if is_table_row {
            // Don't wrap table rows, just add them as-is
            wrapped_lines.push(line);
            continue;
        }

        // Check if this is a list item and get the continuation indent
        let list_continuation_indent = list_item_indents.get(idx).copied().flatten();

        // Calculate the visual width of the line
        let mut current_width = 0;
        let mut current_line_spans: Vec<Span> = Vec::new();
        let mut first_segment = true;
        let mut prev_was_bullet = false;

        for span in line.spans {
            let span_text = span.content.to_string();
            let span_width = span_text.chars().count();

            // Detect if this span is a bullet marker
            let is_bullet_span = list_continuation_indent.is_some() &&
                span_width <= 5 && // Bullets are short: "• ", "- ", "1. ", "100. " etc.
                (span_text.starts_with('•') ||
                 span_text.starts_with('-') ||
                 span_text.starts_with('*') ||
                 span_text.starts_with('+') ||
                 span_text.chars().next().map_or(false, |c| c.is_ascii_digit()));

            if current_width + span_width <= available_width {
                // Fits on current line
                current_line_spans.push(span);
                current_width += span_width;
            } else {
                // Need to wrap
                // Determine if we should wrap now or try to keep content together
                // Don't wrap if the span that doesn't fit is very short (< 15 chars)
                // and we have minimal content - this prevents orphaning styled text
                let span_is_short = span_width < 15;
                let current_content_width = if first_segment {
                    current_width.saturating_sub(content_start)
                } else {
                    let extra_indent = list_continuation_indent.unwrap_or(0);
                    let indent_width = content_start + extra_indent;
                    current_width.saturating_sub(indent_width)
                };

                // Never wrap immediately after a bullet or before adding any content
                // Keep bullet with content, and don't create empty lines
                let should_wrap = if prev_was_bullet || current_content_width == 0 {
                    false // Force keeping content with bullet, or don't create empty lines
                } else if !current_line_spans.is_empty() {
                    // If the span is short and we have little content, try to keep them together
                    // by not wrapping yet (let the span overflow and wrap within itself)
                    if span_is_short && current_content_width < 20 {
                        false
                    } else {
                        true
                    }
                } else {
                    false
                };

                if should_wrap {
                    wrapped_lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                    current_width = 0;
                    first_segment = false;
                }

                // Add indentation for continuation lines
                if !first_segment {
                    // For list items, add extra indent to align with content after marker
                    let extra_indent = list_continuation_indent.unwrap_or(0);
                    let total_indent = content_start + extra_indent;
                    current_line_spans.push(Span::raw(" ".repeat(total_indent)));
                    current_width = total_indent;
                }

                // Word-aware wrapping within the span
                let mut remaining = span_text.as_str();
                while !remaining.is_empty() {
                    let available = if first_segment {
                        available_width - current_width
                    } else {
                        // For continuation lines, account for list item indentation
                        let extra_indent = list_continuation_indent.unwrap_or(0);
                        content_width.saturating_sub(extra_indent)
                    };

                    let remaining_len = remaining.chars().count();

                    if remaining_len <= available {
                        // Entire remaining text fits
                        current_line_spans.push(Span::styled(remaining.to_string(), span.style));
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
                            let after_space = remaining[word_end..]
                                .char_indices()
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

                        // Safety: ensure we consume at least one character to avoid infinite loops
                        let safe_split_pos = if split_pos.0 == 0 && !remaining.is_empty() {
                            let first_char_len = remaining.chars().next().unwrap().len_utf8();
                            (first_char_len, first_char_len)
                        } else {
                            split_pos
                        };

                        let (chunk, rest) = remaining.split_at(safe_split_pos.0);
                        let rest = &rest[safe_split_pos.1 - safe_split_pos.0..];

                        if !chunk.is_empty() {
                            current_line_spans.push(Span::styled(chunk.to_string(), span.style));
                            wrapped_lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();

                            // For list items, add extra indent to align with content after marker
                            let extra_indent = list_continuation_indent.unwrap_or(0);
                            let total_indent = content_start + extra_indent;
                            current_line_spans.push(Span::raw(" ".repeat(total_indent)));
                            current_width = total_indent;
                            first_segment = false;
                        }
                        remaining = rest;
                    }
                }
            }

            // Update prev_was_bullet for the next iteration
            prev_was_bullet = is_bullet_span;
        }

        if !current_line_spans.is_empty() {
            wrapped_lines.push(Line::from(current_line_spans));
        }
    }

    let paragraph = Paragraph::new(wrapped_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .style(app.theme.base);

    frame.render_widget(paragraph, content_area);

    // Render scrollbar if enabled
    if let Some(scrollbar_area) = scrollbar_area {
        render_scrollbar(
            frame,
            app,
            scrollbar_area,
            pane_id,
            doc_line_count,
            viewport_height,
        );
    }
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
            let available = max_breadcrumb_width
                .saturating_sub(current_width)
                .saturating_sub(1);
            if available > 3 {
                format!(
                    "{}…",
                    &crumb.chars().take(available - 1).collect::<String>()
                )
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
        let padding_width = area
            .width
            .saturating_sub(current_width as u16 + status_text.len() as u16 + 2);
        if padding_width > 0 {
            spans.push(Span::raw(" ".repeat(padding_width as usize)));
        }

        spans.push(Span::styled(status_text, Style::default().fg(status_color)));
    }

    let breadcrumb_line = Line::from(spans);
    frame.render_widget(Paragraph::new(vec![breadcrumb_line]), area);
}

/// Render scrollbar for the pane
fn render_scrollbar(
    frame: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    pane_id: usize,
    doc_line_count: usize,
    viewport_height: usize,
) {
    use ratatui::text::Span;

    // Get the pane's scroll position
    let pane = match app.panes.panes.get(&pane_id) {
        Some(p) => p,
        None => return,
    };

    let scroll = pane.view.scroll_line;
    let is_focused = app.panes.focused == pane_id;

    // Calculate scrollbar dimensions
    let scrollbar_height = area.height as usize;
    if scrollbar_height == 0 {
        return;
    }

    // Calculate thumb position and size
    let max_scroll = doc_line_count.saturating_sub(viewport_height);

    // Reserve space for top/bottom indicators
    let available_height = scrollbar_height.saturating_sub(2); // Reserve 1 line each for top/bottom

    // Calculate thumb size - ensure it's proportional but leave room for indicators
    // Minimum thumb size of 2, maximum of available_height - 2 (to always show some track)
    let thumb_size = if available_height > 0 {
        let ratio = viewport_height as f32 / doc_line_count as f32;
        let size = (ratio * available_height as f32).ceil() as usize;
        size.max(2).min(available_height.saturating_sub(2))
    } else {
        1
    };

    // Calculate thumb position within the available space (between top and bottom indicators)
    let thumb_position = if max_scroll > 0 && available_height > thumb_size {
        let available_for_movement = available_height - thumb_size;
        let pos =
            ((scroll as f32 / max_scroll as f32) * available_for_movement as f32).round() as usize;
        pos + 1 // +1 to account for top indicator
    } else {
        1 // Start after top indicator
    };

    // Build scrollbar lines with visual indicators
    let mut lines = Vec::new();
    let (scrollbar_char, thumb_char, top_char, bottom_char) = if app.config.render.use_utf8_graphics
    {
        ("┊", "█", "▴", "▾") // Lighter track, solid thumb, small arrows
    } else {
        (".", "#", "^", "v")
    };

    let track_style = if is_focused {
        Style::default().fg(app.theme.scrollbar_track)
    } else {
        Style::default().fg(app.theme.scrollbar_track_unfocused)
    };

    let thumb_style = if is_focused {
        Style::default().fg(app.theme.scrollbar_thumb)
    } else {
        Style::default().fg(app.theme.scrollbar_thumb_unfocused)
    };

    // Build the scrollbar with clear visual indicators
    for i in 0..scrollbar_height {
        let (char_to_use, style) = if i == 0 {
            // Always show top indicator
            if scroll > 0 {
                (top_char, thumb_style) // Highlight if can scroll up
            } else {
                (top_char, track_style) // Dimmed if at top
            }
        } else if i == scrollbar_height - 1 {
            // Always show bottom indicator
            if scroll + viewport_height < doc_line_count {
                (bottom_char, thumb_style) // Highlight if can scroll down
            } else {
                (bottom_char, track_style) // Dimmed if at bottom
            }
        } else if i >= thumb_position && i < thumb_position + thumb_size {
            // Thumb area
            (thumb_char, thumb_style)
        } else {
            // Track area
            (scrollbar_char, track_style)
        };

        lines.push(Line::from(Span::styled(char_to_use, style)));
    }

    let paragraph = Paragraph::new(lines);

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
        let line_text = sanitize_for_terminal(line_text.trim_end_matches('\n'));

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
        line_spans.push(Span::styled(line_text.to_string(), app.theme.base));

        // Check if this line is selected or cursor
        let is_selected = if let Some((start, end)) = selection_range {
            line_idx >= start && line_idx <= end
        } else {
            false
        };

        // Apply highlighting directly to spans - priority order: selection > cursor
        if is_focused && is_selected {
            // Visual line selection: apply cyan background to each span
            line_spans = line_spans
                .into_iter()
                .map(|mut span| {
                    let new_style = span.style.bg(Color::Cyan).fg(Color::Black);
                    span.style = new_style;
                    span
                })
                .collect();
        } else if is_focused && line_idx == cursor {
            // Cursor line: apply cursor background to each span
            line_spans = line_spans
                .into_iter()
                .map(|mut span| {
                    let new_style = span.style.bg(app.theme.cursor_line_bg);
                    span.style = new_style;
                    span
                })
                .collect();
        }

        let line = Line::from(line_spans);
        lines.push(line);
    }

    // Create border style
    let border_style = if is_focused {
        Style::default().fg(app.theme.toc_active.bg.unwrap_or(Color::LightCyan))
    } else {
        Style::default().fg(app.theme.toc_border)
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Raw "),
        )
        .style(app.theme.base);

    frame.render_widget(paragraph, area);
}

/// Render a code block line with syntax highlighting
fn render_code_line(
    text: &str,
    theme: &crate::theme::Theme,
    search_query: Option<&str>,
) -> Vec<Span<'static>> {
    // Code block background color from theme
    let code_bg = theme.code_block_bg;

    // Define syntax highlighting colors
    let keyword_color = Color::Rgb(198, 120, 221); // Purple for keywords
    let string_color = Color::Rgb(152, 195, 121); // Green for strings
    let comment_color = Color::Rgb(92, 99, 112); // Gray for comments
    let number_color = Color::Rgb(209, 154, 102); // Orange for numbers
    let function_color = Color::Rgb(97, 175, 239); // Blue for functions

    // Common keywords across languages
    let keywords = [
        "fn",
        "func",
        "function",
        "def",
        "class",
        "struct",
        "enum",
        "impl",
        "trait",
        "let",
        "const",
        "var",
        "mut",
        "pub",
        "priv",
        "private",
        "public",
        "protected",
        "if",
        "else",
        "match",
        "switch",
        "case",
        "for",
        "while",
        "loop",
        "break",
        "continue",
        "return",
        "yield",
        "async",
        "await",
        "import",
        "export",
        "from",
        "use",
        "type",
        "interface",
        "extends",
        "implements",
        "new",
        "this",
        "self",
        "super",
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
            while i < chars.len()
                && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_')
            {
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
                    Style::default()
                        .fg(keyword_color)
                        .bg(code_bg)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                // Regular identifier - check if followed by '(' for function
                let is_function = i < chars.len() && chars[i] == '(';
                let color = if is_function {
                    function_color
                } else {
                    theme.code.fg.unwrap_or(Color::White)
                };
                spans.push(Span::styled(word, Style::default().fg(color).bg(code_bg)));
            }
            continue;
        }

        // Regular character
        let ch = chars[i].to_string();
        spans.push(Span::styled(
            ch,
            Style::default()
                .fg(theme.code.fg.unwrap_or(Color::White))
                .bg(code_bg),
        ));
        i += 1;
    }

    // Apply search highlighting on top of syntax highlighting
    if let Some(query) = search_query {
        if !query.is_empty() {
            spans = apply_search_highlighting_to_spans(spans, query);
        }
    }

    spans
}

/// Apply search highlighting on top of existing styled spans
/// Preserves the original foreground color but adds yellow background for matches
fn apply_search_highlighting_to_spans(
    spans: Vec<Span<'static>>,
    query: &str,
) -> Vec<Span<'static>> {
    let mut result = Vec::new();
    let query_lower = query.to_lowercase();

    for span in spans {
        let text = span.content.to_string();
        let text_lower = text.to_lowercase();

        // Find all matches in this span
        let mut last_end = 0;
        let mut has_match = false;

        for (idx, _) in text_lower.match_indices(&query_lower) {
            has_match = true;

            // Add text before match (if any) with original style
            if idx > last_end {
                result.push(Span::styled(text[last_end..idx].to_string(), span.style));
            }

            // Add highlighted match - preserve fg color, add yellow bg
            let match_style = span
                .style
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD);

            result.push(Span::styled(
                text[idx..idx + query.len()].to_string(),
                match_style,
            ));

            last_end = idx + query.len();
        }

        if has_match {
            // Add remaining text after last match (if any) with original style
            if last_end < text.len() {
                result.push(Span::styled(text[last_end..].to_string(), span.style));
            }
        } else {
            // No match in this span, keep it as-is
            result.push(span);
        }
    }

    result
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

fn build_table_separator_cell(width: usize, raw: &str, use_utf8: bool) -> String {
    if width == 0 {
        return String::new();
    }

    let trimmed = raw.trim();
    let fill_char = if use_utf8 { '─' } else { '-' };
    let mut cell: Vec<char> = vec![fill_char; width];
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
        let line_text = sanitize_for_terminal(line_text.trim_end_matches('\n'));
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

            let separator_char = if app.config.render.use_utf8_graphics {
                "│"
            } else {
                "|"
            };
            line_spans.push(Span::styled(
                separator_char.to_string(),
                Style::default().fg(Color::Cyan),
            ));

            for (col_idx, width) in widths.iter().enumerate() {
                line_spans.push(Span::raw(" ".to_string()));

                if is_separator {
                    let cell_text = build_table_separator_cell(
                        *width,
                        &padded_cells[col_idx],
                        app.config.render.use_utf8_graphics,
                    );
                    line_spans.push(Span::styled(
                        cell_text,
                        Style::default().fg(Color::DarkGray),
                    ));
                } else {
                    let cell_line = wrapped_cells[col_idx]
                        .get(line_offset)
                        .map(String::as_str)
                        .unwrap_or("");
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
                line_spans.push(Span::styled(
                    separator_char.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }

            let is_selected = if let Some((start, end)) = selection_range {
                *source_idx >= start && *source_idx <= end
            } else {
                false
            };

            // Apply highlighting directly to spans - priority order: selection > cursor
            if is_focused && is_selected {
                // Visual line selection: apply cyan background to each span
                line_spans = line_spans
                    .into_iter()
                    .map(|mut span| {
                        let new_style = span.style.bg(Color::Cyan).fg(Color::Black);
                        span.style = new_style;
                        span
                    })
                    .collect();
            } else if is_focused && *source_idx == cursor {
                // Cursor line: apply cursor background to each span
                line_spans = line_spans
                    .into_iter()
                    .map(|mut span| {
                        let new_style = span.style.bg(app.theme.cursor_line_bg);
                        span.style = new_style;
                        span
                    })
                    .collect();
            }

            let line = Line::from(line_spans);
            rendered.push(line);
        }
    }

    (rendered, consumed)
}

/// Style a single line of markdown text
/// Detect if a line is a list item and calculate the indent for continuation lines
/// Returns Some(indent_width) if it's a list item, None otherwise
fn detect_list_item_indent(line: &str) -> Option<usize> {
    let trimmed_start = line.trim_start();
    let leading_spaces = line.len() - trimmed_start.len();

    // Check for unordered list (-, *, +)
    if let Some(_rest) = trimmed_start.strip_prefix("- ") {
        return Some(leading_spaces + 2); // "- " is 2 chars
    } else if let Some(_rest) = trimmed_start.strip_prefix("* ") {
        return Some(leading_spaces + 2); // "* " is 2 chars
    } else if let Some(_rest) = trimmed_start.strip_prefix("+ ") {
        return Some(leading_spaces + 2); // "+ " is 2 chars
    }

    // Check for ordered list (number followed by . or ))
    if let Some(pos) = trimmed_start.find(|c| c == '.' || c == ')') {
        let prefix = &trimmed_start[..pos];
        if prefix.chars().all(|c| c.is_ascii_digit()) && trimmed_start.len() > pos + 1 {
            // Marker is like "1. " or "1) " - pos + 2 chars
            return Some(leading_spaces + pos + 2);
        }
    }

    None
}

fn style_markdown_line(
    line: &str,
    theme: &crate::theme::Theme,
    render_config: &mdx_core::config::RenderConfig,
    search_query: Option<&str>,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    // Check for horizontal rule
    let trimmed = line.trim();
    if (trimmed.chars().all(|c| c == '-') && trimmed.len() >= 3)
        || (trimmed.chars().all(|c| c == '*') && trimmed.len() >= 3)
        || (trimmed.chars().all(|c| c == '_') && trimmed.len() >= 3)
    {
        let rule_text = if render_config.use_utf8_graphics {
            // Use UTF-8 box-drawing horizontal line
            "─".repeat(trimmed.len())
        } else {
            line.to_string()
        };
        spans.push(Span::styled(
            rule_text,
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
                let separator = if render_config.use_utf8_graphics {
                    "│" // UTF-8 box-drawing vertical line
                } else {
                    "|"
                };
                spans.push(Span::styled(
                    separator.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
            // Check if this is a separator row (contains only -, :, and spaces)
            let is_separator = part
                .trim()
                .chars()
                .all(|c| c == '-' || c == ':' || c == ' ');
            if is_separator && !part.trim().is_empty() {
                let separator_text = if render_config.use_utf8_graphics {
                    // Convert alignment markers to UTF-8 table separators
                    let trimmed = part.trim();
                    let left_align = trimmed.starts_with(':');
                    let right_align = trimmed.ends_with(':');

                    let leading_spaces = part.len() - part.trim_start().len();
                    let trailing_spaces = part.len().saturating_sub(leading_spaces + trimmed.len());

                    let mut result = String::new();
                    if leading_spaces > 0 {
                        result.push_str(&" ".repeat(leading_spaces));
                    }

                    if left_align && right_align {
                        result.push(':');
                        result.push_str(&"─".repeat(trimmed.len().saturating_sub(2)));
                        result.push(':');
                    } else if left_align {
                        result.push(':');
                        result.push_str(&"─".repeat(trimmed.len().saturating_sub(1)));
                    } else if right_align {
                        result.push_str(&"─".repeat(trimmed.len().saturating_sub(1)));
                        result.push(':');
                    } else {
                        result.push_str(&"─".repeat(trimmed.len()));
                    }

                    if trailing_spaces > 0 {
                        result.push_str(&" ".repeat(trailing_spaces));
                    }
                    result
                } else {
                    part.to_string()
                };
                spans.push(Span::styled(
                    separator_text,
                    Style::default().fg(Color::DarkGray),
                ));
            } else {
                // Style content within cell
                spans.extend(style_inline_markdown(
                    part,
                    theme.base,
                    theme.code,
                    search_query,
                ));
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
        let display_marker = if render_config.use_utf8_graphics {
            // Use UTF-8 bullets for unordered lists
            if marker.starts_with('-') || marker.starts_with('*') || marker.starts_with('+') {
                "• ".to_string() // UTF-8 bullet point
            } else {
                // Keep numbered list markers as-is
                marker.to_string()
            }
        } else {
            marker.to_string()
        };
        spans.push(Span::styled(
            display_marker,
            Style::default().fg(Color::Yellow),
        ));
        // Style the rest as inline markdown
        spans.extend(style_inline_markdown(
            content,
            theme.base,
            theme.code,
            search_query,
        ));
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

    spans.extend(style_inline_markdown(
        content,
        base_style,
        theme.code,
        search_query,
    ));

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
fn style_inline_markdown(
    text: &str,
    base_style: Style,
    code_style: Style,
    search_query: Option<&str>,
) -> Vec<Span<'static>> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

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
    // Check if we're in visual command mode
    let in_visual_command_mode = if let Some(pane) = app.panes.focused_pane() {
        pane.view.mode == crate::app::Mode::VisualCommand
    } else {
        false
    };

    if in_visual_command_mode {
        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!("|{} ", app.visual_command_buffer),
            Style::default()
                .fg(app.theme.status_bar_fg)
                .bg(app.theme.status_bar_bg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            "(enter to run, esc to cancel)",
            Style::default()
                .fg(app.theme.status_bar_fg)
                .bg(app.theme.status_bar_bg),
        ));

        let status = Paragraph::new(Line::from(spans)).style(app.theme.base);
        frame.render_widget(status, area);
        return;
    }

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
                format!(
                    "/{} [{}/{}] ",
                    app.search_query,
                    current_idx + 1,
                    app.search_matches.len()
                )
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
            crate::app::Mode::VisualCommand => {
                let count = pane.view.selection.as_ref().map(|sel| {
                    let (start, end) = sel.range();
                    end - start + 1
                });
                ("CMD", count)
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
        crate::app::KeyPrefix::Z => "  z-",
    };

    let fold_indicator = if app.is_cursor_under_collapsed_heading() {
        "  [COLLAPSED]"
    } else if app.is_cursor_on_heading() {
        "  [FOLDABLE]"
    } else {
        // Check if cursor is anywhere under a foldable section
        if let Some(pane) = app.panes.focused_pane() {
            let cursor_line = pane.view.cursor_line;
            // Find nearest heading above
            let has_heading_above = app.doc.headings.iter().any(|h| h.line <= cursor_line);
            if has_heading_above {
                "  [IN SECTION]"
            } else {
                ""
            }
        } else {
            ""
        }
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

    // If there's a status message, display it prominently
    if let Some((message, kind)) = &app.status_message {
        use ratatui::style::Color;

        let (fg_color, bg_color, prefix) = match kind {
            crate::app::StatusMessageKind::Error => (Color::White, Color::Red, "ERROR: "),
            crate::app::StatusMessageKind::Success => (Color::Black, Color::Green, "SUCCESS: "),
            crate::app::StatusMessageKind::Info => (Color::Black, Color::Cyan, "INFO: "),
        };

        let status_text = format!(" {}{}", prefix, message);
        let status = Paragraph::new(Line::from(vec![Span::styled(
            status_text,
            Style::default()
                .fg(fg_color)
                .bg(bg_color)
                .add_modifier(Modifier::BOLD),
        )]));

        frame.render_widget(status, area);
        return;
    }

    // Normal status bar
    let status_text = format!(
        " mdx  {}  {} lines  {} headings  {}:{}/{}  [{}{}]{}  [{}]{}{}{}{}",
        filename,
        line_count,
        heading_count,
        filename,
        current_line,
        line_count,
        mode_str,
        selection_str,
        toc_indicator,
        theme_str,
        prefix_str,
        watch_str,
        search_str,
        fold_indicator
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

fn render_command_output(frame: &mut Frame, app: &App) {
    if let Some(output) = &app.command_output {
        let area = frame.area();
        let mut lines = Vec::new();
        lines.push(Line::from(Span::styled(
            format!("| {} ", output.command),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            "(press any key to return)",
            Style::default().fg(Color::DarkGray),
        )));

        if output.output.is_empty() {
            lines.push(Line::from(Span::styled(
                "<no output>",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for line in output.output.lines() {
                lines.push(Line::from(Span::raw(line.to_string())));
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::LightBlue))
            .title(" Command Output ");

        let paragraph = Paragraph::new(lines).block(block).style(app.theme.base);

        frame.render_widget(Clear, area);
        frame.render_widget(paragraph, area);
    }
}

fn render_help_popup(frame: &mut Frame, _app: &App) {
    use ratatui::widgets::{Clear, Paragraph};

    // Create a centered popup area
    let area = frame.area();
    let popup_width = 70.min(area.width.saturating_sub(4));
    let popup_height = 52.min(area.height.saturating_sub(4));

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
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  j/k, ↓/↑          Move cursor down/up"),
        Line::from("  Ctrl+d/u          Scroll half page down/up"),
        Line::from("  Space, PgDn       Scroll full page down"),
        Line::from("  PgUp              Scroll full page up"),
        Line::from("  g, Home           Go to top"),
        Line::from("  G, End            Go to bottom"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Search",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  /                 Start search"),
        Line::from("  n                 Next match"),
        Line::from("  N                 Previous match"),
        Line::from("  Esc               Cancel search"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Visual Mode",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  V                 Enter visual line mode"),
        Line::from("  Y                 Yank (copy) selected lines"),
        Line::from("  Esc               Exit visual mode"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Folding",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ←                 Collapse current section"),
        Line::from("  →                 Expand current section"),
        Line::from("  za                Toggle fold of current section"),
        Line::from("  zo                Open fold of current section"),
        Line::from("  zc                Close fold of current section"),
        Line::from("  zM                Close all folds"),
        Line::from("  zR                Open all folds"),
        Line::from("  Note: Works on heading or anywhere in section"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Panes",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Ctrl+w s          Split horizontally"),
        Line::from("  Ctrl+w v          Split vertically"),
        Line::from("  Ctrl+w hjkl/↑↓←→  Move focus between panes"),
        Line::from("  Ctrl+↑↓←→         Move focus between panes"),
        Line::from("  q                 Close pane (quit if last)"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Mouse",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Click pane        Focus pane and move cursor"),
        Line::from("  Click+drag        Select text (line-based)"),
        Line::from("  Ctrl+Shift+C      Copy selection to clipboard"),
        Line::from("  Click TOC         Jump to heading"),
        Line::from("  Scroll wheel      Scroll pane or TOC"),
        Line::from("  Drag border       Resize split panes"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Other",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  t                 Toggle TOC sidebar"),
        Line::from("  T                 Open TOC dialog (full screen)"),
        Line::from("  m                 Toggle theme (dark/light)"),
        Line::from("  O                 Open options dialog"),
        Line::from("  W                 Toggle security warnings pane"),
        Line::from("  e                 Open in $EDITOR"),
        Line::from("  r                 Toggle raw/rendered mode"),
        Line::from("  R                 Reload document"),
        Line::from("  Ctrl+L            Redraw/refresh screen"),
        Line::from("  ?                 Toggle this help"),
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
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
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
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .style(Style::default().bg(Color::Rgb(30, 34, 42)));

    frame.render_widget(popup, popup_area);
}

fn render_options_dialog(frame: &mut Frame, app: &App) {
    use ratatui::widgets::{Clear, Paragraph};

    let Some(ref dialog) = app.options_dialog else {
        return;
    };

    // Create a centered popup area
    let area = frame.area();
    let popup_width = 60.min(area.width.saturating_sub(4));
    let popup_height = 25.min(area.height.saturating_sub(4));

    let popup_area = ratatui::layout::Rect {
        x: (area.width.saturating_sub(popup_width)) / 2,
        y: (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    // Find maximum label width for alignment
    let max_label_width = dialog
        .fields
        .iter()
        .map(|f| f.label().len())
        .max()
        .unwrap_or(0);

    // Build option lines with aligned columns
    let mut option_lines = vec![];

    for (idx, field) in dialog.fields.iter().enumerate() {
        let is_selected = idx == dialog.selected_index;
        let label = field.label();
        let value = dialog.get_value_string(field);

        // Pad label to max width for alignment
        let padded_label = format!("{:width$}", label, width = max_label_width);

        let label_style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let value_style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::Cyan)
        };

        option_lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(padded_label, label_style),
            Span::raw(": "),
            Span::styled(value, value_style),
        ]));
    }

    option_lines.push(Line::from(""));
    option_lines.push(Line::from(""));
    option_lines.push(Line::from(vec![Span::styled(
        "↑/↓: navigate  ←/→: change  Tab: select button  Enter: execute",
        Style::default().fg(Color::DarkGray),
    )]));

    // Buttons line
    let buttons_line = {
        let cancel_style = if matches!(
            dialog.focused_button,
            crate::options_dialog::DialogButton::Cancel
        ) {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::White)
        };

        let ok_style = if matches!(
            dialog.focused_button,
            crate::options_dialog::DialogButton::Ok
        ) {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::White)
        };

        let save_style = if matches!(
            dialog.focused_button,
            crate::options_dialog::DialogButton::Save
        ) {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(Color::White)
        };

        Line::from(vec![
            Span::raw("  "),
            Span::styled("[ Cancel ]", cancel_style),
            Span::raw("  "),
            Span::styled("[ Ok ]", ok_style),
            Span::raw("  "),
            Span::styled("[ Save ]", save_style),
        ])
    };

    option_lines.push(buttons_line);

    // Clear the background
    frame.render_widget(Clear, popup_area);

    // Render the popup
    let popup = Paragraph::new(option_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Options - Press O or Esc to close ")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .style(Style::default().bg(Color::Rgb(30, 34, 42)));

    frame.render_widget(popup, popup_area);
}

/// Render image (metadata placeholder)
#[cfg(feature = "images")]
fn render_image(
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
    // Try to read image metadata
    let metadata_result = try_load_image(app, image, content_area);

    match metadata_result {
        Ok(Some(metadata)) => {
            // Successfully read - show placeholder with image info
            render_image_info_placeholder(
                app,
                image,
                &metadata,
                source_line,
                line_num_width,
                is_focused,
                cursor,
                selection_range,
                left_margin_width,
            )
        }
        _ => {
            // Failed to read - show placeholder
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

/// Try to read image metadata
#[cfg(feature = "images")]
fn try_load_image(
    app: &App,
    image: &mdx_core::image::ImageNode,
    _content_area: ratatui::layout::Rect,
) -> anyhow::Result<Option<crate::image_cache::ImageMetadata>> {
    use mdx_core::image::ImageSource;

    // Resolve image source
    let allow_absolute = app.config.images.allow_absolute && !app.config.security.safe_mode;
    let allow_remote = app.config.images.allow_remote && !app.config.security.safe_mode;
    let source = image.resolve_with_policy(&app.doc.path, allow_absolute, allow_remote);

    let source = match source {
        Some(s) => s,
        None => return Ok(None),
    };

    // Read metadata based on source type
    let metadata = match source {
        ImageSource::Local(path) => {
            if app.config.images.max_bytes > 0 {
                if let Ok(meta) = std::fs::metadata(&path) {
                    if meta.len() > app.config.images.max_bytes {
                        return Ok(None);
                    }
                }
            }
            crate::image_cache::ImageMetadata::from_path(&path)?
        }
        ImageSource::Remote(_url) => {
            // Don't fetch remote images
            return Ok(None);
        }
    };

    Ok(Some(metadata))
}

/// Render placeholder with image information
#[cfg(feature = "images")]
fn render_image_info_placeholder(
    app: &App,
    image: &mdx_core::image::ImageNode,
    metadata: &crate::image_cache::ImageMetadata,
    source_line: usize,
    line_num_width: usize,
    is_focused: bool,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
    _left_margin_width: u16,
) -> (Vec<Line<'static>>, usize) {
    let mut lines = Vec::new();

    let alt_text = if image.alt.is_empty() {
        "Image"
    } else {
        &image.alt
    };
    let alt_text = sanitize_for_terminal(alt_text);

    // Format image information
    let info_text = format!("🖼  {} | {}x{}", alt_text, metadata.width, metadata.height);

    // Check if this line is selected
    let is_selected = if let Some((start, end)) = selection_range {
        source_line >= start && source_line <= end
    } else {
        false
    };

    // Show informative placeholder - just show single line with info
    let mut line_spans: Vec<Span> = Vec::new();

    // Line number
    let line_num = format!("{:>width$} ", source_line + 1, width = line_num_width);
    let line_num_color = if is_focused && source_line == cursor {
        Color::White
    } else {
        Color::DarkGray
    };
    line_spans.push(Span::styled(line_num, Style::default().fg(line_num_color)));

    // Git diff gutter
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

    // Add placeholder content - just the info text without borders
    line_spans.push(Span::styled(
        info_text.clone(),
        Style::default()
            .fg(Color::Rgb(100, 200, 255))
            .bg(Color::Rgb(30, 40, 50))
            .add_modifier(Modifier::BOLD),
    ));

    // Apply highlighting directly to spans - priority order: selection > cursor
    if is_focused && is_selected {
        // Visual line selection: apply cyan background to each span
        line_spans = line_spans
            .into_iter()
            .map(|mut span| {
                let new_style = span.style.bg(Color::Cyan).fg(Color::Black);
                span.style = new_style;
                span
            })
            .collect();
    } else if is_focused && source_line == cursor {
        // Cursor line: apply cursor background to each span
        line_spans = line_spans
            .into_iter()
            .map(|mut span| {
                let new_style = span.style.bg(app.theme.cursor_line_bg);
                span.style = new_style;
                span
            })
            .collect();
    }

    let line = Line::from(line_spans);
    lines.push(line);

    (lines, 1)
}

/// Render image placeholder when image cannot be loaded
#[cfg(feature = "images")]
fn render_image_placeholder(
    app: &App,
    _content_area: ratatui::layout::Rect,
    source_line: usize,
    image: &mdx_core::image::ImageNode,
    line_num_width: usize,
    is_focused: bool,
    cursor: usize,
    selection_range: Option<(usize, usize)>,
    _left_margin_width: u16,
) -> (Vec<Line<'static>>, usize) {
    let mut lines = Vec::new();

    let alt_text = if image.alt.is_empty() {
        "Image"
    } else {
        &image.alt
    };
    let alt_text = sanitize_for_terminal(alt_text);

    // Format error message
    let info_text = format!("🖼  {} | [unable to read]", alt_text);

    // Check if this line is selected
    let is_selected = if let Some((start, end)) = selection_range {
        source_line >= start && source_line <= end
    } else {
        false
    };

    // Show simple single-line placeholder
    let mut line_spans: Vec<Span> = Vec::new();

    // Line number
    let line_num = format!("{:>width$} ", source_line + 1, width = line_num_width);
    let line_num_color = if is_focused && source_line == cursor {
        Color::White
    } else {
        Color::DarkGray
    };
    line_spans.push(Span::styled(line_num, Style::default().fg(line_num_color)));

    // Git diff gutter
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

    // Add error placeholder
    line_spans.push(Span::styled(
        info_text,
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    ));

    // Apply highlighting directly to spans - priority order: selection > cursor
    if is_focused && is_selected {
        // Visual line selection: apply cyan background to each span
        line_spans = line_spans
            .into_iter()
            .map(|mut span| {
                let new_style = span.style.bg(Color::Cyan).fg(Color::Black);
                span.style = new_style;
                span
            })
            .collect();
    } else if is_focused && source_line == cursor {
        // Cursor line: apply cursor background to each span
        line_spans = line_spans
            .into_iter()
            .map(|mut span| {
                let new_style = span.style.bg(app.theme.cursor_line_bg);
                span.style = new_style;
                span
            })
            .collect();
    }

    let line = Line::from(line_spans);
    lines.push(line);

    (lines, 1)
}

#[cfg(test)]
mod security_tests {
    use super::sanitize_for_terminal;

    #[test]
    fn security_sanitises_control_characters() {
        let input = "safe\x1b[31mred\x07text";
        let output = sanitize_for_terminal(input);
        assert!(!output.contains('\x1b'));
        assert!(!output.contains('\x07'));
    }

    #[test]
    fn security_allows_utf8_characters() {
        // Test that UTF-8 box-drawing characters are preserved
        let input = "│─┌┐└┘• Text";
        let output = sanitize_for_terminal(input);
        assert_eq!(input, output, "UTF-8 characters should be preserved");

        // Verify specific characters
        assert!(output.contains('│'));
        assert!(output.contains('─'));
        assert!(output.contains('•'));
    }

    #[cfg(feature = "images")]
    #[test]
    fn security_image_size_limit_blocks_metadata() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"not a real image").unwrap();
        file.flush().unwrap();

        let mut doc_file = NamedTempFile::new().unwrap();
        writeln!(doc_file, "![alt]({})", file.path().display()).unwrap();
        doc_file.flush().unwrap();

        let (doc, _warnings) = Document::load(doc_file.path()).unwrap();
        let mut config = Config::default();
        config.images.enabled = true;
        config.images.allow_absolute = true;
        config.images.max_bytes = 1;

        let app = App::new(config, doc, vec![]);
        let image = app.doc.images.first().unwrap();
        let result = super::try_load_image(&app, image, ratatui::layout::Rect::default()).unwrap();

        assert!(result.is_none());
    }
}

#[cfg(test)]
mod utf8_rendering_tests {
    use super::style_markdown_line;
    use crate::theme::Theme;
    use mdx_core::config::Config;
    use ratatui::style::Color;

    fn get_text_from_spans(spans: &[ratatui::text::Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_horizontal_rule_utf8() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = true;

        let line = "---";
        let spans = style_markdown_line(line, &theme, &config.render, None);
        let output = get_text_from_spans(&spans);

        // Should be UTF-8 horizontal lines
        assert_eq!(output, "───");
    }

    #[test]
    fn test_horizontal_rule_ascii() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = false;

        let line = "---";
        let spans = style_markdown_line(line, &theme, &config.render, None);
        let output = get_text_from_spans(&spans);

        // Should remain as ASCII
        assert_eq!(output, "---");
    }

    #[test]
    fn test_table_separator_utf8() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = true;

        let line = "| Header 1 | Header 2 |";
        let spans = style_markdown_line(line, &theme, &config.render, None);
        let output = get_text_from_spans(&spans);

        // Should use UTF-8 vertical bars
        assert!(output.contains('│'));
        assert!(!output.contains('|'));
    }

    #[test]
    fn test_table_separator_ascii() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = false;

        let line = "| Header 1 | Header 2 |";
        let spans = style_markdown_line(line, &theme, &config.render, None);
        let output = get_text_from_spans(&spans);

        // Should remain as ASCII pipe
        assert!(output.contains('|'));
        assert!(!output.contains('│'));
    }

    #[test]
    fn test_table_alignment_utf8() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = true;

        let line = "|:---|---:|:---:|";
        let spans = style_markdown_line(line, &theme, &config.render, None);
        let output = get_text_from_spans(&spans);

        // Should use UTF-8 horizontal lines for separators
        assert!(output.contains('─'));
        assert!(output.contains(':'));
    }

    #[test]
    fn test_unordered_list_utf8() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = true;

        let test_cases = vec!["- Item 1", "* Item 2", "+ Item 3"];

        for line in test_cases {
            let spans = style_markdown_line(line, &theme, &config.render, None);
            let output = get_text_from_spans(&spans);

            // Should use UTF-8 bullet point
            assert!(output.contains('•'), "Failed for line: {}", line);
            assert!(
                !output.starts_with('-') && !output.starts_with('*') && !output.starts_with('+'),
                "Should not start with ASCII markers for: {}",
                line
            );
        }
    }

    #[test]
    fn test_unordered_list_ascii() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = false;

        let line = "- Item 1";
        let spans = style_markdown_line(line, &theme, &config.render, None);
        let output = get_text_from_spans(&spans);

        // Should remain as ASCII
        assert!(output.starts_with("- "));
        assert!(!output.contains('•'));
    }

    #[test]
    fn test_ordered_list_unchanged() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = true;

        let line = "1. First item";
        let spans = style_markdown_line(line, &theme, &config.render, None);
        let output = get_text_from_spans(&spans);

        // Ordered lists should keep their numbers
        assert!(output.starts_with("1. "));
        assert!(!output.contains('•'));
    }

    #[test]
    fn test_default_config_uses_utf8() {
        let config = Config::default();
        // Default should enable UTF-8 graphics
        assert!(config.render.use_utf8_graphics);
    }

    #[test]
    fn test_utf8_preserves_styling() {
        let theme = Theme::dark();
        let mut config = Config::default();
        config.render.use_utf8_graphics = true;

        let line = "| Header 1 | Header 2 |";
        let spans = style_markdown_line(line, &theme, &config.render, None);

        // Verify we have multiple spans (content + separators)
        assert!(spans.len() > 1);

        // Find the separator span
        let separator_span = spans.iter().find(|s| s.content.as_ref() == "│").unwrap();

        // Verify separator has cyan color
        assert_eq!(separator_span.style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_default_config_has_utf8_enabled() {
        let config = Config::default();

        let theme = Theme::dark();
        let line = "| Col1 | Col2 |";
        let spans = style_markdown_line(line, &theme, &config.render, None);
        let output = get_text_from_spans(&spans);

        // With default config (UTF-8 enabled), should have UTF-8 chars
        assert!(
            output.contains('│'),
            "Expected UTF-8 vertical bar '│' in output: {}",
            output
        );
    }
}
