# MDX Implementation Plan

**Project**: Rust TUI Markdown viewer/editor launcher
**Goal**: Build a fast, polished TUI Markdown viewer with Vim-style navigation, TOC, splits, git diff, and file watching

## How to Use This Plan

This plan breaks the full mdx-spec.md into 18 incremental stages. Each stage includes:

- **Goal**: What you're building
- **Success Criteria**: How you know it's done
- **Spec References**: Sections in mdx-spec.md with detailed technical guidance
- **Tasks**: Concrete implementation steps
- **Tests**: What to verify
- **Status**: Track your progress (Not Started → In Progress → Complete)

**Important**: The spec references point to specific sections in `mdx-spec.md` that contain detailed implementation guidance, code sketches, data structures, and architectural decisions. Always consult the referenced spec sections before implementing a stage.

---

## Stage 0: Project Scaffold & Dependencies

**Goal**: Set up workspace structure and core dependencies
**Success Criteria**:
- Workspace compiles with all three crates
- Basic dependencies declared in Cargo.toml files
- Can run `cargo build` successfully
- Project structure matches planned architecture

**Spec References**:
- Section 1: Crate layout (workspace) and responsibilities
- Section 2: Key dependencies (Rust crates)
- Workspace layout + feature flags (around line 622)

**Tasks**:
- [ ] Create workspace Cargo.toml with three members: `mdx-core`, `mdx-tui`, `mdx`
- [ ] Create `mdx-core` library crate (no terminal deps)
- [ ] Create `mdx-tui` library crate (ratatui + crossterm)
- [ ] Create `mdx` binary crate
- [ ] Add core dependencies to each crate:
  - Core: `ropey`, `pulldown-cmark`, `serde`, `serde_yaml`, `directories`, `anyhow`
  - TUI: `ratatui`, `crossterm`, `unicode-width`
  - Binary: `clap`
- [ ] Set up feature flags: `clipboard`, `watch`, `git` (all default enabled)
- [ ] Create basic module structure in each crate
- [ ] Verify `cargo build` and `cargo test` run

**Tests**:
- Workspace builds without errors
- Each crate can be built independently
- Basic smoke test in each crate

**Status**: Complete

---

## Stage 1: Core Document Model & Parsing

**Goal**: Implement Document structure with Rope-based text storage and basic Markdown parsing
**Success Criteria**:
- Can load a file into a Document with Rope
- Can extract headings from Markdown
- Line access works correctly
- File metadata (mtime, revision) tracked properly

**Spec References**:
- Section 3: Data model → Document model (lines 100-130)
- Section 5: Table of Contents → Building the TOC (lines 247-264)
- Core data structures → Document + parsing outputs (lines 648-688)
- TOC extraction (v1: regex scan) (lines 691-702)

**Tasks**:
- [ ] Implement `Document` struct in `mdx-core/src/doc.rs`
  - path, rope, headings, mtime tracking, revision counter
- [ ] Implement `Document::load(path)` - read file, parse, extract headings
- [ ] Implement `Document::reload()` - preserve cursor position semantics
- [ ] Implement `Heading` struct with level, text, line, anchor
- [ ] Create `mdx-core/src/toc.rs` with `extract_headings(rope)`:
  - ATX heading regex scan (`^#{1,6}\s+`)
  - Setext heading detection (underline with `===` or `---`)
- [ ] Implement `Document::get_lines(start, end)` for yank operations
- [ ] Add helper `rope_to_string()` for conversions when needed

**Tests**:
- [x] Load simple markdown file with headings
- [x] Extract ATX headings (levels 1-6)
- [x] Extract Setext headings (H1 and H2 styles)
- [x] Verify line_count() matches actual lines
- [x] Test get_lines() extracts correct range
- [x] Test revision counter increments on reload
- [x] Handle empty file, file with no headings, very large file

**Status**: Complete

---

## Stage 2: Selection Model & Config

**Goal**: Implement linewise selection model and YAML configuration system
**Success Criteria**:
- Selection range calculation works correctly
- Config loads from platform-appropriate paths
- Config merges with defaults properly
- All config fields parse correctly from YAML

**Spec References**:
- Section 8: Config (lines 354-388)
- Selection model (Visual line mode) (lines 704-723)
- Config model (serde_yaml) (lines 760-809)

