Below is a build plan for a Rust TUI Markdown viewer/editor launcher called **`mdx`**, built on **ratatui + tui-markdown**, with **md-tui–level rendering polish**, plus Vim-style navigation, TOC, splits, selection/yank, file watching, and inline git diff.

---

## 0) Goals and non-goals

### Goals

* Fast, low-latency TUI for large Markdown files (thousands of lines).
* High-quality Markdown rendering (tables, lists, headings, code blocks, links) comparable to **md-tui**:

  * correct wrapping, proper indentation, list continuation, code fences styling, inline emphasis, and sensible defaults.
* **Vim-ish navigation** in “normal mode”:

  * `hjkl`, `^u`, `^d`, `gg`, `G`, `/` search (worth doing early), `n`/`N` next/prev match.
* **TOC sidebar**: `T` toggles TOC (default visible on left).
* **Theme toggle**: `M` toggles dark/light (default dark).
* **Splits**:

  * `^w s` horizontal split
  * `^w v` vertical split
  * focus movement: `^↑ ^↓ ^← ^→` (Ctrl+Arrow) across panes
* **Visual line selection**: `Shift+V` to enter linewise selection; `hjkl/^u/^d` grows selection; `Y` yanks selection to clipboard.
* `e` opens current document in external editor (`$EDITOR`, fallback).
* **Config**: `mdx.yaml` under platform config dir (e.g. XDG on Linux).
* **File watching**:

  * show “modified on disk” indicator when file changes externally
  * optional auto-reload mode (configurable)
* **Inline git diff gutter** for changed lines vs `HEAD` (and optionally vs index), like a minimal `gitsigns`.

### Non-goals (initially)

* Full Markdown editing inside TUI (keep it a viewer + selection/yank + open-in-editor).
* Image preview, HTML rendering, mermaid, etc (later).
* Multi-file workspace (later).

---

## 1) Crate layout (workspace) and responsibilities

**Workspace** (recommended):

* `mdx-core`: parsing, document model, toc, selection model, git diff model, config.
* `mdx-tui`: ratatui app, panes, input handling, rendering glue to tui-markdown.
* `mdx`: binary crate (CLI args, init, run loop, logging).

This separation keeps TUI glue from leaking into core logic and makes testing much easier.

---

## 2) Key dependencies (Rust crates)

### TUI + input

* `ratatui` (UI)
* `crossterm` (terminal backend + key events)
* `unicode-width` (layout correctness)
* `textwrap` or ratatui wrapping utilities (if needed for custom wrapping beyond tui-markdown)

### Markdown parsing / rendering

* `tui-markdown` (render widget)
* `pulldown-cmark` (parse + heading extraction / anchors; tui-markdown likely uses it internally but you’ll want it for TOC and link targets)

### Clipboard

* `arboard` (cross-platform clipboard)

### File watching

* `notify` (recommended; cross-platform)

### Git diff

* `gix` (gitoxide) OR `git2`

  * `gix` is modern/pure-Rust-ish and fast; `git2` is stable but binds libgit2.
* Also useful: `similar` (diff algorithms) if you end up diffing text yourself (but prefer git APIs).

### Config and paths

* `serde`, `serde_yaml`
* `directories` (platform config dir resolution)
* `clap` (CLI)

### Editor launching

* `open` (optional; for opening links)
* `std::process::Command` for `$EDITOR`

### Performance & correctness

* `ropey` (optional but strongly recommended for large files; enables efficient line slicing & mapping)
* `parking_lot` or standard `Mutex`/`RwLock` for shared state between watcher thread and UI loop
* `crossbeam-channel` (event bus)

---

## 3) Data model

### Document model

Represent the file in a structure optimized for:

* mapping scroll position ⇄ rendered lines
* linewise selection and yank
* TOC jump positions
* diff gutter line mapping

Suggested:

* Keep the source text in a `Rope` (ropey).
* Maintain a `Vec<LineInfo>` index where each entry points to a line slice in the rope (or line offsets).
* Maintain parsed Markdown metadata:

  * headings: `Vec<Heading { level, text, line_no, anchor }>`
  * links: optional extraction for “open link under cursor” later

```rust
struct Document {
  path: PathBuf,
  rope: Rope,
  headings: Vec<Heading>,
  // revision tracking
  disk_mtime: SystemTime,
  loaded_mtime: SystemTime,
  dirty_on_disk: bool,
}
```

### Viewport state (per pane)

Each pane has its own view state, even if showing same document:

```rust
struct ViewState {
  scroll_line: usize,
  cursor_line: usize,      // “cursor” for selection and TOC highlight
  mode: Mode,              // Normal | VisualLine
  selection: Option<LineSelection>,
  show_toc: bool,          // may be global, but pane-local is also valid
  theme: ThemeVariant,     // global usually
}
```

### Pane tree (splits)

Represent splits as a tree:

```rust
enum PaneNode {
  Leaf(PaneId),
  Split { dir: SplitDir, a: Box<PaneNode>, b: Box<PaneNode>, ratio: u16 }
}
```

And a `HashMap<PaneId, Pane>` storing per-pane doc + view state.

This makes resize and focus traversal deterministic.

---

## 4) Rendering architecture (md-tui level polish)

### Layout

At each draw:

1. Compute full terminal layout:

   * Top status bar (1 line)
   * Body area
2. Within body:

   * If TOC enabled: left column for TOC (fixed width or %)
   * Remaining area: pane tree layout (splits)

### Markdown rendering strategy

`tui-markdown` renders Markdown into ratatui text primitives. But to match **md-tui’s “nice” feel**, you’ll likely need:

**A. Preprocessing pipeline**

* Normalize tabs, line endings.
* Optional “smart wrap” rules:

  * preserve code blocks (no wrapping)
  * wrap paragraphs to viewport width
  * lists: wrap with hanging indent
  * blockquotes: wrap with prefix
* `tui-markdown` may handle some of this, but you’ll want deterministic behavior across panes with different widths.

**B. Width-dependent caching**
Rendering Markdown is width-sensitive. Each pane width change can trigger reflow.

* Cache rendered output by `(doc_revision_id, pane_width, theme_variant)`:

  * `Rendered { lines: Vec<RenderedLine>, line_map: Vec<SourceLineRange> }`
* If using `Rope`, assign a monotonic `doc_revision_id` on reload.

**C. Source→render mapping (for selection + diff)**
You need a mapping from:

* source line numbers ↔ visible rendered lines
  So selection by *source lines* can highlight correctly even when wrapped.

Implementation plan:

* Keep selection in **source line space**.
* During render, produce an approximate mapping:

  * For each source line, how many rendered lines did it produce?
  * For non-trivial Markdown, perfect mapping is hard; but linewise selection typically expects “original lines” highlight.
* Practical approach for v1:

  * In normal rendering: highlight entire rendered area lines that correspond to selected source line range using a best-effort map:

    * For each source line in range, mark its rendered line spans if known.
  * For code blocks and plain paragraphs: mapping is easier.
  * For complex constructs: accept approximate highlight initially.

If you want *precise*, you can implement a “line tokenization phase” using pulldown-cmark events and track which source lines contributed to which blocks, then apply wrapping inside those blocks yourself.

**D. Styling/theme**
Define a theme struct:

```rust
struct Theme {
  base: Style,
  heading: [Style; 6],
  code: Style,
  link: Style,
  quote: Style,
  list_marker: Style,
  toc_active: Style,
  diff_add: Style,
  diff_del: Style,
  diff_mod: Style,
}
```

Have two built-ins (dark/light) and allow override in config.

---

## 5) Table of Contents (TOC) sidebar

### Building the TOC

Use `pulldown-cmark` to parse headings and also store:

* `level` (1..=6)
* `display text` (strip formatting reasonably)
* `source line` (best-effort; pulldown-cmark doesn’t always give direct line numbers; you can approximate by scanning source for heading markers and matching text, or use a parser that can give offsets. Another approach: pre-scan for ATX headings `^#{1,6}\s+` and Setext style; it’s good enough for v1.)

### TOC UI

* Left pane list with indentation by heading level.
* Highlight the heading corresponding to `cursor_line`.
* `Enter` or `l` jumps to selected heading.
* `T` toggles visibility.
* When TOC open, `^←/^→` focus moves between TOC and main panes (or treat TOC as another pane).

---

## 6) Input system and keybinding engine

### Event loop

Use a central event bus:

* `crossterm` key events → `AppEvent::Input(KeyEvent)`
* watcher thread → `AppEvent::FileChanged`
* periodic tick (e.g. 30–60 fps max) → `AppEvent::Tick`

Prefer `crossbeam-channel` with `select!` in the main loop.

### Modes

* **Normal**
* **VisualLine** (entered by `Shift+V`)
* (Optional later) Search mode

### Keybinding parsing

Implement a small “key chord” state machine:

* when `Ctrl+W` is pressed, enter a transient prefix state waiting for `s` or `v`
* likewise for `g` → waiting for `g`

Keep it simple:

```rust
enum KeyPrefix { None, CtrlW, G }
```

### Required bindings (v1)

Navigation:

* `j/k`: cursor line +/- 1 (bounded)
* `^d`: scroll half-page down (cursor follows)
* `^u`: half-page up
* `h/l`: if you later add “soft wrap column movement”, otherwise treat as no-op or pane focus? (I’d keep `h/l` as horizontal scroll later; for now, map to `^←/^→` pane focus only when ctrl is held.)
* `gg`: top
* `G`: bottom

TOC:

* `T`: toggle toc
* With TOC focused: `j/k` move; `Enter` jump; `q` close TOC

Theme:

* `M`: toggle dark/light

Splits:

* `^w s`: horizontal split of focused leaf pane (duplicate doc view)
* `^w v`: vertical split
* `^↑/^↓/^←/^→`: move focus across panes (define traversal in pane tree)

Selection/yank:

* `Shift+V`: enter VisualLine; anchor at cursor line
* In VisualLine:

  * `j/k/^u/^d/gg/G` grow/shrink selection by moving cursor
  * `Y`: yank selected source lines to clipboard (join with `\n`)
  * `Esc`: exit VisualLine (clear selection)

Editor open:

* `e`: open `$EDITOR <file>` at current line if possible:

  * support `nvim +{line}` / `vim +{line}` / `code --goto file:line:col` heuristics
  * configurable editor command template in config

---

## 7) Clipboard yank implementation

* Selection is linewise: `[start_line, end_line]` in source line numbers.
* On `Y`:

  * Extract rope lines in range, join with `\n`
  * `arboard::Clipboard::new()?.set_text(text)`
* Show status: “Yanked N lines” in status bar.
* Handle clipboard errors gracefully (Wayland/headless): show error in status.

---

## 8) Config: `.config/mdx/mdx.yaml` (platform-aware)

### Config path resolution

Use `directories::ProjectDirs::from("…", "…", "mdx")`

* Linux: `$XDG_CONFIG_HOME/mdx/mdx.yaml` or `~/.config/mdx/mdx.yaml`
* macOS: `~/Library/Application Support/mdx/mdx.yaml`
* Windows: `{FOLDERID_RoamingAppData}\mdx\mdx.yaml`

### Config schema (v1)

```yaml
theme: dark            # dark|light
toc:
  enabled: true
  side: left           # left|right
  width: 32            # columns
editor:
  command: "$EDITOR"   # or explicit "nvim"
  args: ["+{line}", "{file}"]
watch:
  enabled: true
  auto_reload: false
git:
  diff: true
  base: head           # head|index
keys:
  # allow overrides later
```

### Config loading

* Load at startup; if missing, use defaults.
* `:reload-config` command not needed initially; but consider `R` to reload config later.

---

## 9) File watching + “changed on disk” indicator

### Watcher

* `notify::recommended_watcher` in a separate thread.
* Watch the file itself and parent directory (some editors use atomic rename).
* On events: send `AppEvent::FileChanged(path)`.

### Behavior

* When change detected:

  * Set `dirty_on_disk = true`
  * Show indicator in status bar: `● modified on disk`
* If `auto_reload: true`:

  * debounce (e.g. 150–300ms)
  * reload file, re-parse headings, recompute diff, clear dirty flag
* If `auto_reload: false`:

  * prompt hint: “Press r to reload” (implement `r`)

Reload should preserve:

* cursor line (clamp to new length)
* scroll position (best-effort)

---

## 10) Git diff gutter (inline column)

### What to show

A single-character gutter per source line:

* `+` added line
* `~` modified line
* `-` deletions: tricky because deleted lines don’t exist in working tree; you can show deletions as markers on adjacent lines or show count in a “virtual” row. For v1: ignore deletions or show a `▾` marker at nearest line.

### How to compute

Two approaches:

**Approach A: Git API (recommended)**

* Open repo containing file path.
* Get blob at `HEAD:<path>` (or index version).
* Compare blob content to working tree content.
* Produce a `Vec<DiffMark>` aligned to *working tree lines*:

  * for each working tree line, mark unchanged/added/modified.

With `gix`, you can load object + content and run diffs. With `git2`, you can use libgit2 diff APIs, but mapping to line numbers is still some work.

**Approach B: Text diff fallback**

* If file not in git repo or git fails:

  * no gutter
* If in repo:

  * use `similar::TextDiff` between base text and current
  * walk ops and build line maps

This is often easier to control and test. You don’t need to stage anything; just compare text.

### Rendering gutter

* When drawing the markdown pane:

  * Reserve 2–3 columns on the left: `[diff_mark][space]`
  * Then the markdown rendering area.
* Because tui-markdown is a widget, you may need to render into a `Paragraph` yourself, or wrap it:

  * easiest: render markdown into `Vec<Line>` first (via tui-markdown conversion), then prefix each line with gutter spans.
  * This also helps with selection highlight overlays.

