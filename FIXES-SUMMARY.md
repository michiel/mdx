# Fixes Summary - Collapse Feature

## Issues Fixed

### 1. Theme Toggle Keybinding Conflict
**Problem**: Pressing `zM` triggered theme toggle instead of "close all folds"
- `M` (Shift+M) was being caught before the `z` prefix handler could process `zM`

**Solution**: Changed theme toggle from `M` to `m` (lowercase)
- Moved theme toggle handler to AFTER z prefix handling in input.rs
- Updated all documentation (README, help dialog, changelog)

**Files Changed**:
- `mdx-tui/src/input.rs` - Moved and changed keybinding
- `mdx-tui/src/ui.rs` - Updated help dialog
- `README.md` - Updated keybindings table and quick start
- `CHANGELOG-collapse.md` - Added note about keybinding change

### 2. TOC Navigation Not Expanding Collapsed Sections
**Problem**: Selecting a collapsed heading from TOC didn't expand it
- Both `toc_jump_to_selected()` and `toc_dialog_jump_to_selected()` were directly setting cursor position
- They bypassed the `jump_to_line()` function which handles collapse expansion

**Solution**: Modified both TOC functions to use `jump_to_line()`
```rust
// Before: Direct cursor setting
pane.view.cursor_line = heading.line;
pane.view.scroll_line = heading.line;

// After: Use jump_to_line for proper expansion
self.jump_to_line(target_line);
pane.view.scroll_line = target_line;
```

**Files Changed**:
- `mdx-tui/src/app.rs` - Updated `toc_jump_to_selected()` and `toc_dialog_jump_to_selected()`

### 3. Nested Collapsed Headings Not Expanding
**Problem**: When navigating to a nested heading (e.g., "### Added" under "## [Unreleased]"), only the immediate collapsed section was expanded, not parent sections
- `jump_to_line()` only removed ONE collapsed range containing the target
- Nested parent headings remained collapsed, hiding the target

**Solution**: Modified `jump_to_line()` to expand ALL collapsed ranges containing target
```rust
// Before: Only expand one range
if let Some(range) = find_range_containing_line(&collapsed_ranges, target_line) {
    pane.view.collapsed_headings.remove(&range.start);
}

// After: Loop until all parent ranges are expanded
loop {
    let collapsed_ranges = compute_all_collapsed_ranges(...);
    let containing_range = collapsed_ranges.iter()
        .find(|r| r.contains_line(target_line) || r.start == target_line);

    if let Some(range) = containing_range {
        pane.view.collapsed_headings.remove(&range.start);
    } else {
        break; // No more collapsed parents
    }
}
```

**Files Changed**:
- `mdx-tui/src/app.rs` - Enhanced `jump_to_line()` with multi-level expansion loop

## Current Behavior

### Keybindings
✅ **Theme Toggle**: Now uses `m` (lowercase)
✅ **Fold All**: `zM` (z followed by Shift+M) works correctly
✅ **Unfold All**: `zR` (z followed by Shift+R) works correctly

### TOC Navigation
✅ **Sidebar TOC** (`t` key):
- Navigate to any heading
- Press Enter
- Collapsed sections automatically expand

✅ **Dialog TOC** (`T` key):
- Navigate to any heading
- Press Enter
- Collapsed sections automatically expand

### Expansion Logic
The `jump_to_line()` function now:
1. Expands any collapsed block containing the target line
2. Expands the target line itself if it's a collapsed heading
3. Ensures you always land where intended with content visible

## Testing

### Test All Folds
```bash
cargo run --release -- test-docs/collapse-demo.md
```

1. Press `zM` - all sections collapse ✅
2. Press `zR` - all sections expand ✅
3. Press `m` - theme toggles ✅

### Test TOC Navigation
```bash
cargo run --release -- test-toc-collapse.md
```

1. Press `zM` to collapse all
2. Press `t` or `T` for TOC
3. Navigate to any heading
4. Press Enter
5. ✅ Section automatically expands!

### Test Results
```
✅ All 139 tests passing
   - 55 mdx-core tests
   - 63 mdx-tui tests
   - 21 integration tests

✅ Release build successful
✅ No compiler warnings
```

## Files Modified Summary

### Code Changes
1. **mdx-tui/src/input.rs**
   - Moved theme toggle handler after z prefix
   - Changed from `M` (Shift+M) to `m` (lowercase)
   - Moved reload handler after z prefix to avoid conflict with `zR`

2. **mdx-tui/src/app.rs**
   - Updated `toc_jump_to_selected()` to use `jump_to_line()`
   - Updated `toc_dialog_jump_to_selected()` to use `jump_to_line()`
   - Enhanced `jump_to_line()` to expand ALL parent collapsed headings using loop

### Documentation Changes
1. **README.md**
   - Updated Quick Start: `M` → `m`
   - Updated keybindings table: `M` → `m`

2. **mdx-tui/src/ui.rs**
   - Updated help dialog: `M` → `m`

3. **CHANGELOG-collapse.md**
   - Added note about keybinding change

### New Test Files
1. **test-toc-collapse.md** - Test document for TOC navigation

## Verification Checklist

- [x] `zM` closes all folds (not theme toggle)
- [x] `zR` opens all folds (not reload)
- [x] `m` toggles theme
- [x] `R` reloads document
- [x] TOC sidebar (`t`) navigation expands collapsed sections
- [x] TOC dialog (`T`) navigation expands collapsed sections
- [x] TOC navigation expands nested parent sections (e.g., "### Added" under "## [Unreleased]")
- [x] All tests passing (63 mdx-tui + 55 mdx-core)
- [x] Documentation updated
- [x] Help dialog updated
- [x] No compiler warnings

## Known Working Scenarios

1. **Collapse all, navigate via TOC**
   - zM → t → select heading → Enter → ✅ expands

2. **Navigate to collapsed heading**
   - Collapse a heading
   - Use TOC to jump to it
   - ✅ Expands on arrival

3. **Navigate to content inside collapsed section**
   - Collapse parent section
   - Use TOC to jump to sub-heading
   - ✅ Parent expands, shows sub-heading

4. **Theme toggle doesn't interfere with folds**
   - Press `z` then `M` → ✅ closes all folds
   - Press `m` alone → ✅ toggles theme

5. **Nested collapsed headings expand properly**
   - Collapse all with `zM`
   - Use TOC to navigate to nested heading (e.g., "### Added" under "## [Unreleased]")
   - ✅ Both parent "## [Unreleased]" and target "### Added" expand
   - ✅ Cursor lands on target heading with content visible

6. **Open all folds doesn't trigger reload**
   - Press `z` then `R` → ✅ opens all folds
   - Press `R` alone → ✅ reloads document
