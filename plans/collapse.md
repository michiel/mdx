# Collapsible Blocks Implementation Plan

## Overview

Implement visual collapsing of markdown blocks (headings, code blocks, etc.) in the active pane based on cursor position. Collapsed blocks show a summary line with visual indicator and can be expanded/collapsed with arrow keys.

## Architecture Context

**Key Files Involved:**
- `mdx-tui/src/app.rs` - Application state and ViewState
- `mdx-tui/src/ui.rs` - Rendering pipeline (lines 240-500)
- `mdx-tui/src/input.rs` - Keyboard input handling
- `mdx-core/src/toc.rs` - Heading extraction (already implemented)
- `mdx-tui/src/theme.rs` - Visual styling for collapse indicators

**Existing Patterns to Leverage:**
- Headings already extracted and stored in `Document.headings: Vec<Heading>`
- Code blocks tracked during rendering with fence detection
- Line-by-line rendering allows skipping collapsed content
- Modal keyboard system for context-sensitive key handling
- Theme system for consistent visual styling

---

## Recommendations

### 1. Collapsible Block Types (Priority Order)

**Phase 1 - Headings (Highest Value):**
- Collapse all content under a heading until next same-level or higher-level heading
- Most common use case, leverages existing heading extraction
- Clear block boundaries defined by heading levels

**Phase 2 - Code Blocks:**
- Already tracked with fence detection in rendering loop
- Show first line or language + line count when collapsed

**Future - Lists & Quotes:**
- More complex parsing, defer until heading collapse is stable

### 2. State Storage

**Recommended: Per-Pane State**
```rust
// In mdx-tui/src/panes.rs, add to ViewState:
pub struct ViewState {
    // ... existing fields ...
    pub collapsed_headings: BTreeSet<usize>,  // Line numbers of collapsed heading starts
}
```

**Rationale:**
- Different panes can have different collapse states for same document
- BTreeSet provides O(log n) lookups and maintains sorted order
- Line numbers are stable until document edits (handle invalidation later)
- Avoids coupling Document (shared) with view-specific state (per-pane)

**Persistence Strategy:**
- Start without persistence (resets on reload)
- Add optional persistence in Phase 3 via config

### 3. Visual Design

**Collapsed Heading Indicator:**
```
▶ ## Implementation Details (27 lines)
```

**Expanded Heading Indicator:**
```
▼ ## Implementation Details
```

**Design Choices:**
- Use `▶` (U+25B6) and `▼` (U+25BC) for expand/collapse indicators
- Prefix heading line with indicator
- Suffix with line count in parentheses: `(N lines)` or `(N lines hidden)`
- Truncate heading text to fit: max 32 chars for heading content
- Apply distinct background color (e.g., slightly darker/lighter than theme)
- Or use theme's cursor_line_bg with dim modifier

**Collapsed Code Block:**
```
▶ ```rust (15 lines)
```

### 4. Keyboard Interaction

**Option A: Arrow Keys (As Requested)**
- **Right Arrow** on collapsed block: Expand
- **Left Arrow** on expanded block: Collapse
- **Fallback**: If not collapsible/expandable, normal cursor movement (if horizontal movement implemented)

**Option B: Vim-Style Fold Keys (Recommended Alternative)**
- `za` - Toggle fold at cursor (most common)
- `zo` - Open fold at cursor
- `zc` - Close fold at cursor
- `zM` - Close all folds in pane
- `zR` - Open all folds in pane

**Recommendation:** Start with **Option A** as specified, but consider **Option B** for consistency with vim users' expectations. The `z` prefix is unused and intuitive for folding.

**Implementation Note:**
- Check cursor line for block start (heading or code fence)
- If on collapsed block, offer expand; if on expandable block start, offer collapse
- Visual indicator in status bar: `[Collapsible]` or `[Collapsed - press → to expand]`

### 5. Cursor Behavior

**When Collapsing:**
- Cursor stays on heading line (block start)
- If cursor was inside block, move to heading line

**When Expanding:**
- Cursor stays on heading line
- User can navigate into expanded content normally

**When Navigating:**
- `j`/`k` skip over collapsed content (treat as single line)
- Cursor cannot land inside collapsed block
- Auto-scroll adjusts for collapsed regions

### 6. Rendering Strategy

**Modify `render_markdown()` in ui.rs:**

```rust
// Pseudocode for integration
let mut line_idx = scroll_line;
let mut collapsed_ranges = compute_collapsed_ranges(&view.collapsed_headings, &doc);