**Important constraint**: gutter is aligned to *source lines*, but rendered lines include wrapping. Decide:

* Gutter applies only to the first rendered line of each source line (best effort).
* Wrapped continuation lines show blank gutter.

That gives a clean look and is consistent with many editors.

---

## 11) Status bar and UX details

Status bar content (example):

* Left: `mdx  file.md  [Ln 120/900]  [Normal|V-Line]`
* Middle: `TOC:on  Theme:dark  Watch:on`
* Right: `● modified on disk` and maybe `Git:HEAD` indicator

Add a help overlay (`?`) later.

---

## 12) Performance plan (so it stays snappy)

* Use `Rope` for text storage to avoid O(n) slicing.
* Cache rendered output per width/theme/doc revision.
* Re-render only when:

  * width changed
  * theme changed
  * doc revision changed
* Diff computation can be expensive; run it:

  * on load/reload
  * on file change debounce
  * optionally in a background thread with results delivered via channel (keep UI responsive)

---

## 13) Testing strategy

### Unit tests (mdx-core)

* TOC extraction correctness for common heading styles.
* Selection expansion logic.
* Diff marking mapping from base/current strings → marks.
* Config parsing defaults and overrides.

### Snapshot-ish tests

* Render a sample markdown at width N and compare produced `Vec<Line>` shape/markers (not colors, but structure).
* For diff gutter + wrapping: ensure gutter placement rules.

### Manual acceptance checklist

* Open big markdown, scroll fast with `^d/^u`, no lag.
* Toggle TOC, jump headings, focus returns.
* Split views show independent scroll/cursor.
* Visual line selection yanks correct text.
* Modify file externally: indicator appears; reload works.

---

## 14) Milestones (build order)

### Milestone 1 — Skeleton TUI

* CLI opens a file
* Basic ratatui frame: status bar + markdown view
* `q` quits
* `j/k/^u/^d/gg/G` navigation

### Milestone 2 — TOC sidebar

* Heading extraction (ATX + Setext scan)
* TOC list UI and `T` toggle
* Jump to heading

### Milestone 3 — Themes

* Dark/light theme structs + `M` toggle
* Style coverage: headings, code, links, blockquotes

### Milestone 4 — Splits & pane tree

* PaneNode tree + splitting
* `^w s` / `^w v`
* `^arrow` focus move
* Each pane has its own ViewState

### Milestone 5 — Visual line selection + yank

* Mode handling (Normal/VisualLine)
* Highlight selection
* Yank with `arboard`
* Status confirmation

### Milestone 6 — Open in editor

* `e` spawns `$EDITOR`
* Add `editor.args` template support (`{file}`, `{line}`)

### Milestone 7 — Config file

* Platform config dir + YAML
* Merge config with defaults
* Key options (theme default, toc default, editor template, watch/auto reload)

### Milestone 8 — File watching

* notify watcher + debounce
* “modified on disk” indicator
* `r` reload & preserve cursor

### Milestone 9 — Git diff gutter

* Detect repo root, base text load (HEAD)
* Diff marks mapping
* Render gutter aligned to source lines (first rendered line only)
* Config flag to disable

---

## 15) Implementation notes / tricky bits to decide early

1. **Rendering pipeline control**

   * If tui-markdown doesn’t let you intercept line generation, you may need to:

     * parse markdown → ratatui `Text` yourself (using pulldown-cmark events)
     * OR fork/extend tui-markdown for better mapping + gutter support.
   * Plan for this: wrap `MarkdownRenderer` behind a trait so you can swap implementation.

2. **TOC line mapping**

   * Getting exact source line numbers from markdown parse is non-trivial.
   * Start with regex scan for headings; it’s stable and fast.

3. **Ctrl+Arrow key events**

   * Terminal support varies. On some terminals, Ctrl+Arrow may not arrive as expected.
   * Provide fallbacks in config (e.g. `Ctrl+h/j/k/l` for pane focus), even if you keep the default as requested.

4. **Clipboard on Wayland**

   * `arboard` is good, but failures happen in headless/ssh.
   * Always show a friendly error and allow copying to stdout via `:yank` later if needed.

---

Here’s a concrete sketch of the **core structs**, the **event loop**, and a **feature-flag layout** that keeps `mdx` clean and testable. I’ll keep it “code-shaped” but not a full dump.

---

## Workspace layout + feature flags

### Crates

* `mdx-core`

  * **no terminal deps**
  * features: `git`, `watch` (watch can live in tui too, but core can hold the state machine)
* `mdx-tui`

  * ratatui/crossterm glue
  * features: `clipboard`, `watch`
* `mdx` (bin)

  * CLI + wiring

### Cargo features (top-level)

* `clipboard` → enables yank-to-clipboard (arboard)
* `watch` → enables notify watcher
* `git` → enables diff gutter (gix/git2 + similar)

You can also do `default = ["clipboard","watch","git"]`.

---

## Core data structures (mdx-core)

### Document + parsing outputs

```rust
// mdx-core/src/doc.rs
use ropey::Rope;

#[derive(Clone)]
pub struct Heading {
    pub level: u8,          // 1..=6
    pub text: String,       // plain-ish
    pub line: usize,        // 0-based source line
    pub anchor: String,     // stable-ish id for future link jumps
}

#[derive(Clone)]
pub struct Document {
    pub path: std::path::PathBuf,
    pub rope: Rope,
    pub headings: Vec<Heading>,

    // disk / reload tracking
    pub loaded_mtime: Option<std::time::SystemTime>,
    pub disk_mtime: Option<std::time::SystemTime>,
    pub dirty_on_disk: bool,

    // monotonic revision id
    pub rev: u64,
}

impl Document {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> { /* ... */ }
    pub fn reload(&mut self) -> anyhow::Result<()> { /* ... */ }

    pub fn line_count(&self) -> usize { self.rope.len_lines() }

    pub fn get_lines(&self, start: usize, end_inclusive: usize) -> String {
        // Extract for yank (linewise)
    }
}
```

### TOC extraction (v1: regex scan)

Keep it in core so it’s testable.

```rust
// mdx-core/src/toc.rs
pub fn extract_headings(rope: &Rope) -> Vec<Heading> {
    // Scan lines for:
    // - ATX: ^#{1,6}\s+
    // - Setext: "Title" + next line "====" or "----"
}
```

### Selection model (Visual line mode)

Selection should be source-line oriented.

```rust
// mdx-core/src/selection.rs
#[derive(Clone, Copy, Debug)]
pub struct LineSelection {
    pub anchor: usize,
    pub cursor: usize,
}

impl LineSelection {
    pub fn range(&self) -> (usize, usize) {
        let a = self.anchor.min(self.cursor);
        let b = self.anchor.max(self.cursor);
        (a, b)
    }
}
```

### Diff model (line-aligned gutter)

Represent working-tree line marks.

```rust
// mdx-core/src/diff.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffMark {
    None,
    Added,
    Modified,
    // Deleted is hard to represent line-aligned; we can store deletion counts by "after line"
    DeletedAfter(u16),
}

#[derive(Clone)]
pub struct DiffGutter {
    pub marks: Vec<DiffMark>, // length == working tree line_count
}

impl DiffGutter {
    pub fn empty(line_count: usize) -> Self { /* ... */ }
}
```

Diff computation (with `similar` fallback) lives behind a trait so you can swap gix/git2 later:

```rust
// mdx-core/src/diff.rs
pub trait DiffProvider: Send + Sync {
    fn compute(&self, path: &std::path::Path, current: &Rope) -> anyhow::Result<DiffGutter>;
}
```

### Config model (serde_yaml)

```rust
// mdx-core/src/config.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: ThemeVariant,
    pub toc: TocConfig,
    pub editor: EditorConfig,
    pub watch: WatchConfig,
    pub git: GitConfig,
    // key overrides later
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThemeVariant { Dark, Light }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocConfig {
    pub enabled: bool,
    pub side: TocSide,
    pub width: u16,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TocSide { Left, Right }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub command: String,     // "$EDITOR" or explicit
    pub args: Vec<String>,   // supports {file} {line}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig { pub enabled: bool, pub auto_reload: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig { pub diff: bool, pub base: GitBase }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitBase { Head, Index }
```

Config path helper:

```rust
pub fn config_path() -> std::path::PathBuf {
    // use directories::ProjectDirs
}
```

---

## TUI core structures (mdx-tui)

### App state

Key idea: keep **pure app state** (docs, panes, focus) separate from **renderer cache** and **IO side effects**.

```rust
// mdx-tui/src/app.rs
use mdx_core::{Document, DiffGutter, Config, selection::LineSelection};

pub struct App {
    pub config: Config,
    pub theme: ThemeVariant,        // current (toggleable)
    pub show_toc: bool,             // global toggle
    pub toc_focus: bool,            // treat TOC as focusable target

    pub panes: PaneManager,
    pub docs: DocStore,

    pub status: StatusLine,
    pub key_prefix: KeyPrefix,
}

pub struct StatusLine {
    pub message: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy)]
pub enum KeyPrefix { None, CtrlW, G }
```

### Document store (multi-doc possible later)

```rust
pub struct DocStore {
    pub active_doc: DocId,
    pub docs: std::collections::HashMap<DocId, DocState>,
    pub next_id: u64,
}

pub struct DocState {
    pub doc: Document,
    pub diff: Option<DiffGutter>,
}
```

### Pane manager / split tree

```rust
pub type PaneId = u64;
pub type DocId = u64;

pub struct PaneManager {
    pub root: PaneNode,
    pub panes: std::collections::HashMap<PaneId, Pane>,
    pub focused: PaneId,
    pub next_id: PaneId,
}

pub enum PaneNode {
    Leaf(PaneId),
    Split { dir: SplitDir, a: Box<PaneNode>, b: Box<PaneNode>, ratio: u16 },
}

#[derive(Clone, Copy)]
pub enum SplitDir { Horizontal, Vertical }

pub struct Pane {
    pub doc_id: DocId,
    pub view: ViewState,
    // width-dependent caches live outside (RendererCache)
}

pub enum Mode { Normal, VisualLine }

pub struct ViewState {
    pub scroll_line: usize,
    pub cursor_line: usize,
    pub mode: Mode,
    pub selection: Option<LineSelection>,
    // later: search term / matches
}
```

Traversal helpers:

* split current leaf
* compute rectangles for each leaf during render
* move focus with Ctrl+Arrow: pick nearest neighbor leaf rectangle in direction

---

## Renderer cache + “gutter + markdown” composition

### Why cache?

Markdown reflow is expensive and width-dependent.

```rust
// mdx-tui/src/render.rs
pub struct RendererCache {
    pub entries: lru::LruCache<RenderKey, RenderedDoc>,
}

#[derive(Hash, PartialEq, Eq)]
pub struct RenderKey {
    pub doc_rev: u64,
    pub width: u16,
    pub theme: ThemeVariant,
}

pub struct RenderedDoc {
    pub lines: Vec<ratatui::text::Line<'static>>,
    // mapping: source_line -> first rendered line index (best effort)
    pub source_to_rendered_first: Vec<usize>,
}
```

**Composition rule for diff gutter**

* When you have `RenderedDoc.lines` (already wrapped):

  * prefix the **first rendered line** corresponding to each source line with gutter mark.
  * continuation lines get blank gutter.

That requires `source_to_rendered_first`, at least best-effort.

**Pragmatic mapping approach for v1**

* render source as “line blocks” and wrap per line for basic constructs
* OR:

  * accept approximate: “one source line → at least one rendered line” for plain text; complex markdown blocks less exact.
* If you want to mimic md-tui quality, you’ll likely end up doing a block-level layout yourself (pulldown-cmark events → blocks → wrap engine), but you can phase that in.

---

## Input + Event loop architecture

### Events

Single channel into the UI loop.

```rust
// mdx-tui/src/event.rs
pub enum AppEvent {
    Input(crossterm::event::KeyEvent),
    Tick,
    FileChanged(std::path::PathBuf),
    DiffReady(DocId, mdx_core::DiffGutter),
}
```

### Threads

* Main thread: TUI + state + rendering
* Watcher thread (feature `watch`): sends `FileChanged`
* Diff worker thread (feature `git`): compute diff asynchronously after load/reload/file change; sends `DiffReady`

Use `crossbeam_channel::{unbounded, Sender, Receiver}`.

### Main loop sketch

```rust
fn run(mut app: App, mut terminal: Terminal<Backend>, rx: Receiver<AppEvent>, tx: Sender<AppEvent>) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        match rx.recv()? {
            AppEvent::Input(key) => {
                if handle_input(&mut app, key, &tx)? == Action::Quit { break; }
            }
            AppEvent::Tick => { /* animations / debounce timers */ }
            AppEvent::FileChanged(path) => {
                // mark dirty; maybe auto-reload
                app.on_file_changed(path, &tx)?;
            }
            AppEvent::DiffReady(doc_id, diff) => {
                app.docs.docs.get_mut(&doc_id).unwrap().diff = Some(diff);
            }
        }
    }
    Ok(())
}
```

### Input handling (prefix-aware)

```rust
fn handle_input(app: &mut App, key: KeyEvent, tx: &Sender<AppEvent>) -> anyhow::Result<Action> {
    // 1) global quit
    if matches!(key, KeyEvent { code: Char('q'), modifiers: NONE }) { return Ok(Action::Quit); }

    // 2) prefix logic
    match app.key_prefix {
        KeyPrefix::CtrlW => { /* expect s or v */ }
        KeyPrefix::G => { /* expect g */ }
        KeyPrefix::None => { /* normal dispatch by mode */ }
    }
    Ok(Action::Continue)
}
```

### Mode dispatch

* If TOC has focus: interpret `j/k/Enter/T/q` there.
* Else dispatch to focused pane based on `pane.view.mode`.

