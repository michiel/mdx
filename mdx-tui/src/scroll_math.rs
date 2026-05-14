//! Pure scroll arithmetic.
//!
//! These helpers do not touch `App`, `Doc`, or any rendering state. They
//! take the numbers they need as arguments and return the new position.
//! Keeping the logic pure makes it unit-testable, and gives wheel,
//! keyboard, resize, and reload code paths one authoritative definition
//! of each operation.
//!
//! Every function clamps to `[bounds_lo, bounds_hi]` where `bounds_lo` is
//! typically `front_matter.end_line + 1` and `bounds_hi` is
//! `line_count - 1`.

/// A scroll position that identifies both which source line is at the top of
/// the viewport AND which visual wrap-row within that line is first shown.
///
/// `wrap_row = 0` means "start from the very first visual row of
/// `source_line`" — the legacy behaviour. `wrap_row > 0` means the user
/// has scrolled partway through a wrapped paragraph. This is the type that
/// replaces the raw `scroll_line: usize` field in `PaneView`.
///
/// Only scroll carries a `wrap_row`; the cursor is always source-line
/// granular because editing / search / TOC operate on source lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VisualPos {
    /// Index into the rope's line array.
    pub source_line: usize,
    /// Which visual row within that source line is shown first (0-based).
    /// Must always satisfy `wrap_row < visual_height_of_line(source_line)`.
    pub wrap_row: u16,
}

impl VisualPos {
    /// Construct a position pointing at the first visual row of a source line.
    #[inline]
    pub fn at(source_line: usize) -> Self {
        Self { source_line, wrap_row: 0 }
    }

    /// Snap `wrap_row` into the valid range for a line whose total visual
    /// height is `line_height`. Called on resize so the stored offset is
    /// never out-of-bounds for the new wrap width.
    #[inline]
    pub fn snap_wrap_row(&mut self, line_height: u16) {
        let max = line_height.saturating_sub(1);
        if self.wrap_row > max {
            self.wrap_row = max;
        }
    }
}

/// Clamp a scroll position to the rendered content range.
///
/// The `line_count` and `visible_height` parameters are accepted for
/// compatibility but no longer enforce the "keep viewport full" constraint.
/// That constraint prevented the user from scrolling to see the last lines
/// of a document when source lines wrap to multiple visual rows (the
/// renderer would show fewer lines than `visible_height` source lines), so
/// we allow scrolling all the way to `bounds_hi` (the last source line).
pub fn clamp_scroll(
    scroll_line: usize,
    bounds_lo: usize,
    bounds_hi: usize,
    _line_count: usize,
    _visible_height: usize,
) -> usize {
    let bounds_hi = bounds_hi.max(bounds_lo);
    scroll_line.clamp(bounds_lo, bounds_hi)
}

/// Clamp a cursor position into the rendered content range.
pub fn clamp_cursor(cursor_line: usize, bounds_lo: usize, bounds_hi: usize) -> usize {
    cursor_line.clamp(bounds_lo, bounds_hi.max(bounds_lo))
}

/// Compute the line step for PgDn/PgUp: `pane_height - overlap`, never less
/// than one line, overlap itself clamped to `pane_height / 2`.
pub fn page_step(pane_height: usize, overlap: usize) -> usize {
    let overlap = overlap.min(pane_height / 2);
    pane_height.saturating_sub(overlap).max(1)
}

/// Compute the half-page step (always visual rows). Floor division — a
/// 1-row pane still advances by 1.
pub fn half_page_step(pane_height: usize) -> usize {
    (pane_height / 2).max(1)
}

/// New scroll position after advancing by `delta` source lines, clamped.
/// `forward` selects direction; the delta itself is unsigned.
pub fn advance_scroll(
    scroll_line: usize,
    delta: usize,
    forward: bool,
    bounds_lo: usize,
    bounds_hi: usize,
    line_count: usize,
    visible_height: usize,
) -> usize {
    let new = if forward {
        scroll_line.saturating_add(delta)
    } else {
        scroll_line.saturating_sub(delta)
    };
    clamp_scroll(new, bounds_lo, bounds_hi, line_count, visible_height)
}