while line_idx < visible_end && y < viewport_height {
    // Check if this line starts a collapsed block
    if let Some(range) = collapsed_ranges.get_range_starting_at(line_idx) {
        // Render summary line with indicator
        render_collapsed_summary(range, theme, frame, area, y);

        // Skip collapsed lines
        line_idx = range.end + 1;
        y += 1;
        continue;
    }

    // Normal rendering...
    // (existing code for markdown, code blocks, tables, etc.)
}
```

**Helper Function:**
```rust
fn compute_collapsed_ranges(
    collapsed_headings: &BTreeSet<usize>,
    doc: &Document
) -> Vec<CollapseRange> {
    // For each collapsed heading line:
    //   1. Find heading in doc.headings
    //   2. Compute end line (next same/higher level heading or EOF)
    //   3. Return range with metadata (level, text, line count)
}
```

---

## Implementation Stages

### Stage 1: Core Collapse State & Block Detection
**Goal**: Add collapse state to ViewState and implement block boundary detection for headings

**Tasks:**
1. Add `collapsed_headings: BTreeSet<usize>` to `ViewState` in `panes.rs`
2. Initialize in `ViewState::default()` as empty set
3. Create utility module `mdx-tui/src/collapse.rs`:
   - `struct CollapseRange { start: usize, end: usize, level: usize, text: String, line_count: usize }`
   - `fn compute_heading_range(heading_line: usize, doc: &Document) -> Option<CollapseRange>`
   - `fn compute_all_collapsed_ranges(collapsed: &BTreeSet<usize>, doc: &Document) -> Vec<CollapseRange>`
4. Write unit tests for boundary detection:
   - Single heading at end of document
   - Nested headings (h1 > h2 > h3)
   - Adjacent same-level headings
   - Heading at line 0
   - Empty document

**Success Criteria:**
- ✅ ViewState compiles with new field
- ✅ Block boundaries correctly computed for all test cases
- ✅ No impact on existing functionality (all tests pass)

**Tests:**
```rust
#[test]
fn test_heading_range_single() {
    // Doc: "# Title\nContent\n## Sub\n"
    // Collapse line 0 → range should be [0, 1] (excludes line 2)
}

#[test]
fn test_heading_range_nested() {
    // Doc: "# H1\n## H2\nContent\n## H2b\n# H1b\n"
    // Collapse line 1 (## H2) → range [1, 2] (stops at ## H2b)
}

