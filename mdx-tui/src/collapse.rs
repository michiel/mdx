//! Collapsible block support for markdown documents
//!
//! This module provides utilities for computing and managing collapsed regions
//! of markdown content, particularly headings and code blocks.

use mdx_core::Document;
use std::collections::BTreeSet;

/// Represents a collapsed region in the document
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollapseRange {
    /// Start line (inclusive) - the heading or block start line
    pub start: usize,
    /// End line (inclusive) - last line of content before next heading/block
    pub end: usize,
    /// Heading level (1-6) if this is a heading collapse
    pub level: Option<u8>,
    /// Text of the heading (truncated for display)
    pub text: String,
    /// Total number of lines in this collapsed range (for display)
    pub line_count: usize,
}

impl CollapseRange {
    /// Check if a line is within this collapsed range (exclusive of start)
    pub fn contains_line(&self, line: usize) -> bool {
        line > self.start && line <= self.end
    }
}

/// Compute the range of lines that would be collapsed for a heading at the given line
///
/// Returns None if the line is not a heading or if there's nothing to collapse
pub fn compute_heading_range(heading_line: usize, doc: &Document) -> Option<CollapseRange> {
    // Find the heading at this line
    let heading = doc.headings.iter().find(|h| h.line == heading_line)?;

    // Find the next heading at the same level or higher (lower level number)
    let next_heading = doc.headings.iter()
        .find(|h| h.line > heading_line && h.level <= heading.level);

    // Determine end line
    let end_line = if let Some(next) = next_heading {
        // End at the line before the next same/higher-level heading
        next.line.saturating_sub(1)
    } else {
        // End at the last line of the document (accounting for Rope's trailing empty line)
        // Rope includes an empty line at the end, so the last actual content line is line_count - 2
        let line_count = doc.line_count();
        if line_count > 0 {
            line_count.saturating_sub(1)
        } else {
            0
        }
    };

    // Only collapse if there's content to collapse (at least one line after the heading)
    if end_line <= heading_line {
        return None;
    }

    let line_count = end_line - heading_line;

    // Truncate heading text for display (max 32 chars for heading content)
    let display_text = if heading.text.len() > 32 {
        format!("{}...", &heading.text[..29])
    } else {
        heading.text.clone()
    };

    Some(CollapseRange {
        start: heading_line,
        end: end_line,
        level: Some(heading.level),
        text: display_text,
        line_count,
    })
}

/// Compute all collapsed ranges from a set of collapsed heading lines
///
/// Returns a sorted vector of non-overlapping collapsed ranges
pub fn compute_all_collapsed_ranges(
    collapsed_headings: &BTreeSet<usize>,
    doc: &Document,
) -> Vec<CollapseRange> {
    let mut ranges = Vec::new();

    for &heading_line in collapsed_headings {
        if let Some(range) = compute_heading_range(heading_line, doc) {
            ranges.push(range);
        }
    }

    // Sort by start line (BTreeSet iteration should already be sorted, but be explicit)
    ranges.sort_by_key(|r| r.start);

    ranges
}

/// Find the collapsed range that starts at the given line, if any
pub fn find_range_at_line(ranges: &[CollapseRange], line: usize) -> Option<&CollapseRange> {
    ranges.iter().find(|r| r.start == line)
}

/// Find the collapsed range that contains the given line (but doesn't start at it)
pub fn find_range_containing_line(ranges: &[CollapseRange], line: usize) -> Option<&CollapseRange> {
    ranges.iter().find(|r| r.contains_line(line))
}

