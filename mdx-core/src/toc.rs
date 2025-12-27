//! Table of Contents extraction from Markdown

use crate::doc::Heading;
use ropey::Rope;

/// Extract headings from markdown text using regex scanning
pub fn extract_headings(_rope: &Rope) -> Vec<Heading> {
    // TODO: Implementation in Stage 1
    // This will scan for:
    // - ATX headings: ^#{1,6}\s+
    // - Setext headings: underlines with === or ---
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_headings_empty() {
        let rope = Rope::from("");
        let headings = extract_headings(&rope);
        assert_eq!(headings.len(), 0);
    }
}
