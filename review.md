# Review: Paging, Scrolling, and Window Resizing

**Scope:** `mdx-tui/src/` (primary) and interaction with `mdx-core/src/doc.rs`.
**Date:** 2026-04-30
**Reviewer:** Detailed static review — no code changes, findings and recommendations only.

---

## 1. Executive Summary

The TUI's scrolling/paging/resize subsystem works for the common case, but has a number of correctness bugs, UX sharp edges, and structural weaknesses that will show up as visible glitches under split panes, wrapped long lines, or rapid terminal resizes. The single biggest structural problem is that **`Event::Resize` is silently discarded** (`mdx-tui/src/lib.rs:115`), so every resize-related correction is piggy-backed onto the next paint tick, which runs at the poll interval (100 ms). The second biggest problem is that the app tracks **source (document) lines for scrolling but renders visual (wrapped) lines**, and the two are only reconciled heuristically. Keyboard half-page scroll tries to account for wrapping; mouse wheel and full page do not. This produces inconsistent distances for what the user perceives as "one page."

Severity legend: **[H]** High (correctness / data-visible), **[M]** Medium (UX / predictability), **[L]** Low (polish, future-proofing).

---

## 2. Findings

### 2.1 Resize events are silently ignored — [H]

**Location:** `mdx-tui/src/lib.rs:115` — the `_ =>` arm in the event match drops `Event::Resize(..)`.

**Symptom:** After a resize, nothing updates until either (a) the next 100 ms poll tick + draw completes, or (b) a user key/mouse event happens. Scroll clamping (`enforce_rendered_bounds` / `validate_scroll_after_resize`) is only invoked at init time and after explicit split operations (`input.rs:500, 513`). A user who resizes the window smaller while scrolled near the bottom will see empty space under the content until they press a key.

**Related:** `layout_context` is refreshed every draw via `update_layout_context` (`app.rs:332`), but the *scroll offset itself* is never re-clamped on resize — only on splits. The first line of defense (`enforce_rendered_bounds`) does not run on natural resize.

**Recommendations:**
- Add an explicit `Event::Resize(w, h) => { app.on_resize(w, h) }` arm. The handler should:
  1. Invalidate any cached viewport-width-dependent state (see §2.3).
  2. Re-clamp every pane's `scroll_line` and `cursor_line` via `enforce_rendered_bounds`.
  3. Re-clamp `toc_scroll`, `toc_dialog_scroll`, and any selection anchors.
  4. Force an immediate redraw on the next iteration (no need to wait for the 100 ms poll).
- Consider dropping the poll interval to ~16 ms only when an animation/loading state is active, and going back to longer intervals otherwise; 100 ms is visibly laggy for resize follow-up.

### 2.2 Mouse wheel scroll ignores line wrapping — [H]

**Location:** `mdx-tui/src/input.rs:1891-1945` (`handle_scroll`).

**Symptom:** Mouse wheel moves `scroll_line` in *document* lines. Keyboard half-page (`app.rs:631-644`) uses `calculate_source_lines_for_visual_lines` (`app.rs:564-628`) to approximate visual lines → source lines. Result: on a document full of long wrapped paragraphs, three clicks of the wheel might scroll an entire screen, while PgDn moves a few source lines. This is a UX inconsistency users will notice immediately on prose-heavy markdown.

**Recommendations:**
- Route mouse wheel through the same "advance by N visual rows" path that half-page uses. Define one function, `advance_scroll(pane, delta_visual_rows)`, and have both call it.
- Mouse wheel magnitude should come from config (default 3 rows; allow "smooth" vs "line-by-line" modes).
- If the terminal reports pixel deltas (kitty, iTerm2), honor them for trackpad inertia.

### 2.3 Wrapped-line height is recomputed every call, not cached — [M]

**Location:** `app.rs:606-614` estimates `((line_len + content_width - 1) / content_width).max(1)` on every half-page press. The real wrapping happens in `ui.rs` (~700-800) and uses a more sophisticated algorithm (word boundaries, tabs, CJK, etc. depending on the implementation). The two can disagree.

