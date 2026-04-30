//! Wrapped-line layout cache.
//!
//! Scroll math and the renderer both need to know "how many visual rows
//! does source line N occupy at the current content width?" The answer
//! depends on the width, the document revision, and nothing else we
//! care about here (fonts, fold state, and image rows are applied on
//! top of this by the caller if needed).
//!
//! Walking the rope per input event costs O(lines) per keystroke on a
//! held-j burst. This cache amortizes that to O(1) per query after a
//! O(lines) rebuild whenever the width or document changes.
//!
//! Wrapping heuristic: `(line_len_chars + width - 1) / width`, floored
//! at 1 — matches the heuristic that used to live on App and that
//! `scroll_math` implicitly assumed. It is NOT word-boundary aware, so
//! it can disagree with the renderer's word-wrap by a row or two on
//! very long paragraphs. Aligning the two is a follow-up.
//!
//! Invalidation key: `(width, doc_rev, layout_generation)`. Any mismatch
//! triggers a rebuild on next `ensure_for` call.
//!
//! See bead mdx-ryv.

use crate::app::LayoutGeneration;
use ropey::Rope;

/// Minimum content width below which wrapping math falls back to a 1:1
/// mapping. Mirrors `layout_const::MIN_WRAP_AWARE_WIDTH`.
pub const MIN_WRAP_AWARE_WIDTH: usize = 40;

#[derive(Debug, Clone)]
pub struct LineLayoutCache {
    width: usize,
    doc_rev: u64,
    generation: LayoutGeneration,
    /// Visual row count per source line. `heights[i]` is the number of
    /// rendered rows source line `i` occupies at `self.width`. 1 for
    /// empty lines, `ceil(len / width)` otherwise.
    heights: Vec<u16>,
    /// Whether `heights` is valid for the current (width, doc_rev, gen).
    /// Starts false; flipped by `rebuild`.
    valid: bool,
}

impl LineLayoutCache {
    pub fn new() -> Self {
        Self {
            width: 0,
            doc_rev: 0,
            generation: 0,
            heights: Vec::new(),
            valid: false,
        }
    }

    /// Returns true when the cache is valid for (width, doc_rev, gen).
    pub fn is_valid_for(&self, width: usize, doc_rev: u64, gen: LayoutGeneration) -> bool {
        self.valid && self.width == width && self.doc_rev == doc_rev && self.generation == gen
    }

    /// Rebuild the cache if any of the keys changed. No-op otherwise.
    pub fn ensure_for(
        &mut self,
        width: usize,
        doc_rev: u64,
        gen: LayoutGeneration,
        rope: &Rope,
    ) {
        if self.is_valid_for(width, doc_rev, gen) {
            return;
        }
        self.rebuild(width, doc_rev, gen, rope);
    }

    fn rebuild(&mut self, width: usize, doc_rev: u64, gen: LayoutGeneration, rope: &Rope) {
        let line_count = rope.len_lines();
        self.heights.clear();
        self.heights.reserve(line_count);

        let effective_width = if width < MIN_WRAP_AWARE_WIDTH { 0 } else { width };

        for i in 0..line_count {
            let h = if effective_width == 0 {
                1u16
            } else {
                // `Rope::line` includes the trailing newline; exclude it
                // so the visual-row count doesn't tick over for every
                // line whose content happens to exactly fill the width.
                let mut len = rope.line(i).len_chars();
                if len > 0 {
                    let line = rope.line(i);
                    if line.char(len - 1) == '\n' {
                        len -= 1;
                    }
                }
                if len == 0 {
                    1
                } else {
                    let rows = (len + effective_width - 1) / effective_width;
                    rows.min(u16::MAX as usize) as u16
                }
            }
            .max(1);
            self.heights.push(h);
        }

        self.width = width;
        self.doc_rev = doc_rev;
        self.generation = gen;
        self.valid = true;
    }

    /// Visual height (rendered row count) of a single source line.
    /// Returns 1 when the cache is not populated or the index is out of
    /// range, so callers do not need a separate fallback.
    pub fn visual_height_of_line(&self, line: usize) -> u16 {
        if !self.valid {
            return 1;
        }
        self.heights.get(line).copied().unwrap_or(1)
    }

    /// Number of source lines between `start_line` (inclusive) and the
    /// target that sits approximately `visual_delta` visual rows away.
    ///
    /// `forward = true`  walks downward (increasing line index).
    /// `forward = false` walks upward.
    ///
    /// Always returns at least 1 when `visual_delta > 0` so the caller
    /// makes progress.
    pub fn advance_visual(
        &self,
        start_line: usize,
        visual_delta: usize,
        forward: bool,
    ) -> usize {
        if visual_delta == 0 {
            return 0;
        }
        if !self.valid || self.heights.is_empty() {
            // Graceful fallback: 1:1 mapping.
            return visual_delta;
        }

        let len = self.heights.len();
        let mut visual_count: usize = 0;
        let mut source_count: usize = 0;

        loop {
            if visual_count >= visual_delta {
                break;
            }
            let line_idx = if forward {
                start_line.saturating_add(source_count)
            } else {
                match start_line.checked_sub(source_count + 1) {
                    Some(idx) => idx,
                    None => break,
                }
            };
            if line_idx >= len {
                break;
            }
            visual_count = visual_count.saturating_add(self.heights[line_idx] as usize);
            source_count = source_count.saturating_add(1);
        }

        source_count.max(1)
    }