**Tasks**:
- [ ] Implement `LineSelection` in `mdx-core/src/selection.rs`
  - anchor and cursor fields
  - `range()` method returning sorted (min, max)
- [ ] Implement `Config` structs in `mdx-core/src/config.rs`:
  - Main Config with theme, toc, editor, watch, git sections
  - ThemeVariant enum (Dark, Light)
  - TocConfig, EditorConfig, WatchConfig, GitConfig
- [ ] Implement `config_path()` using `directories` crate
- [ ] Implement config loading with defaults fallback
- [ ] Add serde derives for all config types

**Tests**:
- [x] Selection range() returns correct min/max regardless of direction
- [x] Config parses valid YAML with all fields
- [x] Config uses defaults when file missing
- [x] Config merges partial YAML with defaults
- [x] Platform-specific config paths resolve correctly
- [x] Invalid YAML shows helpful error

**Status**: Complete

---

## Stage 3: Skeleton TUI Application

**Goal**: Basic ratatui app that opens a file, displays markdown, and responds to quit command
**Success Criteria**:
- CLI accepts file path argument
- Terminal enters/exits alt screen cleanly
- Markdown renders in viewport
- `q` quits without errors
- Status bar shows file info

**Spec References**:
- Section 14: Milestones → Milestone 1 - Skeleton TUI (lines 533-538)
- Section 4: Rendering architecture (lines 164-244)
- Section 11: Status bar and UX details (lines 478-486)
- TUI core structures → App state (lines 812-842)
- Input + Event loop architecture (lines 950-996)

**Tasks**:
- [ ] Implement CLI in `mdx/src/main.rs` using clap
  - Accept file path as required argument
- [ ] Create `App` struct in `mdx-tui/src/app.rs`:
  - config, theme, single document, status line
- [ ] Create basic event loop in `mdx-tui/src/event.rs`:
  - AppEvent enum (Input, Tick, FileChanged, DiffReady)
  - Event channel setup with crossbeam
- [ ] Implement terminal init/cleanup in `mdx-tui/src/lib.rs`
- [ ] Create basic UI layout in `mdx-tui/src/ui.rs`:
  - Status bar (top 1 line)
  - Markdown viewport (rest)
- [ ] Integrate `tui-markdown` for rendering
- [ ] Implement quit handler for 'q' key
- [ ] Add basic status bar: filename, line count, mode

**Tests**:
- [ ] CLI rejects missing file argument
- [ ] CLI shows help with --help
- [ ] App opens valid markdown file
- [ ] App exits cleanly on 'q'
- [ ] Terminal state restored after exit
- [ ] Status bar displays correct filename

**Status**: Complete

---

## Stage 4: Vim-Style Navigation

**Goal**: Implement core Vim navigation commands
**Success Criteria**:
- j/k moves cursor line by line
- ^u/^d scrolls half-page
- gg/G jumps to top/bottom
- Viewport follows cursor correctly
- Navigation bounded by document length

**Spec References**:
- Section 6: Input system and keybinding engine (lines 267-339)
- Section 3: Viewport state (per pane) (lines 132-145)
- Keybinding behavior sketch → Normal mode (lines 1026-1036)

**Tasks**:
- [ ] Create `ViewState` in `mdx-tui/src/app.rs`:
  - scroll_line, cursor_line, mode, selection
- [ ] Implement `Mode` enum (Normal, VisualLine)
- [ ] Create input handler in `mdx-tui/src/input.rs`:
  - Key event to action mapping
  - Mode-aware dispatch
- [ ] Implement navigation commands:
  - `j`: cursor_line += 1 (bounded)
  - `k`: cursor_line -= 1 (bounded to 0)
  - `^d`: half-page down
  - `^u`: half-page up
  - `gg`: jump to line 0
  - `G`: jump to last line
- [ ] Implement viewport auto-scroll when cursor moves outside visible area
- [ ] Add cursor line highlight in renderer

**Tests**:
- [ ] j/k navigation moves cursor correctly
- [ ] j at last line stays at last line
- [ ] k at first line stays at first line
- [ ] ^d/^u scroll by half viewport height
- [ ] gg goes to first line, G to last
- [ ] Viewport scrolls when cursor moves off-screen
- [ ] Navigation works with single-line file
- [ ] Navigation works with empty file

**Status**: Complete

---

## Stage 5: TOC Sidebar