**Symptoms / Risks:**
- Half-page "lands in a different place than what was visible" when the rendering algorithm word-wraps but the scroll estimator character-wraps.
- O(lines) work on every wheel tick. For a 10k-line document this is fine, but for a 200k-line log it becomes perceptible — and half-page is called twice per PgDn (scroll + auto-scroll).

**Recommendations:**
- Introduce a `LineLayoutCache` keyed by `(content_width, doc_revision)` that stores `visual_height_per_source_line: Vec<u16>` plus cumulative prefix sums. Query is O(log n) via binary search ("which source line starts at visual row V?"). Invalidate on resize (§2.1) and on doc reload.
- Use the *same* wrapping function for display and for scroll math. The easiest path: make rendering produce a `Vec<VisualLine>` and make scroll math index into that. Don't maintain two wrapping algorithms.
- If full caching is too invasive, at minimum extract one shared `visual_height_of_line(line, width)` function used by both sites.

### 2.4 Page size subtracts a constant for "borders + status bar" — [M]

**Location:** `lib.rs:57` uses `term_size.height.saturating_sub(3)`. This is baked in regardless of:
- Whether the status bar is visible (could be toggled).
- Whether the pane actually has top+bottom borders (split layouts may share a border).
- Whether a modal dialog / options dialog / TOC dialog is open — in those cases the "visible document" is smaller than `viewport_height`.
- Whether a breadcrumb row is shown (the per-pane calculation subtracts another 1 at `app.rs:111-126`, but the top-level value in `lib.rs` does not).

**Symptom:** Paging near the top/bottom while a dialog is open overshoots by a few lines; a `PgDn` followed by `PgUp` doesn't return to the original position.

**Recommendations:**
- Stop computing page size in `lib.rs`. Pass only `term_size` into `handle_input`; let each key handler ask the *focused pane's* `PaneViewport.visible_height` for page math. `focused_viewport()` already exists (`app.rs:340`).
- Define and document: "PgDn moves the cursor by exactly `visible_height − overlap_context` visual rows."
- Preserve 1-2 lines of overlap between pages (standard pager behavior — `less`, `vim`). Add a config knob `pager.overlap_rows` (default 2).

### 2.5 Cursor detaches from viewport during mouse scroll — [M]

**Location:** `input.rs:1935-1936` — mouse scroll moves `scroll_line` but not `cursor_line`.

**Why it's a problem:** When the user later presses `j`, `k`, or enters a selection, the cursor is reported *off-screen* and `auto_scroll` jerks the viewport back. The user loses their place.

**Design choice:** Some editors (Vim) keep the cursor on-screen when scrolling (`Ctrl-D` moves both). Others (less, most IDEs) decouple. The app currently decouples on mouse and couples on keyboard — worst of both worlds.

**Recommendations:**
- Pick one model per input source and document it.
  - Option A (Vim-like, recommended for a reader app): Any scroll keeps the cursor within the viewport; if the cursor would go off-screen, snap it to the nearest visible row.
  - Option B (IDE-like): All scrolls decouple; `j`/`k` only move cursor and auto-scroll *only* if cursor would leave the viewport.
- Either way, be consistent across keyboard and mouse wheel.

### 2.6 No minimum viable viewport check — [M]

**Location:** Everywhere that uses `saturating_sub` on terminal dims.

**Symptom:** When the terminal is resized to 1-2 rows, or narrower than the TOC sidebar, the `visible_height` and `content_width` saturate to 0. `ui.rs` can then attempt to render into a zero-sized rect. Observed risk: divide-by-zero in `wrapped_lines = (line_len + content_width - 1) / content_width` at `app.rs:606` when `content_width == 0`.

