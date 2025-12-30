# MDX Architecture

## Overview

MDX is a high-performance, terminal-based Markdown viewer with Vim-style navigation. It's built in Rust using a modular workspace architecture that separates concerns between document processing, terminal UI, and application entry point.

**Key Features:**
- Fast markdown rendering with syntax highlighting
- Vim-style keyboard navigation
- Split pane support with flexible layouts
- Git diff integration with gutter indicators
- File watching for automatic reloads
- Table of contents with breadcrumb navigation
- Visual line mode with clipboard support
- Security-first design with sandboxing
- Search with highlighting
- Theme support (Dark/Light)

## Project Structure

The project uses a Cargo workspace with three crates:

```
mdx-workspace/
├── mdx-core/          # Platform-independent document processing
│   ├── config.rs      # Configuration management
│   ├── doc.rs         # Document model (Rope-based text)
│   ├── git.rs         # Git integration (gix)
│   ├── diff.rs        # Line-by-line diff computation
│   ├── toc.rs         # Table of contents extraction
│   ├── security.rs    # Security event tracking
│   └── selection.rs   # Text selection utilities
├── mdx-tui/           # Terminal UI implementation
│   ├── app.rs         # Application state and logic
│   ├── panes.rs       # Split pane tree management
│   ├── ui.rs          # Rendering and styling
│   ├── input.rs       # Keyboard input handling
│   ├── theme.rs       # Color schemes
│   ├── diff_worker.rs # Background diff computation
│   ├── watcher.rs     # File system watching
│   ├── editor.rs      # External editor integration
│   └── options_dialog.rs # Configuration UI
└── mdx/               # CLI entry point (~85 LOC)
    └── main.rs        # Argument parsing and initialization
```

### Crate Responsibilities

#### mdx-core
**Purpose**: Platform-independent document processing and business logic.

**Dependencies**:
- `ropey` - Efficient rope-based text storage
- `pulldown-cmark` - Markdown parsing
- `serde` + `toml` - Configuration serialization
- `gix` (optional) - Git repository access
- `similar` (optional) - Diff algorithm
- `notify` (optional) - File system watching
- `arboard` (optional) - Clipboard access

**Key Types**:
```rust
pub struct Document {
    pub path: PathBuf,
    pub rope: Rope,           // Efficient text representation
    pub headings: Vec<Heading>,
    pub rev: u64,             // Version counter
    pub diff_gutter: DiffGutter,
    pub images: Vec<ImageNode>,
}

pub struct Config {
    pub theme: ThemeVariant,
    pub toc: TocConfig,
    pub security: SecurityConfig,
    pub git: GitConfig,
    // ... other sections
}
```

#### mdx-tui
**Purpose**: Terminal user interface using ratatui.

**Dependencies**:
- `ratatui` - Terminal UI framework
- `crossterm` - Cross-platform terminal manipulation
- `crossbeam-channel` - Multi-producer multi-consumer channels
- `lru` - LRU cache for rendered output

**Key Types**:
```rust
pub struct App {
    pub config: Config,
    pub doc: Document,
    pub panes: PaneManager,
    pub theme: Theme,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    // ... UI state
}

pub struct PaneManager {
    pub root: PaneNode,       // Binary tree of panes
    pub panes: HashMap<PaneId, Pane>,
    pub focused: PaneId,
}
```

#### mdx
**Purpose**: CLI entry point and argument parsing.

Uses `clap` for command-line argument parsing. Responsible for:
- Parsing CLI arguments
- Loading configuration
- Creating initial document
- Initializing and running the TUI

---

## Core Systems

### 1. Document Model (Rope-Based Text)

**Location**: `mdx-core/src/doc.rs`

**Design**: Uses the `ropey` crate for efficient text storage and manipulation.

**Benefits**:
- O(log n) insertions/deletions
- Efficient for large documents (up to 10MB limit)
- No full-text copying on edits
- Line-based access for rendering

**Resource Limits** (enforced for security):
- 10MB maximum file size
- 1000 maximum headings
- 100 maximum images
- Warnings at 80% thresholds

**Key Operations**:
```rust
// Loading
let (doc, warnings) = Document::load(path)?;

// Line access
let line_text: String = doc.rope.line(line_idx).chunks().collect();

// Reloading
doc.reload()?;
```