---

## Keybinding behavior sketch

### Normal mode

* `j/k`: move cursor 1 line; if cursor moves past viewport, adjust scroll
* `^d/^u`: half page scroll (cursor follows)
* `gg`/`G`: jump
* `T`: toggle TOC (and possibly focus it if opening)
* `M`: theme toggle (invalidate renderer cache)
* `Ctrl+W` prefix: split
* `Ctrl+Arrow`: focus move
* `e`: open editor
* `r`: reload file (clears dirty flag, kicks diff worker)

### Visual line mode

* entered by `Shift+V`: selection = { anchor=cursor_line, cursor=cursor_line }
* movement keys adjust cursor, selection range updates
* `Y`: yank selection text; exit visual or stay? (vim stays, but for viewer you can choose either; I’d stay in VisualLine until Esc)
* `Esc`: exit visual (selection None)

---

## Editor launching details

Implement “editor command template”:

* Resolve command:

  * if config `editor.command` is `"$EDITOR"`: use env var `EDITOR`, else fallback `nvim`, `vim`, `nano` (in that order)
* Expand args:

  * replace `{file}` with path
  * `{line}` with 1-based line number
* Spawn:

  * restore terminal state (leave alt screen) before launching
  * after editor exits, re-init terminal and redraw
  * watch out: many TUI apps do “suspend” style; do the simplest robust approach.

---

## File watching + debounce

Keep a small debounce state in `DocState` or `App`:

```rust
pub struct WatchState {
    pub pending_reload: bool,
    pub last_event_at: Instant,
}
```

On `FileChanged`:

* mark dirty flag
* if auto_reload:

  * set pending_reload=true, last_event_at=now
    On each Tick:
* if pending_reload and now-last_event_at > 250ms:

  * reload file
  * clear dirty
  * request diff recompute

This avoids thrashing when editors write temp files.

---

## Diff worker thread sketch (feature `git`)

Use a request channel to avoid recomputing too often:

* UI thread sends `DiffRequest(doc_id, path, rope_snapshot/rev)`
* worker coalesces latest per doc_id
* worker replies `DiffReady(doc_id, diff)` if rev still current

This prevents slow diff calculations from applying to old revisions.

---

## Rendering: UI layout sketch

### Top-level draw

* `StatusBar` (1 row)
* `Body` (rest)

### Body split

If TOC enabled:

* left fixed width = config.toc.width (or 25% clamped)
* right = panes area

TOC on right if configured.

### Pane rectangles

* recursively split the right area according to `PaneNode`
* for each leaf:

  * compute inner rect
  * draw:

    * optional border (highlight focused)
    * markdown viewer content inside
    * overlay selection highlight (if possible) or style in composed lines
    * draw scrollbar indicator (optional)

---

## What I’d implement “first” to de-risk

1. **Pane tree + focus movement** (this determines everything else)
2. **Renderer cache keyed by width/theme/rev**
3. **TOC extraction + jump** (validates line-based navigation)
4. **Diff gutter prefix composition** (proves your line mapping approach is viable)
5. **Selection highlight + yank** (tests source↔render mapping assumptions)

---

## Small but important UX details

* Cursor is conceptual (line-based). Indicate with:

  * highlight current line background, OR
  * a left “cursor marker” column separate from diff gutter (e.g. `>`), so you can show both.
* Status bar shows mode (`NORMAL` / `V-LINE`) like Vim.
* When dirty-on-disk: show `●` and optionally disable yank? (no need; just inform)

---

## 1) Ctrl+Arrow key decoding across terminals (and robust fallbacks)

### The annoying reality

`Ctrl+Arrow` is **not reliably standardized** across terminals. Some send real “modified arrow” sequences; others don’t send anything special (or the OS/WM eats it). With crossterm, you’ll *sometimes* get:

* `KeyCode::Up` with `KeyModifiers::CONTROL`
* or just `KeyCode::Up` (no modifier)
* or escape sequences that crossterm may decode inconsistently depending on terminal + mode

So: implement **three layers**:

1. **Preferred**: `Ctrl+Arrow` if it arrives
2. **Fallback A**: `Ctrl+h/j/k/l` (very reliable everywhere)
3. **Fallback B**: `Alt+Arrow` (often works when Ctrl doesn’t)
4. Make it configurable so you can swap per environment.

### What to implement (pragmatic approach)

In your input handler, define a helper like:

* Pane focus move:

  * If `(code is Arrow && modifiers has CONTROL)` → move focus
  * Else if `(code is Char('h'|'j'|'k'|'l') && modifiers has CONTROL)` → move focus
  * Else if `(code is Arrow && modifiers has ALT)` → move focus (optional)
* Split prefix (`^w s`, `^w v`) stays separate.

#### Key mapping table (recommended defaults)

| Action      | Primary      | Fallback 1 | Fallback 2  |
| ----------- | ------------ | ---------- | ----------- |
| Focus Left  | `Ctrl+Left`  | `Ctrl+h`   | `Alt+Left`  |
| Focus Down  | `Ctrl+Down`  | `Ctrl+j`   | `Alt+Down`  |
| Focus Up    | `Ctrl+Up`    | `Ctrl+k`   | `Alt+Up`    |
| Focus Right | `Ctrl+Right` | `Ctrl+l`   | `Alt+Right` |

This also *fits your vim brain* nicely.

### crossterm event matching snippet

This is the level of matching you want (note: **don’t** assume exact equality for modifiers; use `.contains()`):

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy)]
enum Dir { Left, Right, Up, Down }

fn pane_move_dir(key: KeyEvent) -> Option<Dir> {
    let mods = key.modifiers;

    // Primary: Ctrl + Arrow
    if mods.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Left  => return Some(Dir::Left),
            KeyCode::Right => return Some(Dir::Right),
            KeyCode::Up    => return Some(Dir::Up),
            KeyCode::Down  => return Some(Dir::Down),
            KeyCode::Char('h') => return Some(Dir::Left),   // fallback A
            KeyCode::Char('j') => return Some(Dir::Down),
            KeyCode::Char('k') => return Some(Dir::Up),
            KeyCode::Char('l') => return Some(Dir::Right),
            _ => {}
        }
    }

    // Optional fallback B: Alt + Arrow
    if mods.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Left  => return Some(Dir::Left),
            KeyCode::Right => return Some(Dir::Right),
            KeyCode::Up    => return Some(Dir::Up),
            KeyCode::Down  => return Some(Dir::Down),
            _ => {}
        }
    }

    None
}
```

### Terminal-specific notes (what tends to happen)

* **Kitty / WezTerm / Alacritty**: often deliver `Ctrl+Arrow` correctly, but not always through multiplexers.
* **tmux**: frequently **normalizes or drops** modifiers unless configured. If you care about tmux users (you do 😄), document that `Ctrl+h/j/k/l` is the reliable fallback.
* **Wayland compositor shortcuts**: can eat `Ctrl+Arrow` globally. Again: fallback.

### Make it configurable (so users stop filing bugs)

Add to config:

```yaml
keys:
  pane_focus:
    primary: "C-Arrow"
    fallback1: "C-hjkl"
    fallback2: "M-Arrow"
```

You don’t have to implement a full key DSL immediately—just offer booleans like `enable_ctrl_hjkl_fallback: true`.

---

## 2) Git diff gutter marking using `similar::TextDiff` (line-aligned, testable)

### Goal

Given:

* `base_text` (from `HEAD:path` or index:path)
* `current_text` (working tree file)

Produce:

* `marks: Vec<DiffMark>` length == `current_lines`
* Each current line is `None | Added | Modified`
* Deletions: optionally store as `DeletedAfter(n)` on the nearest following line, or ignore for v1.

### Approach

Use `similar::TextDiff::from_lines(base, current)` which yields grouped “ops” (insert/delete/replace/equal) in terms of line ranges. Map those ranges to marks.

**Rules**

* `Insert` → mark those inserted current lines as `Added`
* `Delete` → represent as `DeletedAfter(count)` at the boundary (optional)
* `Replace` (delete+insert) → mark inserted lines as `Modified` (or `Added` if you prefer). Most gutters show `~` for replace.
* `Equal` → `None`

### Concrete algorithm (real Rust)

This is solid enough to drop in as v1:

```rust
use similar::{ChangeTag, TextDiff};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffMark {
    None,
    Added,
    Modified,
    DeletedAfter(u16),
}

pub fn diff_gutter_from_text(base: &str, current: &str) -> Vec<DiffMark> {
    let diff = TextDiff::from_lines(base, current);

    let current_line_count = current.lines().count().max(1); // keep non-empty vector for empty file edge cases
    let mut marks = vec![DiffMark::None; current_line_count];

    // Track deletions that occur between current lines (e.g. deletions at end of file)
    // deleted_after[i] = number of deleted base lines after current line i
    let mut deleted_after = vec![0u16; current_line_count];

    for op in diff.ops() {
        let old = op.old_range(); // base line indices
        let new = op.new_range(); // current line indices

        match op.tag() {
            ChangeTag::Equal => {}

            ChangeTag::Insert => {
                for i in new.start..new.end {
                    if i < marks.len() {
                        marks[i] = DiffMark::Added;
                    }
                }
            }

            ChangeTag::Delete => {
                // No current lines to mark. Attach deletion count to the nearest boundary.
                let del_count = (old.end - old.start) as u16;

                if marks.is_empty() {
                    // nothing to do
                } else if new.start == 0 {
                    // deletion before first current line -> attach to line 0
                    deleted_after[0] = deleted_after[0].saturating_add(del_count);
                } else if new.start >= marks.len() {
                    // deletion after last current line -> attach to last
                    let last = marks.len() - 1;
                    deleted_after[last] = deleted_after[last].saturating_add(del_count);
                } else {
                    // deletion between current lines -> attach to previous line
                    let idx = new.start.saturating_sub(1);
                    deleted_after[idx] = deleted_after[idx].saturating_add(del_count);
                }
            }

            ChangeTag::Replace => {
                // Replace means old-range swapped with new-range.
                // Mark new lines as Modified.
                for i in new.start..new.end {
                    if i < marks.len() {
                        marks[i] = DiffMark::Modified;
                    }
                }

                // If you want to also track deletions when replacement shrinks:
                let del_count = (old.end - old.start).saturating_sub(new.end - new.start) as u16;
                if del_count > 0 && !marks.is_empty() {
                    let idx = if new.end == 0 { 0 } else { (new.end - 1).min(marks.len() - 1) };
                    deleted_after[idx] = deleted_after[idx].saturating_add(del_count);
                }
            }
        }
    }

    // Overlay deletions as DeletedAfter markers (optional).
    for (i, d) in deleted_after.into_iter().enumerate() {
        if d > 0 {
            // If line already Added/Modified, you can keep that and ignore deletion count,
            // or encode both. For v1, encode deletions only if None.
            if marks[i] == DiffMark::None {
                marks[i] = DiffMark::DeletedAfter(d);
            }
        }
    }

    marks
}
```

### Rendering gutter marks

In your renderer composition:

* `None` → `' '`
* `Added` → `'+'`
* `Modified` → `'~'`
* `DeletedAfter(n)` → maybe `'_'` or `'▾'` and show `n` in status on hover later (v1 just `'▾'`)

And remember: **only put a gutter mark on the first wrapped line** of a source line.

### Getting `base_text` (HEAD or index)

Two-stage provider design:

* Core trait: `DiffProvider::compute(path, current_rope) -> DiffGutter`
* Implementations:

  * `GitHeadProvider` (preferred)
  * `GitIndexProvider` (optional)
  * `NoopProvider` fallback

For **v1**, it’s acceptable to:

* find repo root by walking parents for `.git`
* call out to `git show HEAD:path` (subprocess) if you want speed of implementation

  * it’s not “pure Rust”, but it’s insanely pragmatic and reliable
* later, replace with `gix` APIs for speed and no external dependency

If you want pure Rust from day 1, tell me whether you prefer `gix` or `git2` and I’ll sketch the exact provider code path for reading `HEAD:<path>` content.

---

## Quick integration points (so it actually plugs into mdx cleanly)

* On load/reload:

  * compute `headings`
  * set `doc.rev += 1`
  * enqueue diff request (async)
* On `DiffReady`:

  * `doc_state.diff = Some(diff)`
  * redraw
* On render for a pane:

  * get cached `RenderedDoc` for `(rev,width,theme)`
  * merge:

    * gutter prefix (diff)
    * cursor marker (optional)
    * selection highlight (VisualLine)

---

Cool — using **`gix` now** is totally doable, but the API surface is big, and there are a couple of tricky bits you want to get right early:

* resolve repo + workdir
* resolve the file path *relative to repo root*
* read **`HEAD` tree entry** for that path and decode blob bytes → text
* handle “not tracked / not committed / path not found in HEAD” gracefully
* handle submodules / worktrees / bare repos (at least don’t crash)

Below is a concrete design + code-shaped sketch you can drop into `mdx-core` (or `mdx-tui` diff worker), and then feed the text into your `similar::TextDiff` gutter function.

---

## 1) DiffProvider design with gix

### Types

```rust
pub enum GitBase {
    Head,
    Index, // optional v1; head-only is simpler
}

pub struct GixDiffProvider {
    pub base: GitBase,
}

