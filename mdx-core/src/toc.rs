//! Table of Contents extraction from Markdown

use crate::doc::Heading;
use ropey::Rope;

/// Extract headings from markdown text using regex scanning
pub fn extract_headings(rope: &Rope) -> Vec<Heading> {
    let mut headings = Vec::new();
    let line_count = rope.len_lines();

    let mut line_idx = 0;
    while line_idx < line_count {
        let line = rope.line(line_idx);
        let line_str: String = line.chunks().collect();
        let trimmed = line_str.trim_end();

        // Check for ATX headings: ^#{1,6}\s+
        if let Some(level) = parse_atx_heading(trimmed) {
            let text = trimmed[level..].trim().to_string();
            let anchor = make_anchor(&text);

            headings.push(Heading {
                level: level as u8,
                text,
                line: line_idx,
                anchor,
            });
        }
        // Check for Setext headings (look ahead to next line)
        else if line_idx + 1 < line_count && !trimmed.is_empty() {
            let next_line = rope.line(line_idx + 1);
            let next_str: String = next_line.chunks().collect();
            let next_trimmed = next_str.trim();

            if let Some(level) = parse_setext_underline(next_trimmed) {
                let text = trimmed.to_string();
                let anchor = make_anchor(&text);

                headings.push(Heading {
                    level,
                    text,
                    line: line_idx,
                    anchor,
                });

                // Skip the underline
                line_idx += 1;
            }
        }

        line_idx += 1;
    }

    headings
}

/// Parse ATX heading (returns level if valid, None otherwise)
fn parse_atx_heading(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }

    let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
    if hash_count > 6 {
        return None;
    }

    // Must be followed by whitespace or be at end
    let rest = &trimmed[hash_count..];
    if rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace()) {
        Some(hash_count)
    } else {
        None
    }
}

/// Parse Setext heading underline (returns level if valid)
fn parse_setext_underline(line: &str) -> Option<u8> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Check if line is all '=' (level 1) or all '-' (level 2)
    let first_char = trimmed.chars().next()?;
    if first_char == '=' && trimmed.chars().all(|c| c == '=') {
        Some(1)
    } else if first_char == '-' && trimmed.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

/// Create an anchor from heading text (simplified version)
fn make_anchor(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
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

    #[test]
    fn test_atx_headings() {
        let text = "# Level 1\n## Level 2\n### Level 3\n";
        let rope = Rope::from(text);
        let headings = extract_headings(&rope);

        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Level 1");
        assert_eq!(headings[0].line, 0);

        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "Level 2");
        assert_eq!(headings[1].line, 1);

        assert_eq!(headings[2].level, 3);
        assert_eq!(headings[2].text, "Level 3");
        assert_eq!(headings[2].line, 2);
    }

    #[test]
    fn test_setext_headings() {
        let text = "Heading 1\n=========\n\nHeading 2\n---------\n";
        let rope = Rope::from(text);
        let headings = extract_headings(&rope);

        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Heading 1");
        assert_eq!(headings[0].line, 0);

        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "Heading 2");
        assert_eq!(headings[1].line, 3);
    }

    #[test]
    fn test_mixed_headings() {
        let text = "# ATX Level 1\n\nSetext Level 1\n==============\n\n## ATX Level 2\n";
        let rope = Rope::from(text);
        let headings = extract_headings(&rope);

        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "ATX Level 1");

        assert_eq!(headings[1].level, 1);
        assert_eq!(headings[1].text, "Setext Level 1");

        assert_eq!(headings[2].level, 2);
        assert_eq!(headings[2].text, "ATX Level 2");
    }

    #[test]
    fn test_not_headings() {
        let text = "Not a #heading\n\nJust text\n";
        let rope = Rope::from(text);
        let headings = extract_headings(&rope);

        assert_eq!(headings.len(), 0);
    }

    #[test]
    fn test_anchor_generation() {
        assert_eq!(make_anchor("Hello World"), "hello-world");
        assert_eq!(make_anchor("Test & Demo"), "test-_-demo");
        assert_eq!(make_anchor("Multiple   Spaces"), "multiple---spaces");
    }

    #[test]
    fn test_all_levels() {
        let text = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n";
        let rope = Rope::from(text);
        let headings = extract_headings(&rope);

        assert_eq!(headings.len(), 6);
        for (i, heading) in headings.iter().enumerate() {
            assert_eq!(heading.level, (i + 1) as u8);
        }
    }

    #[test]
    fn test_seven_hashes_not_heading() {
        let text = "####### Not a heading\n";
        let rope = Rope::from(text);
        let headings = extract_headings(&rope);

        assert_eq!(headings.len(), 0);
    }
}