### 2. Pane Tree System

**Location**: `mdx-tui/src/panes.rs`

**Architecture**: Recursive binary tree for flexible split layouts.

**Tree Structure**:
```rust
pub enum PaneNode {
    Leaf(PaneId),                    // Individual pane
    Split {
        dir: SplitDir,               // Horizontal or Vertical
        left: Box<PaneNode>,
        right: Box<PaneNode>,
        ratio: f32,                  // 0.0-1.0 split ratio
    },
}
```

**Visual Example**:
```
PaneNode::Split (Vertical, ratio=0.5)
  ├─ Leaf(0)
  └─ Split (Horizontal, ratio=0.6)
      ├─ Leaf(1)
      └─ Leaf(2)

Terminal Layout:
┌─────────┬─────────┐
│         │    1    │
│    0    ├─────────┤
│         │    2    │
└─────────┴─────────┘
```

**Key Operations**:
- `split_focused()` - Replace focused leaf with a split node
- `close_focused()` - Remove pane and collapse parent split
- `compute_layout()` - Calculate Rect for each pane given terminal area
- `move_focus()` - Navigate using geometric distance (not tree-based)

**Focus Navigation**: Uses Euclidean distance to find nearest pane in requested direction. This works better than tree-based navigation for complex layouts created by multiple split operations.

### 3. Rendering System

**Location**: `mdx-tui/src/ui.rs`

**LRU Render Cache**:
```rust
cache: LruCache<(u64, u16, String), Vec<Line>>
//              doc_rev, width, theme_name
```

**Cache Key Components**:
- `doc_rev` - Document version (invalidates on edit)
- `width` - Terminal width (invalidates on resize)
- `theme_name` - Theme variant (invalidates on theme change)

**Benefits**:
- Avoids re-rendering unchanged documents
- 32-entry cache handles multiple panes
- Smart invalidation on changes

**Rendering Pipeline**:
1. Check LRU cache for `(doc_rev, width, theme)`
2. If miss, render markdown with syntax highlighting
3. Apply search highlighting overlay (if active)
4. Cache result
5. Extract visible lines for viewport
6. Apply selection/cursor highlighting

**Syntax Highlighting**:
- Code blocks: Keyword, string, comment, number detection
- Markdown: Headings (6 levels), bold, italic, inline code, links, lists, tables
- Search highlighting: Yellow background overlaid on existing styles

### 4. Background Workers

#### DiffWorker

**Location**: `mdx-tui/src/diff_worker.rs`

**Purpose**: Compute git diffs asynchronously to avoid blocking UI.

**Architecture**:
```rust
pub struct DiffWorker {
    sender: Sender<DiffRequest>,
    receiver: Receiver<DiffResult>,
}
```

**Request Coalescing**:
- 75ms window to batch multiple requests
- Prevents redundant diff computation during rapid scrolling
- Only processes latest request if multiple arrive

**Stale Result Handling**:
- Results tagged with `(doc_id, doc_rev)`
- App ignores results for outdated document versions
- Prevents race conditions

**Thread Communication**:
```
Main Thread                Worker Thread
    │                          │
    ├─ send(request)──────────>│
    │                          ├─ coalesce (75ms)
    │                          ├─ compute diff
    │                          │
    │<──────────send(result)───┤
    ├─ check (doc_id, rev)     │
    └─ apply if current        │
```

#### FileWatcher

**Location**: `mdx-tui/src/watcher.rs`

**Purpose**: Monitor file changes for automatic reload.

**Implementation**:
- Uses `notify` crate (cross-platform)
- Watches both file and parent directory
- Debouncing to prevent rapid reload spam
- Reports changes via channel

**Configuration**:
- `watch.enabled` - Enable/disable watching
- `auto_reload` - Automatic reload vs indicator only

**Use Cases**:
- External editor modifications
- Git operations (checkout, pull, etc.)
- Build system output updates

### 5. Configuration System

**Location**: `mdx-core/src/config.rs`

**File Locations** (platform-specific):
- **Linux**: `~/.config/mdx/mdx.toml` (or `$XDG_CONFIG_HOME/mdx/mdx.toml`)
- **macOS**: `~/Library/Application Support/mdx/mdx.toml`
- **Windows**: `%APPDATA%\mdx\mdx.toml`