impl DiffProvider for GixDiffProvider {
    fn compute(&self, path: &Path, current: &Rope) -> anyhow::Result<DiffGutter> {
        // 1) locate repo containing `path`
        // 2) compute repo-relative path
        // 3) load base text from HEAD (or index)
        // 4) diff base vs current
    }
}
```

### Repo discovery strategy

Given an absolute `path`:

* Walk parents until you find a git repo (or use gix discovery helpers).
* Then compute `repo_relative = path.strip_prefix(repo_workdir)`.

You’ll want a helper:

```rust
pub struct RepoContext {
    pub repo: gix::Repository,
    pub workdir: std::path::PathBuf,     // repo.work_dir().unwrap().to_path_buf()
    pub rel_path: std::path::PathBuf,    // path relative to workdir
}
```

---

## 2) Getting base text from `HEAD:<path>` using gix

### What “HEAD:<path>” means

* Resolve `HEAD` → commit → tree → entry at rel_path → blob → bytes.
* Decode bytes into text (assume UTF-8, fallback lossily).

### Sketch: `read_head_file_text(repo, rel_path) -> Option<String>`

This is the core.

```rust
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn read_head_file_text(repo: &gix::Repository, rel_path: &Path) -> Result<Option<String>> {
    // If HEAD is unborn (fresh repo), return None
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };

    // Peel HEAD to a commit-ish / object
    let head_commit = match head.peel_to_commit() {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let tree = head_commit.tree().context("peel HEAD commit to tree")?;

    // gix trees are traversed by entry lookup; we need a repo-path in slash form.
    // Convert rel_path to a `BStr` with forward slashes:
    let repo_path = path_to_repo_bstr(rel_path)?;

    // Look up entry in the HEAD tree
    let entry = match tree.lookup_entry_by_path(repo_path.as_ref()) {
        Ok(Some(e)) => e,
        Ok(None) => return Ok(None), // not present in HEAD (untracked or new file)
        Err(err) => return Err(err).context("lookup path in HEAD tree"),
    };

    // Ensure it's a blob
    let oid = entry.oid();
    let obj = repo.find_object(oid)?.into_blob().ok(); // ok() converts Result -> Option; adjust as needed
    let blob = match obj {
        Some(b) => b,
        None => return Ok(None),
    };

    let bytes = blob.data; // depending on gix version: may be `blob.data` or method to read bytes
    let text = String::from_utf8(bytes.to_vec()).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
    Ok(Some(text))
}
```

**Notes**

* The exact blob accessors can vary by gix version. In some versions you do:

  * `repo.find_object(oid)?.try_into_blob()?` or
  * `repo.find_object(oid)?.peel_to_blob()?`
  * and then `blob.data` / `blob.data.to_vec()`
* The *shape* above is correct: commit → tree → lookup path → blob → bytes.

### Helper: convert OS path to git-internal slash path

Git paths are `/` separated and byte strings. Keep it simple:

```rust
use anyhow::{Result, bail};
use bstr::{BString, ByteSlice};

pub fn path_to_repo_bstr(rel_path: &Path) -> Result<BString> {
    // Reject weird paths early (e.g. ..)
    for comp in rel_path.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            bail!("path contains '..' which is not valid for repo lookup");
        }
    }

    let s = rel_path
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    Ok(BString::from(s))
}
```

---

## 3) Repo discovery with gix (robust)

You want:

* if file is in a worktree: open repo, get workdir, strip prefix
* if path not inside workdir: no git diff

A pragmatic helper:

```rust
use anyhow::{Result, Context};
use std::path::{Path, PathBuf};

pub fn open_repo_for_path(path: &Path) -> Result<Option<RepoContext>> {
    let abs = path.canonicalize().context("canonicalize path")?;

    // Walk up parents looking for a repo.
    // (You can swap this for gix discovery helpers later.)
    let mut cur = abs.parent();
    while let Some(dir) = cur {
        // Try open repo at this dir
        if let Ok(repo) = gix::open(dir) {
            // Must have a workdir (non-bare) for this use-case
            if let Some(workdir) = repo.work_dir() {
                let wd = workdir.to_path_buf();
                if let Ok(rel) = abs.strip_prefix(&wd) {
                    return Ok(Some(RepoContext {
                        repo,
                        workdir: wd,
                        rel_path: rel.to_path_buf(),
                    }));
                }
            }
        }
        cur = dir.parent();
    }
    Ok(None)
}
```

This is “good enough” for v1 and easy to test. Later, you can replace the manual walk with gix’s discover if you want.

---

## 4) Putting it together: `GixDiffProvider::compute`

```rust
use ropey::Rope;
use anyhow::Result;

impl GixDiffProvider {
    pub fn compute_marks(&self, file_path: &Path, current: &Rope) -> Result<Vec<DiffMark>> {
        let ctx = match open_repo_for_path(file_path)? {
            Some(c) => c,
            None => {
                // Not in a repo
                let n = current.len_lines();
                return Ok(vec![DiffMark::None; n.max(1)]);
            }
        };

        let current_text = rope_to_string(current);

        // Base text from HEAD
        let base_text = match self.base {
            GitBase::Head => read_head_file_text(&ctx.repo, &ctx.rel_path)?,
            GitBase::Index => {
                // optional, see next section
                read_index_file_text(&ctx.repo, &ctx.workdir, &ctx.rel_path)?
            }
        }.unwrap_or_default();

        Ok(diff_gutter_from_text(&base_text, &current_text))
    }
}

fn rope_to_string(r: &Rope) -> String {
    // Rope -> String. For big docs, doing this on every frame is bad.
    // Only do it in diff worker on load/reload events.
    let mut s = String::new();
    for chunk in r.chunks() { s.push_str(chunk); }
    s
}
```

**Performance note:** do this in the diff worker thread. Don’t diff on the UI tick.

---

## 5) Optional: “index” base (staged version)

This is harder than HEAD because you need to read the index entry’s blob OID. With gix, you can load the index, locate the entry for rel_path, then read its blob.

High-level sketch:

```rust
pub fn read_index_file_text(repo: &gix::Repository, workdir: &Path, rel_path: &Path) -> Result<Option<String>> {
    // 1) load index
    // 2) find entry by path
    // 3) entry provides oid for staged blob
    // 4) repo.find_object(oid) -> blob -> bytes -> String
    todo!()
}
```

If you want index in v1, I can give you a concrete implementation once you decide which gix index API you’re using (it has evolved across versions). HEAD-only is a strong v1.

---

## 6) Edge cases you should handle explicitly

### Unborn HEAD (empty repo)

`repo.head()?.peel_to_commit()` fails → return `None` base text → all changes look “Added/Modified”. That’s fine.

### File not in HEAD (new file)

`lookup_entry_by_path` returns None → base is empty → diff marks lines as Added/Modified (depending on algorithm). I’d treat this as **Added** for all lines by special-casing:

* if base is empty and current non-empty → mark all lines Added

### Binary files

If blob bytes aren’t UTF-8 and `lossy` yields lots of replacement characters:

* show “(binary)” in status, disable diff gutter (or treat as none)

### Submodule paths

If entry is a submodule commit (not blob), ignore diff.

---

## 7) Recommended small improvement to diff classification (new file heuristic)

Before calling `diff_gutter_from_text`:

```rust
if base_text.is_empty() && !current_text.is_empty() {
    return Ok(vec![DiffMark::Added; current.len_lines().max(1)]);
}
```

This makes new files look right.

---

## 8) Pane focus keys: make Ctrl+Arrow the *configured* primary, not the only one

Given tmux + kitty + random Linux key handling: I’d ship defaults:

* focus move: `Ctrl+h/j/k/l` **always enabled**
* accept `Ctrl+Arrow` when it arrives
* optionally accept `Alt+Arrow`

…and document that “Ctrl+Arrow depends on terminal/mux”.

---

## 9) Quick tests (worth writing immediately)

### TOC + selection tests: already in core

### Diff tests (core)

* base==current → all None
* insert block → marks Added in correct range
* replace line → mark Modified for that line
* delete line → DeletedAfter somewhere (if you implement it), or ignored

### Git base text tests (integration-ish)

* Create temp repo, commit a file, modify it, verify `read_head_file_text` returns committed content.

---

## 1) Diff worker: request/coalesce pattern (so UI stays snappy)

### Goals

* Diff computation can be “moderately expensive” (repo open, read blob, diff lines).
* You only want to do it on **load / reload / debounced file change**, not every tick.
* If multiple changes come quickly, compute **only the latest** for that document revision.
* Never apply diff results computed for an old revision.

### Message types

```rust
// mdx-tui/src/diff_worker.rs
use std::path::PathBuf;
use mdx_core::{DiffGutter};

pub struct DiffRequest {
    pub doc_id: u64,
    pub path: PathBuf,
    pub rev: u64,          // document revision at request time
    pub current_text: String, // snapshot string; avoids sharing Rope across threads
}

pub struct DiffResult {
    pub doc_id: u64,
    pub rev: u64,
    pub gutter: DiffGutter,
}
```

### Worker design: “latest-only per doc_id”

Maintain a `HashMap<DocId, DiffRequest>` inside the worker, and a loop that:

* pulls requests from channel
* overwrites any existing pending request for that doc_id
* computes diffs on the newest pending request(s)

There are two patterns:

#### Pattern A (simple): compute immediately on each recv, but overwrite queue

Good enough if diff is fast-ish.

#### Pattern B (better): batch/coalesce with small wait

Collect for ~50–100ms then compute only most recent.

I’d do B.

### Worker loop sketch (Pattern B)

```rust
use crossbeam_channel::{Receiver, Sender, select};
use std::{collections::HashMap, time::{Duration, Instant}};

pub fn run_diff_worker(
    rx: Receiver<DiffRequest>,
    tx: Sender<DiffResult>,
    provider: mdx_core::GixDiffProvider,
) {
    let mut pending: HashMap<u64, DiffRequest> = HashMap::new();

    loop {
        // Block for first request
        let first = match rx.recv() {
            Ok(r) => r,
            Err(_) => break,
        };
        pending.insert(first.doc_id, first);

        // Coalesce a short window
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(75) {
            match rx.try_recv() {
                Ok(r) => { pending.insert(r.doc_id, r); }
                Err(_) => break,
            }
        }

        // Compute for each pending request (could prioritize focused doc if you pass that)
        for (_, req) in pending.drain() {
            let gutter = match provider.compute_from_text(&req.path, &req.current_text) {
                Ok(g) => g,
                Err(_) => mdx_core::DiffGutter::empty(req.current_text.lines().count().max(1)),
            };

            let _ = tx.send(DiffResult { doc_id: req.doc_id, rev: req.rev, gutter });
        }
    }
}
```

### Provider shape: avoid Rope in worker

Make a provider function that accepts `current_text: &str` so the worker doesn’t need Rope.

In `mdx-core`:

```rust
impl GixDiffProvider {
    pub fn compute_from_text(&self, path: &Path, current_text: &str) -> anyhow::Result<DiffGutter> {
        // open repo for path
        // read base text from HEAD via gix
        // if base empty & current non-empty -> all Added
        // else diff_gutter_from_text(base, current)
    }
}
```

### UI integration: applying results safely

When you receive `DiffResult { doc_id, rev, gutter }`:

* Look up `DocState`
* If `doc.rev == rev`, apply it
* Else ignore (stale result)

```rust
if let Some(ds) = app.docs.docs.get_mut(&doc_id) {
    if ds.doc.rev == rev {
        ds.diff = Some(gutter);
    }
}
```

### When to send DiffRequest

* On initial file load
* On manual reload (`r`)
* On auto-reload completion after debounce
* (Optional) On theme toggle? No — diff is independent.

**Important:** Build `current_text` snapshot once per reload, not per request flood.

---

## 2) Render composition: gutter + cursor + selection highlight over tui-markdown output

You want:

* Markdown rendering from tui-markdown (nice formatting)
* Add a left gutter column for git diff
* Add cursor indicator (optional separate column)
* Apply selection highlight (Visual line range)
* Handle wrapping: gutter applies only to first wrapped line of a source line; continuation lines are blank gutter.

### Practical plan (works with tui-markdown)

1. Render Markdown into `Vec<ratatui::text::Line>` for the pane width (minus gutter columns).
2. Have a **line mapping**: `source_line -> first_rendered_line_index`.
3. Build a new `Vec<Line>` where each rendered line is prefixed with gutter and maybe cursor marker.
4. Apply highlight styles to lines that correspond to selected source lines or cursor line.

#### Key: you need a mapping

Because tui-markdown doesn’t guarantee source line mapping, start with a **best-effort mapper**:

**v1 mapping strategy (good enough, incremental):**

* Pre-scan the source file into `Vec<String>` source lines.
* Render **each source line separately** as Markdown inline (or as plain text) and wrap; but this loses cross-line Markdown constructs.
* Better: render the whole doc once, but also compute mapping by:

  * Counting how many “hard line breaks” exist in the source and mapping those to rendered “paragraph starts” is not reliable.

So for v1 *with real markdown constructs*, I recommend a compromise:

### v1 Mapping: “block-level + line anchor” (best compromise)

* Parse markdown into blocks with pulldown-cmark and track approximate source offsets:

  * For headings (ATX), code fences, blank-line separated paragraphs:

    * you can determine start line by scanning source around the block.
* Then you render per-block, and within block:

  * preserve original lines for code fences (exact mapping)
  * for paragraphs/lists: mapping is approximate but consistent

This is more work, but it’s the only way to get md-tui-level behavior *and* a usable gutter/selection.

**However**, you asked for a “sketch” now, so here’s the implementation shape that lets you start with approximate mapping and swap it later without rewriting UI:

---

### Renderer interface (so you can evolve mapping quality)

```rust
pub struct RenderInput<'a> {
    pub markdown: &'a str,
    pub width: u16,
    pub theme: ThemeVariant,
}

pub struct RenderOutput {
    pub lines: Vec<ratatui::text::Line<'static>>,
    pub source_first_rendered: Vec<usize>, // len = source_line_count, best-effort
    pub rendered_to_source: Vec<usize>,    // len = rendered lines, best-effort
}