#[test]
fn test_heading_range_eof() {
    // Doc: "# Title\nContent\n"
    // Collapse line 0 → range [0, 1] (to EOF)
}
```

**Status**: Not Started

---

### Stage 2: Rendering Collapsed Blocks
**Goal**: Modify rendering pipeline to display collapsed block summaries and skip hidden content

**Tasks:**
1. Update `render_markdown()` in `ui.rs`:
   - Call `compute_all_collapsed_ranges()` before render loop
   - Check each line_idx against collapsed ranges
   - Skip rendering of lines inside collapsed blocks
2. Create `render_collapsed_summary()` function:
   - Format: `▶ {heading} ({N} lines)`
   - Truncate heading text to ~32 chars (account for indicator + count)
   - Apply distinct background color from theme
3. Add theme colors to `theme.rs`:
   - `collapsed_block_bg: Color` (slightly different from normal bg)
   - `collapsed_indicator_fg: Color` (for ▶ symbol)
4. Handle edge cases:
   - Collapsed block extending beyond viewport
   - Multiple consecutive collapsed blocks
   - Scroll position adjustment when blocks collapse

**Success Criteria:**
- ✅ Collapsed headings render as single summary line
- ✅ Hidden content is not rendered (performance: O(visible_lines), not O(total_lines))
- ✅ Scrolling works correctly with collapsed blocks
- ✅ Visual indicator is clear and distinct
- ✅ No off-by-one errors in line counting

**Tests:**
- Manual: Open `test-docs/collapse-test.md` with various heading structures
- Manual: Collapse first heading, verify content hidden
- Manual: Scroll through document with mixed collapsed/expanded blocks
- Manual: Verify line counts are accurate

**Status**: Not Started

---

### Stage 3: Keyboard Controls (Arrow Keys)
**Goal**: Implement collapse/expand interactions with arrow keys

**Tasks:**
1. Update `handle_input()` in `input.rs`:
   - Detect `KeyCode::Right` when cursor is on collapsed block start
   - Detect `KeyCode::Left` when cursor is on expandable block start (heading line)
2. Add helper in `app.rs`:
   - `fn is_cursor_on_collapsible_block(&self) -> bool`
   - `fn is_cursor_on_collapsed_block(&self) -> bool`
   - `fn toggle_collapse_at_cursor(&mut self)`
   - `fn expand_at_cursor(&mut self)`
   - `fn collapse_at_cursor(&mut self)`
3. Update collapse state:
   - Insert/remove line number from `view.collapsed_headings`
   - Adjust cursor position if needed (keep on heading line)
   - Trigger re-render
4. Add status indicator:
   - Show `[Collapsible]` or `[Collapsed]` in status bar when applicable

**Success Criteria:**
- ✅ Right arrow expands collapsed block at cursor
- ✅ Left arrow collapses expandable block at cursor
- ✅ Cursor remains on heading line after collapse/expand
- ✅ No action if cursor not on block start
- ✅ Visual feedback via status bar

**Tests:**
- Manual: Navigate to heading, press left arrow, verify collapse
- Manual: Press right arrow on collapsed block, verify expand
- Manual: Press arrows on non-heading lines, verify no action
- Manual: Collapse block with cursor inside, verify cursor moves to heading

**Status**: Not Started

---

### Stage 4: Cursor Navigation with Collapsed Blocks
**Goal**: Update cursor movement to correctly handle collapsed regions

**Tasks:**
1. Update `move_cursor_down()` in `app.rs`:
   - Check if next line is inside collapsed block
   - If yes, skip to next visible line (after collapsed range)
2. Update `move_cursor_up()` in `app.rs`:
   - Check if previous line is inside collapsed block
   - If yes, skip to heading line (start of collapsed range)
3. Update `auto_scroll()`:
   - Adjust viewport calculations to account for collapsed lines
   - Ensure cursor stays visible when expanding/collapsing
4. Update `jump_to_line()` (for TOC navigation):
   - If target line is inside collapsed block, expand it automatically
   - Or move cursor to collapsed heading line and show hint

**Success Criteria:**
- ✅ `j` key skips over collapsed blocks (cursor lands on next visible line)
- ✅ `k` key skips over collapsed blocks (cursor lands on heading)
- ✅ Page up/down correctly calculates visible lines
- ✅ Auto-scroll keeps cursor in viewport after collapse/expand
- ✅ Jumping to collapsed heading works intuitively

**Tests:**
- Manual: Navigate with `j` past collapsed block, verify skip
- Manual: Navigate with `k` above collapsed block, verify skip
- Manual: Use `Ctrl+d` (half page down) across collapsed blocks
- Manual: Jump to heading inside collapsed block via TOC
- Manual: Expand block while cursor at top/bottom of viewport

**Status**: Not Started

---

### Stage 5: Polish & Advanced Features
**Goal**: Add configuration, visual polish, and optional advanced features

**Tasks:**
1. Add configuration in `config.rs`:
   ```toml
   [collapse]
   enabled = true
   indicator_expanded = "▼"
   indicator_collapsed = "▶"
   show_line_count = true
   max_summary_length = 32
   ```
2. Add global fold commands:
   - `zM` - Collapse all headings at level >= N (e.g., all h2+)
   - `zR` - Expand all headings
   - Optionally: `zj` / `zk` to jump to next/prev fold
3. Improve visual feedback:
   - Dim/fade collapsed block background
   - Add subtle border or separator
   - Highlight collapsible headings on hover (if mouse enabled)
4. Optional: Persist collapse state
   - Save per-file collapse state to `~/.cache/mdx/collapse-state.json`
   - Restore on document load
   - Clear cache on document edit (line numbers invalidate)
5. Code block collapsing:
   - Detect fenced code blocks in same way as headings
   - Show: `▶ ```language (N lines)`
   - Use same expand/collapse mechanism