**Structure**:
```toml
theme = "Dark"  # or "Light"

[toc]
enabled = false
side = "Left"   # or "Right"
width = 32

[security]
safe_mode = true
no_exec = true

[git]
diff = true
base = "Head"   # or "Index"

[images]
enabled = false
allow_absolute = false
allow_remote = false
max_bytes = 10485760

[watch]
enabled = true
auto_reload = false

[editor]
command = "$EDITOR"
args = ["+{line}", "{file}"]
```

**Security Enforcement**:
- **Permission checks** (Unix): Rejects world-writable config files
- **Safe mode override**: Automatically disables `images.enabled` if `safe_mode = true`
- **File mode**: Config files written with `0o644` permissions

**Validation**:
```rust
// Unix permission check
if mode & 0o002 != 0 {
    bail!("Config file is world-writable");
}

// Safe mode enforcement
if config.security.safe_mode {
    config.images.enabled = false;
}
```

### 6. Git Integration

**Location**: `mdx-core/src/git.rs`, `mdx-core/src/diff.rs`

**Library**: Uses `gix` (pure Rust git implementation, not libgit2).

**Strategy**: Only show diffs for **tracked files** (files in git index).
- Untracked files are treated as if ignored
- Prevents confusion with gitignore patterns
- Simple, reliable approach

**Diff Computation** (`similar` crate):
```rust
pub enum DiffMark {
    None,
    Added,
    Modified,
    DeletedAfter(usize),  // Shows deletion below this line
}
```

**Gutter Indicators**:
- `│` (green) - Added line
- `│` (yellow) - Modified line
- `│` (red) - Deleted line(s)
- Two spaces - Unchanged line

**Performance**:
- Computed in background worker
- Debounced to avoid thrashing
- Results cached in `Document::diff_gutter`

### 7. Search System

**Location**: `mdx-tui/src/app.rs` (search logic), `mdx-tui/src/ui.rs` (highlighting)

**Search Process**:
1. User enters search mode with `/`
2. Query entered character-by-character
3. Matches found with case-insensitive search
4. Navigate matches with `n` (next) and `N` (previous)
5. Circular navigation (wraps at boundaries)

**Match Storage**:
```rust
pub search_query: String,
pub search_matches: Vec<usize>,      // Line numbers
pub search_current_match: Option<usize>,  // Index into matches
```

**Highlighting**:
- Rendered markdown: Yellow background overlay on matching text
- Code blocks: Applied after syntax highlighting
- Preserves existing foreground colors
- Bold modifier added to matches

**Implementation** (`apply_search_highlighting_to_spans`):
1. Takes existing styled spans
2. Splits spans containing matches
3. Applies yellow background + black foreground to matches
4. Preserves original styling for non-matching text

---

## Security Model

### Defense in Depth

MDX implements multiple security layers:

#### 1. Secure Defaults
```rust
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            safe_mode: true,   // Blocks images and external commands
            no_exec: true,     // Blocks external editor
        }
    }
}
```

#### 2. Resource Limits
- **10MB** max file size
- **1000** max headings
- **100** max images
- **10MB** max image size (per image)

Prevents:
- Memory exhaustion
- Parser abuse
- Denial of service

#### 3. Input Sanitization
```rust
fn sanitize_for_terminal(s: String) -> String {
    s.chars()
        .map(|c| if c.is_control() && c != '\t' { ' ' } else { c })
        .collect()
}
```

Prevents terminal escape sequence injection.

#### 4. Path Restrictions
- Absolute paths blocked by default (`images.allow_absolute`)
- Remote URLs blocked by default (`images.allow_remote`)
- Path traversal prevention in image resolution

#### 5. Config File Security
- **Unix**: World-writable configs rejected
- **All**: Files written with restricted permissions (`0o644`)

#### 6. External Command Execution
- Blocked in safe mode
- Blocked if `no_exec = true`
- Template expansion prevents injection:
  ```rust
  command = "$EDITOR"
  args = ["+{line}", "{file}"]
  // Executes: $EDITOR +42 /path/to/file.md
  // NOT: sh -c "$EDITOR +42 /path/to/file.md"
  ```

