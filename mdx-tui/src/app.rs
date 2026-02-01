//! Application state

use crate::panes::{PaneId, PaneManager};
use crate::theme::Theme;
use log::info;
use mdx_core::{
    config::ThemeVariant, detect_front_matter, Config, Document, FrontMatter, LineSelection,
};
use ratatui::layout::Rect;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    VisualLine,
    Search,
    VisualCommand,
}

/// Mouse interaction state
#[derive(Debug, Clone, PartialEq)]
pub enum MouseState {
    Idle,
    PendingSelection {
        pane_id: PaneId,
        anchor_line: usize,
    },
    Selecting {
        pane_id: PaneId,
        anchor_line: usize,
    },
    Resizing {
        split_path: Vec<usize>, // Path to the split being resized
        start_ratio: f32,
        start_pos: (u16, u16), // Starting mouse position
    },
}

/// Key prefix state for multi-key sequences
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPrefix {
    None,
    CtrlW,
    Z, // For fold commands (za, zo, zc, zM, zR)
}

/// View state for a document viewport
#[derive(Debug, Clone)]
pub struct ViewState {
    pub scroll_line: usize,
    pub cursor_line: usize,
    pub mode: Mode,
    pub selection: Option<LineSelection>,
    pub show_raw: bool, // Toggle between rendered markdown and raw text
    pub collapsed_headings: std::collections::BTreeSet<usize>, // Line numbers of collapsed headings
}