pub trait MarkdownRenderer {
    fn render(&mut self, input: RenderInput) -> RenderOutput;
}
```

Have one implementation:

* `TuiMarkdownRenderer` (uses tui-markdown) producing `lines`
* For mapping in v1:

  * `source_first_rendered` can be a naive uniform mapping (each source line maps to nearest rendered line) initially
  * but the interface is ready for your better block renderer later

---

### Composition step: prefix gutter + cursor + selection

Assume:

* `rendered` is `RenderOutput`
* `diff_marks: Option<&[DiffMark]>` length = source_line_count
* `cursor_line` and `selection_range: Option<(usize, usize)>`
* `scroll_line` decides which portion to show (or rely on ratatui Paragraph scrolling)

#### Choose column widths

* diff gutter: 1 char + 1 space
* cursor marker: 1 char + 1 space (optional)
  Total left columns e.g. 4.

```rust
let diff_col_w = 2;   // "± "
let cur_col_w  = 2;   // "> "
let left_w = diff_col_w + cur_col_w;
let md_w = pane_width.saturating_sub(left_w as u16);
```

#### Apply to each rendered line

We need `rendered_to_source[i] -> src_line`.

```rust
fn prefix_lines(
    out: &RenderOutput,
    diff: Option<&[DiffMark]>,
    cursor_line: usize,
    selection: Option<(usize, usize)>,
    styles: &Theme,
) -> Vec<ratatui::text::Line<'static>> {
    use ratatui::text::{Line, Span};
    let mut new_lines = Vec::with_capacity(out.lines.len());

    for (i, line) in out.lines.iter().enumerate() {
        let src = out.rendered_to_source.get(i).copied().unwrap_or(0);

        // Only show diff mark on first rendered line of that source line.
        // If you have source_first_rendered[src] == i -> first line.
        let is_first = out.source_first_rendered.get(src).copied() == Some(i);

        let diff_ch = if is_first {
            match diff.and_then(|d| d.get(src)).copied().unwrap_or(DiffMark::None) {
                DiffMark::None => ' ',
                DiffMark::Added => '+',
                DiffMark::Modified => '~',
                DiffMark::DeletedAfter(_) => '▾',
            }
        } else { ' ' };

        let cur_ch = if src == cursor_line && is_first { '>' } else { ' ' };

        // Selection highlight: linewise by source line
        let sel = selection.map(|(a,b)| src >= a && src <= b).unwrap_or(false);

        // Start building spans
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled(format!("{} ", diff_ch), styles.gutter_style(diff_ch)));
        spans.push(Span::styled(format!("{} ", cur_ch), styles.cursor_style(cur_ch)));

        // Clone the markdown-rendered line spans
        // Then overlay selection by applying background style to all spans in line.
        if sel {
            for s in line.spans.iter() {
                spans.push(Span::styled(
                    s.content.clone(),
                    s.style.patch(styles.selection_bg),
                ));
            }
        } else {
            spans.extend(line.spans.iter().cloned());
        }

        new_lines.push(Line::from(spans));
    }

    new_lines
}
```

**Note:** `Span<'static>` requires owned strings. If your renderer produces `Line<'static>` already, you can clone spans safely. Otherwise store owned content.

---

### Scrolling

Two options:

**Option A:** Let ratatui `Paragraph` handle scrolling.

* You render all lines into a `Text`, set `.scroll((scroll as u16, 0))`.
* Cursor movement updates `scroll_line`.

**Option B:** Slice visible lines yourself for speed.

* Compute visible height
* Render only `[scroll..scroll+height]`
* This reduces allocations and is good for huge docs.

Given you’ll cache rendered lines anyway, slicing is great.

---

## Putting it together: end-to-end per pane draw sketch

1. Get `DocState` and current `Document`.
2. Get cached `RenderOutput` for `(doc.rev, md_w, theme)`.
3. Compose prefixed lines with diff/cursor/selection → `Vec<Line>`.
4. Slice by scroll → visible `Text`.
5. Render with `Paragraph`.

---

## Implementation advice: make “mapping quality” a second-pass

You can ship v1 with:

* **exact mapping for code fences** (easy)
* approximate mapping elsewhere (cursor/selection still usable)
* diff gutter is still useful even with approximate wrapping behavior because you’ll mark only first lines.

Then iterate:

* implement a “block renderer” that:

  * parses markdown into blocks with known start source lines
  * wraps text inside blocks
  * yields reliable `rendered_to_source`

That’s how you get the md-tui vibe and correct gutter/selection.

---

## Quick config note for pane focus keys

Even if you keep `Ctrl+Arrow` as *documented*, enable `Ctrl+h/j/k/l` by default. Users in tmux will thank you.

---

Alright — here’s a solid **block renderer** plan that gets you to **md-tui-level rendering + reliable source↔render mapping**, while still integrating cleanly with ratatui.

The key idea: **don’t rely on tui-markdown as the final renderer** if you need a gutter + selection + accurate mapping. Instead, use `pulldown-cmark` to build a lightweight block model, then render blocks into ratatui `Line`s with a wrapping engine that you control. You can still **borrow styling choices** and feature parity from md-tui (headings, lists, quotes, code fences, tables) and you can keep `tui-markdown` as a fallback renderer initially.

---

## 1) Architecture: two renderers behind one trait

```rust
pub trait MarkdownRenderer {
    fn render(&mut self, input: RenderInput) -> RenderOutput;
}

pub struct TuiMarkdownRenderer { /* quick fallback */ }
pub struct BlockMarkdownRenderer { /* mdx “real” renderer */ }
```

Start with `TuiMarkdownRenderer` to get something working, but aim to ship `BlockMarkdownRenderer` as default once it supports the essentials.

---

## 2) Block model (minimal AST)

You do **not** need a full Markdown AST. You need blocks that:

* render nicely
* wrap predictably
* preserve indent/continuations
* expose a **start source line** (and ideally per-line mapping for code blocks)

### Core block types

```rust
pub enum Block {
    Heading { level: u8, inlines: Vec<Inline>, src_line: usize },
    Paragraph { inlines: Vec<Inline>, src_line: usize },
    List { items: Vec<ListItem>, ordered: bool, start: u64, src_line: usize },
    Quote { blocks: Vec<Block>, src_line: usize },
    CodeFence { lang: Option<String>, lines: Vec<String>, src_line: usize },
    ThematicBreak { src_line: usize },
    Table { head: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>>, src_line: usize }, // optional v1.5
    Blank { src_line: usize }, // or just handle spacing between blocks
}

pub struct ListItem {
    pub blocks: Vec<Block>,
    pub src_line: usize,
}

pub enum Inline {
    Text(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Code(String),
    Link { text: Vec<Inline>, url: String },
    SoftBreak, // wrap opportunity
    HardBreak,
}
```

---

## 3) Parsing strategy: pulldown-cmark + “source line mapping”

### Hard part: source line numbers

`pulldown-cmark` does not reliably give you per-event line numbers. So you create them by combining:

1. **A pre-scan pass over the raw source** to build a `Vec<LineKind>` and detect:

   * headings (ATX + Setext)
   * fences (``` / ~~~)
   * list markers (`-`, `*`, `+`, `1.`)
   * blockquote markers (`>`)
   * thematic breaks (`---`, `***`)
   * blank lines
2. Parse structure using those hints + pulldown-cmark for inline formatting inside text spans.

This is exactly what many terminal renderers do: use simple line heuristics for blocks; use proper markdown parser for inlines.

### Why this is good

* You get **exact `src_line`** for block starts.
* Code fences become trivial and exact (every fence line maps to that source line).
* Lists/quotes have predictable indentation behavior.

### “Pre-scan” result

```rust
enum LineKind {
    Blank,
    AtxHeading{ level: u8, text: String },
    SetextCandidate{ text: String },
    SetextUnderline{ level: u8 }, // '=' -> 1, '-' -> 2
    FenceStart{ fence: String, lang: Option<String> },
    FenceEnd{ fence: String },
    ListItem{ ordered: bool, marker_len: usize, indent: usize, text: String },
    Quote{ indent: usize, text: String },
    Hr,
    Text{ text: String },
}
```

Then build blocks in a line-driven parser:

* read headings
* read fences
* accumulate paragraphs until blank line or new block starts
* read lists by collecting consecutive ListItem lines (respect indent)
* read quotes by stripping leading `>` and recursively parsing inner lines into blocks

For each “paragraph” or “list item body”, run pulldown-cmark *inline parsing* on that text only, to get `Inline` nodes.

---

## 4) Wrapping engine with hanging indents (critical for md-tui quality)

You need wrapping that:

* respects word boundaries
* preserves indentation
* supports “prefix” spans (list bullets, quote `│`, etc.)
* produces **rendered_to_source** mapping

### Wrap API

```rust
pub struct WrapSpec<'a> {
    pub width: u16,
    pub first_prefix: &'a str,  // e.g. "- " or "1. "
    pub cont_prefix: &'a str,   // e.g. "  " or "   "
    pub src_line: usize,        // source line that this wrapped text belongs to (block start)
}
pub struct WrappedLines {
    pub lines: Vec<ratatui::text::Line<'static>>,
    pub rendered_to_source: Vec<usize>,
    pub source_first_rendered: Option<usize>, // set by caller
}
```

For paragraphs/lists/quotes:

* wrap tokens (words + spaces) into lines <= width
* on first line use `first_prefix`, subsequent use `cont_prefix`
* `rendered_to_source.push(src_line)` for each produced line

### Inline tokenization

Flatten `Inline` tree into a stream of styled “atoms”:

```rust
struct Atom {
    text: String,
    style: Style,
    breakable: bool, // can split here
}
```

Rules:

* `Text`: split into words/spaces atoms (spaces breakable)
* `Emph/Strong`: apply style patches
* `Code`: treat as unbreakable chunk unless too long; if too long, hard-wrap
* `Link`: style + maybe underline; keep URL for later “open link under cursor”

Then the wrapper packs atoms into lines by measuring `unicode_width::UnicodeWidthStr`.

---

## 5) Rendering blocks to ratatui lines + mapping

### Renderer output

```rust
pub struct RenderOutput {
    pub lines: Vec<Line<'static>>,
    pub rendered_to_source: Vec<usize>,     // for each rendered line, which src line it belongs to
    pub source_first_rendered: Vec<usize>,  // for each source line, index of first rendered line (usize::MAX if none)
}
```

### Rendering rules (v1)

* **Heading**

  * one line, no wrap unless extremely long (wrap ok)
  * add blank line after (configurable)
* **Paragraph**

  * wrap with no prefix
* **List**

  * for each item:

    * first line prefix: `"- "` or `"N. "`
    * continuation prefix: `"  "` or width-aligned spaces
    * nested blocks inside items: render with extra indent
* **Quote**

  * prefix every line with `"> "` or a vertical bar `│ `
  * nested blocks render inside quote with reduced width
* **CodeFence**

  * render as:

    * optional “```lang” header line style (or just a top border)
    * each code line exactly, no wrapping by default (config option `code_wrap`)
  * mapping:

    * each rendered code line maps to the exact source line (src_line + offset)
* **HR**

  * render `────────` (fit width)
* **Tables**

  * optional; if you implement: compute column widths, render with box chars
  * mapping can map each table row to approximate src_line (or keep table start line)

### Mapping construction

As you push each rendered line:

* `rendered_to_source.push(src_line_of_this_rendered_line)`
* If `source_first_rendered[src]` not set, set it to this new index

For code fences:

* `src_line + i` (exact)

For wrapped paragraphs/lists:

* all produced lines map to the block’s `src_line` initially (good enough for selection/cursor)
* if you want more exact, map to the nearest contributing source line (later improvement)

---

## 6) Gutter + cursor + selection become straightforward

Once you have:

* `rendered_to_source[i] -> src_line`
* `source_first_rendered[src] -> first rendered index`

then:

* diff gutter mark only when `i == source_first_rendered[src]`
* cursor mark same condition
* selection highlight: `src_line in [a..b]` applies to all rendered lines belonging to those lines (or only first lines if you prefer)

This gives you **stable behavior even with wrapping**.

---

## 7) Tables of contents in this world

TOC extraction becomes trivial in block parser:

* whenever you produce `Block::Heading { level, text, src_line }`, push a TOC entry
* jump-to-heading just sets `cursor_line = src_line` and scroll to the first rendered line: `source_first_rendered[src_line]`

No heuristics, no regex after the fact.

---

## 8) Incremental delivery plan (so you don’t boil the ocean)

### Phase 1 (ship quickly)

* Implement Block parser for:

  * headings (ATX + Setext)
  * fenced code blocks
  * paragraphs
  * blank lines separation
* Inline parsing:

  * support `Text`, `Emph`, `Strong`, `Code`, `Link` (pulldown-cmark for inline-only)
* Wrapping engine
* Mapping output
* Use this renderer for main pane; keep tui-markdown behind feature flag or fallback.

### Phase 2 (md-tui feel)

* Lists (unordered/ordered) with hanging indent
* Blockquotes
* HR
* Better spacing rules (tight/loose lists)

### Phase 3 (polish)

* Tables (basic)
* Link navigation (open under cursor)
* Search highlighting

---

## 9) Implementation sketches (core functions)

### Parse blocks from lines

```rust
pub fn parse_blocks(lines: &[String]) -> Vec<Block> {
    // iterate with index i
    // detect block starts by LineKind from pre-scan
    // accumulate paragraphs
    // recurse for quotes/lists
}
```

### Inline parse with pulldown-cmark (inline-only)

You can feed a paragraph string and interpret events:

* `Event::Text`, `Event::Code`, `Event::Start(Tag::Emphasis)`, etc.

```rust
pub fn parse_inlines(text: &str) -> Vec<Inline> {
    // pulldown_cmark::Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH | ...)
}
```

### Wrap atoms

```rust
pub fn wrap_atoms(atoms: &[Atom], spec: WrapSpec) -> WrappedLines {
    // measure widths and pack
}
```

### Render blocks

```rust
pub fn render_blocks(blocks: &[Block], width: u16, theme: &Theme) -> RenderOutput {
    let mut out = RenderOutput::new(source_line_count);
    for b in blocks { render_block(&mut out, b, width, theme, indent=0); }
    out
}
```

---

## 10) Two “gotchas” to plan for

1. **Indent and width accounting must include prefixes**
   If prefix is `"- "` and width=80, your wrap width for content is `78`.

2. **Unicode width**
   Use `unicode_width` for correct layout; emoji and wide chars matter.

---

## 11) How this integrates with the rest of mdx

* Document load:

  * read rope
  * build `Vec<String>` lines (or iterate rope lines)
  * parse blocks + toc
* Render cache:

  * key `(doc.rev, width, theme)` → `RenderOutput` (lines + mapping)
* UI:

  * compose gutter/cursor/selection prefixes on top (or bake gutter directly into render stage)

---

Great — this is the **“make it feel right” layer**. Below is a **concrete, line-by-line pre-scan design** and a **robust inline atomization pipeline** that together give you:

* predictable block detection
* exact source-line anchors
* md-tui-level wrapping quality
* clean integration with ratatui styles

I’ll keep it implementation-shaped, not hand-wavy.

---

# PART A — Line Pre-Scan (Block Detection Backbone)

## A1) Pre-scan goals

The pre-scan answers **only one question**:

> “What *kind* of thing does this source line start?”

It must be:

* **fast**
* **deterministic**
* **line-number exact**

It does **not** try to understand inline formatting.

---

## A2) LineKind enum (final)

````rust
#[derive(Debug, Clone)]
pub enum LineKind<'a> {
    Blank,

    AtxHeading {
        level: u8,          // 1..=6
        text: &'a str,      // trimmed, no leading #'s
    },

    SetextCandidate {
        text: &'a str,
    },
    SetextUnderline {
        level: u8,          // 1 or 2
    },

    FenceStart {
        fence: &'a str,     // ``` or ~~~
        lang: Option<&'a str>,
    },
    FenceEnd {
        fence: &'a str,
    },

    ListItem {
        ordered: bool,
        marker_len: usize,  // length of "- " or "10. "
        indent: usize,      // leading spaces
        text: &'a str,
    },

    Quote {
        indent: usize,
        text: &'a str,
    },

    Hr,

    Text {
        text: &'a str,
    },
}
````

