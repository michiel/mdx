//! Front matter detection helpers.

use ropey::Rope;
use std::fmt;

/// Types of front matter markers that mdx recognizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontMatterKind {
    Yaml,
    Toml,
    Json,
}

impl FrontMatterKind {
    fn as_str(&self) -> &'static str {
        match self {
            FrontMatterKind::Yaml => "yaml",
            FrontMatterKind::Toml => "toml",
            FrontMatterKind::Json => "json",
        }
    }
}

impl fmt::Display for FrontMatterKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Metadata describing a detected front matter block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontMatter {
    pub kind: FrontMatterKind,
    pub start_line: usize,
    pub end_line: usize,
}

impl FrontMatter {
    /// Inclusive range of line numbers covered by the front matter block.
    pub fn line_range(&self) -> std::ops::RangeInclusive<usize> {
        self.start_line..=self.end_line
    }
}

struct FrontMatterMarker {
    kind: FrontMatterKind,
    start: &'static str,
    end: &'static str,
}

const FRONT_MATTER_MARKERS: [FrontMatterMarker; 4] = [
    FrontMatterMarker {
        kind: FrontMatterKind::Yaml,
        start: "---",
        end: "---",
    },
    FrontMatterMarker {
        kind: FrontMatterKind::Toml,
        start: "+++",
        end: "+++",
    },
    FrontMatterMarker {
        kind: FrontMatterKind::Json,
        start: "===",
        end: "===",
    },
    FrontMatterMarker {
        kind: FrontMatterKind::Json,
        start: "{/",
        end: "/}",
    },
];

/// Detects front matter at the top of a document and returns its metadata.
pub fn detect_front_matter(rope: &Rope) -> Option<FrontMatter> {
    if rope.len_lines() == 0 {
        return None;
    }

    let first_line = rope.line(0);
    let first_trimmed = normalize_line(&first_line);

    let marker = FRONT_MATTER_MARKERS
        .iter()
        .find(|marker| first_trimmed == marker.start)?;

    for idx in 1..rope.len_lines() {
        let line = rope.line(idx);
        if normalize_line(&line) == marker.end {
            return Some(FrontMatter {
                kind: marker.kind,
                start_line: 0,
                end_line: idx,
            });
        }
    }

    None
}

fn normalize_line(line: &ropey::RopeSlice<'_>) -> String {
    let content: String = line.chunks().collect();
    content.trim().trim_start_matches('\u{feff}').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn detects_yaml_front_matter() {
        let rope = Rope::from("---\ntitle: hi\n---\n\n# Content\n");
        let fm = detect_front_matter(&rope).expect("should detect yaml front matter");
        assert_eq!(fm.kind, FrontMatterKind::Yaml);
        assert_eq!(fm.start_line, 0);
        assert_eq!(fm.end_line, 2);
    }

    #[test]
    fn detects_toml_front_matter() {
        let rope = Rope::from("+++\ntitle = \"hi\"\n+++\n# Heading\n");
        let fm = detect_front_matter(&rope).expect("should detect toml front matter");
        assert_eq!(fm.kind, FrontMatterKind::Toml);
        assert_eq!(fm.end_line, 2);
    }

    #[test]
    fn detects_json_front_matter_with_equal_markers() {
        let rope = Rope::from("===\n{\"title\":\"hi\"}\n===\nText\n");
        let fm = detect_front_matter(&rope).expect("should detect json front matter");
        assert_eq!(fm.kind, FrontMatterKind::Json);
        assert_eq!(fm.end_line, 2);
    }

    #[test]
    fn detects_json_front_matter_with_curly_markers() {
        let rope = Rope::from("{/\n\"title\": \"hi\"\n/}\nContent\n");
        let fm = detect_front_matter(&rope).expect("should detect json curly front matter");
        assert_eq!(fm.kind, FrontMatterKind::Json);
        assert_eq!(fm.end_line, 2);
    }

    #[test]
    fn ignores_missing_closing_marker() {
        let rope = Rope::from("---\ntitle: hi\n# Missing closing\n");
        assert!(detect_front_matter(&rope).is_none());
    }
}