**Goal**: Implement toggleable Table of Contents sidebar with navigation
**Success Criteria**:
- T toggles TOC visibility
- TOC shows all headings with proper indentation
- Current heading highlighted based on cursor position
- Enter/l jumps to selected heading
- TOC focus navigation works

**Spec References**:
- Section 5: Table of Contents (TOC) sidebar (lines 247-264)
- Section 14: Milestones → Milestone 2 - TOC sidebar (lines 540-545)
- Section 15: Implementation notes → TOC line mapping (lines 601-604)

**Tasks**:
- [ ] Add `show_toc` and `toc_focus` to App state
- [ ] Create TOC widget in `mdx-tui/src/toc.rs`:
  - Render headings with level-based indentation
  - Highlight current heading
  - Handle focus state
- [ ] Modify layout to split left/right when TOC enabled
  - Use config.toc.width for TOC column size
  - Support config.toc.side (left/right)
- [ ] Implement TOC keybindings:
  - `T`: toggle TOC visibility and focus
  - With focus: `j/k` navigate headings
  - With focus: `Enter` or `l` jump to heading
  - With focus: `q` or `T` close TOC
- [ ] Implement heading position tracking:
  - Find nearest heading above cursor_line
  - Highlight in TOC
- [ ] Add TOC indicator to status bar

**Tests**:
- [ ] T toggles TOC on/off
- [ ] TOC shows all headings from test markdown
- [ ] Heading indentation reflects nesting levels
- [ ] Current heading highlighted correctly
- [ ] Enter jumps cursor to heading line
- [ ] Navigation in TOC with j/k works
- [ ] TOC respects config width and side
- [ ] Works with no headings in document

**Status**: Complete

---

## Stage 6: Theme System

**Goal**: Implement dark/light themes with toggle
**Success Criteria**:
- M toggles between dark and light themes
- Themes define styles for all markdown elements
- Theme applied consistently across UI
- Current theme shown in status bar

**Spec References**:
- Section 4: Rendering architecture → Styling/theme (lines 226-244)
- Section 14: Milestones → Milestone 3 - Themes (lines 546-550)

**Tasks**:
- [ ] Define `Theme` struct in `mdx-tui/src/theme.rs`:
  - base, heading[6], code, link, quote, list_marker styles
  - toc_active, diff_add, diff_del, diff_mod
- [ ] Implement built-in dark theme
- [ ] Implement built-in light theme
- [ ] Add theme to RenderInput/context
- [ ] Implement `M` key handler to toggle theme
- [ ] Update markdown renderer to use theme styles
- [ ] Update TOC renderer to use theme
- [ ] Add theme indicator to status bar
- [ ] Respect config.theme as default

**Tests**:
- [ ] M toggles theme correctly
- [ ] Dark theme has readable contrast
- [ ] Light theme has readable contrast
- [ ] Headings use appropriate styles per level
- [ ] Code blocks styled distinctly
- [ ] Config default theme loads correctly
- [ ] Theme persists across reloads (or resets - clarify)

**Status**: Complete

---

## Stage 7: Pane Management & Splits

**Goal**: Implement split panes with independent viewports
**Success Criteria**:
- ^w s creates horizontal split
- ^w v creates vertical split
- Ctrl+Arrow moves focus between panes
- Each pane has independent scroll and cursor
- Visual indicator shows focused pane

**Spec References**:
- Section 3: Pane tree (splits) (lines 148-161)
- Section 14: Milestones → Milestone 4 - Splits & pane tree (lines 551-557)
- Pane manager / split tree (lines 859-901)
- Section 1) Ctrl+Arrow key decoding across terminals (lines 1157-1258)

**Tasks**:
- [ ] Implement `PaneNode` tree in `mdx-tui/src/panes.rs`:
  - Leaf(PaneId) and Split variants
  - SplitDir enum (Horizontal, Vertical)
- [ ] Implement `PaneManager`:
  - root PaneNode tree
  - HashMap<PaneId, Pane> storage
  - focused PaneId tracking
- [ ] Implement `Pane` struct:
  - doc_id reference
  - ViewState (scroll, cursor, mode, selection)
- [ ] Implement split operations:
  - `split_focused(dir)` - split current leaf
  - Update tree structure
  - Create new Pane with same doc_id
- [ ] Implement focus traversal:
  - Compute Rect for each leaf during layout
  - `move_focus(direction)` - find nearest pane in direction
