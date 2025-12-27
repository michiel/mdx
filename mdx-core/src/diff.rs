//! Git diff gutter computation

/// Diff mark for a single line
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffMark {
    None,
    Added,
    Modified,
    DeletedAfter(u16),
}

/// Diff gutter aligned to working tree lines
#[derive(Clone, Debug)]
pub struct DiffGutter {
    pub marks: Vec<DiffMark>,
}

impl DiffGutter {
    /// Create an empty diff gutter
    pub fn empty(line_count: usize) -> Self {
        Self {
            marks: vec![DiffMark::None; line_count],
        }
    }
}

/// Compute diff gutter from base and current text
#[cfg(feature = "git")]
pub fn diff_gutter_from_text(_base: &str, _current: &str) -> Vec<DiffMark> {
    // TODO: Implementation in Stage 12
    // This will use similar::TextDiff to compute line-aligned marks
    Vec::new()
}