---

## A3) Pre-scan algorithm (single pass)

```rust
pub fn prescan_lines<'a>(lines: &'a [String]) -> Vec<LineKind<'a>> {
    let mut out = Vec::with_capacity(lines.len());

    for line in lines {
        let raw = line.as_str();

        // 1. Blank
        if raw.trim().is_empty() {
            out.push(LineKind::Blank);
            continue;
        }

        let indent = raw.chars().take_while(|c| *c == ' ').count();
        let rest = &raw[indent..];

        // 2. ATX heading
        if let Some((level, text)) = parse_atx_heading(rest) {
            out.push(LineKind::AtxHeading { level, text });
            continue;
        }

        // 3. Fence start/end
        if let Some((fence, lang)) = parse_fence(rest) {
            out.push(LineKind::FenceStart { fence, lang });
            continue;
        }
        if let Some(fence) = parse_fence_end(rest) {
            out.push(LineKind::FenceEnd { fence });
            continue;
        }

        // 4. HR
        if is_hr(rest) {
            out.push(LineKind::Hr);
            continue;
        }

        // 5. Blockquote
        if let Some(text) = rest.strip_prefix('>') {
            out.push(LineKind::Quote {
                indent,
                text: text.trim_start(),
            });
            continue;
        }

        // 6. List item
        if let Some((ordered, marker_len, text)) = parse_list_item(rest) {
            out.push(LineKind::ListItem {
                ordered,
                marker_len,
                indent,
                text,
            });
            continue;
        }

        // 7. Setext handling
        if is_setext_underline(rest) {
            out.push(LineKind::SetextUnderline {
                level: if rest.starts_with('=') { 1 } else { 2 },
            });
            continue;
        }

        // 8. Fallback
        out.push(LineKind::Text { text: rest });
    }

    out
}
```

---

## A4) Regex-free helpers (important for speed)

### ATX headings

```rust
fn parse_atx_heading(s: &str) -> Option<(u8, &str)> {
    let hashes = s.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = s[hashes..].trim_start();
    Some((hashes as u8, rest))
}
```

### Fenced code

````rust
fn parse_fence(s: &str) -> Option<(&str, Option<&str>)> {
    if s.starts_with("```") || s.starts_with("~~~") {
        let fence = &s[..3];
        let lang = s[3..].trim();
        return Some((fence, if lang.is_empty() { None } else { Some(lang) }));
    }
    None
}

fn parse_fence_end(s: &str) -> Option<&str> {
    if s.starts_with("```") || s.starts_with("~~~") {
        Some(&s[..3])
    } else {
        None
    }
}
````

### Lists

```rust
fn parse_list_item(s: &str) -> Option<(bool, usize, &str)> {
    if let Some(rest) = s.strip_prefix("- ") {
        return Some((false, 2, rest));
    }
    if let Some(rest) = s.strip_prefix("* ") {
        return Some((false, 2, rest));
    }
    if let Some(rest) = s.strip_prefix("+ ") {
        return Some((false, 2, rest));
    }

    // ordered: 1. 2. 10.
    let mut chars = s.chars();
    let mut digits = 0;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            digits += 1;
        } else if c == '.' && digits > 0 {
            if chars.next() == Some(' ') {
                let marker_len = digits + 2;
                let text = &s[marker_len..];
                return Some((true, marker_len, text));
            }
            break;
        } else {
            break;
        }
    }
    None
}
```

### HR

```rust
fn is_hr(s: &str) -> bool {
    let t = s.trim();
    t.len() >= 3 && (t.chars().all(|c| c == '-') ||
                     t.chars().all(|c| c == '*') ||
                     t.chars().all(|c| c == '_'))
}
```

---

## A5) Building blocks from LineKind

Now consume `Vec<LineKind>` into `Vec<Block>`:

### Key rules

* Paragraph = consecutive `Text`
* Setext heading = `Text` line followed by `SetextUnderline`
* Fence = everything between `FenceStart` and matching `FenceEnd`
* List = consecutive `ListItem` with same indent
* Quote = strip `>` and recursively parse inner lines

This is deterministic and stable.

---

# PART B — Inline Atomization (The Other Half)

This is what makes wrapping **not suck**.

---

## B1) Inline parse strategy

Use **pulldown-cmark only for inline parsing**, not blocks.

```rust
use pulldown_cmark::{Parser, Event, Tag, Options};
```

Input = a single paragraph/list item text
Output = `Vec<Inline>`

---

## B2) Inline enum (final)

```rust
pub enum Inline {
    Text(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Code(String),
    Link { text: Vec<Inline>, url: String },
    SoftBreak,
    HardBreak,
}
```

---

## B3) Inline parse implementation (stack-based)

```rust
pub fn parse_inlines(text: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut stack: Vec<Vec<Inline>> = Vec::new();

    let parser = Parser::new_ext(
        text,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );

    for ev in parser {
        match ev {
            Event::Text(t) => push(&mut stack, &mut out, Inline::Text(t.to_string())),
            Event::Code(t) => push(&mut stack, &mut out, Inline::Code(t.to_string())),

            Event::SoftBreak => push(&mut stack, &mut out, Inline::SoftBreak),
            Event::HardBreak => push(&mut stack, &mut out, Inline::HardBreak),

            Event::Start(Tag::Emphasis) => stack.push(Vec::new()),
            Event::End(Tag::Emphasis) => {
                let inner = stack.pop().unwrap();
                push(&mut stack, &mut out, Inline::Emph(inner));
            }

            Event::Start(Tag::Strong) => stack.push(Vec::new()),
            Event::End(Tag::Strong) => {
                let inner = stack.pop().unwrap();
                push(&mut stack, &mut out, Inline::Strong(inner));
            }

            Event::Start(Tag::Link(_, url, _)) => {
                stack.push(Vec::new());
                stack.push(vec![Inline::Text(url.to_string())]); // stash url
            }
            Event::End(Tag::Link(..)) => {
                let url = match stack.pop().unwrap().pop().unwrap() {
                    Inline::Text(u) => u,
                    _ => unreachable!(),
                };
                let text = stack.pop().unwrap();
                push(&mut stack, &mut out, Inline::Link { text, url });
            }

            _ => {}
        }
    }

    out
}

fn push(stack: &mut Vec<Vec<Inline>>, out: &mut Vec<Inline>, node: Inline) {
    if let Some(top) = stack.last_mut() {
        top.push(node);
    } else {
        out.push(node);
    }
}
```

---

## B4) Atomization: Inline → Atoms

Atoms are the **wrap units**.

```rust
pub struct Atom {
    pub text: String,
    pub style: Style,
    pub breakable: bool,
}
```

### Atomization rules

```rust
pub fn inline_to_atoms(inl: &Inline, theme: &Theme, atoms: &mut Vec<Atom>) {
    match inl {
        Inline::Text(t) => split_text_atoms(t, theme.base, atoms),

        Inline::Code(t) => atoms.push(Atom {
            text: t.clone(),
            style: theme.code,
            breakable: false,
        }),

        Inline::Emph(inner) => {
            let style = theme.emph;
            for i in inner {
                inline_to_atoms(i, theme, atoms);
                atoms.last_mut().unwrap().style = style;
            }
        }

        Inline::Strong(inner) => {
            let style = theme.strong;
            for i in inner {
                inline_to_atoms(i, theme, atoms);
                atoms.last_mut().unwrap().style = style;
            }
        }

        Inline::Link { text, .. } => {
            for i in text {
                inline_to_atoms(i, theme, atoms);
                atoms.last_mut().unwrap().style = theme.link;
            }
        }

        Inline::SoftBreak => atoms.push(Atom {
            text: " ".into(),
            style: theme.base,
            breakable: true,
        }),

        Inline::HardBreak => atoms.push(Atom {
            text: "\n".into(),
            style: theme.base,
            breakable: false,
        }),
    }
}
```

### Text splitting (important)

Split on **grapheme boundaries**, not bytes.

```rust
fn split_text_atoms(text: &str, style: Style, atoms: &mut Vec<Atom>) {
    for part in text.split_inclusive(' ') {
        atoms.push(Atom {
            text: part.to_string(),
            style,
            breakable: true,
        });
    }
}
```

---

## B5) Wrapping atoms into lines (core loop)

This is the heart of md-tui feel.

```rust
pub fn wrap_atoms(
    atoms: &[Atom],
    spec: &WrapSpec,
) -> WrappedLines {
    let mut lines = Vec::new();
    let mut rendered_to_source = Vec::new();

    let mut cur = Vec::new();
    let mut cur_w = 0;
    let mut first = true;

    let content_w = spec.width as usize
        - spec.first_prefix.len();

    for atom in atoms {
        let w = unicode_width::UnicodeWidthStr::width(atom.text.as_str());

        if cur_w + w > content_w && !cur.is_empty() && atom.breakable {
            // flush line
            let prefix = if first { spec.first_prefix } else { spec.cont_prefix };
            lines.push(make_line(prefix, &cur));
            rendered_to_source.push(spec.src_line);

            cur.clear();
            cur_w = 0;
            first = false;
        }

        cur.push(atom.clone());
        cur_w += w;
    }

    if !cur.is_empty() {
        let prefix = if first { spec.first_prefix } else { spec.cont_prefix };
        lines.push(make_line(prefix, &cur));
        rendered_to_source.push(spec.src_line);
    }

    WrappedLines { lines, rendered_to_source, source_first_rendered: None }
}
```

---

# Why this works (and why mdx will feel *right*)

* **Exact source anchors** → TOC, cursor, selection, git gutter all line up
* **Predictable wrapping** → lists, quotes, paragraphs behave like real editors
* **Block-level rendering** → no surprises when resizing panes
* **Incremental** → you can ship without tables, add them later

This is the architecture used (in spirit) by **md-tui**, **glow**, and several pager-style tools — but tailored to your **vim-centric, multi-pane, git-aware** TUI.

---

## 1) Exact list indentation + nested list parsing rules

Markdown lists are where terminal renderers usually fall apart. The trick is to treat lists as an **indent-sensitive line grammar**, not as “just parse paragraphs”.

### 1.1 What “exact” means for mdx

For each list item you want:

* correct bullet/number prefix (`- `, `* `, `10. `)
* **hanging indent** for wrapped lines
* continuation lines belonging to the item body (indented or blank)
* nested lists when the indentation increases enough
* loose vs tight spacing (blank lines between items)

### 1.2 Represent list markers precisely

Extend your `ListItem` to store:

* `indent` = leading spaces before marker
* `marker_len` = length of marker including trailing space
* `content_indent` = `indent + marker_len` (where item content begins visually)

```rust
pub struct ListItem {
    pub indent: usize,
    pub marker_len: usize,
    pub content_indent: usize,
    pub src_line: usize,
    pub blocks: Vec<Block>,
}
```

### 1.3 List parsing state machine (line-driven)

Assume you already have `LineKind::ListItem { ordered, marker_len, indent, text }` from prescan.

Algorithm overview:

1. When you see a `ListItem` line, start a list block at that indentation.
2. Consume subsequent lines while they belong to this list (either new items at same indent, or continuation lines indented >= content_indent, or blank lines inside a loose list).
3. For each item:

   * the first line supplies the initial text (`text`)
   * subsequent continuation lines:

     * if blank → may separate paragraphs within item (loose list)
     * if indented enough → append to item buffer
     * if looks like nested list marker at deeper indent → parse nested list recursively

