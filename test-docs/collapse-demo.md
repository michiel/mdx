# Collapsible Blocks Demo

This document demonstrates the new collapsible blocks feature in the MDX viewer.

## Keyboard Controls

### Arrow Keys
- Press **←** (Left Arrow) to collapse the current section
- Press **→** (Right Arrow) to expand the current section

### Vim-Style Folding
- Press **za** to toggle fold of current section
- Press **zo** to open fold of current section
- Press **zc** to close fold of current section
- Press **zM** to close all folds
- Press **zR** to open all folds

**Note:** All collapse commands work whether you're on the heading itself OR anywhere within the section below it! This means you can collapse a section you're currently reading without having to navigate back to the heading.

## Feature Highlights

### Visual Indicators

When collapsed, a heading shows:
- A **▶** indicator at the start
- The heading level markers (e.g., `# ## ###`)
- Truncated heading text (max 32 chars)
- Line count in parentheses: `(N lines)`
- Distinct background color

The status bar provides helpful feedback:
- **[FOLDABLE]** - Cursor is on a heading that can be collapsed
- **[IN SECTION]** - Cursor is anywhere under a collapsible section
- **[COLLAPSED]** - Cursor is under a collapsed section (press → to expand)

### Smart Navigation

The cursor automatically skips over collapsed content:
- **j/k** or **↓/↑** jump over hidden lines
- When moving down, cursor lands after the collapsed block
- When moving up, cursor lands on the heading line

### TOC Integration

Table of Contents navigation is smart about collapsed sections:
- Jumping to a heading via TOC (`t` sidebar or `T` dialog) automatically expands it
- If the target heading is collapsed, it will be expanded when you jump to it
- If the target is inside a collapsed section, the parent section expands automatically
- This ensures you always land exactly where you wanted, with content visible

## Testing Section 1

This is some content under Section 1.
It has multiple lines.

**Try this:** You can collapse this entire section WITHOUT moving back to the heading! Just press the left arrow (←) or type `zc` right here, while reading this content. The collapse command will find the nearest heading above (in this case "Testing Section 1") and collapse it.

### Nested Subsection 1.1

This is nested content under subsection 1.1.

#### Deep Nesting 1.1.1

Even deeper content that can be collapsed independently.

When you collapse the parent (Section 1), all nested content is hidden too!

### Nested Subsection 1.2

Another subsection at the same level.

## Testing Section 2

This section is independent from Section 1.

Try collapsing Section 1 above - this section will remain visible.

### Code Example

Here's a code block inside a collapsible section:

```rust
fn main() {
    println!("Hello, collapsible world!");
    // Code blocks are preserved when sections are collapsed
    // Future enhancement: make code blocks themselves collapsible
}
```

### Lists Example

Collapsible sections can contain lists:

- Item 1
- Item 2
  - Nested item 2.1
  - Nested item 2.2
- Item 3

## Testing Section 3

### Many Lines

Line 1 of content
Line 2 of content
Line 3 of content
Line 4 of content
Line 5 of content
Line 6 of content
Line 7 of content
Line 8 of content
Line 9 of content
Line 10 of content

This section has lots of lines, so when collapsed, you'll see a higher line count.

## Performance Notes

The collapsible blocks feature is designed for efficiency:
- Collapsed ranges are computed once per render cycle
- Only visible lines are processed
- Cursor navigation has O(log n) lookup for collapsed headings (using BTreeSet)

## Future Enhancements

Potential features that could be added:
- Persistence of collapse state across sessions
- Configuration options for default collapse levels
- Collapsible code blocks
- Collapsible lists and quotes
- Visual minimap showing document structure

# Final Section

Press **?** to see the help dialog with all folding commands!

Try the following workflow:
1. Press **zM** to collapse all sections
2. Navigate with **j/k** to see how cursor skips collapsed blocks
3. Press **→** on a collapsed section to expand just that one
4. Press **zR** to expand everything again