**Success Criteria:**
- ✅ Configuration options work correctly
- ✅ Global fold commands affect all blocks
- ✅ Visual design is polished and consistent with theme
- ✅ (Optional) Collapse state persists across sessions
- ✅ (Optional) Code blocks can be collapsed

**Tests:**
- Manual: Test config options via Options dialog (`O` key)
- Manual: Use `zM` to collapse all, `zR` to expand all
- Manual: Reload file and verify persistence (if enabled)
- Manual: Collapse code block and verify rendering

**Status**: Not Started

---

## Technical Decisions & Rationale

### Why BTreeSet for Collapse State?
- **O(log n) lookups**: Efficient checking if line is collapsed
- **Sorted order**: Easy to find next/previous collapsed block
- **Range queries**: Can use `range()` to find collapsed blocks in viewport
- **Alternative considered**: `HashSet` - faster lookups but no ordering

### Why Per-Pane Instead of Per-Document?
- **User expectation**: Different panes are independent views
- **Flexibility**: Same document in two panes can have different collapse states
- **Simplicity**: No need to sync state across panes
- **Precedent**: `scroll_line` and `cursor_line` are already per-pane

### Why Arrow Keys?
- **User requested**: Specification asks for arrow-left/arrow-right
- **Simple mental model**: Right = expand, Left = collapse
- **Low conflict**: Horizontal movement not currently used heavily
- **Alternative**: vim-style `z` prefix keys (can add later)

### Why Headings First, Code Later?
- **Highest value**: Headings define document structure, most common fold use case
- **Leverage existing**: Heading extraction already implemented in `toc.rs`
- **Clear boundaries**: Heading levels define unambiguous block ranges
- **Code blocks**: Already tracked, but less critical for initial version

### Handling Line Number Invalidation
- **Phase 1-4**: Ignore document edits (view-only for now)
- **Future**: On document edit, clear all collapse state (simple but loses state)
- **Advanced**: Track edits and adjust line numbers (complex, defer)

---

## Open Questions

1. **Collapse on document load?**
   - Auto-collapse headings at certain levels (e.g., all h3+)?
   - Or start fully expanded (safer default)?
   - **Recommendation**: Start expanded, let user collapse as needed

2. **Visual indicator for expandable (non-collapsed) headings?**
   - Show `▼` on all headings even when expanded?
   - Or only show indicator when cursor is on heading?
   - **Recommendation**: Always show `▼` when expanded for consistency

3. **Collapse nested headings recursively?**
   - If h1 collapsed, also collapse all h2/h3 under it?
   - Or keep sub-heading collapse state independent?
   - **Recommendation**: Independent state (simpler, more flexible)

4. **Mouse interaction?**
   - Click on `▶` / `▼` to toggle?
   - **Recommendation**: Phase 5 enhancement, keyboard-first for Phase 1-4

5. **Search behavior with collapsed blocks?**
   - Auto-expand blocks containing search matches?
   - Or show indicator like `▶ ## Heading (2 matches)`?
   - **Recommendation**: Auto-expand blocks with matches (better UX)