**Recommendations:**
- At the top of the draw loop, if `term_size.height < MIN_ROWS || term_size.width < MIN_COLS` (say 10×20), render a single "Terminal too small" message and skip pane layout entirely.
- In `calculate_source_lines_for_visual_lines`, guard `content_width == 0` and return early.
- Clamp `content_width` to `>= 1` in `PaneViewport::from_rect` (`app.rs:111-126`).

### 2.7 Split-pane resize doesn't re-validate sibling scroll — [M]

**Location:** Mouse drag on split border (`input.rs:1826-1857`) updates ratio (`panes.rs:404`) but does not call `enforce_rendered_bounds`. If the user narrows a pane enough that a previously-one-visual-line becomes 3 visual lines, the pane's `scroll_line` (source line) may now render past the bottom of the viewport.

**Recommendations:**
- After every split-ratio change, keypress-based split creation, or TOC toggle (anything that changes pane width), call `enforce_rendered_bounds`.
- Better: hook `enforce_rendered_bounds` into `update_layout_context` itself, so any layout change revalidates. Guard against infinite recursion / redundant work with a "dirty" flag or by comparing the new viewports to the previous.

### 2.8 Scroll offset stored as source-line is lossy across width changes — [M]

**Symptom / design smell:** The same `scroll_line = 100` means different visual positions depending on terminal width. If a user has scrolled to an interesting paragraph at wide width, then narrows the terminal, the top of the pane will show a *different* point in the paragraph — because "line 100" wraps differently now.

**Recommendations:**
- Track scroll as `(source_line, intra_line_visual_offset)`. When rendering, skip the first `intra_line_visual_offset` wrapped rows of `source_line`.
- On resize, snap `intra_line_visual_offset` to a valid value (< new visual height of that source line).
- This is the same model `less -S` and modern editors use. It's a larger change but eliminates a whole class of "where did I end up?" bugs.

### 2.9 Page/half-page math uses `viewport_height` but keyboard PgDn uses pane height — [L]

**Location:** `lib.rs:57` computes a single `viewport_height` for the whole app; `input.rs:1429-1454` uses `pane_height`; `app.rs:631` uses yet another derived `viewport_height / 2`. Three slightly different notions of "page." After splitting the window horizontally, a PgDn moves cursor by *full-terminal* rows while half-page moves by *half pane*.

**Recommendations:** Converge on `PaneViewport.visible_height` as the single source of truth for paging. Drop `viewport_height` at the call site in `lib.rs`.

### 2.10 `validate_scroll_after_resize` and `enforce_rendered_bounds` are similar but not the same — [L]

**Location:** `app.rs:290-330`. There appear to be two functions doing related clamping. Behavior drift across codepaths (splits vs init vs resize) is near-certain over time.

**Recommendations:** Merge into one `clamp_all_pane_scroll_state(&mut self)`. Have every caller invoke it (init, resize, split change, doc reload, config change, TOC toggle).

### 2.11 Front-matter skip is re-checked on every bounds call — [L]

**Location:** `app.rs:271-288`. Pure O(1) so perf isn't the issue; correctness is. If a user edits front matter live (via external editor / watcher), `self.front_matter` may need refreshing; ensure the watcher path (`lib.rs:124`) recomputes it.

**Recommendations:** Recompute `front_matter` on doc reload and clamp `scroll_line >= front_matter.end_line + 1` after reload, not just after resize.

### 2.12 TOC and main pane scroll are independent by design — [L]

This is likely correct and desired, but worth verifying: when the user clicks a TOC item, `app.rs:795` jumps main scroll. There is no reverse — scrolling the main pane does **not** update the TOC's highlight / scroll to keep the current heading visible. Most IDEs do this.

**Recommendations:**
- Add "auto-track current heading in TOC" behavior: after any scroll in the main pane, locate the last heading ≤ `scroll_line` and ensure it's visible in the TOC (and optionally highlighted).
- Guard against feedback loops (TOC click → main scroll → TOC re-scroll on self) with a `suppress_tracking` flag for one tick after a TOC-originated jump.

