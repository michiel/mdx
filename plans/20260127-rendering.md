## Context
- Split panes share a single viewport height/width from `mdx-tui/src/lib.rs:52-59`, even though `ui::render_markdown` calculates each pane's content size differently (`mdx-tui/src/ui.rs:240-264`).
- Navigation helpers (`App::auto_scroll`, `scroll_half_page_*`, `calculate_source_lines_for_visual_lines`) receive the global numbers, so cursors can escape their local viewport as shown in the attached screenshot (cursor renders outside the lower pane).

## Findings
1. *Viewport mismatch*: `run_loop` derives `viewport_height/width` from the terminal once per frame and feeds that into every input handler. Split panes may have significantly smaller heights, causing `auto_scroll` to never trigger when the cursor moves in a smaller pane, so the cursor is painted outside the visible area (`mdx-tui/src/app.rs:432-461`).
2. *Layout is authoritative*: `compute_layout` and the renderer already know each pane's `Rect`. The rendering code uses `content_area.height - 2`/`width - 2` for borders/breadcrumbs (`mdx-tui/src/ui.rs:240-315`). The inconsistency leads to incorrect scrollbar calculations and mouse hit-testing drift unless the same values are shared.
3. *Shared metrics missing*: `App` and `input` lack access to per-pane viewport dimensions, while `ui::draw` recomputes layout each frame. Input, navigation, and scrolling logic should operate on the same metrics as rendering to stay in sync.

## Structural Recommendations
1. **Expose per-pane viewport metrics** from the layout computation to the rest of the system. Extend `PaneManager::compute_layout` or the `LayoutInfo` structure used by input handling to store `content_width`/`content_height` per `PaneId`, accounting for borders/breadcrumbs/scrollbar space.
2. **Share the metrics between renderer and navigator**. After `ui::draw` (or just before it) calculates the layout, persist the `PaneRect` map (with derived content sizes) in a new shared context accessible to `input::handle_input` and `App`. A global `ViewportRegistry` (maybe stored in `App`) can cache the Map each frame.
3. **Use the per-pane viewport in scrolling logic**. Update `App::auto_scroll`, `scroll_half_page_*`, `calculate_source_lines_for_visual_lines`, and any cursor-jumping helpers to look up the focused pane’s actual height/width instead of the terminal-level numbers. Likewise, `handle_mouse` should reuse the layout context already produced.

### Data Flow Overview
- `ui::draw` already calls `app.panes.compute_layout`/`compute_layout_info` to produce pane rectangles before rendering each pane. Inject here a second pass that derives `PaneViewport { content_height, content_width }` by subtracting the breadcrumb line, borders, and optional scrollbar from each `Rect`. Store this map inside `App` (or a new `LayoutContext` held by `App`), replacing any stale data each frame.
- `handle_input` should read the focused pane’s `PaneViewport` from the shared context instead of using the terminal-derived `viewport_height`/`width`. When it calls `App::auto_scroll` or passing viewport values to other helpers, forward the per-pane metrics to keep cursor positioning and page navigation aligned with what the renderer actually displays.
- `App::auto_scroll` and related helpers should accept those per-pane dimensions and be updated to favor them whenever available; fall back to the terminal-level size only when the context is missing (e.g., during unit tests).

## Implementation Steps
1. Create a struct (e.g., `PaneViewport { content_height: usize, content_width: usize }`) and store it alongside `PaneRect` in a new `PaneLayoutContext` owned by `App` or `PaneManager`. When `ui::draw` computes layout for rendering, also populate this context with breadcrumb/border adjustments.
2. Thread the focused pane’s viewport into input helpers: `handle_input` should fetch `app.layout_context.get_viewport(app.panes.focused)` (or a default fallback) and use it when calling `auto_scroll`, `scroll_half_page_*`, and `calculate_source_lines_for_visual_lines`. Also, propagate it to any other navigation helpers that need viewport size (e.g., page up/down, search result navigation).
3. Update `App::auto_scroll` and related helpers to accept `Option<PaneViewport>` or `usize width/height` and clamp scroll/cursor calculations accordingly. Adjust `auto_scroll` so that it considers the actual content height minus breadcrumb+border lines from rendering.
4. Validate by writing new assertions or unit tests around `App::auto_scroll` (if feasible) and by manually reproducing the split-pane scenario with the cursor staying in view and scrollbars updating correctly. Update `plans/20260127-rendering.md` with any follow-up notes from implementation/testing.

## Visual Pipe Command
- Added `Mode::VisualCommand`, `CommandOutput`, and a visual command buffer so `|` can prompt for a shell command while a visual selection remains active (`mdx-tui/src/app.rs`, `mdx-tui/src/input.rs`).
- Visual command input is rendered in the status bar (mirroring search prompts) to show the typed command and helper tips (`mdx-tui/src/ui.rs`).
- Executing the command pipes the selection into the shell (`sh -c`/`cmd /C`), captures stdout/stderr, and displays the result in a full-screen overlay that waits for any key before returning to the main view (`mdx-tui/src/app.rs`, `mdx-tui/src/ui.rs`).
- Verified via `cargo test` and manual runs (`| sort | uniq -c | wc -l` in visual mode) to ensure the overlay and piping logic behave as expected.
- The command output overlay now clears the entire frame before rendering so the terminal is blanked before echoing stdout/stderr (`mdx-tui/src/ui.rs`).

## Implementation progress
- Added `LayoutContext` and `PaneViewport`, wired them into `App`, and update them every frame from `ui::draw`.
- Input now derives `pane_height`/`pane_width` from the focused pane and feeds those values into scrolling, paging, and search navigation helpers.
- Implemented the visual pipe command (`|`), command buffer, and full-screen result overlay covering the terminal until any key clears it.

## Verification
- Manual verification: Run `mdx` with split panes, move the cursor around the lower pane, and confirm auto-scroll keeps the cursor visible (no off-screen drawing). Scrollbars should reflect the focused pane size.
- Optional: add regression tests for `PaneManager::compute_layout` plus new helper methods if logic moves there.