#### 7. SecurityEvent Tracking
```rust
pub struct SecurityEvent {
    level: SecurityEventLevel,  // Info, Warning, Error
    message: String,
    source: String,
}
```

Visible in dedicated pane (toggle with key binding).

---

## Data Flow

### Application Lifecycle

```
1. Startup
   ├─ Parse CLI arguments (clap)
   ├─ Load config from file
   ├─ Load document from path
   │   ├─ Parse markdown (pulldown-cmark)
   │   ├─ Extract headings
   │   ├─ Extract images (if enabled)
   │   └─ Build rope structure
   ├─ Initialize App state
   │   ├─ Create PaneManager (single pane)
   │   ├─ Load theme
   │   ├─ Spawn DiffWorker (if git enabled)
   │   ├─ Spawn FileWatcher (if watch enabled)
   │   └─ Request initial diff
   └─ Enter event loop

2. Event Loop
   ├─ Poll for events (keyboard, resize, file changes)
   ├─ Handle input → Update app state
   ├─ Check background workers for results
   │   ├─ DiffWorker → Update diff_gutter
   │   └─ FileWatcher → Set dirty_on_disk flag
   ├─ Render UI
   │   ├─ Compute pane layouts
   │   ├─ Render each pane
   │   │   ├─ Check LRU cache
   │   │   ├─ Render markdown if cache miss
   │   │   ├─ Apply search highlighting
   │   │   └─ Extract visible lines
   │   ├─ Render status bar
   │   ├─ Render dialogs (if active)
   │   └─ Swap buffers
   └─ Loop until quit

3. Shutdown
   ├─ Drop DiffWorker (thread joins)
   ├─ Drop FileWatcher (thread joins)
   └─ Restore terminal state
```

### Input Handling Flow

```
Keyboard Event
    │
    ├─ crossterm::event::read()
    │
    ├─ input::handle_input(app, key, ...)
    │   │
    │   ├─ Match key to action
    │   ├─ Update app state
    │   │   ├─ Move cursor
    │   │   ├─ Toggle modes
    │   │   ├─ Split panes
    │   │   └─ ... other actions
    │   │
    │   └─ Return Action (Continue/Quit)
    │
    └─ Render updated state
```

### Background Worker Communication

```
Main Thread              DiffWorker Thread
    │                          │
    ├─ Document loaded         │
    │                          │
    ├─ send(DiffRequest {      │
    │    doc_id,               │
    │    doc_rev,              │
    │    path,                 │
    │    current_text          │
    │  })                      │
    │                          │
    │                          ├─ Wait/coalesce (75ms)
    │                          │
    │                          ├─ Open git repo
    │                          ├─ Read HEAD version
    │                          ├─ Compute diff (similar)
    │                          │
    │<───send(DiffResult {     │
    │      doc_id,             │
    │      doc_rev,            │
    │      diff_gutter         │
    │    })                    │
    │                          │
    ├─ Check doc_id/rev match  │
    ├─ Apply to doc.diff_gutter│
    └─ Trigger re-render       │
```

---

## Testing Strategy

### Unit Tests (92 total)

**mdx-core** (54 tests):
- Config loading and serialization
- Document loading (empty, simple, large)
- TOC extraction (all heading types)
- Diff computation (add, modify, delete)
- Image extraction
- Security validation
- Selection utilities

**mdx-tui** (38 tests):
- App navigation (cursor movement, jumps, scrolling)
- Pane management (split, close, layout)
- Visual line mode (enter, navigate, yank)
- TOC navigation and dialog
- Search functionality
- Security enforcement (safe mode, no_exec)
- DiffWorker (spawn, coalesce, process)
- FileWatcher (detect changes, debounce)

### Integration Tests (20 tests)

**Location**: `mdx-tui/tests/integration_tests.rs`

**Coverage**:
- App initialization
- End-to-end navigation flows
- Search with highlighting
- Visual line mode workflows
- Pane splitting and closing
- Configuration integration
- Theme toggling
- Multi-pane independence
- Scrolling and jumping
- Dialog interactions

**Approach**:
- Create temporary markdown files
- Initialize app with test content
- Exercise public API methods
- Verify state changes
- Test edge cases (empty docs, single lines)

