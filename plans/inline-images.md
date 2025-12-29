# Inline Image Rendering Plan

## Goal

Add optional inline image rendering to the TUI so Markdown images (`![alt](path)` and `![alt](url)`) can appear in the content area. The feature must preserve fast scrolling, remain robust on terminals without image support, and degrade gracefully to text placeholders.

## Non-goals (initial)

- Full HTML or rich media rendering.
- Animated images (GIF/APNG) beyond first frame.
- Arbitrary CSS sizing; start with simple width/height handling.
- Network fetching by default (keep off unless explicitly enabled).

## Constraints and assumptions

- Terminals vary: some support Kitty graphics protocol, some support iTerm2, some support Sixel, many support none.
- `ratatui` text layout remains the primary pipeline; images should be injected as layout blocks with stable line heights.
- Default behaviour must be safe and offline (no network by default).

## Architecture overview

1. **Markdown parse** extracts image nodes with alt text, URL/path, and optional title.
2. **Image registry** resolves and caches image metadata and pixel buffers.
3. **Layout planner** maps images to rectangular blocks in the render tree with a known terminal cell size.
4. **Renderer** emits terminal-specific image escape sequences and reserves corresponding blank lines in the text buffer.
5. **Fallback** renders a textual placeholder when images are disabled or unsupported.

## Terminal support strategy

Support multiple backends behind a trait, chosen at runtime:

- Kitty graphics protocol (best quality, common in modern terminals).
- iTerm2 inline images (macOS).
- Sixel (legacy but still useful in some terminals).
- None (text placeholder).

Detection order:

1. Respect explicit config override.
2. Inspect environment variables where applicable (e.g. `TERM`, `KITTY_WINDOW_ID`, `ITERM_SESSION_ID`).
3. Probe support via a small capability test when safe.

## Configuration

Add config options to `mdx.yaml`:

```yaml
images:
  enabled: false
  backend: auto        # auto|kitty|iterm2|sixel|none
  max_width: 60        # in terminal cells
  max_height: 20       # in terminal cells
  allow_remote: false
  cache_dir: "~/.cache/mdx/images"
```

Defaults: `enabled: false`, `backend: auto`, `allow_remote: false`.

## Data model changes

In `mdx-core`:

- Extend the Markdown parse output to include image nodes with:
  - `src: String`
  - `alt: String`
  - `title: Option<String>`
  - `source_line: usize`

In `mdx-tui`:

- Add an image layout node:
  - terminal cell width/height
  - reference to cached image buffer
  - alt text for fallback

## Rendering approach

### 1) Layout planning

- When parsing Markdown for display, convert image nodes to layout blocks.
- Each image block reserves a fixed number of rows in the text layout.
- Use a size policy:
  - If Markdown includes explicit dimensions, respect within limits.
  - Otherwise, fit to `max_width` and preserve aspect ratio.
  - Clamp to `max_height` if needed.

### 2) Image decoding and scaling

- Use `image` crate to decode PNG/JPEG/WebP (initially).
- Pre-scale to a pixel size that matches the cell grid.
- Store cached pre-scaled buffers keyed by:
  - image content hash
  - terminal cell size (width/height in pixels)
  - scale policy

### 3) Terminal emission

- During frame draw, emit image escape sequences before or after the text render pass.
- Each image block is anchored to a cursor position; record its top-left cell.
- For unsupported terminals, render placeholder:
  - `[image: alt text]` or `[image]` if alt is empty.

## Caching strategy

- Use an on-disk cache (optional) for decoded image buffers or resized PNGs.
- In-memory LRU cache for the active session keyed by hash and size.
- Ensure cache eviction avoids large memory spikes.

## File and URL handling

- Local paths resolve relative to the Markdown file location.
- Remote URLs:
  - Disabled by default.
  - If enabled, download with size limits and timeouts.
  - Store in cache with hash of URL and headers.

## Error handling

- Decode failures fall back to placeholder.
- Oversized images are scaled down or replaced by placeholder if decode fails.
- Log failures with enough context for debugging, but avoid noisy output.

## Input and UX

- Add a toggle key for images (e.g. `I`) to enable/disable at runtime.
- Display a status indicator when images are active.
- When images are disabled, ensure layout remains stable (no jumps).

## Testing plan

Unit tests (core):

- Image node extraction from Markdown (alt/title/source line).
- Path resolution relative to document.

Integration tests (tui):

- Layout planner produces stable row counts for images.
- Fallback placeholder rendering in `backend: none`.

Manual tests:

- Kitty terminal with PNG/JPEG/WebP.
- iTerm2 on macOS with PNG/JPEG.
- Terminal without image support should show placeholders.

## Milestones

1. **Parsing and data model**
   - Extract image nodes with source lines and metadata.
2. **Layout and placeholders**
   - Render text placeholders with reserved space.
3. **Kitty backend**
   - Implement basic Kitty image rendering with scaling.
4. **iTerm2 backend**
   - Add iTerm2 image protocol support.
5. **Sixel backend**
   - Add optional Sixel support behind feature flag.
6. **Caching and config**
   - Add config settings and image cache.
7. **Remote images (optional)**
   - Gate behind `allow_remote` and add size limits.

## Open questions

- Do we want to render images inline within paragraphs or as block elements only?
- Should images participate in selection and yank, or be skipped?
- How to treat images inside lists and blockquotes for indentation?
- Should image rendering be disabled automatically over SSH?
