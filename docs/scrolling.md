# Scrolling, Paging, and Resize

This document describes the user-facing scrolling model in `mdx-tui` and
the invariants callers should rely on. Implementation details live in
`mdx-tui/src/scroll_math.rs` (pure math) and `mdx-tui/src/app.rs`
(state + layout).

## Keybindings

| Key                   | Action                                         |
|-----------------------|------------------------------------------------|
| `j` / `Down`          | Cursor down 1 source line                      |
| `k` / `Up`            | Cursor up 1 source line                        |
| `Ctrl-D`              | Half-page down (visual-line aware)             |
| `Ctrl-U`              | Half-page up (visual-line aware)               |
| `PgDn` / `Space`      | Full page down minus `page_overlap_rows`       |
| `PgUp`                | Full page up minus `page_overlap_rows`         |
| `Home` / `gg`         | Jump to first rendered line                    |
| `End` / `G`           | Jump to last line                              |
| Mouse wheel           | Scroll the **hovered** pane (not focused)      |

## Model

### A "line" has two meanings

- **Source line**: a line in the rope (`\n`-delimited). Cursor and scroll
  are stored as source lines. Line count does not depend on width.
- **Visual row**: a rendered row on screen. A single long source line may
  occupy several visual rows after wrapping.

Paging tries to approximate visual-row distances for UX but still stores
source lines underneath. Migrating to `(source_line, wrap_row)` is tracked
in bead `mdx-irv`.

### Page size

The page is the **focused pane's `PaneViewport.visible_height`** — not
the terminal height. A 40-row terminal split horizontally gives each pane
a page of roughly 18 rows (40/2, minus borders and status).

`PgDn` advances by `page_step = visible_height - page_overlap_rows`, with
`page_overlap_rows` defaulting to 2 (the same behavior as `less` and
`vim`). The overlap is clamped to `visible_height / 2` and the step is
never below 1.

### Mouse wheel vs keyboard

Both wheel and `Ctrl-D`/`Ctrl-U` use the same wrapping heuristic
(`App::visual_delta_to_source_lines`), so a wheel tick covers roughly the
same visual distance as a keystroke on the same content. A wheel tick
is 3 visual rows.

**The wheel scrolls the hovered pane**, not the focused one. Focus does
not change unless you click.

### Cursor vs viewport

After any scroll (wheel or key), the cursor is snapped into the visible
window. This keeps `j`/`k` after a wheel scroll from jerking the viewport
back.

### Wrapping and width

- Wrapping is visual-only. `doc.line_count()` is stable across resizes.
- The same `scroll_line` means different visual positions at different
  widths because wrapping changes. This is a known limitation; see
  `mdx-irv` for the fix.
- Below `layout_const::MIN_WRAP_AWARE_WIDTH` (40 columns), scroll math
  falls back to a 1:1 visual-to-source mapping.

### Resize behavior

On `Event::Resize(w, h)`, the app:

1. Refreshes the layout context with the new dimensions.
2. Clamps every pane's `scroll_line` and `cursor_line` to
   `[front_matter_end + 1, line_count - 1]` and to a position that keeps
   the viewport full (when the document is at least `visible_height`
   tall).
3. Clamps `toc_scroll` and `toc_dialog_scroll` to the heading count.
4. Clears the terminal so stale cells from the prior geometry don't
   leak through.

A resize smaller than `MIN_TERMINAL_COLS` × `MIN_TERMINAL_ROWS` does not
crash — `saturating_sub` and the wrap-width guards tolerate
zero-dimension panes. (Showing an explicit "terminal too small" panel is
tracked separately.)

### Split panes

Each pane has its own `scroll_line`, `cursor_line`, and viewport. After
dragging a split boundary:

1. `PaneManager::update_split_ratio` clamps the ratio to `[0.1, 0.9]`.
2. `App::enforce_rendered_bounds` re-clamps every pane so the narrowed
   sibling doesn't end up scrolled past its new end-of-content.

### Front matter

When `render.skip_front_matter = true` and the document begins with a
front matter block, `rendered_content_bounds().0` is set to
`front_matter.end_line + 1`. Home/`gg`/auto-scroll all honor this lower
bound.

### Reload

`reload_document` re-parses front matter, recomputes bounds, and
re-clamps. If the document shrank, cursor and scroll are pulled back to
the new end-of-content.

## Invariants

Code that mutates `scroll_line` or `cursor_line` should call, or
delegate to:

- `scroll_math::clamp_scroll` for the viewport top,
- `scroll_math::clamp_cursor` for the cursor,
- `scroll_math::snap_cursor_into_view` when scrolling without moving the
  cursor (wheel path),
- `scroll_math::auto_scroll_to_cursor` when moving the cursor and
  following it (keyboard path).

`App::enforce_rendered_bounds` is the single clamp applied to every pane
after any geometry or document change. Call it from any new site that
changes:

- terminal size (already wired in `on_resize`),
- split ratio (already wired in the drag handler),
- config that affects content width (already wired in
  `apply_options` / `save_options`),
- document length (already wired in `refresh_front_matter_info`, which
  is called from `reload_document`).

## Configuration

```toml
[render]
page_overlap_rows = 2     # rows preserved between pages on PgUp/PgDn
show_scrollbar    = true  # scrollbar column steals 1 col from content_width
skip_front_matter = true  # if true, cursor/scroll cannot enter front matter
```