/// If the cursor sits outside the viewport defined by `[scroll_line,
/// scroll_line + visible_height)`, snap it to the nearest edge. Returns
/// the new cursor position.
pub fn snap_cursor_into_view(
    cursor_line: usize,
    scroll_line: usize,
    visible_height: usize,
    bounds_lo: usize,
    bounds_hi: usize,
) -> usize {
    if visible_height == 0 {
        return cursor_line;
    }
    let top = scroll_line;
    let bot = scroll_line.saturating_add(visible_height.saturating_sub(1));
    let clamped = cursor_line.clamp(top, bot);
    clamp_cursor(clamped, bounds_lo, bounds_hi)
}

/// How to position the viewport relative to a new cursor target when
/// `goto()`-style navigation runs. See `App::goto` for the dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPolicy {
    /// Minimal movement: if the cursor is already visible, keep the
    /// viewport where it is; otherwise snap it to the nearest edge.
    /// Same behaviour `auto_scroll_to_cursor` has always had.
    NearestEdge,
    /// Center the cursor in the viewport (vim `zz`).
    Center,
    /// Place the cursor near the top of the viewport, with roughly a
    /// quarter of the viewport above it (vim `zt` leaves 0 rows above;
    /// this policy intentionally keeps a little context).
    TopQuarter,
    /// Keep the cursor at the same row offset within the viewport it
    /// previously occupied. Falls back to NearestEdge when the prior
    /// cursor was not inside the viewport.
    KeepOffset,
}

/// Compute the new scroll position after moving the cursor to
/// `new_cursor`, given the previous cursor/scroll and the policy.
pub fn scroll_for_policy(
    new_cursor: usize,
    prev_cursor: usize,
    scroll_line: usize,
    visible_height: usize,
    bounds_lo: usize,
    bounds_hi: usize,
    line_count: usize,
    policy: ScrollPolicy,
) -> usize {
    if visible_height == 0 {
        return scroll_line;
    }
    let new_scroll = match policy {
        ScrollPolicy::NearestEdge => {
            return auto_scroll_to_cursor(
                new_cursor,
                scroll_line,
                visible_height,
                bounds_lo,
                bounds_hi,
                line_count,
            );
        }
        ScrollPolicy::Center => {
            let half = visible_height / 2;
            new_cursor.saturating_sub(half)
        }
        ScrollPolicy::TopQuarter => {
            let quarter = visible_height / 4;
            new_cursor.saturating_sub(quarter)
        }
        ScrollPolicy::KeepOffset => {
            let top = scroll_line;
            let bot = scroll_line.saturating_add(visible_height.saturating_sub(1));
            if prev_cursor >= top && prev_cursor <= bot {
                let offset = prev_cursor - top;
                new_cursor.saturating_sub(offset)
            } else {
                // Fallback: treat as NearestEdge.
                return auto_scroll_to_cursor(
                    new_cursor,
                    scroll_line,
                    visible_height,
                    bounds_lo,
                    bounds_hi,
                    line_count,
                );
            }
        }
    };
    clamp_scroll(new_scroll, bounds_lo, bounds_hi, line_count, visible_height)
}