### 2.13 No smooth-scroll / no keyboard-repeat rate limiting — [L]

Holding `j` on a fast terminal can generate events faster than the 100 ms poll can drain. Events accumulate; the screen appears to scroll *after* the key is released.

**Recommendations:**
- In the poll loop, drain all pending events up to a small budget (e.g. 32) before redrawing.
- Optionally coalesce repeated scroll events of the same kind.

### 2.14 Missing end-of-document affordance — [L]

If the user scrolls past content boundaries via mouse wheel, they see empty rows beneath the last line. A common pager convention is to either (a) refuse to scroll past `last_line − visible_height + 1`, or (b) show a `~` or `(END)` marker for visual lines past the document.

**Recommendations:** Pick (a) unless you want vi-style `~` markers. Mouse-wheel path currently lacks the clamp that `enforce_rendered_bounds` provides on other paths (§2.1).

### 2.15 Scrollbar rendering is width-stealing — [L]

`app.rs:111-126` reduces `content_width` by 1 if `show_scrollbar` is enabled. This is a silent re-wrap trigger — toggling the scrollbar mid-session changes what wraps where and thus the meaning of `scroll_line`.

**Recommendations:** After toggling the scrollbar config, run the same resize pipeline (invalidate wrap cache, re-clamp scroll). If you adopt §2.8 (intra-line offsets), this becomes self-healing.

---

## 3. Structural / Architectural Opportunities

These aren't bugs; they're debts worth paying.

### 3.1 One authoritative "visual line" type

Introduce

```rust
struct VisualPos { source_line: usize, wrap_row: u16 }
```

and use it everywhere scroll/cursor position is stored. This forces the codebase to confront the source-vs-visual distinction at the type level instead of silently conflating them in `usize`. It also makes §2.8 trivial.

### 3.2 Central event dispatcher

`lib.rs` currently does the event match and forwards to `input::handle_input` / `handle_mouse`. Resize, focus, paste, and any future events (terminal capability, OSC replies) have to thread through. A small `EventDispatcher` struct that owns `app` and implements `on_key`, `on_mouse`, `on_resize`, `on_paste`, `on_tick` gives you a single place to add debouncing, logging, and test injection.

### 3.3 Testability of scroll math

None of the scroll logic is trivially unit-testable today because it's methods on `App` which owns the whole world. Extract a pure `ScrollMath { line_count, visible_height, wrap_heights: Vec<u16> }` with methods `page_down`, `half_page_down`, `line_down`, `clamp`. Then a table-driven test can cover:
- PgDn at top, middle, end.
- PgDn with wrapping.
- PgDn with front matter skipped.
- PgDn while at `line_count < visible_height`.
- PgUp symmetry.

These tests are missing today and each of the bugs above would have been caught.

### 3.4 Deterministic "jump and scroll" primitive

Many operations are sequences of (move cursor) → (adjust scroll). Each implemented inline, each with slightly different clamping. Replace with one `goto(pane, target_visual_pos, scroll_policy)` where `ScrollPolicy ∈ { Center, TopQuarter, NearestEdge, KeepOffset }`. Maps to vim's `zz`, `zt`, `zb`.

### 3.5 Layout dirty-flag

`update_layout_context` runs every frame unconditionally. It's cheap but it papers over the fact that nothing else is invalidated on resize. A `LayoutGeneration: u64` incremented on any geometry change, with caches keyed by it, would make invalidation explicit.

### 3.6 Replace magic numbers

- `lib.rs:57`: `saturating_sub(3)` / `saturating_sub(2)` — name these constants.
- `app.rs:305`: fallback viewport height of 20 — a "reasonable default" but silently wrong.
- `panes.rs:404`: split ratio clamp `[0.1, 0.9]` — expose as config.
- `app.rs:609`: the `viewport_width - 10` fallback is surprising; document or remove.

### 3.7 Consider a "scroll context" struct passed through input

