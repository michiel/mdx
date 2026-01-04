# mdx

A fast, terminal-based Markdown viewer and editor launcher built in Rust. Designed for developers who want to quickly browse Markdown documentation with Vim-style navigation, table of contents, split panes, and git diff integration.

## Features

### Rendering and Display

  - **Fast rendering** - Low-latency display optimised for large Markdown files
  - **High-quality Markdown** - Supports headings, lists, tables, code blocks with syntax highlighting, and inline formatting
  - **Git diff gutter** - Visual indicators showing added, modified, and deleted lines compared to git HEAD or index
  - **Dual themes** - Toggle between dark and light colour schemes
  - **Split panes** - View multiple sections simultaneously with horizontal and vertical splits

### Navigation and Editing

  - **Vim-style navigation** - Familiar keybindings (`hjkl`, `gg`, `G`, `Ctrl-u`, `Ctrl-d`, `/`, `n`, `N`)
  - **Collapsible sections** - Fold/unfold headings to focus on relevant content with vim-style (`za`, `zo`, `zc`, `zM`, `zR`) or arrow keys
  - **Table of contents** - Sidebar with document outline and quick heading navigation
  - **Visual line mode** - Select and yank multiple lines to clipboard
  - **Search** - Forward search with next/previous match navigation
  - **External editor** - Launch your preferred editor at the current line

### File Management

  - **File watching** - Automatic detection of on-disk changes with optional auto-reload
  - **Multi-document** - Open multiple files in split panes
  - **Cross-platform** - Works on Linux, macOS, and Windows

## Installation

### Quick Install (Recommended)

**Linux/macOS** (one-line install):
```bash
curl -fsSL https://raw.githubusercontent.com/michiel/mdx/main/scripts/install.sh | bash
```

**Windows** (PowerShell):
```powershell
iwr -useb https://raw.githubusercontent.com/michiel/mdx/main/scripts/install.ps1 | iex
```

The installer will:
- Detect your platform and architecture automatically
- Download the latest release from GitHub
- Install to `~/.local/bin` (Linux/macOS) or `%LOCALAPPDATA%\mdx` (Windows)
- Add to PATH if needed

### Pre-built Binaries