---

## Testing Strategy

### Unit Tests (Rust)
- Block boundary detection (Stage 1)
- Collapsed range computation with edge cases
- Line number operations (skip, adjust, etc.)

### Integration Tests (Manual with Test Documents)
Create `test-docs/collapse-test.md`:
```markdown
# Top Level
Content under h1.

## Section 1
Content under h2.

### Subsection 1.1
Content under h3.

### Subsection 1.2
More content.

## Section 2
Different section.

# Another Top Level
Final section.
```

**Test Cases:**
1. Collapse h1 "Top Level" → should hide all until "Another Top Level"
2. Collapse h2 "Section 1" → should hide until "Section 2"
3. Collapse h3 "Subsection 1.1" → should hide only its content
4. Navigate with j/k across collapsed blocks
5. Expand/collapse with arrow keys
6. Search inside collapsed blocks
7. Jump to collapsed heading via TOC

### Regression Tests
- Ensure all existing features still work:
  - Visual line mode selection
  - TOC navigation
  - Search highlighting
  - Code block rendering
  - Table rendering
  - Pane splits and focus

---

## Definition of Done (All Stages)

- [ ] All tests pass (unit + manual test cases)
- [ ] No compiler warnings
- [ ] Code follows existing patterns in codebase
- [ ] Collapse state correctly maintained across:
  - [ ] Scrolling
  - [ ] Pane focus changes
  - [ ] Document switches
  - [ ] Terminal resize
- [ ] Visual design matches theme system
- [ ] Keyboard shortcuts documented in help dialog (`?`)
- [ ] No performance regression (test with large documents)
- [ ] No off-by-one errors in line calculations

---

## Risk Mitigation

**Risk 1: Line wrapping complexity**
- Collapsed blocks measured in source lines, but rendering uses visual lines
- Long lines wrap, affecting visible line count
- **Mitigation**: Store and display source line count, not visual line count

**Risk 2: Performance with many collapsed blocks**
- Computing collapsed ranges on every frame could be expensive
- **Mitigation**: Cache collapsed ranges, invalidate on state change

**Risk 3: Interaction with existing features**
- Search, selection, TOC might behave unexpectedly with collapsed blocks
- **Mitigation**: Test each feature explicitly, expand blocks as needed

**Risk 4: Theme compatibility**
- Collapse indicators might not be visible on all themes
- **Mitigation**: Use theme system colors, test on Dark and Light themes

---

## Future Enhancements (Beyond Scope)

- [ ] Collapse list items (bullet and numbered lists)
- [ ] Collapse block quotes
- [ ] Collapse custom MDX components
- [ ] Regex-based custom fold definitions
- [ ] Minimap showing document structure with collapsed sections
- [ ] Fold level controls (fold all h3+, h4+, etc.)
- [ ] Sticky headers (show parent heading when scrolling through section)

---

## References

**Codebase Files:**
- `/home/michiel/dev/mdx-workspace/mdx-tui/src/app.rs` - App state, ViewState
- `/home/michiel/dev/mdx-workspace/mdx-tui/src/ui.rs` - Rendering (lines 240-500)
- `/home/michiel/dev/mdx-workspace/mdx-tui/src/input.rs` - Keyboard handling
- `/home/michiel/dev/mdx-workspace/mdx-core/src/toc.rs` - Heading extraction
- `/home/michiel/dev/mdx-workspace/mdx-tui/src/panes.rs` - ViewState definition
- `/home/michiel/dev/mdx-workspace/mdx-tui/src/theme.rs` - Color definitions

**Vim Folding Reference:**
- `za` - Toggle fold
- `zo` - Open fold
- `zc` - Close fold
- `zR` - Open all folds
- `zM` - Close all folds

**Similar Features:**
- Visual Line Mode (lines 464-470 in ui.rs) - Shows how to style line ranges
- Code Block Rendering (lines 478-486) - Shows how to skip fence lines
- Table Rendering - Shows how to handle multi-line blocks