- [ ] Implement key prefix state machine:
  - KeyPrefix enum (None, CtrlW, G)
  - Handle ^w → wait for s/v
- [ ] Implement focus movement keys:
  - Ctrl+Arrow (primary)
  - Ctrl+hjkl (fallback)
  - Alt+Arrow (optional fallback)
- [ ] Update layout to recursively render pane tree
- [ ] Add border to panes with focused highlight

**Tests**:
- [ ] ^w s creates horizontal split
- [ ] ^w v creates vertical split
- [ ] Multiple splits create correct tree structure
- [ ] Ctrl+arrows move focus correctly
- [ ] Ctrl+hjkl move focus correctly
- [ ] Each pane scrolls independently
- [ ] Focused pane highlighted visually
- [ ] Works with 1, 2, 3, 4+ panes
- [ ] Prefix timeout/cancel works

**Status**: Not Started

---

## Stage 8: Visual Line Selection & Yank

**Goal**: Implement visual line mode with clipboard yank
**Success Criteria**:
- Shift+V enters visual line mode
- Selection expands/shrinks with navigation
- Y yanks selection to clipboard
- Selected lines highlighted
- Status shows line count

**Spec References**:
- Section 7: Clipboard yank implementation (lines 342-351)
- Section 14: Milestones → Milestone 5 - Visual line selection + yank (lines 558-564)
- Keybinding behavior sketch → Visual line mode (lines 1038-1043)

**Tasks**:
- [ ] Add `arboard` dependency (feature = clipboard)
- [ ] Implement VisualLine mode entry with Shift+V:
  - Set mode to VisualLine
  - Create selection with anchor=cursor, cursor=cursor
- [ ] Extend navigation to handle selection in VisualLine mode:
  - j/k/^u/^d/gg/G update cursor, selection range updates
- [ ] Implement selection highlight in renderer:
  - Map source line selection to rendered lines
  - Apply highlight style to selection range
- [ ] Implement Y (yank) handler:
  - Get selection range from LineSelection::range()
  - Extract lines with Document::get_lines()
  - Use arboard to set clipboard text
  - Show "Yanked N lines" in status
  - Handle clipboard errors gracefully (Wayland/headless)
- [ ] Implement Esc to exit VisualLine mode
- [ ] Update status bar to show V-LINE mode and selection count

**Tests**:
- [ ] Shift+V enters visual line mode
- [ ] j/k expands selection correctly
- [ ] Selection handles both directions (anchor < cursor and vice versa)
- [ ] Y yanks correct text to clipboard
- [ ] Yanked text matches source lines exactly
- [ ] Clipboard error doesn't crash app
- [ ] Esc exits visual mode
- [ ] Status shows correct line count
- [ ] Selection highlight visible

**Status**: Not Started

---

## Stage 9: Open in Editor

**Goal**: Launch external editor with e key
**Success Criteria**:
- e opens $EDITOR with current file
- Editor opens at current cursor line
- Terminal state restored after editor exits
- Config supports editor templates with {file} and {line}

**Spec References**:
- Section 14: Milestones → Milestone 6 - Open in editor (lines 565-569)
- Editor launching details (lines 1046-1062)

**Tasks**:
- [ ] Implement editor command resolution:
  - If config.editor.command is "$EDITOR", use env var
  - Fallback chain: nvim, vim, nano
- [ ] Implement template expansion:
  - Replace {file} with document path
  - Replace {line} with (cursor_line + 1) for 1-based
- [ ] Implement editor spawning in `mdx-tui/src/editor.rs`:
  - Suspend terminal (exit alt screen)
  - Spawn editor process with Command
  - Wait for editor exit
  - Restore terminal state
  - Trigger redraw
- [ ] Add 'e' key handler
- [ ] Add config defaults for common editors:
  - nvim/vim: `["+{line}", "{file}"]`
  - code: `["--goto", "{file}:{line}:0"]`

**Tests**:
- [ ] e launches default editor
- [ ] Editor opens at correct line (manual test)
- [ ] Terminal restored after editor exit
- [ ] Works with custom editor in config
- [ ] Template expansion works correctly
- [ ] {file} and {line} substituted properly
- [ ] Handles editor not found gracefully

**Status**: Not Started

---

## Stage 10: Configuration File