Right now input handlers take `(app, key, viewport_height, viewport_width)`. That last pair is insufficient under splits. Replace with a `ScrollContext { focused_pane, viewport, wrap_cache }` computed once at the top of `run_loop`.

---

## 4. Smaller Opportunities / Nice-to-Haves

- **Search result navigation:** document whether `n`/`N` scroll to keep match centered. If not, apply the §3.4 `Center` policy.
- **Sticky headings:** when scrolled inside a long section, pin the current `#`/`##` heading at the top row. High-value UX win.
- **Relative line numbers:** toggle for vim users; cheap to implement and orthogonal.
- **Horizontal scroll for non-wrap mode:** currently wrap is assumed; a `:set nowrap` equivalent with `h`/`l` horizontal scroll would help code blocks.
- **"Jump back" stack:** `Ctrl-O`/`Ctrl-I` history of scroll positions after jumps (TOC click, search, heading jump). Trivial with a `VecDeque<(PaneId, VisualPos)>`.
- **Scroll inertia / animation:** optional easing over 1-2 frames on PgDn/PgUp makes large jumps less disorienting. Gated behind a config flag.
- **Pane-aware mouse wheel:** verify the wheel event's click coordinate selects the hovered pane, not just the focused one. A common expectation with split UIs.
- **Debounced watcher-reload does not re-clamp scroll:** see §2.11.
- **Telemetry/logging hook:** a `#[cfg(feature = "trace")]` log line on every scroll adjustment makes diagnosing user reports of "it jumped" straightforward.
- **Document the model:** a short `docs/scrolling.md` describing (a) what a "page" means, (b) how wrapping affects scroll, (c) the cursor-follow-viewport policy, (d) keybindings. Future contributors will thank you, and ambiguity in the docs often surfaces ambiguity in the code.

---

## 5. Prioritized Remediation Plan

1. **Immediate (correctness):** §2.1 (handle `Event::Resize`), §2.6 (min viewport guards), §2.2 (unify wheel with keyboard scroll math), §2.11 (re-clamp after reload).
2. **Short term (predictability):** §2.4 (single definition of page size), §2.5 (pick one cursor-follow policy), §2.7 (re-clamp on split-ratio change), §2.10 (merge the two clamp functions).
3. **Medium term (architecture):** §3.1 (`VisualPos` type), §3.3 (extract `ScrollMath`, add tests), §2.3 (cache visual-line heights), §2.8 (store intra-line offset).
4. **Longer term (UX polish):** §3.4 (`goto` with `ScrollPolicy`), §2.12 (TOC tracking), sticky headings, jump-back stack.

---

## 6. Suggested Test Matrix

A resize/scroll test pass should exercise at minimum:

| Scenario | Expected |
|---|---|
| Resize terminal smaller while scrolled near bottom | No empty rows; scroll re-clamps immediately |
| Resize to 1×1 | "Terminal too small" message; no crash |
| PgDn × 5, PgUp × 5 in wrapped prose | Cursor returns to starting line |
| Mouse wheel through a document with mixed short/long lines | Each tick advances the same visual distance |
| Split vertically, PgDn in left pane | Moves exactly one pane-height in the left pane only |
| Drag split border narrower | Right pane's scroll doesn't leave content off-bottom |
| Reload file that's half the previous length | Cursor/scroll clamp, no panic |
| Toggle scrollbar | Wrap recomputes; visible top line stays stable |
| TOC click on heading near bottom | Heading lands at top of pane, not center |
| Hold `j` for 2 seconds | Smooth scroll; no catch-up burst on release |
| Document with front matter; press Home | Jumps to first rendered line, not line 0 |

---

## 7. Summary

The subsystem is functional but leaks its internal model (source-line scrolling, no resize handler, two clamp paths, two notions of page) into user-visible inconsistencies. The highest-leverage fix is adopting a single *visual-line* representation backed by a layout cache, and routing every scroll origin — keyboard, mouse, TOC, search, reload, resize — through one well-tested primitive. That single change would subsume roughly half the findings above.