### Test Organization

```
tests/
├── Unit tests (in source files)
│   ├── #[cfg(test)] mod tests { ... }
│   └── Colocated with implementation
└── Integration tests
    └── mdx-tui/tests/integration_tests.rs
```

**Best Practices**:
- Test behavior, not implementation
- Clear test names describing scenario
- Use helper functions for setup
- Test edge cases and boundaries
- Deterministic tests (no flaky async)

---

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Load document | O(n) | Parse markdown, build rope |
| Line access | O(log n) | Rope structure |
| Render viewport | O(m) | m = visible lines |
| Search | O(n) | Full document scan |
| Diff computation | O(n) | Line-by-line comparison |
| Pane layout | O(p) | p = number of panes |
| Focus navigation | O(p) | Geometric distance |

### Space Complexity

| Structure | Size | Notes |
|-----------|------|-------|
| Rope | O(n) | Document text |
| Headings | O(h) | h ≤ 1000 limit |
| Images | O(i) | i ≤ 100 limit |
| Diff gutter | O(n) | One mark per line |
| Render cache | O(32) | Fixed 32 entries |
| Pane tree | O(p) | p panes = 2p-1 nodes |

### Optimization Techniques

1. **LRU Render Cache**: Avoid re-rendering unchanged documents
2. **Rope Text Structure**: Efficient large file handling
3. **Background Workers**: Non-blocking diff and watch
4. **Request Coalescing**: Batch rapid operations (75ms window)
5. **Lazy Image Loading**: Only parse metadata, don't render
6. **Resource Limits**: Prevent memory exhaustion
7. **Viewport Culling**: Only render visible lines

---

## Future Architecture Considerations

### Planned Enhancements

1. **Custom Keybindings** (Task #10)
   - Config file section for key mappings
   - Action enum for bindable commands
   - Runtime keymap construction

2. **Multi-Document Editing**
   - Already supported by pane architecture
   - Need document switching UI
   - Per-pane document IDs implemented

3. **Plugin System**
   - Custom markdown parsers
   - Syntax highlighters
   - Export formatters

4. **Web Frontend**
   - Reuse `mdx-core` for parsing
   - WebAssembly compilation
   - Browser-based rendering

### Scalability Limits

**Current Limits** (sufficient for typical use):
- 10MB max file size
- 1000 max headings
- 100 max images
- 32 render cache entries

**To Scale Further**:
- Streaming document loading
- Virtual scrolling (only render viewport)
- Incremental parsing
- Disk-backed caching

### Technical Debt

**Minor**:
- Some code duplication in rendering logic
- Could extract more shared utilities
- Test coverage for rendering could be higher

**Major**:
- None identified

---

## Dependencies

### Core Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| ropey | 1.6 | Rope-based text |
| pulldown-cmark | 0.13 | Markdown parsing |
| serde | 1.0 | Serialization |
| toml | 0.8 | Config format |
| directories | 5.0 | Platform paths |
| anyhow | 1.0 | Error handling |

### UI Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| ratatui | 0.30 | Terminal UI framework |
| crossterm | 0.28 | Terminal manipulation |
| tui-markdown | 0.3 | Markdown rendering |
| crossbeam-channel | 0.5 | Threading |
| lru | 0.12 | Cache |

### Optional Dependencies

| Crate | Feature | Purpose |
|-------|---------|---------|
| gix | git | Git operations |
| similar | git | Diff algorithm |
| notify | watch | File watching |
| arboard | clipboard | Clipboard access |
| image | images | Image metadata |

All dependencies are:
- Actively maintained
- Widely used in the Rust ecosystem
- No known security vulnerabilities
- Pure Rust (except crossterm's libc dependency)

---

## Summary

MDX is a well-architected terminal application with:

✅ **Clean separation of concerns** (3-crate workspace)
✅ **Security-first design** (safe defaults, sandboxing)
✅ **High performance** (rope structure, LRU cache, background workers)
✅ **Extensible architecture** (feature flags, plugin-ready)
✅ **Comprehensive testing** (112 total tests)
✅ **Cross-platform support** (Linux, macOS, Windows)

The architecture balances simplicity with flexibility, making it easy to understand, maintain, and extend while providing excellent performance and security for users.
