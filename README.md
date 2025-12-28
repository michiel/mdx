# mdx

`mdx` is a fast TUI Markdown viewer and editor launcher built in Rust. It aims
for md-tui level rendering polish with Vim-style navigation, a TOC sidebar,
split panes, linewise selection and yank, file watching, and an inline git diff
gutter.

## Features

- Fast, low-latency rendering for large Markdown files
- High-quality Markdown rendering (headings, lists, tables, code fences, links)
- Vim-style navigation in normal mode (`hjkl`, `gg`, `G`, `^u`, `^d`, `/`, `n/N`)
- TOC sidebar toggle and jump-to-heading
- Split panes (horizontal and vertical) with focus movement
- Visual line selection and yank to clipboard
- Open current file in an external editor (`$EDITOR`)
- File watching with on-disk change indicator and optional auto-reload
- Inline git diff gutter against `HEAD` or index
- Dark and light theme toggle

## Installation

This repository is a Cargo workspace, so `cargo install --path .` fails with a
virtual manifest error. Install the `mdx` package from its crate directory:

```bash
cargo install --path mdx
```

For a debug build without installing:

```bash
cargo run -p mdx -- --help
```

## Usage

```bash
mdx <path>
```

To open the current file in your editor, `mdx` uses `$EDITOR` by default. You
can configure a custom command template in `mdx.yaml`.

## Keybindings (default)

Navigation:

- `j/k`: move cursor line
- `^d` / `^u`: half-page down/up
- `gg` / `G`: top / bottom
- `/` then `Enter`: search forward
- `n` / `N`: next / previous match

TOC:

- `T`: toggle TOC
- `j/k`: move within TOC
- `Enter`: jump to heading
- `q`: close TOC

Splits:

- `^w s`: horizontal split
- `^w v`: vertical split
- `^↑ ^↓ ^← ^→`: move focus across panes

Selection and yank:

- `Shift+V`: enter visual line mode
- `j/k/^u/^d/gg/G`: expand selection
- `Y`: yank selection to clipboard
- `Esc`: exit visual line mode

Other:

- `M`: toggle theme
- `e`: open in external editor
- `r`: reload file (when auto-reload is off)
- `q`: quit

## Configuration

`mdx` reads a YAML config file from the platform config directory:

- Linux: `~/.config/mdx/mdx.yaml` (or `$XDG_CONFIG_HOME/mdx/mdx.yaml`)
- macOS: `~/Library/Application Support/mdx/mdx.yaml`
- Windows: `%APPDATA%\\mdx\\mdx.yaml`

Example:

```yaml
theme: dark            # dark|light
toc:
  enabled: true
  side: left           # left|right
  width: 32            # columns
editor:
  command: "$EDITOR"
  args: ["+{line}", "{file}"]
watch:
  enabled: true
  auto_reload: false
git:
  diff: true
  base: head           # head|index
```

## Workspace layout

- `mdx-core`: parsing, document model, TOC, selection, git diff, config
- `mdx-tui`: TUI app, panes, input handling, rendering
- `mdx`: CLI and runtime wiring

## Build and test

```bash
cargo build
cargo test
```

## Roadmap highlights

- Improved source-to-render mapping for selection and diff gutters
- Enhanced markdown rendering controls and theming
- Additional keybinding customisation

## Licence

Dual-licensed under MIT or Apache-2.0.