/// If the cursor is outside the viewport, move the viewport so the cursor
/// sits at the nearest edge. Used by keyboard paths where we want the
/// viewport to follow the cursor rather than the other way around.
pub fn auto_scroll_to_cursor(
    cursor_line: usize,
    scroll_line: usize,
    visible_height: usize,
    bounds_lo: usize,
    bounds_hi: usize,
    line_count: usize,
) -> usize {
    if visible_height == 0 {
        return scroll_line;
    }
    let top = scroll_line;
    let bot = scroll_line.saturating_add(visible_height.saturating_sub(1));
    let new_scroll = if cursor_line < top {
        cursor_line
    } else if cursor_line > bot {
        cursor_line.saturating_sub(visible_height.saturating_sub(1))
    } else {
        scroll_line
    };
    clamp_scroll(new_scroll, bounds_lo, bounds_hi, line_count, visible_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- VisualPos -----------------------------------------------------------

    #[test]
    fn visual_pos_at_produces_zero_wrap_row() {
        let p = VisualPos::at(42);
        assert_eq!(p.source_line, 42);
        assert_eq!(p.wrap_row, 0);
    }

    #[test]
    fn visual_pos_snap_wrap_row_clamps_to_last_valid_row() {
        let mut p = VisualPos { source_line: 10, wrap_row: 5 };
        p.snap_wrap_row(3); // line_height=3, valid rows=0..2 → max=2
        assert_eq!(p.wrap_row, 2);
    }

    #[test]
    fn visual_pos_snap_wrap_row_noop_when_valid() {
        let mut p = VisualPos { source_line: 10, wrap_row: 1 };
        p.snap_wrap_row(3);
        assert_eq!(p.wrap_row, 1);
    }

    #[test]
    fn visual_pos_snap_wrap_row_single_row_line() {
        let mut p = VisualPos { source_line: 0, wrap_row: 3 };
        p.snap_wrap_row(1); // 1-row line, only valid row is 0
        assert_eq!(p.wrap_row, 0);
    }

    #[test]
    fn visual_pos_default_is_zero() {
        let p = VisualPos::default();
        assert_eq!(p.source_line, 0);
        assert_eq!(p.wrap_row, 0);
    }

    // --- clamp_scroll -----------------------------------------------------

    #[test]
    fn clamp_scroll_respects_bounds_lo() {
        assert_eq!(clamp_scroll(0, 3, 99, 100, 20), 3);
    }

    #[test]
    fn clamp_scroll_respects_bounds_hi() {
        // bounds_hi is 99; value above it clamps to 99.
        assert_eq!(clamp_scroll(200, 0, 99, 100, 20), 99);
    }

    #[test]
    fn clamp_scroll_allows_scroll_to_doc_end() {
        // 100 lines, visible 20. User should be able to scroll to line 99
        // (last source line at top), even though it leaves empty space below.
        // This is the vim-style behaviour that fixes the "can't reach end"
        // bug when source lines wrap to multiple visual rows.
        assert_eq!(clamp_scroll(95, 0, 99, 100, 20), 95);
        assert_eq!(clamp_scroll(99, 0, 99, 100, 20), 99);
    }

    #[test]
    fn clamp_scroll_doc_shorter_than_viewport() {
        // If the document is shorter than the viewport, scroll must stay at
        // bounds_lo (don't force a "full" viewport we can't fill).
        assert_eq!(clamp_scroll(5, 0, 9, 10, 20), 5);
    }

    #[test]
    fn clamp_scroll_zero_height() {
        // Degenerate viewport: clamp still works, just to the bounds.
        assert_eq!(clamp_scroll(5, 0, 9, 10, 0), 5);
    }

    #[test]
    fn clamp_scroll_with_front_matter_bounds() {
        // Front matter consumed lines 0..=4, rendered content starts at 5.
        assert_eq!(clamp_scroll(3, 5, 99, 100, 20), 5);
    }

    // --- clamp_cursor -----------------------------------------------------

    #[test]
    fn clamp_cursor_basic() {
        assert_eq!(clamp_cursor(3, 5, 99), 5);
        assert_eq!(clamp_cursor(150, 5, 99), 99);
        assert_eq!(clamp_cursor(42, 5, 99), 42);
    }

    #[test]
    fn clamp_cursor_inverted_bounds_safe() {
        // bounds_hi < bounds_lo shouldn't panic; pick bounds_lo.
        assert_eq!(clamp_cursor(0, 5, 1), 5);
    }

    // --- page_step --------------------------------------------------------

    #[test]
    fn page_step_applies_overlap() {
        assert_eq!(page_step(20, 2), 18);
    }

    #[test]
    fn page_step_never_zero() {
        assert_eq!(page_step(1, 99), 1);
        assert_eq!(page_step(0, 99), 1);
    }

    #[test]
    fn page_step_clamps_overlap_to_half() {
        // overlap > pane_height/2 clamped to pane_height/2
        assert_eq!(page_step(10, 50), 5);
    }

    // --- half_page_step --------------------------------------------------

    #[test]
    fn half_page_step_basic() {
        assert_eq!(half_page_step(20), 10);
        assert_eq!(half_page_step(1), 1);
        assert_eq!(half_page_step(0), 1);
    }

    // --- advance_scroll --------------------------------------------------

    #[test]
    fn advance_scroll_forward() {
        assert_eq!(advance_scroll(10, 5, true, 0, 99, 100, 20), 15);
    }

    #[test]
    fn advance_scroll_backward() {
        assert_eq!(advance_scroll(10, 5, false, 0, 99, 100, 20), 5);
    }

    #[test]
    fn advance_scroll_saturates_backward_at_zero() {
        assert_eq!(advance_scroll(3, 100, false, 0, 99, 100, 20), 0);
    }

    #[test]
    fn advance_scroll_clamps_at_end() {
        // 100-line doc, bounds_hi=99 → max scroll is 99 (last source line).
        assert_eq!(advance_scroll(70, 50, true, 0, 99, 100, 20), 99);
    }

    #[test]
    fn advance_scroll_round_trip_on_stable_range() {
        let start = 30;
        let after_fwd = advance_scroll(start, 10, true, 0, 99, 100, 20);
        let after_back = advance_scroll(after_fwd, 10, false, 0, 99, 100, 20);
        assert_eq!(after_back, start);
    }

    // --- snap_cursor_into_view -------------------------------------------

    #[test]
    fn snap_cursor_above_viewport() {
        // scroll=30, visible=20 → viewport [30, 49].
        assert_eq!(snap_cursor_into_view(10, 30, 20, 0, 99), 30);
    }

    #[test]
    fn snap_cursor_below_viewport() {
        assert_eq!(snap_cursor_into_view(80, 30, 20, 0, 99), 49);
    }

    #[test]
    fn snap_cursor_inside_viewport() {
        assert_eq!(snap_cursor_into_view(35, 30, 20, 0, 99), 35);
    }

    #[test]
    fn snap_cursor_zero_height_noop() {
        assert_eq!(snap_cursor_into_view(42, 30, 0, 0, 99), 42);
    }

    // --- auto_scroll_to_cursor -------------------------------------------

    #[test]
    fn auto_scroll_cursor_above() {
        // cursor 5 above viewport [30, 49] → scroll moves to 5.
        assert_eq!(auto_scroll_to_cursor(5, 30, 20, 0, 99, 100), 5);
    }

    #[test]
    fn auto_scroll_cursor_below() {
        // cursor 80 below viewport [30, 49] → scroll = 80 - 19 = 61.
        assert_eq!(auto_scroll_to_cursor(80, 30, 20, 0, 99, 100), 61);
    }

    #[test]
    fn auto_scroll_cursor_visible_noop() {
        assert_eq!(auto_scroll_to_cursor(35, 30, 20, 0, 99, 100), 30);
    }

    #[test]
    fn auto_scroll_end_of_doc_clamps() {
        // cursor at last line (99); scroll goes to 99 - 19 = 80, clamped to
        // bounds_hi (99). The "keep viewport full" cap is gone — users can
        // now always scroll to the last source line.
        assert_eq!(auto_scroll_to_cursor(99, 0, 20, 0, 99, 100), 80);
    }

    // --- scroll_for_policy -----------------------------------------------

    #[test]
    fn policy_center_places_cursor_near_middle() {
        // 100-line doc, 20 visible, target line 50. Center → scroll 40.
        let s = scroll_for_policy(50, 0, 0, 20, 0, 99, 100, ScrollPolicy::Center);
        assert_eq!(s, 40);
    }

    #[test]
    fn policy_top_quarter_places_cursor_near_top() {
        // 20 visible, quarter = 5. target 50 → scroll 45.
        let s = scroll_for_policy(50, 0, 0, 20, 0, 99, 100, ScrollPolicy::TopQuarter);
        assert_eq!(s, 45);
    }

    #[test]
    fn policy_keep_offset_preserves_relative_position() {
        // prev_cursor=35, scroll=30 → offset=5. new_cursor=70 → scroll=65.
        let s = scroll_for_policy(70, 35, 30, 20, 0, 199, 200, ScrollPolicy::KeepOffset);
        assert_eq!(s, 65);
    }

    #[test]
    fn policy_keep_offset_falls_back_when_prev_invisible() {
        // prev_cursor=5, scroll=30 — prev was above viewport.
        // Behaves like NearestEdge.
        let s = scroll_for_policy(70, 5, 30, 20, 0, 199, 200, ScrollPolicy::KeepOffset);
        assert_eq!(s, 51); // 70 - 19 = 51 (cursor at bottom edge)
    }

    #[test]
    fn policy_nearest_edge_matches_auto_scroll() {
        // Should produce identical output to auto_scroll_to_cursor.
        let a = auto_scroll_to_cursor(80, 30, 20, 0, 99, 100);
        let b = scroll_for_policy(80, 0, 30, 20, 0, 99, 100, ScrollPolicy::NearestEdge);
        assert_eq!(a, b);
    }

    #[test]
    fn policy_center_clamps_at_doc_start() {
        // target 2, visible 20. Half = 10. cursor - half saturates to 0.
        let s = scroll_for_policy(2, 0, 0, 20, 0, 99, 100, ScrollPolicy::Center);
        assert_eq!(s, 0);
    }

    #[test]
    fn policy_center_clamps_at_doc_end() {
        // target 99, visible 20, half=10 → 89. clamp_scroll allows up to
        // bounds_hi=99, so scroll lands at 89 (cursor centered in viewport).
        let s = scroll_for_policy(99, 0, 0, 20, 0, 99, 100, ScrollPolicy::Center);
        assert_eq!(s, 89);
    }

    // --- Table-driven test matrix mirroring review.md §6 -----------------

    #[test]
    fn test_matrix_pgdn_pgup_roundtrip() {
        // PgDn×5 then PgUp×5 returns to start, no overlap.
        let page = 20;
        let overlap = 0;
        let step = page_step(page, overlap);
        let mut s = 50usize;
        for _ in 0..5 {
            s = advance_scroll(s, step, true, 0, 999, 1000, page);
        }
        for _ in 0..5 {
            s = advance_scroll(s, step, false, 0, 999, 1000, page);
        }
        assert_eq!(s, 50);
    }

    #[test]
    fn test_matrix_pgdn_with_overlap_is_consistent() {
        // With overlap=2 we cover (page - 2) rows per press; round trip still
        // returns to start on a stable range.
        let page = 20;
        let overlap = 2;
        let step = page_step(page, overlap);
        let start = 100;
        let mut s = start;
        for _ in 0..3 {
            s = advance_scroll(s, step, true, 0, 999, 1000, page);
        }
        for _ in 0..3 {
            s = advance_scroll(s, step, false, 0, 999, 1000, page);
        }
        assert_eq!(s, start);
    }

    #[test]
    fn test_matrix_resize_smaller_while_at_bottom() {
        // Without the keep-full constraint, clamp_scroll only clips to
        // bounds_hi. Both values are within [0, 99] so they pass through.
        assert_eq!(clamp_scroll(80, 0, 99, 100, 10), 80);
        assert_eq!(clamp_scroll(95, 0, 99, 100, 20), 95);
    }

    #[test]
    fn test_matrix_reload_shorter_doc_clamps() {
        // Doc shrank from 1000 lines to 50. cursor at 800, scroll at 900.
        assert_eq!(clamp_cursor(800, 0, 49), 49);
        // scroll 900 clamped to bounds_hi=49.
        assert_eq!(clamp_scroll(900, 0, 49, 50, 20), 49);
    }

    #[test]
    fn test_matrix_front_matter_skip() {
        // front_matter ends at line 9, content starts at 10.
        assert_eq!(clamp_cursor(3, 10, 99), 10);
        assert_eq!(clamp_scroll(3, 10, 99, 100, 20), 10);
    }

    #[test]
    fn test_matrix_one_by_one_terminal() {
        // Degenerate but non-panicking.
        assert_eq!(clamp_scroll(0, 0, 0, 1, 0), 0);
        assert_eq!(clamp_cursor(0, 0, 0), 0);
        assert_eq!(page_step(0, 0), 1);
        assert_eq!(half_page_step(0), 1);
    }
}