    /// Total visual rows for source lines in `[start, end)`. Exposed so
    /// the renderer (or a future scrollbar) can size things correctly
    /// without re-walking the rope.
    pub fn visual_rows_in_range(&self, start: usize, end: usize) -> usize {
        if !self.valid {
            return end.saturating_sub(start);
        }
        let end = end.min(self.heights.len());
        let start = start.min(end);
        self.heights[start..end]
            .iter()
            .map(|&h| h as usize)
            .sum()
    }
}

impl Default for LineLayoutCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    fn rope_from(lines: &[&str]) -> Rope {
        let mut s = String::new();
        for l in lines {
            s.push_str(l);
            s.push('\n');
        }
        Rope::from_str(&s)
    }

    #[test]
    fn rebuild_matches_simple_wrap() {
        let r = rope_from(&["short", &"a".repeat(100)]);
        let mut c = LineLayoutCache::new();
        c.ensure_for(50, 1, 1, &r);
        // "short" -> 1 row. "aaa..."(100 chars) -> ceil(100/50)=2 rows.
        assert_eq!(c.visual_height_of_line(0), 1);
        assert_eq!(c.visual_height_of_line(1), 2);
    }

    #[test]
    fn empty_line_still_takes_one_row() {
        let r = rope_from(&["", "x", ""]);
        let mut c = LineLayoutCache::new();
        c.ensure_for(80, 1, 1, &r);
        assert_eq!(c.visual_height_of_line(0), 1);
        assert_eq!(c.visual_height_of_line(2), 1);
    }

    #[test]
    fn narrow_width_falls_back_to_1to1() {
        let r = rope_from(&[&"x".repeat(100)]);
        let mut c = LineLayoutCache::new();
        // Below MIN_WRAP_AWARE_WIDTH, every line is 1 row.
        c.ensure_for(20, 1, 1, &r);
        assert_eq!(c.visual_height_of_line(0), 1);
    }

    #[test]
    fn invalidation_on_width_change() {
        let r = rope_from(&[&"a".repeat(120)]);
        let mut c = LineLayoutCache::new();
        c.ensure_for(80, 1, 1, &r);
        assert_eq!(c.visual_height_of_line(0), 2); // ceil(120/80)
        c.ensure_for(60, 1, 1, &r);
        assert_eq!(c.visual_height_of_line(0), 2); // ceil(120/60)=2
        c.ensure_for(40, 1, 1, &r);
        assert_eq!(c.visual_height_of_line(0), 3); // ceil(120/40)=3
    }

    #[test]
    fn advance_visual_respects_wrap() {
        // Lines: 10 empty lines (each 1 row), then a 160-char line (at
        // width 80 that is 2 rows).
        let mut lines: Vec<String> = vec![String::new(); 10];
        lines.push("a".repeat(160));
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let r = rope_from(&refs);
        let mut c = LineLayoutCache::new();
        c.ensure_for(80, 1, 1, &r);

        // 5 visual rows forward from line 0 = 5 source lines (all empty).
        assert_eq!(c.advance_visual(0, 5, true), 5);
        // 2 visual rows forward from line 10 (the long one) = 1 source
        // line (it takes 2 rows).
        assert_eq!(c.advance_visual(10, 2, true), 1);
        // 11 visual rows forward from line 0 = 10 empties + 1 wrapped
        // row = 11 source-line steps... but the long line takes 2 rows,
        // so after consuming 1 empty*10 + long = 12 visual we stop.
        let n = c.advance_visual(0, 11, true);
        assert!(n >= 10);
    }

    #[test]
    fn advance_visual_backward_stops_at_zero() {
        let r = rope_from(&["a", "b", "c", "d"]);
        let mut c = LineLayoutCache::new();
        c.ensure_for(80, 1, 1, &r);
        // From line 2, going back 5 visual rows — stops at line 0 (2 steps).
        assert_eq!(c.advance_visual(2, 5, false), 2);
    }

    #[test]
    fn advance_visual_zero_is_zero() {
        let r = rope_from(&["a", "b"]);
        let mut c = LineLayoutCache::new();
        c.ensure_for(80, 1, 1, &r);
        assert_eq!(c.advance_visual(0, 0, true), 0);
    }

    #[test]
    fn visual_rows_in_range_sums_heights() {
        let r = rope_from(&["", &"a".repeat(100), ""]);
        let mut c = LineLayoutCache::new();
        c.ensure_for(50, 1, 1, &r);
        // heights: [1, 2, 1] -> range 0..3 sums to 4.
        assert_eq!(c.visual_rows_in_range(0, 3), 4);
        assert_eq!(c.visual_rows_in_range(1, 2), 2);
        assert_eq!(c.visual_rows_in_range(0, 0), 0);
    }

    #[test]
    fn is_valid_for_tracks_all_keys() {
        let r = rope_from(&["x"]);
        let mut c = LineLayoutCache::new();
        c.ensure_for(80, 1, 5, &r);
        assert!(c.is_valid_for(80, 1, 5));
        assert!(!c.is_valid_for(79, 1, 5));
        assert!(!c.is_valid_for(80, 2, 5));
        assert!(!c.is_valid_for(80, 1, 6));
    }

    #[test]
    fn advance_when_unpopulated_falls_back_to_1to1() {
        let c = LineLayoutCache::new();
        assert_eq!(c.advance_visual(0, 10, true), 10);
        assert_eq!(c.visual_height_of_line(0), 1);
    }
}