/// Check if a line is the start of a collapsible heading
pub fn is_heading_line(line: usize, doc: &Document) -> bool {
    doc.headings.iter().any(|h| h.line == line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdx_core::Document;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_doc(content: &str) -> Document {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        let (doc, _warnings) = Document::load(file.path()).unwrap();
        doc
    }

    #[test]
    fn test_heading_range_single() {
        let doc = create_test_doc("# Title\nContent\n## Sub\n");
        let range = compute_heading_range(0, &doc).unwrap();

        // When collapsing # Title, we include everything until next same-or-higher level heading
        // ## Sub is a lower level (h2 under h1), so it's included in the collapse
        // The collapse goes to EOF (line 3 in Rope, which is the empty trailing line)
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 3); // Includes line 2 (## Sub) because it's a sub-heading
        assert_eq!(range.level, Some(1));
        assert_eq!(range.text, "Title");
        assert_eq!(range.line_count, 3);
    }

    #[test]
    fn test_heading_range_nested() {
        let doc = create_test_doc("# H1\n## H2\nContent\n## H2b\n# H1b\n");

        // Collapse ## H2 (line 1)
        let range = compute_heading_range(1, &doc).unwrap();
        assert_eq!(range.start, 1);
        assert_eq!(range.end, 2); // Stops at line 3 (## H2b)
        assert_eq!(range.level, Some(2));
    }

    #[test]
    fn test_heading_range_eof() {
        let doc = create_test_doc("# Title\nContent\nMore content\n");
        let range = compute_heading_range(0, &doc).unwrap();

        assert_eq!(range.start, 0);
        assert_eq!(range.end, 3); // To EOF (line 3 is the trailing empty line in Rope)
        assert_eq!(range.line_count, 3);
    }

    #[test]
    fn test_heading_range_empty_section() {
        let doc = create_test_doc("# Title\n# Another\n");
        // Heading at line 0 has no content before next heading
        let range = compute_heading_range(0, &doc);

        // Should return None because there's nothing to collapse
        assert!(range.is_none());
    }

    #[test]
    fn test_heading_range_last_heading() {
        let doc = create_test_doc("# First\nContent\n# Last\nLast content\n");
        let range = compute_heading_range(2, &doc).unwrap();

        assert_eq!(range.start, 2);
        assert_eq!(range.end, 4); // To EOF (line 4 is the trailing empty line)
    }

    #[test]
    fn test_heading_range_not_heading() {
        let doc = create_test_doc("# Title\nNot a heading\n");
        let range = compute_heading_range(1, &doc);

        assert!(range.is_none());
    }

    #[test]
    fn test_compute_all_collapsed_ranges() {
        let doc = create_test_doc("# H1\nContent\n## H2\nMore\n# H1b\nFinal\n");

        let mut collapsed = BTreeSet::new();
        collapsed.insert(0); // Collapse first H1
        collapsed.insert(2); // Collapse H2

        let ranges = compute_all_collapsed_ranges(&collapsed, &doc);

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[1].start, 2);
    }

    #[test]
    fn test_find_range_at_line() {
        let doc = create_test_doc("# H1\nContent\n## H2\nMore\n");
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0);

        let ranges = compute_all_collapsed_ranges(&collapsed, &doc);

        assert!(find_range_at_line(&ranges, 0).is_some());
        assert!(find_range_at_line(&ranges, 1).is_none());
        assert!(find_range_at_line(&ranges, 2).is_none());
    }

    #[test]
    fn test_find_range_containing_line() {
        let doc = create_test_doc("# H1\nContent\nMore\n## H2\n");
        let mut collapsed = BTreeSet::new();
        collapsed.insert(0); // Collapse H1

        let ranges = compute_all_collapsed_ranges(&collapsed, &doc);

        assert!(find_range_containing_line(&ranges, 0).is_none()); // Start line doesn't count
        assert!(find_range_containing_line(&ranges, 1).is_some()); // Content line
        assert!(find_range_containing_line(&ranges, 2).is_some()); // More content
        assert!(find_range_containing_line(&ranges, 3).is_some()); // H2 IS contained (it's a sub-heading)
    }

    #[test]
    fn test_is_heading_line() {
        let doc = create_test_doc("# H1\nContent\n## H2\n");

        assert!(is_heading_line(0, &doc));
        assert!(!is_heading_line(1, &doc));
        assert!(is_heading_line(2, &doc));
    }

    #[test]
    fn test_collapse_range_contains_line() {
        let range = CollapseRange {
            start: 5,
            end: 10,
            level: Some(1),
            text: "Test".to_string(),
            line_count: 5,
        };

        assert!(!range.contains_line(5)); // Start line excluded
        assert!(range.contains_line(6));
        assert!(range.contains_line(10));
        assert!(!range.contains_line(11));
        assert!(!range.contains_line(4));
    }

    #[test]
    fn test_heading_text_truncation() {
        let long_heading = "This is a very long heading that should be truncated for display purposes";
        let content = format!("# {}\nContent\n", long_heading);
        let doc = create_test_doc(&content);

        let range = compute_heading_range(0, &doc).unwrap();
        assert!(range.text.len() <= 35); // 32 + "..."
        assert!(range.text.ends_with("..."));
    }

    #[test]
    fn test_multiple_heading_levels() {
        let doc = create_test_doc(
            "# H1\n\
             ## H2a\n\
             Content\n\
             ### H3\n\
             More\n\
             ## H2b\n\
             ### H3b\n\
             Final\n"
        );

        // Collapse H2a (line 1)
        let range = compute_heading_range(1, &doc).unwrap();
        assert_eq!(range.start, 1);
        assert_eq!(range.end, 4); // Stops before ## H2b (line 5)

        // Collapse H3 (line 3)
        let range = compute_heading_range(3, &doc).unwrap();
        assert_eq!(range.start, 3);
        assert_eq!(range.end, 4); // Stops before ## H2b
    }
}
