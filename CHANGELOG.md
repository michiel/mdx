# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-01-05

### Added
- Visual scrollbar in the right margin of panes
  - Up/down arrow indicators (▴/▾) that highlight when scrolling is available
  - Proportional thumb showing current viewport position
  - Track showing scrollable area
  - Focus-aware colors matching border theme
  - Only appears when document height exceeds viewport height
- `show_scrollbar` option in RenderConfig (default: `true`)
- Scrollbar toggle in the interactive options dialog
- Stdin support - pipe markdown content directly to mdx (e.g., `cat file.md | mdx`, `curl url | mdx`)
- Unified Vim-style navigation for TOC (both sidebar and dialog)
  - Ctrl+d/Ctrl+u for half-page scrolling
  - g/G for jump to top/bottom
  - PageDown/PageUp and Space for full-page scrolling
  - Home/End for jump to top/bottom
- Ctrl+L key binding to force screen redraw
- Lowercase `y` key for yank in visual line mode (in addition to `Y`)

### Fixed
- Scrollbar thumb positioning now uses full height, allowing it to reach top and bottom positions
- Collapsed section rendering gaps eliminated
- Improved installer error handling for both bash and PowerShell scripts

## [0.2.0] - 2024-12-XX

### Added
- Configurable UTF-8 graphics rendering for enhanced terminal display
  - UTF-8 box-drawing characters for tables (`│` instead of `|`, `─` instead of `-`)
  - UTF-8 bullet points for unordered lists (`•` instead of `-`, `*`, or `+`)
  - UTF-8 horizontal rules using box-drawing characters
- `RenderConfig` configuration section with `use_utf8_graphics` option (default: `true`)
- UTF-8 Graphics toggle in the interactive options dialog (accessible via `O` key)
- Interactive options dialog for runtime configuration management
  - Toggle all major settings: theme, TOC, security, rendering options
  - Save changes to config file with `Save` button
  - Apply changes temporarily with `Ok` button
  - Cancel to revert changes
- Updated help dialog with complete key binding reference
  - Added `O` key binding for options dialog
  - Added `W` key binding for security warnings dialog

### Fixed
- Character sanitization now preserves UTF-8 characters while still blocking control codes
  - Previously stripped all non-ASCII characters
  - Now allows UTF-8 while blocking C0 and C1 control characters
- Table rendering correctly respects UTF-8 graphics configuration
  - Fixed hardcoded ASCII pipe characters (`|`) in table cell rendering
  - Fixed hardcoded ASCII dash characters (`-`) in table separators
- Text wrapping no longer orphans list bullets on separate lines
  - Bullets now stay with at least some content on the same line
  - Improved wrap detection logic for bullet markers
- Fixed viewport rendering issues with text positioning
  - Content no longer starts on new line after line numbers/gutter
  - Eliminated phantom empty lines in viewport
  - Fixed indentation accumulation bug in word-aware wrapping
  - Added safety check to prevent infinite loops in text wrapping
- Word-aware wrapping now correctly handles:
  - Empty chunks (prevents lost content and indentation buildup)
  - Zero-width splits (ensures at least one character is consumed)
  - Continuation line indentation (only added after actual wraps)

### Changed
- UTF-8 graphics are now enabled by default for better visual appearance
- Configuration cache keys now include `use_utf8_graphics` for proper invalidation
- Text wrapping logic improved to prevent premature line breaks with short styled spans

## [0.1.0] - Previous Release

### Initial Features
- Fast terminal-based Markdown viewer
- Syntax highlighting for code blocks
- Table of contents navigation
- Git diff integration
- File watching and auto-reload
- Image rendering support
- Search functionality
- Security controls (safe mode, no-exec)
- Multiple theme support (dark/light)
- Configuration file management

[Unreleased]: https://github.com/michiel/mdx/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/michiel/mdx/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/michiel/mdx/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/michiel/mdx/releases/tag/v0.1.0