**Goal**: Load and apply configuration from platform config directory
**Success Criteria**:
- Config loads from correct platform path
- Missing config uses sensible defaults
- Config values applied to app behavior
- Invalid config shows helpful error

**Spec References**:
- Section 8: Config (lines 354-388)
- Section 14: Milestones → Milestone 7 - Config file (lines 570-575)

**Tasks**:
- [ ] Implement config path resolution:
  - Use `directories::ProjectDirs::from("", "", "mdx")`
  - Linux: ~/.config/mdx/mdx.yaml
  - macOS: ~/Library/Application Support/mdx/mdx.yaml
  - Windows: AppData/Roaming/mdx/mdx.yaml
- [ ] Implement config loading in `mdx-core/src/config.rs`:
  - Try load from path
  - If missing, return defaults
  - If invalid YAML, return error with message
- [ ] Define default config values:
  - theme: Dark
  - toc.enabled: true, side: Left, width: 32
  - editor.command: "$EDITOR", args: ["+{line}", "{file}"]
  - watch.enabled: true, auto_reload: false
  - git.diff: true, base: Head
- [ ] Apply config in App initialization:
  - Set initial theme from config
  - Set initial show_toc from config
  - Pass config to components
- [ ] Create example config file in docs/mdx.yaml

**Tests**:
- [ ] Config loads from valid YAML file
- [ ] Missing config file uses defaults
- [ ] Partial config merges with defaults
- [ ] Invalid YAML returns clear error
- [ ] Platform paths resolve correctly
- [ ] Each config section applies correctly

**Status**: Not Started

---

## Stage 11: File Watching

**Goal**: Detect external file changes and support reload
**Success Criteria**:
- File changes detected within 1 second
- "modified on disk" indicator shown in status
- r reloads file manually
- auto_reload mode works when enabled
- Debouncing prevents thrashing

**Spec References**:
- Section 9: File watching + "changed on disk" indicator (lines 392-418)
- Section 14: Milestones → Milestone 8 - File watching (lines 576-581)
- File watching + debounce (lines 1065-1090)

**Tasks**:
- [ ] Add `notify` dependency (feature = watch)
- [ ] Create watcher thread in `mdx-tui/src/watcher.rs`:
  - Use `notify::recommended_watcher`
  - Watch file and parent directory (for atomic rename)
  - Send FileChanged event on modify
- [ ] Add dirty_on_disk flag to Document
- [ ] Implement FileChanged event handler:
  - Set dirty_on_disk = true
  - Show indicator in status bar
  - If auto_reload enabled, mark for debounced reload
- [ ] Implement debounce logic:
  - Track last_event_at timestamp
  - On Tick, check if 250ms elapsed since last event
  - If so, trigger reload
- [ ] Implement reload operation:
  - Call Document::reload()
  - Preserve cursor_line (clamped to new length)
  - Preserve scroll_line (clamped)
  - Clear dirty_on_disk
  - Increment doc.rev
  - Trigger diff recompute
- [ ] Implement 'r' key for manual reload
- [ ] Add watch indicator to status bar

**Tests**:
- [ ] External file modification detected
- [ ] dirty_on_disk flag set correctly
- [ ] Manual reload with 'r' works
- [ ] Auto reload triggers after debounce
- [ ] Cursor position preserved across reload
- [ ] Multiple rapid changes debounced
- [ ] Watch can be disabled via config

**Status**: Not Started

---

## Stage 12: Git Diff Gutter (Text-based)

**Goal**: Show git diff gutter using similar::TextDiff
**Success Criteria**:
- Added lines marked with +
- Modified lines marked with ~
- Gutter renders on first wrapped line only
- Works with non-git files gracefully
- Config can disable diff

**Spec References**:
- Section 10: Git diff gutter (inline column) (lines 421-476)
- Section 14: Milestones → Milestone 9 - Git diff gutter (lines 582-588)
- Section 2) Git diff gutter marking using similar::TextDiff (lines 1261-1435)
- Diff model (line-aligned gutter) (lines 725-757)

**Tasks**:
- [ ] Add `similar` dependency (feature = git)
- [ ] Implement `DiffMark` enum in `mdx-core/src/diff.rs`:
  - None, Added, Modified, DeletedAfter(u16)
- [ ] Implement `DiffGutter` struct:
  - marks: Vec<DiffMark> (length = working tree lines)
  - empty(line_count) constructor