#### Key rule: what counts as “continuation”?

For an item with `content_indent`:

* A following line belongs to the item if:

  * it is blank, OR
  * its indent >= `content_indent`, OR
  * it’s a quote line beginning with `>` that is indented enough (treat after stripping), OR
  * it’s a fenced code block start that appears at indent >= `content_indent`

If a line has indent < `indent` of the list, the list ends.

### 1.4 Concrete pseudo-code: parse_list_at_indent

```rust
fn parse_list(lines: &[String], kinds: &[LineKind], i: &mut usize) -> Block {
    // precondition: kinds[*i] is ListItem
    let (ordered, base_indent) = match &kinds[*i] {
        LineKind::ListItem { ordered, indent, .. } => (*ordered, *indent),
        _ => unreachable!(),
    };

    let mut items = Vec::new();
    let mut start_num = 1u64;

    while *i < lines.len() {
        match &kinds[*i] {
            LineKind::ListItem { ordered: o, indent, marker_len, text } if *indent == base_indent && *o == ordered => {
                let src_line = *i;
                let marker_len = *marker_len;
                let content_indent = base_indent + marker_len;

                // Gather raw item lines (first line + continuations)
                let mut raw_item_lines: Vec<(usize, String)> = Vec::new();
                raw_item_lines.push((src_line, text.to_string()));
                *i += 1;

                while *i < lines.len() && line_belongs_to_item(&kinds[*i], base_indent, content_indent) {
                    // Nested list?
                    if let LineKind::ListItem { indent: li_indent, .. } = &kinds[*i] {
                        if *li_indent >= content_indent {
                            // Parse nested list recursively
                            // But only if it’s clearly nested, not same indent
                            // We'll break and let item-body parser handle recursion.
                        }
                    }

                    raw_item_lines.push((*i, extract_item_continuation(&lines[*i], content_indent)));
                    *i += 1;
                }

                // Parse item body blocks from raw_item_lines (re-scan them)
                let blocks = parse_blocks_from_item_lines(&raw_item_lines);

                items.push(ListItem { indent: base_indent, marker_len, content_indent, src_line, blocks });
            }
            _ => break, // list ended or different indent/list type
        }
    }

    Block::List { items, ordered, start: start_num, src_line: items.first().map(|it| it.src_line).unwrap_or(0) }
}
```

### 1.5 Helpers you need

**Belongs-to-item check**

```rust
fn line_belongs_to_item(kind: &LineKind, list_indent: usize, content_indent: usize) -> bool {
    match kind {
        LineKind::Blank => true,
        LineKind::FenceStart { .. } => true, // if actual indent in raw line >= content_indent (track indent separately)
        LineKind::Quote { indent, .. } => *indent >= content_indent,
        LineKind::Text { .. } => true, // again needs actual indent check from raw line
        LineKind::ListItem { indent, .. } => *indent >= content_indent, // nested list
        _ => true,
    }
}
```

**Extract continuation line content**
For continuation lines, strip `content_indent` spaces (or as many as exist), preserving relative indent inside code blocks.

```rust
fn extract_item_continuation(raw_line: &str, content_indent: usize) -> String {
    let n = raw_line.chars().take_while(|c| *c == ' ').count();
    let cut = n.min(content_indent);
    raw_line[cut..].to_string()
}
```

### 1.6 Rendering lists correctly (hanging indent)

When rendering each item:

* prefix first line with bullet (e.g. `"- "` or `"10. "`)
* continuation prefix should be exactly `marker_len` spaces (or `marker_len` + maybe one extra)
* plus any additional indent for nested list depth

Example:

* For `- item text ...`:

  * first_prefix: `"- "`
  * cont_prefix: `"  "`

* For `10. item` (`marker_len = 4`):

  * first_prefix: `"10. "`
  * cont_prefix: `"    "` (4 spaces)

Nested list: add leading indent spaces before both prefixes.

### 1.7 Tight vs loose lists

Loose lists (blank line between items or within item) should render with blank line spacing between blocks inside the item.

Simple rule:

* If an item’s raw buffer contains a blank line, treat item as “loose”.
* Render paragraphs in item separated by blank lines.

This matches common renderers and feels right in terminal.

---

## 2) Search (`/`) + highlight integration using renderer + mapping

### 2.1 Requirements

* `/` enters search mode; user types query
* `Enter` confirms; `Esc` cancels
* highlight all matches in viewport (or whole doc if cached)
* `n` next match; `N` previous match
* keep match state per pane (cursor/scroll differs per pane)

### 2.2 Data model additions

```rust
pub enum Mode {
    Normal,
    VisualLine,
    SearchInput,
}

pub struct SearchState {
    pub query: String,
    pub matches: Vec<Match>, // sorted by (src_line, column)
    pub active: usize,       // index into matches
    pub case_sensitive: bool,
}

pub struct Match {
    pub src_line: usize,
    pub start_col: usize, // column in source line (UTF-8 byte index or char index, choose one)
    pub len: usize,
}
```

Add to `ViewState`:

```rust
pub struct ViewState {
    pub scroll_line: usize,
    pub cursor_line: usize,
    pub mode: Mode,
    pub selection: Option<LineSelection>,
    pub search: Option<SearchState>,
}
```

### 2.3 Match computation: search in source lines

Do not search the rendered text — it changes with wrapping and styling.
Search in **source lines** from the rope.

Implementation plan:

1. On query commit:

   * iterate over source lines (rope lines)
   * find occurrences (simple substring for v1; `regex` optional later)
   * record matches with `src_line`, `start_col` (character index), `len` (chars)
2. Store matches in `SearchState`.

Case sensitivity:

* default insensitive unless query contains uppercase (vim-like smartcase), configurable

### 2.4 Jumping to match (n/N)

When user presses `n`:

* find next match whose `src_line >= cursor_line`, else wrap
* set `cursor_line = match.src_line`
* adjust scroll so that `cursor_line` is visible
* optionally set an “active match” index

### 2.5 Highlighting matches in rendered output

You already have `rendered_to_source[i] -> src_line`.
So per rendered line:

* determine src_line
* get list of matches for that src_line
* apply highlight spans to corresponding text segments

**But:** rendered lines may contain prefixes (quote/list) and wrapping changes column positions.

So do this in stages:

#### Stage A (v1): line-level highlighting (simple)

Highlight whole rendered lines that contain any match on that source line.

* It’s not perfect, but it’s fast and works.

#### Stage B (v2): segment highlighting (precise)

To highlight just the matched substring, you need mapping:

* from source column indices to rendered column segments
  This is hard if you let wrapping reorder text, but with your **atom-based renderer**, it’s feasible:

Because you render from atoms with known text, you can:

* while wrapping atoms into lines, keep a parallel vector of “source span ranges” for each emitted span.

##### Add span mapping during wrap

Extend Atom:

```rust
pub struct Atom {
    pub text: String,
    pub style: Style,
    pub breakable: bool,
    pub src_col_start: Option<usize>, // for paragraph’s source line
    pub src_col_end: Option<usize>,
}
```

When you build atoms from source text, you know which substring they came from, so you can store column ranges.

Then when you emit spans into a line, you can highlight only atoms whose [start,end] intersects match.

This yields precise highlighting even with wrapping.

### 2.6 Input handling for Search mode

* In Normal:

  * `/` → enter SearchInput, clear temporary buffer
* In SearchInput:

  * `Char(c)` append
  * `Backspace` remove
  * `Enter` commit: compute matches
  * `Esc` cancel

Status bar shows: `/query…`

### 2.7 Performance

Match computation is O(total text) per commit. That’s fine.
For very large docs, you can:

* compute matches in a worker thread
* but v1 can be synchronous.

Highlighting:

* For line-level highlighting, just check `matches_by_line[src_line].is_empty()`
* For segment-level, it’s still linear in spans visible.

### 2.8 Data structure for fast per-line queries

Precompute:

```rust
pub matches_by_line: Vec<Vec<Match>> // index by src_line
```