impl ViewState {
    /// Create a new view state at the top of the document
    pub fn new() -> Self {
        Self {
            scroll_line: 0,
            cursor_line: 0,
            mode: Mode::Normal,
            selection: None,
            show_raw: false,
            collapsed_headings: std::collections::BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PaneViewport {
    pub visible_height: usize,
    pub content_width: usize,
}

#[derive(Debug, Clone)]
pub struct LayoutContext {
    viewports: HashMap<PaneId, PaneViewport>,
}

impl LayoutContext {
    pub fn new() -> Self {
        Self {
            viewports: HashMap::new(),
        }
    }

    pub fn update(
        &mut self,
        layout: &HashMap<PaneId, Rect>,
        doc_line_count: usize,
        show_scrollbar_flag: bool,
    ) {
        self.viewports.clear();
        for (pane_id, rect) in layout {
            let viewport = PaneViewport::from_rect(*rect, doc_line_count, show_scrollbar_flag);
            self.viewports.insert(*pane_id, viewport);
        }
    }

    pub fn focused_viewport(&self, pane_id: PaneId) -> Option<PaneViewport> {
        self.viewports.get(&pane_id).copied()
    }
}

impl PaneViewport {
    fn from_rect(rect: Rect, doc_line_count: usize, show_scrollbar_flag: bool) -> Self {
        let content_area_height = rect.height.saturating_sub(1);
        let visible_height = content_area_height.saturating_sub(2) as usize;
        let mut content_width = rect.width.saturating_sub(2);
        let has_scrollbar = show_scrollbar_flag && doc_line_count > visible_height;
        if has_scrollbar {
            content_width = content_width.saturating_sub(1);
        }

        Self {
            visible_height,
            content_width: content_width as usize,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub command: String,
    pub output: String,
}

/// Type of status message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusMessageKind {
    Info,
    Success,
    Error,
}

/// Main application state
pub struct App {
    pub config: Config,
    pub doc: Document,
    pub front_matter: Option<FrontMatter>,
    pub panes: PaneManager,
    pub theme: Theme,
    pub theme_variant: ThemeVariant,
    pub show_toc: bool,
    pub toc_focus: bool,
    pub toc_selected: usize,
    pub toc_scroll: usize,
    pub show_toc_dialog: bool,
    pub toc_dialog_selected: usize,
    pub toc_dialog_scroll: usize,
    pub key_prefix: KeyPrefix,
    pub should_quit: bool,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub search_current_match: Option<usize>,
    pub show_help: bool,
    pub options_dialog: Option<crate::options_dialog::OptionsDialog>,
    pub security_warnings: Vec<mdx_core::SecurityEvent>,
    pub show_security_warnings: bool,
    pub status_message: Option<(String, StatusMessageKind)>,
    pub mouse_state: MouseState,
    pub layout_context: LayoutContext,
    pub visual_command_buffer: String,
    pub command_output: Option<CommandOutput>,
    #[cfg(feature = "watch")]
    pub watcher: Option<crate::watcher::FileWatcher>,
    #[cfg(feature = "git")]
    pub diff_worker: crate::diff_worker::DiffWorker,
}

impl App {
    /// Create a new application instance with a document and security warnings
    pub fn new(config: Config, doc: Document, warnings: Vec<mdx_core::SecurityEvent>) -> Self {
        #[cfg(feature = "images")]
        let config = {
            let mut cfg = config;
            if cfg.security.safe_mode {
                cfg.images.enabled = false;
            }
            cfg
        };
        #[cfg(not(feature = "images"))]
        let config = config;

        let show_toc = config.toc.enabled;
        let theme_variant = config.theme;
        let theme = Theme::for_variant(theme_variant);
        let panes = PaneManager::new(0); // Single pane for single document
        let show_security_warnings = !warnings.is_empty();

        #[cfg(feature = "watch")]
        let watcher = if config.watch.enabled {
            crate::watcher::FileWatcher::new(&doc.path).ok()
        } else {
            None
        };

        #[cfg(feature = "git")]
        let diff_worker = {
            let worker = crate::diff_worker::DiffWorker::spawn();
            // Send initial diff request
            if config.git.diff {
                let current_text: String = doc.rope.chunks().collect();
                worker.request_diff(crate::diff_worker::DiffRequest {
                    doc_id: 0,
                    path: doc.path.clone(),
                    rev: doc.rev,
                    current_text,
                });
            }
            worker
        };

        let mut app = Self {
            config,
            doc,
            front_matter: None,
            panes,
            theme,
            theme_variant,
            show_toc,
            toc_focus: false,
            toc_selected: 0,
            toc_scroll: 0,
            show_toc_dialog: false,
            toc_dialog_selected: 0,
            toc_dialog_scroll: 0,
            key_prefix: KeyPrefix::None,
            should_quit: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_current_match: None,
            show_help: false,
            options_dialog: None,
            security_warnings: warnings,
            show_security_warnings,
            status_message: None,
            mouse_state: MouseState::Idle,
            layout_context: LayoutContext::new(),
            visual_command_buffer: String::new(),
            command_output: None,
            #[cfg(feature = "watch")]
            watcher,
            #[cfg(feature = "git")]
            diff_worker,
        };

        app.refresh_front_matter_info();

        app
    }

    fn refresh_front_matter_info(&mut self) {
        if self.config.render.skip_front_matter {
            self.front_matter = detect_front_matter(&self.doc.rope);
            if let Some(fm) = self.front_matter {
                info!("Skipping {} front matter", fm.kind);
            }
        } else {
            self.front_matter = None;
        }
        self.enforce_rendered_bounds();
    }

    pub(crate) fn rendered_content_bounds(&self) -> (usize, usize) {
        let line_count = self.doc.line_count();
        if line_count == 0 {
            return (0, 0);
        }

        let max_line = line_count - 1;
        let start_line = self
            .front_matter
            .map(|fm| (fm.end_line + 1).min(line_count))
            .unwrap_or(0);

        if start_line > max_line {
            (max_line, max_line)
        } else {
            (start_line, max_line)
        }
    }

    fn enforce_rendered_bounds(&mut self) {
        let bounds = self.rendered_content_bounds();

        for pane in self.panes.panes.values_mut() {
            // Clamp cursor to valid bounds
            pane.view.cursor_line = pane.view.cursor_line.clamp(bounds.0, bounds.1);

            // Clamp scroll to valid bounds (but don't aggressively adjust to prevent empty space
            // at the bottom - that would interfere with TOC jumps that want heading at top)
            pane.view.scroll_line = pane.view.scroll_line.clamp(bounds.0, bounds.1);
        }
    }

    pub fn update_layout_context(&mut self, layout: &HashMap<PaneId, Rect>) {
        self.layout_context.update(
            layout,
            self.doc.line_count(),
            self.config.render.show_scrollbar,
        );
    }

    pub fn focused_viewport(&self) -> Option<PaneViewport> {
        self.layout_context.focused_viewport(self.panes.focused)
    }

    /// Set an error message to display in the status bar
    pub fn set_error_message(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), StatusMessageKind::Error));
    }

    /// Set a success message to display in the status bar
    pub fn set_success_message(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), StatusMessageKind::Success));
    }

    /// Set an info message to display in the status bar
    pub fn set_info_message(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), StatusMessageKind::Info));
    }

    /// Clear the status message
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    /// Toggle help dialog
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Open options dialog
    pub fn open_options(&mut self) {
        self.options_dialog = Some(crate::options_dialog::OptionsDialog::new(&self.config));
    }

    /// Close options dialog without applying changes
    pub fn close_options(&mut self) {
        self.options_dialog = None;
    }

    /// Apply options from dialog (Ok button)
    pub fn apply_options(&mut self) {
        if let Some(dialog) = &self.options_dialog {
            self.config = dialog.get_config();
            self.refresh_front_matter_info();
            // Update theme if it changed
            if self.config.theme != self.theme_variant {
                self.theme_variant = self.config.theme;
                self.theme = crate::theme::Theme::for_variant(self.theme_variant);
            }
            // Update TOC visibility
            self.show_toc = self.config.toc.enabled;
        }
        self.options_dialog = None;
    }

    /// Save options to config file (Save button)
    pub fn save_options(&mut self) -> anyhow::Result<()> {
        if let Some(dialog) = &self.options_dialog {
            let new_config = dialog.get_config();
            // Save to file
            mdx_core::Config::save_to_file(&new_config)?;
            // Apply changes
            self.config = new_config;
            self.refresh_front_matter_info();
            // Update theme if it changed
            if self.config.theme != self.theme_variant {
                self.theme_variant = self.config.theme;
                self.theme = crate::theme::Theme::for_variant(self.theme_variant);
            }
            // Update TOC visibility
            self.show_toc = self.config.toc.enabled;
        }
        self.options_dialog = None;
        Ok(())
    }

    /// Toggle security warnings pane
    pub fn toggle_security_warnings(&mut self) {
        self.show_security_warnings = !self.show_security_warnings;
    }

    /// Add a security warning event
    pub fn add_security_warning(&mut self, event: mdx_core::SecurityEvent) {
        self.security_warnings.push(event);
        self.show_security_warnings = true;
    }

    /// Handle quit request
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Reload document from disk
    pub fn reload_document(&mut self) -> anyhow::Result<()> {
        self.doc.reload()?;
        self.refresh_front_matter_info();

        // Re-validate viewport positions after reload
        // Use a reasonable default height; actual height from layout context
        // will be used inside auto_scroll if available
        let default_height = 20;
        self.auto_scroll(default_height);

        // Request diff computation in background
        #[cfg(feature = "git")]
        if self.config.git.diff {
            let current_text: String = self.doc.rope.chunks().collect();
            self.diff_worker
                .request_diff(crate::diff_worker::DiffRequest {
                    doc_id: 0,
                    path: self.doc.path.clone(),
                    rev: self.doc.rev,
                    current_text,
                });
        }

        Ok(())
    }

    /// Move cursor down by n lines, skipping collapsed blocks
    pub fn move_cursor_down(&mut self, n: usize) {
        let bounds = self.rendered_content_bounds();
        if let Some(pane) = self.panes.focused_pane_mut() {
            let next_line = pane.view.cursor_line.saturating_add(n);
            pane.view.cursor_line = next_line.clamp(bounds.0, bounds.1);

            // Debug assertion: cursor should always be within bounds
            debug_assert!(
                pane.view.cursor_line >= bounds.0 && pane.view.cursor_line <= bounds.1,
                "Cursor line {} out of bounds [{}, {}]",
                pane.view.cursor_line,
                bounds.0,
                bounds.1
            );
        }
        self.adjust_cursor_for_collapsed_blocks(true);
        self.update_selection();
    }

    /// Move cursor up by n lines, skipping collapsed blocks
    pub fn move_cursor_up(&mut self, n: usize) {
        let bounds = self.rendered_content_bounds();
        if let Some(pane) = self.panes.focused_pane_mut() {
            let prev_line = pane.view.cursor_line.saturating_sub(n);
            pane.view.cursor_line = prev_line.clamp(bounds.0, bounds.1);

            // Debug assertion: cursor should always be within bounds
            debug_assert!(
                pane.view.cursor_line >= bounds.0 && pane.view.cursor_line <= bounds.1,
                "Cursor line {} out of bounds [{}, {}]",
                pane.view.cursor_line,
                bounds.0,
                bounds.1
            );
        }
        self.adjust_cursor_for_collapsed_blocks(false);
        self.update_selection();
    }

    /// Adjust cursor position if it lands inside a collapsed block
    /// moving_down: if true, cursor lands on the line after the collapsed block; if false, on the heading
    fn adjust_cursor_for_collapsed_blocks(&mut self, moving_down: bool) {
        let bounds = self.rendered_content_bounds();
        if let Some(pane) = self.panes.focused_pane_mut() {
            let cursor = pane.view.cursor_line;

            // Compute collapsed ranges
            let collapsed_ranges = crate::collapse::compute_all_collapsed_ranges(
                &pane.view.collapsed_headings,
                &self.doc,
            );

            // Check if cursor is inside a collapsed range (but not at the start)
            if let Some(range) =
                crate::collapse::find_range_containing_line(&collapsed_ranges, cursor)
            {
                if moving_down {
                    // When moving down, jump to the line after the collapsed block
                    let target_line = range.end.saturating_add(1);
                    pane.view.cursor_line = target_line.clamp(bounds.0, bounds.1);
                } else {
                    // When moving up, jump to the heading line
                    pane.view.cursor_line = range.start.clamp(bounds.0, bounds.1);
                }
            }
        }
    }

    /// Jump to specific line, expanding collapsed blocks if necessary
    pub fn jump_to_line(&mut self, line: usize) {
        let bounds = self.rendered_content_bounds();
        if let Some(pane) = self.panes.focused_pane_mut() {
            let target_line = line.clamp(bounds.0, bounds.1);

            // Expand ALL collapsed blocks that contain the target line
            // This handles nested collapsed headings (e.g., "## Parent" contains "### Child")
            // We need multiple passes because removing one heading may reveal others
            loop {
                let collapsed_ranges = crate::collapse::compute_all_collapsed_ranges(
                    &pane.view.collapsed_headings,
                    &self.doc,
                );

                // Find any collapsed range containing the target
                let containing_range = collapsed_ranges
                    .iter()
                    .find(|r| r.contains_line(target_line) || r.start == target_line);

                if let Some(range) = containing_range {
                    // Expand this collapsed heading
                    pane.view.collapsed_headings.remove(&range.start);
                } else {
                    // No more collapsed ranges containing target
                    break;
                }
            }

            pane.view.cursor_line = target_line;
        }
        self.update_selection();
    }

    /// Calculate how many source lines to move for a given visual line count
    /// This accounts for line wrapping by estimating wrapped lines
    fn calculate_source_lines_for_visual_lines(
        &self,
        visual_lines: usize,
        viewport_width: usize,
        forward: bool,
    ) -> usize {
        if let Some(pane) = self.panes.focused_pane() {
            let start_line = if forward {
                pane.view.cursor_line
            } else {
                pane.view.cursor_line.saturating_sub(visual_lines)
            };

            // Use actual content width from layout context if available
            // This accounts for split panes having smaller widths
            let content_width = self
                .focused_viewport()
                .map(|v| v.content_width)
                .filter(|&w| w > 0)
                .unwrap_or_else(|| viewport_width.saturating_sub(10)); // Fallback estimate

            if content_width < 40 {
                // Very narrow viewport, fallback to 1:1 mapping
                return visual_lines;
            }

            let mut visual_count = 0;
            let mut source_count = 0;
            let line_count = self.doc.line_count();

            while visual_count < visual_lines && start_line + source_count < line_count {
                let line_idx = if forward {
                    start_line + source_count
                } else {
                    start_line.saturating_sub(source_count).min(start_line)
                };

                if line_idx >= line_count {
                    break;
                }

                // Get line length
                let line_text: String = self.doc.rope.line(line_idx).chars().collect();
                let line_len = line_text.chars().count();

                // Estimate wrapped lines (simple heuristic)
                let wrapped_lines = if line_len == 0 {
                    1
                } else {
                    ((line_len + content_width - 1) / content_width).max(1)
                };

                visual_count += wrapped_lines;
                source_count += 1;

                if visual_count >= visual_lines {
                    break;
                }
            }

            source_count.max(1)
        } else {
            visual_lines
        }
    }

    /// Scroll down by half viewport height (accounting for wrapping)
    pub fn scroll_half_page_down(&mut self, viewport_height: usize, viewport_width: usize) {
        let half_page = viewport_height / 2;
        let source_lines =
            self.calculate_source_lines_for_visual_lines(half_page, viewport_width, true);
        self.move_cursor_down(source_lines);
    }

    /// Scroll up by half viewport height (accounting for wrapping)
    pub fn scroll_half_page_up(&mut self, viewport_height: usize, viewport_width: usize) {
        let half_page = viewport_height / 2;
        let source_lines =
            self.calculate_source_lines_for_visual_lines(half_page, viewport_width, false);
        self.move_cursor_up(source_lines);
    }

    /// Auto-scroll viewport to keep cursor visible
    ///
    /// Uses a very conservative height estimate (40%) to account for line wrapping
    /// in rendered markdown mode. Lines can wrap to 2-3x their original height,
    /// so we assume only ~40% of display rows map to source lines.
    pub fn auto_scroll(&mut self, viewport_height: usize) {
        // Get actual pane height from layout context if available
        let actual_height = self
            .focused_viewport()
            .map(|v| v.visible_height)
            .filter(|&h| h > 0)
            .unwrap_or(viewport_height);

        // Use a very conservative 40% estimate for line wrapping.
        // In rendered mode, markdown bullets, long lines, and styling cause
        // significant wrapping. 40% ensures the cursor stays visible even
        // with heavy wrapping (e.g., narrow panes with TOC open).
        let effective_height = (actual_height * 40 / 100).max(3);

        if let Some(pane) = self.panes.focused_pane_mut() {
            let cursor = pane.view.cursor_line;
            let scroll = pane.view.scroll_line;

            // Cursor above viewport - scroll up
            if cursor < scroll {
                pane.view.scroll_line = cursor;
            }
            // Cursor below viewport - scroll down
            else if cursor >= scroll + effective_height {
                pane.view.scroll_line = cursor.saturating_sub(effective_height.saturating_sub(1));
            }
        }
    }

    /// Toggle between dark and light themes
    pub fn toggle_theme(&mut self) {
        self.theme_variant = match self.theme_variant {
            ThemeVariant::Dark => ThemeVariant::Light,
            ThemeVariant::Light => ThemeVariant::Dark,
        };
        self.theme = Theme::for_variant(self.theme_variant);
    }

    /// Toggle TOC visibility and focus
    pub fn toggle_toc(&mut self) {
        if self.show_toc {
            // If already shown, hide it
            self.show_toc = false;
            self.toc_focus = false;
        } else {
            // Show and focus TOC
            self.show_toc = true;
            self.toc_focus = true;
        }
    }

    /// Move TOC selection down
    pub fn toc_move_down(&mut self, toc_height: usize) {
        if !self.doc.headings.is_empty() {
            self.toc_selected = (self.toc_selected + 1).min(self.doc.headings.len() - 1);
            self.toc_auto_scroll(toc_height);
        }
    }

    /// Move TOC selection up
    pub fn toc_move_up(&mut self, toc_height: usize) {
        self.toc_selected = self.toc_selected.saturating_sub(1);
        self.toc_auto_scroll(toc_height);
    }

    /// Move TOC selection down by half page
    pub fn toc_scroll_half_page_down(&mut self, toc_height: usize) {
        if !self.doc.headings.is_empty() {
            let jump = (toc_height / 2).max(1);
            self.toc_selected = (self.toc_selected + jump).min(self.doc.headings.len() - 1);
            self.toc_auto_scroll(toc_height);
        }
    }

    /// Move TOC selection up by half page
    pub fn toc_scroll_half_page_up(&mut self, toc_height: usize) {
        let jump = (toc_height / 2).max(1);
        self.toc_selected = self.toc_selected.saturating_sub(jump);
        self.toc_auto_scroll(toc_height);
    }

    /// Move TOC selection down by full page
    pub fn toc_scroll_full_page_down(&mut self, toc_height: usize) {
        if !self.doc.headings.is_empty() {
            let jump = toc_height.max(1);
            self.toc_selected = (self.toc_selected + jump).min(self.doc.headings.len() - 1);
            self.toc_auto_scroll(toc_height);
        }
    }

    /// Move TOC selection up by full page
    pub fn toc_scroll_full_page_up(&mut self, toc_height: usize) {
        let jump = toc_height.max(1);
        self.toc_selected = self.toc_selected.saturating_sub(jump);
        self.toc_auto_scroll(toc_height);
    }

    /// Jump to top of TOC
    pub fn toc_jump_to_top(&mut self, toc_height: usize) {
        self.toc_selected = 0;
        self.toc_auto_scroll(toc_height);
    }

    /// Jump to bottom of TOC
    pub fn toc_jump_to_bottom(&mut self, toc_height: usize) {
        if !self.doc.headings.is_empty() {
            self.toc_selected = self.doc.headings.len() - 1;
            self.toc_auto_scroll(toc_height);
        }
    }

    /// Auto-scroll TOC to keep selection visible
    pub fn toc_auto_scroll(&mut self, toc_height: usize) {
        let selected = self.toc_selected;
        let scroll = self.toc_scroll;

        // Selection above viewport - scroll up
        if selected < scroll {
            self.toc_scroll = selected;
        }
        // Selection below viewport - scroll down
        else if selected >= scroll + toc_height {
            self.toc_scroll = selected.saturating_sub(toc_height - 1);
        }
    }

    /// Jump to the selected heading in TOC, making it the top line
    pub fn toc_jump_to_selected(&mut self) {
        if let Some(heading) = self.doc.headings.get(self.toc_selected) {
            let target_line = heading.line;
            let bounds = self.rendered_content_bounds();

            if let Some(pane) = self.panes.focused_pane_mut() {
                let clamped_target = target_line.clamp(bounds.0, bounds.1);

                // Expand ALL collapsed blocks that contain the target line
                loop {
                    let collapsed_ranges = crate::collapse::compute_all_collapsed_ranges(
                        &pane.view.collapsed_headings,
                        &self.doc,
                    );

                    let containing_range = collapsed_ranges
                        .iter()
                        .find(|r| r.contains_line(clamped_target) || r.start == clamped_target);

                    if let Some(range) = containing_range {
                        pane.view.collapsed_headings.remove(&range.start);
                    } else {
                        break;
                    }
                }

                // Set BOTH cursor and scroll in the same block to ensure consistency
                pane.view.cursor_line = clamped_target;
                pane.view.scroll_line = clamped_target;
            }
        }
        self.update_selection();
    }

    /// Toggle TOC dialog
    pub fn toggle_toc_dialog(&mut self) {
        self.show_toc_dialog = !self.show_toc_dialog;
        if self.show_toc_dialog {
            // Reset selection when opening
            self.toc_dialog_selected = 0;
            self.toc_dialog_scroll = 0;
        }
    }

    /// Move TOC dialog selection down
    pub fn toc_dialog_move_down(&mut self, dialog_height: usize) {
        if !self.doc.headings.is_empty() {
            self.toc_dialog_selected =
                (self.toc_dialog_selected + 1).min(self.doc.headings.len() - 1);
            self.toc_dialog_auto_scroll(dialog_height);
        }
    }

    /// Move TOC dialog selection up
    pub fn toc_dialog_move_up(&mut self, dialog_height: usize) {
        self.toc_dialog_selected = self.toc_dialog_selected.saturating_sub(1);
        self.toc_dialog_auto_scroll(dialog_height);
    }

    /// Move TOC dialog selection down by half page
    pub fn toc_dialog_scroll_half_page_down(&mut self, dialog_height: usize) {
        if !self.doc.headings.is_empty() {
            let jump = (dialog_height / 2).max(1);
            self.toc_dialog_selected =
                (self.toc_dialog_selected + jump).min(self.doc.headings.len() - 1);
            self.toc_dialog_auto_scroll(dialog_height);
        }
    }

    /// Move TOC dialog selection up by half page
    pub fn toc_dialog_scroll_half_page_up(&mut self, dialog_height: usize) {
        let jump = (dialog_height / 2).max(1);
        self.toc_dialog_selected = self.toc_dialog_selected.saturating_sub(jump);
        self.toc_dialog_auto_scroll(dialog_height);
    }

    /// Move TOC dialog selection down by full page
    pub fn toc_dialog_scroll_full_page_down(&mut self, dialog_height: usize) {
        if !self.doc.headings.is_empty() {
            let jump = dialog_height.max(1);
            self.toc_dialog_selected =
                (self.toc_dialog_selected + jump).min(self.doc.headings.len() - 1);
            self.toc_dialog_auto_scroll(dialog_height);
        }
    }

    /// Move TOC dialog selection up by full page
    pub fn toc_dialog_scroll_full_page_up(&mut self, dialog_height: usize) {
        let jump = dialog_height.max(1);
        self.toc_dialog_selected = self.toc_dialog_selected.saturating_sub(jump);
        self.toc_dialog_auto_scroll(dialog_height);
    }

    /// Jump to top of TOC dialog
    pub fn toc_dialog_jump_to_top(&mut self, dialog_height: usize) {
        self.toc_dialog_selected = 0;
        self.toc_dialog_auto_scroll(dialog_height);
    }

    /// Jump to bottom of TOC dialog
    pub fn toc_dialog_jump_to_bottom(&mut self, dialog_height: usize) {
        if !self.doc.headings.is_empty() {
            self.toc_dialog_selected = self.doc.headings.len() - 1;
            self.toc_dialog_auto_scroll(dialog_height);
        }
    }

    /// Auto-scroll TOC dialog to keep selection visible
    pub fn toc_dialog_auto_scroll(&mut self, dialog_height: usize) {
        let selected = self.toc_dialog_selected;
        let scroll = self.toc_dialog_scroll;

        // Selection above viewport - scroll up
        if selected < scroll {
            self.toc_dialog_scroll = selected;
        }
        // Selection below viewport - scroll down
        else if selected >= scroll + dialog_height {
            self.toc_dialog_scroll = selected.saturating_sub(dialog_height - 1);
        }
    }

    /// Jump to the selected heading in TOC dialog and close dialog
    pub fn toc_dialog_jump_to_selected(&mut self) {
        if let Some(heading) = self.doc.headings.get(self.toc_dialog_selected) {
            let target_line = heading.line;
            let bounds = self.rendered_content_bounds();

            if let Some(pane) = self.panes.focused_pane_mut() {
                let clamped_target = target_line.clamp(bounds.0, bounds.1);

                // Expand ALL collapsed blocks that contain the target line
                loop {
                    let collapsed_ranges = crate::collapse::compute_all_collapsed_ranges(
                        &pane.view.collapsed_headings,
                        &self.doc,
                    );

                    let containing_range = collapsed_ranges
                        .iter()
                        .find(|r| r.contains_line(clamped_target) || r.start == clamped_target);

                    if let Some(range) = containing_range {
                        pane.view.collapsed_headings.remove(&range.start);
                    } else {
                        break;
                    }
                }

                // Set BOTH cursor and scroll in the same block to ensure consistency
                pane.view.cursor_line = clamped_target;
                pane.view.scroll_line = clamped_target;
            }
        }
        self.update_selection();
        // Close the dialog
        self.show_toc_dialog = false;
    }

    /// Get the index of the current heading based on cursor position
    pub fn current_heading_index(&self) -> Option<usize> {
        if self.doc.headings.is_empty() {
            return None;
        }

        let cursor_line = self.panes.focused_pane()?.view.cursor_line;

        // Find the last heading that's at or before the cursor
        for (i, heading) in self.doc.headings.iter().enumerate().rev() {
            if heading.line <= cursor_line {
                return Some(i);
            }
        }

        None
    }

    /// Get breadcrumb path for a specific pane based on its cursor position
    pub fn get_breadcrumb_path(&self, pane_id: usize) -> Vec<String> {
        let mut breadcrumbs = Vec::new();

        if self.doc.headings.is_empty() {
            return breadcrumbs;
        }

        let pane = match self.panes.panes.get(&pane_id) {
            Some(p) => p,
            None => return breadcrumbs,
        };

        let cursor_line = pane.view.cursor_line;

        // Find the current heading
        let current_idx = self
            .doc
            .headings
            .iter()
            .enumerate()
            .rev()
            .find(|(_, h)| h.line <= cursor_line)
            .map(|(i, _)| i);

        let current_idx = match current_idx {
            Some(idx) => idx,
            None => return breadcrumbs,
        };

        // Build breadcrumb path by walking back through headings
        let current_heading = &self.doc.headings[current_idx];
        let mut path_headings = vec![current_heading];

        // Walk backwards to find parent headings
        let mut current_level = current_heading.level;
        for heading in self.doc.headings[..current_idx].iter().rev() {
            if heading.level < current_level {
                path_headings.push(heading);
                current_level = heading.level;
                if current_level == 1 {
                    break; // Stop at top-level heading
                }
            }
        }

        // Reverse to get top-down order
        path_headings.reverse();

        // Extract text
        for heading in path_headings {
            breadcrumbs.push(heading.text.clone());
        }

        breadcrumbs
    }

    /// Get git status for the document (overall file status)
    #[cfg(feature = "git")]
    pub fn get_git_status(&self) -> Option<&'static str> {
        if !self.config.git.diff {
            return None;
        }

        // Check if there are any changes in the diff gutter
        let has_added = self
            .doc
            .diff_gutter
            .marks
            .iter()
            .any(|m| matches!(m, mdx_core::diff::DiffMark::Added));
        let has_modified = self
            .doc
            .diff_gutter
            .marks
            .iter()
            .any(|m| matches!(m, mdx_core::diff::DiffMark::Modified));
        let has_deleted = self
            .doc
            .diff_gutter
            .marks
            .iter()
            .any(|m| matches!(m, mdx_core::diff::DiffMark::DeletedAfter(_)));

        // Priority: new > modified > deleted
        if has_added && !has_modified && !has_deleted {
            Some("new")
        } else if has_modified {
            Some("modified")
        } else if has_deleted {
            Some("deleted")
        } else {
            None
        }
    }

    /// Split the focused pane
    pub fn split_focused(&mut self, dir: crate::panes::SplitDir) {
        self.panes.split_focused(dir, 0); // doc_id is 0 for single document
        // Note: layout_context will be stale after this.
        // It will be refreshed on next draw() or via refresh_layout_context_with_area()
    }

    /// Refresh layout context with estimated area
    ///
    /// Call this when layout context may be stale (e.g., after splits)
    /// and you need accurate viewport dimensions before the next draw.
    pub fn refresh_layout_context_with_area(&mut self, viewport_width: u16, viewport_height: u16) {
        use ratatui::layout::Rect;

        // Estimate the pane area (accounting for status bar and TOC)
        let status_bar_height = 1;
        let toc_width = if self.show_toc {
            self.config.toc.width as u16
        } else {
            0
        };
        let security_height = if !self.security_warnings.is_empty() && self.show_security_warnings {
            4
        } else {
            0
        };

        let pane_area = Rect {
            x: if self.show_toc && self.config.toc.side == mdx_core::config::TocSide::Left {
                toc_width
            } else {
                0
            },
            y: 0,
            width: viewport_width.saturating_sub(toc_width),
            height: viewport_height.saturating_sub(status_bar_height + security_height),
        };

        let pane_layouts = self.panes.compute_layout(pane_area);
        self.update_layout_context(&pane_layouts);
    }

    /// Enter visual line mode
    pub fn enter_visual_line_mode(&mut self) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            pane.view.mode = Mode::VisualLine;
            let cursor = pane.view.cursor_line;
            pane.view.selection = Some(LineSelection::new(cursor));
        }
    }

    /// Exit visual line mode
    pub fn exit_visual_line_mode(&mut self) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            pane.view.mode = Mode::Normal;
            pane.view.selection = None;
        }
    }

    /// Update selection cursor in visual line mode
    pub fn update_selection(&mut self) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            if pane.view.mode == Mode::VisualLine {
                if let Some(ref mut selection) = pane.view.selection {
                    selection.cursor = pane.view.cursor_line;
                }
            }
        }
    }

    /// Enter visual command mode (press '|' while in visual line mode)
    pub fn enter_visual_command_mode(&mut self) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            if pane.view.mode == Mode::VisualLine {
                pane.view.mode = Mode::VisualCommand;
                self.visual_command_buffer.clear();
            }
        }
    }

    /// Cancel command entry (Esc)
    pub fn cancel_visual_command(&mut self) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            pane.view.mode = Mode::VisualLine;
        }
        self.visual_command_buffer.clear();
    }

    /// Add a character to the visual command buffer
    pub fn append_visual_command_char(&mut self, c: char) {
        self.visual_command_buffer.push(c);
    }

    /// Remove the last character from the visual command buffer
    pub fn backspace_visual_command(&mut self) {
        self.visual_command_buffer.pop();
    }

    /// Execute the buffered command with the current visual selection as STDIN
    pub fn run_visual_command(&mut self) {
        let command_line = self.visual_command_buffer.trim().to_string();
        let output_text = if command_line.is_empty() {
            "Command cannot be empty".to_string()
        } else if let Some(selection) = self.visual_selection_text() {
            self.execute_shell_command(&command_line, &selection)
        } else {
            "Visual selection is missing".to_string()
        };

        self.command_output = Some(CommandOutput {
            command: command_line.clone(),
            output: output_text,
        });

        if let Some(pane) = self.panes.focused_pane_mut() {
            pane.view.mode = Mode::VisualLine;
        }
        self.visual_command_buffer.clear();
    }

    /// Get the text encompassed by the current visual selection
    pub fn visual_selection_text(&self) -> Option<String> {
        let pane = self.panes.focused_pane()?;
        let selection = pane.view.selection?;
        let (start, end) = selection.range();
        if start > end {
            return None;
        }
        let mut text = self.doc.get_lines(start, end);
        if !text.ends_with('\n') {
            text.push('\n');
        }
        Some(text)
    }

    fn execute_shell_command(&self, command: &str, input: &str) -> String {
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.arg("/C");
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c");
            c
        };

        cmd.arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(input.as_bytes());
                }
                match child.wait_with_output() {
                    Ok(output) => {
                        let mut result = String::new();
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        result.push_str(&stdout);

                        if !output.stderr.is_empty() {
                            if !result.ends_with('\n') {
                                result.push('\n');
                            }
                            result.push_str("--- stderr ---\n");
                            result.push_str(&String::from_utf8_lossy(&output.stderr));
                        }

                        let code = output.status.code().unwrap_or(-1);
                        result.push_str(&format!("\n[exit code {}]", code));
                        result
                    }
                    Err(e) => format!("Failed to capture command output: {}", e),
                }
            }
            Err(e) => format!("Failed to start shell: {}", e),
        }
    }

    /// Yank selected lines to clipboard
    #[cfg(feature = "clipboard")]
    pub fn yank_selection(&self) -> anyhow::Result<usize> {
        use arboard::Clipboard;

        let pane = self
            .panes
            .focused_pane()
            .ok_or_else(|| anyhow::anyhow!("No focused pane"))?;

        if pane.view.mode != Mode::VisualLine {
            return Err(anyhow::anyhow!("Not in visual line mode"));
        }

        let selection = pane
            .view
            .selection
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No selection"))?;

        let (start, end) = selection.range();
        let text = self.doc.get_lines(start, end);
        let line_count = end - start + 1;

        let mut clipboard =
            Clipboard::new().map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
        clipboard
            .set_text(text)
            .map_err(|e| anyhow::anyhow!("Failed to set clipboard: {}", e))?;

        Ok(line_count)
    }

    /// Yank selected lines (no-op without clipboard feature)
    #[cfg(not(feature = "clipboard"))]
    pub fn yank_selection(&self) -> anyhow::Result<usize> {
        Err(anyhow::anyhow!("Clipboard feature not enabled"))
    }

    /// Open the current file in an external editor
    pub fn open_in_editor(&self) -> anyhow::Result<()> {
        use crate::editor;

        if self.config.security.no_exec {
            anyhow::bail!("External editor execution is disabled (security.no_exec = true)");
        }

        if self.config.security.safe_mode {
            anyhow::bail!("External commands are disabled (security.safe_mode = true)");
        }

        let pane = self
            .panes
            .focused_pane()
            .ok_or_else(|| anyhow::anyhow!("No focused pane"))?;

        // Get current line (1-indexed for editors)
        let line = pane.view.cursor_line + 1;

        // Resolve editor command
        let command = editor::resolve_editor_command(&self.config.editor.command);

        // Launch editor (terminal suspend/restore handled by caller)
        editor::launch_editor(&command, &self.config.editor.args, &self.doc.path, line)?;

        Ok(())
    }

    /// Search for text in the document
    pub fn search(&mut self, query: &str) {
        if query.is_empty() {
            self.search_query.clear();
            self.search_matches.clear();
            self.search_current_match = None;
            return;
        }

        self.search_query = query.to_lowercase();
        self.search_matches.clear();
        self.search_current_match = None;

        // Find all matching lines
        let line_count = self.doc.line_count();
        for line_idx in 0..line_count {
            let line_text: String = self.doc.rope.line(line_idx).chunks().collect();
            if line_text.to_lowercase().contains(&self.search_query) {
                self.search_matches.push(line_idx);
            }
        }

        // Jump to first match if any
        if !self.search_matches.is_empty() {
            self.search_current_match = Some(0);
            let first_match = self.search_matches[0];
            let bounds = self.rendered_content_bounds();
            if let Some(pane) = self.panes.focused_pane_mut() {
                pane.view.cursor_line = first_match.clamp(bounds.0, bounds.1);
            }
        }
    }

    /// Jump to next search match
    pub fn next_search_match(&mut self, viewport_height: usize) {
        if self.search_matches.is_empty() {
            return;
        }

        if let Some(current_idx) = self.search_current_match {
            let next_idx = (current_idx + 1) % self.search_matches.len();
            self.search_current_match = Some(next_idx);
            let match_line = self.search_matches[next_idx];
            let bounds = self.rendered_content_bounds();
            if let Some(pane) = self.panes.focused_pane_mut() {
                pane.view.cursor_line = match_line.clamp(bounds.0, bounds.1);
            }
            self.auto_scroll(viewport_height);
        }
    }

    /// Jump to previous search match
    pub fn prev_search_match(&mut self, viewport_height: usize) {
        if self.search_matches.is_empty() {
            return;
        }

        if let Some(current_idx) = self.search_current_match {
            let prev_idx = if current_idx == 0 {
                self.search_matches.len() - 1
            } else {
                current_idx - 1
            };
            self.search_current_match = Some(prev_idx);
            let match_line = self.search_matches[prev_idx];
            let bounds = self.rendered_content_bounds();
            if let Some(pane) = self.panes.focused_pane_mut() {
                pane.view.cursor_line = match_line.clamp(bounds.0, bounds.1);
            }
            self.auto_scroll(viewport_height);
        }
    }

    /// Clear search
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_matches.clear();
        self.search_current_match = None;
    }

    /// Enter search mode
    pub fn enter_search_mode(&mut self) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            pane.view.mode = Mode::Search;
        }
        self.search_query.clear();
    }

    /// Exit search mode
    pub fn exit_search_mode(&mut self) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            pane.view.mode = Mode::Normal;
        }
    }

    /// Add character to search query
    pub fn search_add_char(&mut self, c: char) {
        self.search_query.push(c);
        self.search(&self.search_query.clone());
    }

    /// Remove last character from search query
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.search(&self.search_query.clone());
    }

    // ===== Collapse/Fold Operations =====

    /// Find the nearest heading at or above the cursor position
    fn find_nearest_heading_above(&self, cursor_line: usize) -> Option<usize> {
        // Find the last heading that is at or before the cursor line
        self.doc
            .headings
            .iter()
            .rev()
            .find(|h| h.line <= cursor_line)
            .map(|h| h.line)
    }

    /// Check if the cursor is on a heading line (collapsible)
    pub fn is_cursor_on_heading(&self) -> bool {
        if let Some(pane) = self.panes.focused_pane() {
            let cursor_line = pane.view.cursor_line;
            crate::collapse::is_heading_line(cursor_line, &self.doc)
        } else {
            false
        }
    }

    /// Check if the cursor is on a collapsed heading
    pub fn is_cursor_on_collapsed_heading(&self) -> bool {
        if let Some(pane) = self.panes.focused_pane() {
            let cursor_line = pane.view.cursor_line;
            pane.view.collapsed_headings.contains(&cursor_line)
        } else {
            false
        }
    }

    /// Check if cursor is under a collapsed heading
    pub fn is_cursor_under_collapsed_heading(&self) -> bool {
        if let Some(pane) = self.panes.focused_pane() {
            let cursor_line = pane.view.cursor_line;
            if let Some(heading_line) = self.find_nearest_heading_above(cursor_line) {
                pane.view.collapsed_headings.contains(&heading_line)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Toggle collapse at cursor (collapse if expanded, expand if collapsed)
    /// Works on the heading at cursor, or the nearest heading above
    pub fn toggle_collapse_at_cursor(&mut self) {
        // Get cursor line and find target heading first
        let target_heading = if let Some(pane) = self.panes.focused_pane() {
            let cursor_line = pane.view.cursor_line;

            // Find target heading: either at cursor or nearest above
            if crate::collapse::is_heading_line(cursor_line, &self.doc) {
                Some(cursor_line)
            } else {
                self.find_nearest_heading_above(cursor_line)
            }
        } else {
            None
        };

        // Now mutably borrow to update
        if let Some(heading_line) = target_heading {
            if let Some(pane) = self.panes.focused_pane_mut() {
                // Toggle: remove if present, add if absent
                if pane.view.collapsed_headings.contains(&heading_line) {
                    pane.view.collapsed_headings.remove(&heading_line);
                } else {
                    pane.view.collapsed_headings.insert(heading_line);
                }
            }
        }
    }

    /// Expand (open) fold at cursor or nearest heading above
    pub fn expand_at_cursor(&mut self) {
        // Get cursor line and find target heading first
        let target_heading = if let Some(pane) = self.panes.focused_pane() {
            let cursor_line = pane.view.cursor_line;

            // Find target heading: either at cursor or nearest above
            if crate::collapse::is_heading_line(cursor_line, &self.doc) {
                Some(cursor_line)
            } else {
                self.find_nearest_heading_above(cursor_line)
            }
        } else {
            None
        };

        // Now mutably borrow to update
        if let Some(heading_line) = target_heading {
            if let Some(pane) = self.panes.focused_pane_mut() {
                pane.view.collapsed_headings.remove(&heading_line);
            }
        }
    }

    /// Collapse (close) fold at cursor or nearest heading above
    pub fn collapse_at_cursor(&mut self) {
        // Get cursor line and find target heading first
        let target_heading = if let Some(pane) = self.panes.focused_pane() {
            let cursor_line = pane.view.cursor_line;

            // Find target heading: either at cursor or nearest above
            if crate::collapse::is_heading_line(cursor_line, &self.doc) {
                Some(cursor_line)
            } else {
                self.find_nearest_heading_above(cursor_line)
            }
        } else {
            None
        };

        // Now mutably borrow to update
        if let Some(heading_line) = target_heading {
            if let Some(pane) = self.panes.focused_pane_mut() {
                pane.view.collapsed_headings.insert(heading_line);
            }
        }
    }

    /// Collapse all headings at or above a certain level
    pub fn collapse_all_headings(&mut self, max_level: Option<u8>) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            for heading in &self.doc.headings {
                // If max_level is specified, only collapse headings at that level or higher
                if let Some(max) = max_level {
                    if heading.level <= max {
                        pane.view.collapsed_headings.insert(heading.line);
                    }
                } else {
                    // Collapse all headings
                    pane.view.collapsed_headings.insert(heading.line);
                }
            }
        }
    }

    /// Expand (open) all folds
    pub fn expand_all_headings(&mut self) {
        if let Some(pane) = self.panes.focused_pane_mut() {
            pane.view.collapsed_headings.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdx_core::Document;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_doc(lines: usize) -> Document {
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..lines {
            if i > 0 {
                writeln!(file).unwrap();
            }
            write!(file, "Line {}", i + 1).unwrap();
        }
        file.flush().unwrap();
        let (doc, _warnings) = Document::load(file.path()).unwrap();
        doc
    }

    #[test]
    fn test_move_cursor_down() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
        app.move_cursor_down(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 1);
        app.move_cursor_down(3);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 4);
    }

    #[test]
    fn test_move_cursor_down_bounded() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        // Try to move beyond last line
        app.move_cursor_down(100);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 9); // 0-indexed, so line 9 is the last
    }

    #[test]
    fn test_move_cursor_up() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        app.panes.focused_pane_mut().unwrap().view.cursor_line = 5;
        app.move_cursor_up(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 4);
        app.move_cursor_up(3);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 1);
    }

    #[test]
    fn test_move_cursor_up_bounded() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        app.panes.focused_pane_mut().unwrap().view.cursor_line = 2;
        // Try to move before first line
        app.move_cursor_up(100);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
    }

    #[test]
    fn test_jump_to_line() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        app.jump_to_line(5);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 5);

        app.jump_to_line(0);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);

        app.jump_to_line(9);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 9);

        // Beyond bounds
        app.jump_to_line(100);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 9);
    }

    #[test]
    fn test_scroll_half_page() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc, vec![]);

        let viewport_height = 20;
        let viewport_width = 80;

        // Half page down (10 lines, no wrapping with short lines)
        app.scroll_half_page_down(viewport_height, viewport_width);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 10);

        // Half page up
        app.scroll_half_page_up(viewport_height, viewport_width);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
    }

    #[test]
    fn test_auto_scroll_down() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc, vec![]);
        let viewport_height = 10;

        // Move cursor to line 15 (beyond effective viewport of 4 lines with 40% conservative estimate)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 15;
        app.auto_scroll(viewport_height);

        // Scroll should adjust so cursor is visible with very conservative height
        // effective_height = 10 * 40% = 4, so scroll = 15 - 3 = 12
        assert_eq!(app.panes.focused_pane_mut().unwrap().view.scroll_line, 12);
    }

    #[test]
    fn test_auto_scroll_up() {
        let config = Config::default();
        let doc = create_test_doc(50);
        let mut app = App::new(config, doc, vec![]);
        let viewport_height = 10;

        // Start scrolled down
        app.panes.focused_pane_mut().unwrap().view.scroll_line = 20;
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 15; // Above current scroll

        app.auto_scroll(viewport_height);

        // Scroll should move up to show cursor
        assert_eq!(app.panes.focused_pane_mut().unwrap().view.scroll_line, 15);
    }

    #[test]
    fn test_navigation_with_empty_doc() {
        let config = Config::default();
        let doc = create_test_doc(0);
        let mut app = App::new(config, doc, vec![]);

        // Should handle empty doc gracefully
        app.move_cursor_down(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);

        app.move_cursor_up(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
    }

    #[test]
    fn test_navigation_with_single_line() {
        let config = Config::default();
        let doc = create_test_doc(1);
        let mut app = App::new(config, doc, vec![]);

        app.move_cursor_down(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0); // Can't move beyond line 0

        app.move_cursor_up(1);
        assert_eq!(app.panes.focused_pane().unwrap().view.cursor_line, 0);
    }

    #[test]
    fn test_toggle_toc() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        // Initially hidden (from config default)
        assert!(!app.show_toc);
        assert!(!app.toc_focus);

        // Toggle - should show and focus
        app.toggle_toc();
        assert!(app.show_toc);
        assert!(app.toc_focus);

        // Toggle again - should hide
        app.toggle_toc();
        assert!(!app.show_toc);
        assert!(!app.toc_focus);
    }

    #[test]
    fn test_toc_navigation() {
        let config = Config::default();
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "# Heading 1\nSome text\n## Heading 2\nMore text\n### Heading 3"
        )
        .unwrap();
        file.flush().unwrap();
        let (doc, _warnings) = Document::load(file.path()).unwrap();
        let mut app = App::new(config, doc, vec![]);

        assert_eq!(app.toc_selected, 0);

        let toc_height = 10; // Simulated TOC viewport height

        // Move down in TOC
        app.toc_move_down(toc_height);
        assert_eq!(app.toc_selected, 1);

        app.toc_move_down(toc_height);
        assert_eq!(app.toc_selected, 2);

        // Try to move beyond last heading
        app.toc_move_down(toc_height);
        assert_eq!(app.toc_selected, 2); // Should stay at 2

        // Move up
        app.toc_move_up(toc_height);
        assert_eq!(app.toc_selected, 1);

        app.toc_move_up(toc_height);
        assert_eq!(app.toc_selected, 0);

        // Try to move above first heading
        app.toc_move_up(toc_height);
        assert_eq!(app.toc_selected, 0); // Should stay at 0
    }

    #[test]
    fn test_toc_jump_to_heading() {
        let config = Config::default();
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "# Heading 1\nSome text\n## Heading 2\nMore text\n### Heading 3"
        )
        .unwrap();
        file.flush().unwrap();
        let (doc, _warnings) = Document::load(file.path()).unwrap();
        let mut app = App::new(config, doc, vec![]);

        // Jump to second heading
        app.toc_selected = 1;
        app.toc_jump_to_selected();

        // Heading 2 should be at line 2 (0-indexed)
        // And it should be the top line (scroll = cursor)
        let pane = app.panes.focused_pane().unwrap();
        assert_eq!(pane.view.cursor_line, 2);
        assert_eq!(pane.view.scroll_line, 2);
    }

    #[test]
    fn test_current_heading_index() {
        let config = Config::default();
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "# Heading 1\ntext\ntext\n## Heading 2\ntext\n### Heading 3\ntext"
        )
        .unwrap();
        file.flush().unwrap();
        let (doc, _warnings) = Document::load(file.path()).unwrap();
        let mut app = App::new(config, doc, vec![]);

        // At line 0 - should be heading 0
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 0;
        assert_eq!(app.current_heading_index(), Some(0));

        // At line 2 (still under heading 1)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 2;
        assert_eq!(app.current_heading_index(), Some(0));

        // At line 3 (heading 2)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 3;
        assert_eq!(app.current_heading_index(), Some(1));

        // At line 5 (heading 3)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 5;
        assert_eq!(app.current_heading_index(), Some(2));
    }

    #[test]
    fn test_enter_visual_line_mode() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        // Move to line 3
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 3;

        // Enter visual line mode
        app.enter_visual_line_mode();

        let pane = app.panes.focused_pane().unwrap();
        assert_eq!(pane.view.mode, Mode::VisualLine);
        assert!(pane.view.selection.is_some());

        let selection = pane.view.selection.unwrap();
        assert_eq!(selection.anchor, 3);
        assert_eq!(selection.cursor, 3);
    }

    #[test]
    fn test_visual_line_selection_navigation() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        // Start at line 3
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 3;
        app.enter_visual_line_mode();

        // Move down 2 lines
        app.move_cursor_down(2);

        let pane = app.panes.focused_pane().unwrap();
        assert_eq!(pane.view.cursor_line, 5);

        let selection = pane.view.selection.unwrap();
        assert_eq!(selection.anchor, 3);
        assert_eq!(selection.cursor, 5);
        assert_eq!(selection.range(), (3, 5));
    }

    #[test]
    fn test_visual_line_selection_backward() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        // Start at line 5
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 5;
        app.enter_visual_line_mode();

        // Move up 3 lines
        app.move_cursor_up(3);

        let pane = app.panes.focused_pane().unwrap();
        assert_eq!(pane.view.cursor_line, 2);

        let selection = pane.view.selection.unwrap();
        assert_eq!(selection.anchor, 5);
        assert_eq!(selection.cursor, 2);
        assert_eq!(selection.range(), (2, 5));
    }

    #[test]
    fn test_exit_visual_line_mode() {
        let config = Config::default();
        let doc = create_test_doc(10);
        let mut app = App::new(config, doc, vec![]);

        app.enter_visual_line_mode();
        assert_eq!(
            app.panes.focused_pane().unwrap().view.mode,
            Mode::VisualLine
        );

        app.exit_visual_line_mode();
        let pane = app.panes.focused_pane().unwrap();
        assert_eq!(pane.view.mode, Mode::Normal);
        assert!(pane.view.selection.is_none());
    }

    #[test]
    #[cfg(feature = "clipboard")]
    fn test_yank_selection() {
        let config = Config::default();
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5").unwrap();
        file.flush().unwrap();
        let (doc, _warnings) = Document::load(file.path()).unwrap();
        let mut app = App::new(config, doc, vec![]);

        // Select lines 1-3 (0-indexed)
        app.panes.focused_pane_mut().unwrap().view.cursor_line = 1;
        app.enter_visual_line_mode();
        app.move_cursor_down(2);

        // Yank - might fail in headless environment, but should not panic
        let result = app.yank_selection();
        match result {
            Ok(count) => {
                assert_eq!(count, 3);
            }
            Err(_) => {
                // Clipboard might not be available in test environment
            }
        }
    }

    #[test]
    fn security_no_exec_blocks_editor() {
        let mut config = Config::default();
        config.security.no_exec = true;
        let doc = create_test_doc(1);
        let app = App::new(config, doc, vec![]);

        let result = app.open_in_editor();
        assert!(result.is_err());
    }

    #[test]
    fn security_safe_mode_blocks_editor() {
        let mut config = Config::default();
        config.security.safe_mode = true;
        let doc = create_test_doc(1);
        let app = App::new(config, doc, vec![]);

        let result = app.open_in_editor();
        assert!(result.is_err());
    }
}