- [ ] Implement `diff_gutter_from_text(base, current)`:
  - Use `similar::TextDiff::from_lines`
  - Walk ops (Equal, Insert, Delete, Replace)
  - Map to DiffMark based on change type
  - Handle deletions as DeletedAfter markers
- [ ] Implement base text loading as subprocess for v1:
  - Find .git by walking parents
  - `git show HEAD:path` subprocess
  - Parse stdout as base text
  - If not in repo or file not in HEAD, return empty base
- [ ] Create diff computation in `mdx-tui/src/diff_worker.rs`:
  - For now, compute on main thread (worker thread in next stage)
- [ ] Add diff column to renderer (2 chars: mark + space)
- [ ] Implement gutter rendering:
  - None → ' '
  - Added → '+'
  - Modified → '~'
  - DeletedAfter → '▾'
- [ ] Map diff marks to rendered lines (first line only)
- [ ] Respect config.git.diff to enable/disable

**Tests**:
- [ ] File not in git repo shows no gutter
- [ ] New file (not in HEAD) shows all Added
- [ ] Modified line shows Modified mark
- [ ] Added lines show Added marks
- [ ] Deleted lines show DeletedAfter markers
- [ ] Gutter only on first wrapped line
- [ ] Works with config.git.diff = false

**Status**: Not Started

---

## Stage 13: Git Integration with gix

**Goal**: Replace git subprocess with gix library for performance
**Success Criteria**:
- Same diff results as subprocess method
- Faster than subprocess on large files
- Handles edge cases (unborn HEAD, new files, binary)
- No subprocess dependency

**Spec References**:
- Using gix now (lines 1437-1755)
- Section 1) DiffProvider design with gix (lines 1449-1471)
- Section 2) Getting base text from HEAD:<path> using gix (lines 1492-1577)
- Section 3) Repo discovery with gix (lines 1580-1622)
- Section 4) Putting it together: GixDiffProvider::compute (lines 1625-1664)
- Section 6) Edge cases to handle (lines 1690-1712)

**Tasks**:
- [ ] Add `gix` dependency (feature = git)
- [ ] Add `bstr` dependency for path handling
- [ ] Implement `RepoContext` in `mdx-core/src/git.rs`:
  - repo: gix::Repository
  - workdir: PathBuf
  - rel_path: PathBuf
- [ ] Implement `open_repo_for_path(path)`:
  - Walk parents to find .git
  - Open with gix::open()
  - Get work_dir()
  - Compute relative path
- [ ] Implement `read_head_file_text(repo, rel_path)`:
  - Get HEAD reference
  - Peel to commit
  - Get tree
  - Lookup entry by path (slash-separated)
  - Read blob data
  - Decode as UTF-8 (lossy)
- [ ] Implement `path_to_repo_bstr(rel_path)`:
  - Convert OS path to forward-slash BString
  - Validate no parent dir components
- [ ] Create `GixDiffProvider` implementing DiffProvider trait
- [ ] Handle edge cases:
  - Unborn HEAD → no diff
  - File not in HEAD → all Added
  - Binary files → disable gutter
  - Submodule entries → ignore
- [ ] Add new file heuristic:
  - If base empty and current non-empty → all Added
- [ ] Replace subprocess implementation with gix

**Tests**:
- [ ] Results match git subprocess output
- [ ] Unborn HEAD doesn't crash
- [ ] New file shows Added correctly
- [ ] Submodule paths ignored
- [ ] Binary files handled gracefully
- [ ] Performance better than subprocess (benchmark)
- [ ] Works in nested subdirectories

**Status**: Not Started

---

## Stage 14: Diff Worker Thread

**Goal**: Move diff computation to background thread
**Success Criteria**:
- UI stays responsive during diff
- Multiple rapid changes coalesce
- Stale results ignored (revision checking)
- No race conditions

**Spec References**:
- Section 1) Diff worker: request/coalesce pattern (lines 1757-1892)
- Diff worker thread sketch (lines 1093-1103)

**Tasks**:
- [ ] Create `DiffRequest` struct:
  - doc_id, path, rev, current_text snapshot
- [ ] Create `DiffResult` struct:
  - doc_id, rev, gutter
- [ ] Implement diff worker thread in `mdx-tui/src/diff_worker.rs`:
  - Receive DiffRequest via channel
  - Coalesce requests (keep latest per doc_id)
  - Compute diffs with 75ms window
  - Send DiffResult back