Then highlight check is O(#matches on that line).

---

## Integration checklist (so both features work together)

### Lists

* Parser: nested list recursion + item continuation
* Renderer: hanging indents via prefix/cont_prefix
* Mapping: list item rendered lines map to item src_line (good), code lines map exactly

### Search

* Search matches computed against rope lines
* Highlight uses rendered_to_source (line-level v1)
* `n/N` jumps update cursor_line and scroll based on `source_first_rendered`

---

Below are the **nested quote/list recursion rules** you want, with the specific ambiguity-breakers that prevent “markdown feels random” bugs.

The key principle is:

> Parse **structural blocks** by indentation + sentinel markers, and when in doubt, prefer **continuation** over **new block**, except when a marker appears at an indentation that clearly establishes a new nesting level.

---

## 1) Track indentation explicitly (don’t infer from LineKind)

During pre-scan, store per line:

* `raw: &str`
* `indent: usize` (leading spaces)
* `kind: LineKind`
* (optional) `tabs_present: bool` and normalize tabs to 4 spaces for parsing

```rust
pub struct ScannedLine<'a> {
    pub raw: &'a str,
    pub indent: usize,
    pub kind: LineKind<'a>,
}
```

This is important because e.g. `LineKind::Text` by itself doesn’t say whether it’s indented enough to be a list continuation.

---

## 2) Canonical nesting model

Define a “current container” with a required content indent:

* **List item container**: `content_indent = item.indent + item.marker_len`
* **Quote container**: content indent is usually `quote.indent + 2` (because `> `), but allow `>` with no following space too.

We parse recursively by passing an `Env`:

```rust
pub struct Env {
    pub min_indent: usize,       // lines with indent < min_indent end this container
    pub quote_prefix: usize,     // if inside quote, how many chars were stripped ("?>" + space)
}
```

---

## 3) Belongs-to-container rules (the crux)

### 3.1 Inside a list item (continuation rules)

Given:

* list item `indent = li`
* item `content_indent = ci`

A line belongs to the **current item body** if any of these hold:

1. `Blank` line
2. `indent >= ci` (indented enough to be item content)
3. It’s a blockquote marker at indent >= ci **OR** it’s `>` immediately after ci spaces
4. It’s a fenced code start and either:

   * indent >= ci, OR
   * the fence is at column 0 but we’re already inside a fenced block (special case)
5. It’s a nested list marker with indent >= ci

A line belongs to the list (not necessarily same item) if:

* it’s a new list item with indent == li (same list level)

A line ends the list if:

* indent < li and it’s not blank (blank can appear between blocks outside list—commonmark nuance; for mdx keep it simple: blank ends loose list only if followed by non-list/non-continuation)

### 3.2 Inside a quote (quote continuation rules)

A line belongs to the quote container if:

* it begins with optional indentation (>= quote base indent) and then `>` marker
* OR it’s blank (blank lines are allowed inside quote)

If a non-blank line does not begin with `>` at the required indentation, quote ends.

Important: quotes can be nested: `>>` or `> >`. Treat each `>` as nesting level only if there’s a second `>` after trimming one `>`.

---

## 4) Ambiguity breakers (what prevents weird nesting)

These are the “if you do nothing else, do this” rules.

### Rule A — Nested list only if indent crosses a threshold

A list item line at indent `nli` is nested under current item only if:

* `nli >= ci` **AND**
* `nli >= li + 2` (at least 2 spaces deeper than parent list indent)

Why: avoids accidental nesting from aligned text.

### Rule B — A quote inside a list item is content if it’s indented to item content

If `indent >= ci` and line (after stripping indent) starts with `>` → parse quote as a nested block in the item.

If `indent < ci` but starts with `>`:

* treat as a quote that **ends the list** (common in markdown when quote is not indented)

### Rule C — Code fences inside lists: fence line must be indented to content

If fence starts at indent >= ci: it is inside the item.
Otherwise fence starts outside and ends the list.

### Rule D — Prefer continuation lines to starting a new paragraph when inside list items

If you have a `Text` line with indent >= ci, it’s continuation text of the current paragraph unless:

* there’s a blank line separating paragraphs (loose list)
* or the line starts a nested block marker at deeper indent (nested list/quote/fence/hr)

This matches how most people expect lists to behave.

---

## 5) Concrete recursion: parsing list item bodies

### 5.1 Gather raw “item body lines” with normalized indentation

For each list item:

* take the first item line’s `text` (already after marker)
* then for each continuation line:

  * strip up to `ci` spaces (not all indentation)
  * keep remaining indentation (important for nested blocks)

Store as `Vec<ItemLine { src_line, raw_stripped, indent_after_strip }>`.

```rust
pub struct ItemLine {
    pub src_line: usize,
    pub indent: usize,
    pub text: String,
}
```

Now you can run `prescan_lines` on those `text` strings again, but you must also preserve `src_line` mapping.

### 5.2 Parse blocks from item body lines with an Env

Call `parse_blocks_in_env(item_lines, Env { min_indent: 0, quote_prefix: 0 })`
Because you already normalized to item-content-relative indentation.

Nested lists will appear naturally because their indent (relative) will now be >= 2.

---

## 6) Nested quotes: exact stripping behavior

When parsing a quote block:

* For each line in the quote region:

  * remove leading spaces up to some allowance (commonmark allows up to 3)
  * then require `>`
  * then remove optional single space after `>`
  * what remains becomes the quote’s inner line text
* Preserve original `src_line`.

This produces inner lines you parse recursively into blocks.

### Quote stripping helper

```rust
fn strip_quote_marker(raw: &str) -> Option<&str> {
    let mut s = raw;

    // allow up to 3 leading spaces before '>' (common markdown practice)
    let mut lead = 0;
    while lead < 3 && s.starts_with(' ') {
        s = &s[1..];
        lead += 1;
    }

    if let Some(rest) = s.strip_prefix('>') {
        let rest = rest.strip_prefix(' ').unwrap_or(rest);
        return Some(rest);
    }
    None
}
```

Nested quote detection:

* after stripping one marker, if the remaining begins with `>` again (optionally separated by a space), that becomes nested quote in the next recursive parse. You don’t need special casing; the recursive parser will see it as quote again.

---

## 7) Priority order when parsing a line inside a container

This is what avoids “quote ate my list” bugs.

Inside a list item body (after indentation normalization), handle line kinds in this order:

1. FenceStart / FenceEnd (code fences grab everything until close)
2. ListItem (nested lists)
3. Quote (nested quotes)
4. Hr
5. Heading (rare but allow)
6. Blank (paragraph break)
7. Text (paragraph continuation)

Inside a quote body, same order.

---

## 8) End conditions for containers (so recursion unwinds correctly)

### List ends when:

* you encounter a line with indent < list_indent AND it’s not blank
* OR you encounter a different block start at indent < list_indent (heading, hr, etc.)

### List item ends when:

* you encounter a new list item at indent == list_indent
* OR you encounter a non-blank line with indent < list_indent (list ends)

### Quote ends when:

* a non-blank line doesn’t have a `>` marker (after up to 3 spaces)

---

## 9) Rendering consequences (why these rules matter)

With these rules:

* nested lists wrap with correct hanging indent because each nesting level adds its own prefix width
* quotes inside lists don’t accidentally break the list unless they aren’t indented properly
* code fences inside lists work only if indented — matching common expectations
* blank lines inside list items create loose list rendering (multi-paragraph items)

---

## 10) Minimal test cases you should bake in early

### List with paragraph continuation

```
- hello
  continued
- next
```

### Nested list

```
- a
  - b
  - c
- d
```

### Quote inside list (indented)

```
- item
  > quote line
  > quote line 2
- next
```

### Quote outside list (not indented)

```
- item
> quote breaks list
```

### Fence inside list

````
- item
  ```rust
  fn x() {}
````

* next


### Fence outside list (breaks)

* item

```rust
fn x() {}
```


---


Here’s a **tight parser harness + snapshot testing setup** that will keep `mdx` from regressing as you add more Markdown constructs. The idea is:

* feed markdown text
* run your **pre-scan → block parse → render**
* serialize the rendered lines into a stable plain-text representation (including prefixes and simple style markers if you want)
* snapshot it

I’ll sketch both the **harness code shape** and a good **set of golden tests**.

---

## 1) Testing approach: snapshot rendered output (stable + portable)

### Why snapshot tests are perfect here

Markdown renderers are *all* about “did the wrapping/indent/prefix end up right?”. Snapshot tests catch:

* indentation drift
* list continuation mistakes
* quote nesting bugs
* wrap changes from refactors

### What you snapshot

A stable string like:

```
01 | - hello world
02 |   continued line
03 |
04 | > quoted
05 |   still quote
```

Optionally include simple style tags later (e.g. `[H1]`, `[CODE]`), but v1 should focus on **structure**.

---

## 2) `mdx-core` test harness module

Create `mdx-core/src/test_harness.rs` (only compiled in tests):

### 2.1 A helper to run the whole pipeline

```rust
pub struct HarnessConfig {
    pub width: u16,
    pub theme: crate::config::ThemeVariant,
}

pub fn render_snapshot(md: &str, cfg: HarnessConfig) -> String {
    // 1) normalize line endings for stable snapshots
    let md = md.replace("\r\n", "\n");

    // 2) build line vec (owned) and also preserve original indices
    let lines: Vec<String> = md.split('\n').map(|s| s.to_string()).collect();

    // 3) pre-scan + parse blocks (your code)
    let scanned = crate::parser::prescan_lines(&lines); // or prescan_scanned_lines(...)
    let blocks = crate::parser::parse_blocks(&lines, &scanned);

    // 4) render blocks with your BlockMarkdownRenderer
    let theme = crate::theme::Theme::from_variant(cfg.theme);
    let out = crate::renderer::render_blocks(&blocks, cfg.width, &theme, /*source_line_count*/ lines.len());

    // 5) stringify rendered output in a stable way
    stringify_lines(&out.lines)
}
```

### 2.2 Stringifying `ratatui::Line` deterministically

The easiest stable stringification:

* join spans’ `content`
* ignore style for now
* keep trailing spaces trimmed (or not, but be consistent)

```rust
fn stringify_lines(lines: &[ratatui::text::Line<'static>]) -> String {
    let mut s = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let mut text = String::new();
        for span in line.spans.iter() {
            text.push_str(&span.content);
        }
        // keep it stable
        let text = text.trim_end_matches('\n').to_string();

        // line numbers in snapshot help debugging
        use std::fmt::Write;
        let _ = writeln!(&mut s, "{:02} | {}", idx + 1, text);
    }
    s
}
```

---

## 3) Snapshot framework choice

### Option A (recommended): `insta`

* great DX
* works well for text snapshots
* easy updates with `INSTA_UPDATE=always`

Add to `mdx-core/Cargo.toml`:

```toml
[dev-dependencies]
insta = "1"
```

Test example:

```rust
#[test]
fn list_continuation() {
    let md = r#"- hello
  continued
- next"#;

    let snap = render_snapshot(md, HarnessConfig { width: 40, theme: ThemeVariant::Dark });
    insta::assert_snapshot!(snap);
}
```

### Option B: plain golden files

If you don’t want insta, write to `tests/golden/*.txt` and compare. But insta is nicer.

---

## 4) Golden test suite (what to include early)

These are the minimal set that catches 80% of bugs.

### 4.1 Lists (tight + loose + nested)

**Tight list**

```
- a
- b
```

**Continuation**

```
- hello
  continued line
- next
```

**Loose list (blank line inside item)**

```
- para one

  para two
- next
```

**Nested list**

```
- a
  - b
  - c
- d
```

**Ordered list**

```
1. one
2. two
10. ten
```

### 4.2 Quotes

**Simple quote**

```
> quoted
> still quoted
```

**Quote inside list**

```
- item
  > quote line
  > quote 2
- next
```

**Nested quote**

```
> outer
> > inner
> back
```

### 4.3 Code fences

**Fence**

````
```rust
fn x() {}
````

```

**Fence inside list**
```

* item

  ```rust
  fn x() {}
  ```
* next

```

### 4.4 Headings (ATX + Setext)
```

# Title

## Sub

# Heading

```

### 4.5 Wrapping correctness
This is important: include a test with a small width.

```

* this is a long list item that should wrap nicely and keep hanging indentation

```

Run the snapshot at width 24 or 30.

### 4.6 Inline formatting (atomization sanity)
Paragraph:
```

This has *emph* and **strong** and `code` and a [link](https://example.com).

````

Snapshot should show exactly the text (style ignored), and line breaks should be stable.

---

## 5) Multi-width snapshots (the killer feature)
Wrapping differs by width: test at multiple widths.

Pattern:
```rust
#[test]
fn wrapping_regression() {
    let md = r#"- this is a long list item that should wrap with hanging indent
  and continue correctly"#;

    for w in [20u16, 30u16, 50u16] {
        let snap = render_snapshot(md, HarnessConfig { width: w, theme: ThemeVariant::Dark });
        insta::assert_snapshot!(format!("w{}", w), snap);
    }
}
````

This catches subtle prefix-width bugs immediately.

---

## 6) Optional: snapshot mapping too (source↔render)

Once you rely on mapping for gutter/selection/search, you should also test that mapping stays sane.

Add a debug dump:

* for each rendered line, print `src_line` from `rendered_to_source`
  Example:

```
01 | (src 0) - hello
02 | (src 1)   continued
```

Implementation:

```rust
fn stringify_lines_with_mapping(out: &RenderOutput) -> String {
    let mut s = String::new();
    for (i, line) in out.lines.iter().enumerate() {
        let src = out.rendered_to_source.get(i).copied().unwrap_or(usize::MAX);
        let text = line.spans.iter().map(|sp| sp.content.as_ref()).collect::<String>();
        s.push_str(&format!("{:02} | (src {:03}) {}\n", i+1, src, text));
    }
    s
}
```

This is massively useful when debugging selection or search highlighting later.

---

## 7) Where to put tests

* `mdx-core/tests/render_snapshots.rs` using the harness
* Keep tests in core so no terminal backend is needed.

---

## 8) Developer workflow

* `cargo insta test` to run
* `INSTA_UPDATE=always cargo test` to accept changes when you intentionally change formatting
* Keep snapshots small and focused (one feature per test)

---


Here’s a clean **`--dump-render` debug mode** plan that reuses the exact same pipeline as the TUI and prints a **stable, snapshot-like** representation to stdout. This is insanely useful for iterating on parsing/wrapping/mapping without fighting terminal state.

---

## 1) CLI design (mdx binary)

Add flags:

* `--dump-render`
  Print rendered output (no TUI). Exit 0.

Optional niceties:

* `--width <N>` (defaults to 80)
* `--mapping` include `(src XXX)` per rendered line
* `--no-style` (default) vs `--style-tags` (later)
* `--theme dark|light` (default from config, fallback dark)
* `--wrap` toggles wrap engine if you ever support no-wrap modes

Example:

```bash
mdx README.md --dump-render --width 60 --mapping
```

---

## 2) Implementation layout (reusable + testable)

Put the core “dump” function in `mdx-core` (or a small shared module), so:

* tests can call it
* CLI calls it
* you don’t duplicate code

### `mdx-core/src/debug_dump.rs`

Expose:

```rust
pub struct DumpOptions {
    pub width: u16,
    pub theme: ThemeVariant,
    pub include_mapping: bool,
    pub include_line_numbers: bool,
}

pub fn dump_rendered(markdown: &str, source_path: Option<&Path>, opts: DumpOptions) -> String
```

`source_path` lets you optionally print file header info, or later include git gutter in dump.

---

## 3) Dump output format (stable)

Recommended format:

* Always show 1-based rendered line number
* Optionally include `(src NNN)`
* Then the plain text content (styles stripped)

Example:

```
# mdx dump (width=50, theme=dark)
01 | (src 000) # Title
02 | (src 001)
03 | (src 002) - item that wraps and keeps hanging
04 | (src 002)   indent nicely
```

### Why this format

* easy to diff
* easy to paste into GitHub issues
* directly comparable to insta snapshots

---

## 4) End-to-end dump pipeline (exactly like TUI)

Implementation steps:

1. Normalize `\r\n` → `\n`
2. Split into lines
3. Pre-scan + parse blocks
4. Render blocks to `RenderOutput`
5. Stringify `RenderOutput` with dump options

### Code-shaped sketch

```rust
pub fn dump_rendered(markdown: &str, _source_path: Option<&Path>, opts: DumpOptions) -> String {
    let md = markdown.replace("\r\n", "\n");
    let lines: Vec<String> = md.split('\n').map(|s| s.to_string()).collect();

    let scanned = crate::parser::prescan_lines(&lines);
    let blocks = crate::parser::parse_blocks(&lines, &scanned);

    let theme = crate::theme::Theme::from_variant(opts.theme);
    let out = crate::renderer::render_blocks(&blocks, opts.width, &theme, lines.len());

    stringify_dump(&out, &opts)
}

fn stringify_dump(out: &RenderOutput, opts: &DumpOptions) -> String {
    let mut s = String::new();

    use std::fmt::Write;
    let _ = writeln!(
        &mut s,
        "# mdx dump (width={}, theme={})",
        opts.width,
        match opts.theme { ThemeVariant::Dark => "dark", ThemeVariant::Light => "light" }
    );

    for (i, line) in out.lines.iter().enumerate() {
        let mut text = String::new();
        for span in &line.spans {
            text.push_str(&span.content);
        }
        let text = text.trim_end().to_string();

        if opts.include_line_numbers {
            if opts.include_mapping {
                let src = out.rendered_to_source.get(i).copied().unwrap_or(usize::MAX);
                let _ = writeln!(&mut s, "{:02} | (src {:03}) {}", i + 1, src, text);
            } else {
                let _ = writeln!(&mut s, "{:02} | {}", i + 1, text);
            }
        } else {
            let _ = writeln!(&mut s, "{}", text);
        }
    }

    s
}
```

---

## 5) Wire it into `mdx` CLI

In `mdx/src/main.rs` with `clap`:

```rust
#[derive(clap::Parser)]
struct Cli {
    path: PathBuf,

    #[arg(long)]
    dump_render: bool,

    #[arg(long, default_value_t = 80)]
    width: u16,

    #[arg(long)]
    mapping: bool,

    #[arg(long, value_parser = ["dark", "light"])]
    theme: Option<String>,
}
```

Flow:

* Load config
* Determine theme:

  * `--theme` overrides config
* Read file content
* If `dump_render`:

  * `println!("{}", mdx_core::debug_dump::dump_rendered(&content, Some(&path), opts));`
  * exit
* else run TUI

---

## 6) Useful extensions (optional but cheap)

### `--dump-toc`

Print TOC entries with their `src_line` and anchor:

```
01 | (src 000) Title
02 | (src 010) Subsection
```

### `--dump-blocks`

Pretty-print the block tree:

* helps debug nesting rules
* can be behind `--dump-ast`

### `--dump-diff`

If you pass `source_path`, you can compute gix diff + print gutter marks alongside lines:

```
03 | + | (src 002) - added item
```

(You might do this later; it’s very handy for the diff gutter logic.)

---

## 7) Match it to your snapshot tests (so it’s one format everywhere)

If you use the same `dump_rendered()` output in:

* CLI `--dump-render`
* `insta` snapshots

…then the workflow becomes:

* reproduce a bug quickly via CLI
* paste output in an issue
* add it as a snapshot test

This is a *huge* quality multiplier.

---