Alternatively, download pre-built binaries manually from the [releases page](https://github.com/michiel/mdx/releases):

- **Linux x86_64**: `mdx-linux-x86_64`
- **Linux ARM64**: `mdx-linux-aarch64`
- **macOS Intel**: `mdx-macos-x86_64`
- **macOS Apple Silicon**: `mdx-macos-aarch64`
- **Windows**: `mdx-windows-x86_64.exe`

After downloading, make the binary executable and move it to your PATH:

```bash
# Linux/macOS
chmod +x mdx-linux-x86_64
sudo mv mdx-linux-x86_64 /usr/local/bin/mdx

# Or install to user directory
mkdir -p ~/.local/bin
mv mdx-linux-x86_64 ~/.local/bin/mdx
```

### From Source

This repository is a Cargo workspace. To install from source:

```bash
# Install from the mdx crate directory
cargo install --path mdx

# Or install with all features
cargo install --path mdx --features git,watch
```

For development:

```bash
# Run without installing
cargo run -p mdx -- README.md

# Run tests
cargo test

# Build optimised binary
cargo build --release -p mdx
```

## Usage

Open any Markdown file:

```bash
mdx README.md
mdx docs/guide.md
```

Read from stdin:

```bash
cat README.md | mdx
echo "# Hello World" | mdx
curl https://example.com/doc.md | mdx
```

### Quick Start

- Press `j`/`k` to scroll line by line
- Press `Ctrl-d`/`Ctrl-u` for half-page scrolling
- Press `←`/`→` to collapse/expand sections
- Press `T` to toggle the table of contents sidebar
- Press `/` to search, then `n`/`N` to navigate matches
- Press `e` to open the file in your external editor
- Press `m` to toggle between dark and light themes
- Press `?` to see all keybindings
- Press `q` to quit

The application uses `$EDITOR` by default for external editing. Configure a custom editor in `mdx.yaml`.

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor down/up one line |
| `Ctrl-d` / `Ctrl-u` | Scroll half-page down/up |
| `gg` / `G` | Jump to top/bottom of document |
| `/` | Start search (press Enter to confirm) |
| `n` / `N` | Jump to next/previous search match |

### Table of Contents

| Key | Action |
|-----|--------|
| `T` | Toggle TOC sidebar |
| `j` / `k` | Navigate within TOC |
| `Enter` | Jump to selected heading |
| `q` | Close TOC sidebar |

### Collapsible Sections

| Key | Action |
|-----|--------|
| `←` | Collapse current section |
| `→` | Expand current section |
| `za` | Toggle fold of current section |
| `zo` | Open fold of current section |
| `zc` | Close fold of current section |
| `zM` | Close all folds |
| `zR` | Open all folds |

**Note**: Folding commands work on the heading at cursor or the nearest heading above, allowing you to collapse a section from anywhere within it. TOC navigation automatically expands collapsed sections when jumping to them.

### Split Panes

| Key | Action |
|-----|--------|
| `Ctrl-w s` | Create horizontal split |
| `Ctrl-w v` | Create vertical split |
| `Ctrl-↑` / `Ctrl-↓` / `Ctrl-←` / `Ctrl-→` | Move focus between panes |

### Selection and Clipboard

| Key | Action |
|-----|--------|
| `Shift-V` | Enter visual line mode |
| `j` / `k` / `Ctrl-u` / `Ctrl-d` / `gg` / `G` | Expand selection |
| `Y` | Yank (copy) selection to clipboard |
| `Esc` | Exit visual line mode |

### Other Commands

| Key | Action |
|-----|--------|
| `?` | Show help dialog with all keybindings |
| `m` | Toggle between dark and light themes |
| `O` | Open options dialog |
| `e` | Open file in external editor |
| `r` | Toggle raw/rendered mode |
| `R` | Reload file from disk |
| `q` | Quit application |
| `Ctrl-C` | Force quit |

## Configuration

Configuration is read from a TOML file in the platform-specific config directory:

- **Linux**: `~/.config/mdx/mdx.toml` (or `$XDG_CONFIG_HOME/mdx/mdx.toml`)
- **macOS**: `~/Library/Application Support/mdx/mdx.toml`
- **Windows**: `%APPDATA%\mdx\mdx.toml`

### Example Configuration

```toml
# Theme selection
theme = "dark"  # Options: "dark", "light"

# Table of contents settings
[toc]
enabled = true   # Show TOC on startup
side = "left"    # Options: "left", "right"
width = 32       # Width in columns

# External editor configuration
[editor]
command = "$EDITOR"  # Use $EDITOR environment variable
args = ["+{line}", "{file}"]  # {line} and {file} are replaced at runtime

# Security options (secure defaults)
[security]
safe_mode = true   # Sanitise terminal output, disable external commands, and turn off images
no_exec = true     # Disable external editor execution (use --insecure to override)

# File watching settings
[watch]
enabled = true      # Watch files for changes
auto_reload = false # Automatically reload on change (false = show indicator only)

# Git integration settings
[git]
diff = true       # Show git diff gutter
base = "head"     # Options: "head", "index" (compare against HEAD or staging area)

# Image rendering settings
[images]
enabled = false
allow_absolute = false
allow_remote = false
max_bytes = 10485760
```

### Editor Configuration Examples

**Neovim/Vim**:
```toml
[editor]
command = "nvim"
args = ["+{line}", "{file}"]
```

**VSCode**:
```toml
[editor]
command = "code"
args = ["--goto", "{file}:{line}"]
```

**Emacs**:
```toml
[editor]
command = "emacs"
args = ["+{line}", "{file}"]
```

## Development

### Repository Structure

This is a Cargo workspace containing three crates:

- **mdx-core** - Core library with document parsing, TOC extraction, git diff, and configuration
- **mdx-tui** - Terminal UI implementation with ratatui, pane management, and input handling
- **mdx** - Binary crate that wires everything together

### Building and Testing

```bash
# Build all crates
cargo build

# Build with all features
cargo build --features git,watch

# Run tests
cargo test

# Run clippy
cargo clippy --all-targets --all-features

# Format code
cargo fmt
```

## Contributing

Contributions are welcome. Please ensure:

- All tests pass (`cargo test`)
- Code is formatted (`cargo fmt`)
- Clippy is happy (`cargo clippy`)
- Commit messages are clear and descriptive

## Licence

Dual-licensed under MIT or Apache-2.0.