- [ ] Integrate worker in App:
  - Spawn worker thread on startup
  - Send DiffRequest on load/reload
  - Handle DiffResult event
  - Check rev matches before applying
- [ ] Modify GixDiffProvider to accept &str instead of &Rope
- [ ] Implement rope_to_string snapshot for requests

**Tests**:
- [ ] Worker thread starts and stops cleanly
- [ ] Diff results arrive asynchronously
- [ ] Multiple rapid changes coalesce correctly
- [ ] Stale results (wrong rev) discarded
- [ ] UI remains responsive during diff
- [ ] Worker doesn't crash on malformed input

**Status**: Not Started

---

## Stage 15: Renderer Cache & Source Mapping

**Goal**: Cache rendered output and implement source-to-rendered line mapping
**Success Criteria**:
- Rendering only happens when needed (width/theme/rev change)
- Cache hit avoids re-rendering
- Source lines map to rendered lines for selection/gutter
- Memory usage reasonable (LRU eviction)

**Spec References**:
- Section 4: Rendering architecture → Width-dependent caching (lines 194-223)
- Section 12: Performance plan (lines 490-504)
- Renderer cache + "gutter + markdown" composition (lines 905-947)
- Section 2) Render composition (lines 1894-2000+)

**Tasks**:
- [ ] Implement `RenderKey` in `mdx-tui/src/render.rs`:
  - doc_rev, width, theme (Hash + Eq)
- [ ] Implement `RenderedDoc`:
  - lines: Vec<ratatui::text::Line>
  - source_to_rendered_first: Vec<usize>
  - rendered_to_source: Vec<usize>
- [ ] Implement `RendererCache`:
  - LRU cache with reasonable size limit (e.g., 32 entries)
- [ ] Implement basic line mapping (v1 approximation):
  - Track approximate source line per rendered line
  - First rendered line index per source line
- [ ] Integrate cache in render path:
  - Check cache on render
  - If hit, use cached output
  - If miss, render and store
  - Invalidate on rev/width/theme change
- [ ] Use mapping for:
  - Gutter prefix (first rendered line only)
  - Selection highlight (map source range to rendered)
  - Cursor indicator

**Tests**:
- [ ] Cache hit avoids re-render
- [ ] Cache miss triggers render
- [ ] Width change invalidates cache
- [ ] Theme change invalidates cache
- [ ] Doc reload invalidates cache
- [ ] LRU evicts old entries
- [ ] Mapping approximation reasonable for simple docs

**Status**: Not Started

---

## Stage 16: Polish & Documentation

**Goal**: Final polish, help screen, README, error handling
**Success Criteria**:
- ? shows help overlay
- README documents all features and keys
- Errors show user-friendly messages
- Example config documented
- No panics on common error conditions

**Spec References**:
- Section 11: Status bar and UX details (lines 478-486)
- Section 15: Implementation notes / tricky bits (lines 591-616)
- Small but important UX details (lines 1146-1155)

**Tasks**:
- [ ] Implement help overlay (? key):
  - Modal showing all keybindings
  - Grouped by category
  - Esc to close
- [ ] Write comprehensive README.md:
  - Installation instructions
  - Feature list
  - Keybinding reference
  - Configuration guide
  - Example config
- [ ] Review error handling:
  - File not found → clear message
  - Permission denied → clear message
  - Invalid config → show path and error
  - Clipboard failures → graceful degradation
  - Git errors → disable gutter, log warning
- [ ] Add --version flag
- [ ] Add examples/ directory with sample markdown
- [ ] Document config schema in docs/
- [ ] Add CHANGELOG.md

**Tests**:
- [ ] ? shows and hides help correctly
- [ ] README has all keybindings
- [ ] Error messages tested manually
- [ ] --version shows correct version
- [ ] Config examples are valid

**Status**: Not Started

---

## Stage 17: Performance Optimization

**Goal**: Ensure snappy performance on large files (10k+ lines)
**Success Criteria**:
- Scrolling smooth on 10k line file
- Startup < 200ms on typical file
- No frame drops during navigation
- Diff computation doesn't block UI

**Spec References**:
- Section 0: Goals → Fast, low-latency TUI (lines 7-9)
- Section 12: Performance plan (so it stays snappy) (lines 490-504)
- Section 13: Testing strategy (lines 506-528)

