# Mouse Support Implementation Plan

## Overview

Add comprehensive mouse support including click-to-focus, drag-to-resize splits, text selection, TOC interaction, and mouse wheel scrolling.

## Constraints & Design Decisions

- Mouse selection is line-based using existing visual line mode (no per-character selection yet)
- Avoid Shift-based mouse selection so terminals don't intercept (Shift+drag stays terminal-driven)
- Mouse hit-testing uses the current terminal layout (borders + breadcrumb line)
- Scrolling is line-based (adjust scroll_line/toc_scroll) and clamps to valid ranges
- **TOC clicks do NOT transfer focus** - they navigate but keep focus on TOC
- **Single click without drag clears any existing selection**

## Stage 1: Foundation (Layout & Event Wiring)

**Goal**: Mouse events are captured and coordinates can be mapped to UI elements (panes, TOC, borders)

**Success Criteria**:
- Can map any mouse coordinate to: pane ID, TOC row index, or split boundary
- Mouse events are received and dispatched to appropriate handlers
- No visual changes yet, but logging shows correct hit detection

**Implementation Tasks**:
1. Add layout helper module (new `layout.rs` or extend `ui.rs`):
   - Function to compute TOC rect (if shown), pane rects, content rects (minus borders/breadcrumb)
   - Takes terminal size and app state as input
   - Returns structured layout info for hit-testing
2. Add hit-testing helpers:
   - `hit_test_pane(x, y) -> Option<PaneId>`
   - `hit_test_toc(x, y) -> Option<usize>` (returns row index accounting for scroll)
   - `hit_test_split_border(x, y) -> Option<SplitPath>` (within 1 cell of boundary)
3. Add `MouseState` struct to track interaction state:
   - `Idle`, `Selecting { pane_id, anchor_line }`, `Resizing { split_path, start_ratio, start_pos }`
   - Add to `App` struct
4. Wire up mouse event handling in main event loop:
   - Extend event loop to handle `crossterm::event::Event::Mouse`
   - Create `input::handle_mouse(app, mouse_event)` dispatcher
   - Route to specific handlers based on event type and hit-test results

**Tests**:
- Unit test: layout computation with various terminal sizes and TOC visible/hidden
- Unit test: hit-testing returns correct pane/TOC/border for known coordinates
- Unit test: MouseState transitions (idle -> selecting, idle -> resizing)
- Manual: Run with logging to verify mouse coords map correctly

**Status**: Not Started

---

## Stage 2: Basic Interactions (Focus & Selection)

**Goal**: Click to focus panes, drag to select text (line-based), copy selection to clipboard

**Success Criteria**:
- Left click in pane content focuses that pane and moves cursor to clicked line
- Left click without drag clears any existing selection
- Left drag (no modifier) enters visual line mode and updates selection as mouse moves
- Mouse up exits visual line mode
- Ctrl+Shift+C copies selected text to clipboard
- Selection works across all panes

**Implementation Tasks**:
1. Implement click-to-focus:
   - On left click in pane content area: focus pane, set cursor to clicked line (accounting for scroll_line)
   - Clear selection if click without drag
2. Implement drag-to-select:
   - On left mouse down: store anchor line, enter visual line mode
   - On mouse move while button held: update selection end cursor based on current mouse position
   - On mouse up: exit visual line mode but keep selection
   - Use existing visual line mode infrastructure
3. Add Ctrl+Shift+C keybinding:
   - Check if keybinding already exists (look for `yank_selection`)
   - If not, implement yank functionality using existing clipboard integration
   - Show brief status message "Selection copied" (optional)
4. Handle edge cases:
   - Dragging outside pane bounds (clamp to valid lines)
   - Selection in scrolled content (map mouse y to document line correctly)
   - Single click clears selection before setting new cursor position

**Tests**:
- Unit test: cursor position calculation from mouse coords + scroll offset
- Unit test: selection range updates during drag simulation
- App-level test: focus changes when clicking different panes
- App-level test: selection cleared on single click
- Manual: Click various panes and verify focus, drag to select text, copy with Ctrl+Shift+C

**Status**: Not Started

---

## Stage 3: TOC & Scrolling

**Goal**: TOC clicks navigate to headings, mouse wheel scrolls both pane content and TOC

**Success Criteria**:
- Clicking a TOC row jumps to that heading in the currently focused document pane
- TOC clicks do NOT change focus (TOC stays focused if it was focused)
- Mouse wheel over TOC scrolls the TOC list
- Mouse wheel over pane scrolls that pane's content
- Scrolling does NOT move the cursor (unlike keyboard scrolling with auto_scroll)
- Wheel events are properly clamped to valid scroll ranges

**Implementation Tasks**:
1. Implement TOC click navigation:
   - On left click in TOC: calculate `toc_selected` from click y-coord + `toc_scroll`
   - Call `toc_jump_to_selected()` targeting the currently focused pane
   - Do NOT change focus away from TOC
   - Handle case where focused pane is not a document pane (keep TOC focused, no jump)
2. Implement mouse wheel scrolling for TOC:
   - Detect wheel events (up/down) when mouse is over TOC
   - Adjust `toc_scroll` by scroll delta (typically ±3 lines per wheel notch)
   - Clamp to `0..max(0, toc_items.len() - visible_rows)`
   - Do NOT change TOC selection or focus