**Tasks**:
- [ ] Profile app with large markdown files
- [ ] Optimize hot paths identified:
  - Rendering pipeline
  - Line mapping
  - Scroll calculations
- [ ] Implement viewport culling:
  - Only render visible lines + small buffer
  - Use ratatui Paragraph scrolling
- [ ] Review allocations in tight loops
- [ ] Add benchmarks for core operations:
  - Document load
  - Heading extraction
  - Diff computation
  - Render cache lookup
- [ ] Test with stress cases:
  - 50k line file
  - File with 1000 headings
  - Very wide lines (200+ chars)

**Tests**:
- [ ] Benchmark: load 10k line file < 100ms
- [ ] Benchmark: scroll through file no frame drops
- [ ] Benchmark: diff on 10k lines < 500ms
- [ ] Visual test: smooth scrolling
- [ ] Memory usage reasonable (< 50MB for typical files)

**Status**: Not Started

---

## Stage 18: Search Feature (Bonus)

**Goal**: Implement /search with n/N navigation
**Success Criteria**:
- / enters search mode
- Search highlights all matches
- n/N jumps to next/prev match
- Search wraps around file
- Esc cancels search

**Spec References**:
- Section 0: Goals → Vim-ish navigation → /search (lines 13-15)
- Section 6: Input system → Modes (lines 279-284)

**Tasks**:
- [ ] Add Search mode to Mode enum
- [ ] Implement search input bar at bottom
- [ ] Implement search algorithm:
  - Case-insensitive by default
  - Store match positions
  - Highlight all matches
- [ ] Implement n/N navigation:
  - Jump cursor to next/prev match
  - Wrap at file boundaries
  - Show "N/M" in status
- [ ] Add search keybindings:
  - `/` enter search
  - `Enter` confirm search
  - `Esc` cancel
  - `n` next match
  - `N` previous match

**Tests**:
- [ ] Search finds all matches
- [ ] n/N navigate correctly
- [ ] Search wraps around
- [ ] Case insensitive works
- [ ] Esc cancels search
- [ ] Highlighting visible

**Status**: Not Started

---

## Notes

### Deferred to Future Versions
- Advanced search (regex, case-sensitive toggle)
- Multi-file workspace
- Image preview
- Mermaid diagram rendering
- HTML rendering
- Advanced source-to-rendered mapping (perfect precision)
- Index-based git diff (HEAD is sufficient for v1)
- Search history
- Jump list (Ctrl+O/Ctrl+I)

### Risk Areas
1. **Terminal key handling**: Ctrl+Arrow may not work in all terminals
   - Mitigation: Implement multiple fallbacks (Ctrl+hjkl, Alt+Arrow)
2. **Line mapping precision**: Wrapped markdown is complex
   - Mitigation: Start with best-effort approximation, improve iteratively
3. **Clipboard on Wayland**: arboard has known issues
   - Mitigation: Graceful error messages, document limitations
4. **Performance on huge files**: Rope + rendering can be expensive
   - Mitigation: Cache aggressively, profile early, viewport culling

### Testing Strategy
- Unit tests for core logic (doc, toc, selection, diff, config)
- Integration tests for TUI components where feasible
- Manual acceptance testing for UX (see spec section 13)
- Benchmark suite for performance regression detection
- Test with variety of markdown files (simple, complex, large)

### Development Process
- Follow user's CLAUDE.md guidelines
- Work incrementally, never break builds
- Write tests first when possible
- Commit after each passing stage
- Update this plan's status markers as you go
- Remove this file when all stages complete

### Quick Spec Reference Guide

The `mdx-spec.md` file contains extensive technical details organized as follows:

- **Sections 0-2**: High-level goals, architecture, and dependencies
- **Section 3**: Core data models (Document, ViewState, PaneNode)
- **Section 4**: Rendering architecture and styling
- **Sections 5-11**: Feature implementations (TOC, input, clipboard, config, status, file watching, git diff)
- **Section 12-13**: Performance and testing strategies
- **Section 14**: Build order milestones (quick reference)
- **Section 15**: Tricky implementation details and decisions
- **Lines 622-1755**: Concrete code sketches for data structures, event loops, git integration, etc.

**Pro tip**: When starting a stage, read the referenced spec sections first to understand the complete design before writing code.