3. Implement mouse wheel scrolling for panes:
   - Detect wheel events when mouse is over pane content
   - Adjust that pane's `scroll_line` by scroll delta
   - Clamp to `0..max(0, doc_lines - visible_lines)`
   - Do NOT move cursor (disable auto_scroll for mouse-initiated scrolling)
   - Preserve existing auto_scroll behavior for keyboard actions
4. Handle edge cases:
   - Wheel events on borders (ignore or scroll nearest pane)
   - Wheel events when TOC is hidden (only scroll panes)
   - Rapid wheel events (accumulate or handle per-event)

**Tests**:
- Unit test: TOC row calculation from mouse y + scroll offset
- Unit test: scroll_line clamping with various document lengths
- App-level test: TOC click jumps to correct heading without changing focus
- App-level test: wheel scroll adjusts scroll values but not cursor
- Manual: Scroll TOC with wheel, scroll various panes, click TOC rows, verify focus behavior

**Status**: Not Started

---

## Stage 4: Pane Resizing

**Goal**: Drag split borders to resize panes with smooth updates and minimum size limits

**Success Criteria**:
- Mouse down on split boundary (within 1 cell) enters resize mode
- Dragging adjusts split ratio in real-time with visual feedback
- Minimum pane size enforced (e.g., 10% / 90% ratio limits)
- Mouse up commits the new ratio
- Works for both horizontal and vertical splits
- Nested splits are correctly identified and updated

**Implementation Tasks**:
1. Implement split boundary detection:
   - Extend hit-testing to identify split boundaries (within 1 cell of split line)
   - Return split path (index path from root to the split node being resized)
   - Determine split orientation (horizontal/vertical) for correct resize axis
2. Implement resize drag handling:
   - On mouse down on boundary: enter `Resizing` state with split path, current ratio, start mouse pos
   - On mouse move while resizing: calculate new ratio based on mouse delta and split dimensions
   - Update split node ratio and trigger re-layout
   - Clamp ratio to min/max (e.g., 0.1 to 0.9) to prevent invisible panes
   - On mouse up: commit final ratio and return to `Idle` state
3. Add split path resolution:
   - Helper to traverse split tree and find/update split node by path
   - Ensure updates trigger proper re-render
4. Handle edge cases:
   - Mouse dragged outside terminal bounds (clamp to valid range)
   - Rapid mouse movements (ensure smooth updates)
   - Resizing the only split (should be prevented)
   - Nested splits with multiple boundaries close together (pick closest)

**Tests**:
- Unit test: split boundary hit-testing for various layouts
- Unit test: ratio calculation from mouse delta with clamping
- Unit test: split path resolution finds correct node in nested tree
- App-level test: dragging boundary updates split ratio
- App-level test: minimum size limits enforced
- Manual: Resize various splits (horizontal, vertical, nested), verify smooth updates and limits

**Status**: Not Started

---

## Stage 5: Polish & Documentation

**Goal**: Update help text, add visual feedback, verify all functionality works together

**Success Criteria**:
- Help popup includes all mouse operations with clear descriptions
- Optional: status hints when selection is yanked
- All manual verification tests pass
- No regressions in keyboard-only operation

**Implementation Tasks**:
1. Update help text (`ui.rs` or wherever help is rendered):
   - Add section: "Mouse Support"
   - Click pane to focus, drag to select text (line-based)
   - Ctrl+Shift+C to copy selection
   - Click TOC to navigate
   - Scroll wheel to scroll pane or TOC
   - Drag split borders to resize
2. Add optional status messages:
   - Brief "Selection copied" message after Ctrl+Shift+C
   - Consider visual indicator when in resize mode (e.g., highlight border)
3. Create manual verification checklist:
   - [ ] Click to focus each pane type
   - [ ] Drag to select text in various panes, copy with Ctrl+Shift+C
   - [ ] Single click clears selection
   - [ ] Click TOC rows to navigate (verify focus stays on TOC)
   - [ ] Scroll TOC with mouse wheel
   - [ ] Scroll pane content with mouse wheel (verify cursor doesn't move)
   - [ ] Drag horizontal split boundary
   - [ ] Drag vertical split boundary
   - [ ] Drag nested split boundaries
   - [ ] Verify minimum pane size limits
   - [ ] Verify all keyboard shortcuts still work (no regressions)
4. Final integration testing:
   - Test mouse + keyboard combinations (e.g., keyboard select then mouse click)
   - Test with different terminal sizes
   - Test with TOC hidden/shown
   - Test with various split layouts

**Tests**:
- App-level integration tests covering common workflows
- Manual checklist verification (all items must pass)
- Regression testing: keyboard-only operation unaffected

**Status**: Not Started

---

## Future Enhancements (Not in this plan)

Consider for follow-up work:
- Per-character selection (not line-based)
- Double-click to select word/paragraph
- Right-click context menu or paste
- Middle-click paste on X11
- Alt+wheel for horizontal scroll
- Shift+wheel for faster scroll
- Mouse cursor shape changes (resize arrows on borders)

---

## Notes

- **Dependencies**: Stage 1 must complete before any other stage
- **Incremental commits**: Each stage should result in working, tested code
- **Remove this file** when all stages are marked Complete
