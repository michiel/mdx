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

    /// Get the mark for a given line (0-indexed)
    pub fn get(&self, line: usize) -> DiffMark {
        self.marks.get(line).copied().unwrap_or(DiffMark::None)
    }
}

/// Compute diff gutter from base and current text
#[cfg(feature = "git")]
pub fn diff_gutter_from_text(base: &str, current: &str) -> DiffGutter {
    use similar::{DiffTag, TextDiff};

    let diff = TextDiff::from_lines(base, current);

    let current_lines = current.lines().count().max(1);
    let mut marks = vec![DiffMark::None; current_lines];

    // Process grouped ops to properly distinguish modifications from pure additions/deletions
    let mut current_line_idx = 0;

    for group in diff.grouped_ops(0) {
        for op in &group {
            match op.tag() {
                DiffTag::Equal => {
                    current_line_idx += op.new_range().len();
                }
                DiffTag::Delete => {
                    // Check if this delete is part of a replacement
                    let is_replacement = group.iter().any(|o| o.tag() == DiffTag::Insert);

                    if !is_replacement {
                        // Pure deletion - mark as DeletedAfter on previous line
                        let delete_count = op.old_range().len() as u16;
                        if current_line_idx > 0 {
                            let mark_idx = current_line_idx - 1;
                            if mark_idx < marks.len() {
                                marks[mark_idx] = match marks[mark_idx] {
                                    DiffMark::DeletedAfter(n) => DiffMark::DeletedAfter(n + delete_count),
                                    _ => DiffMark::DeletedAfter(delete_count),
                                };
                            }
                        } else if !marks.is_empty() {
                            // Deletion at start of file
                            marks[0] = match marks[0] {
                                DiffMark::DeletedAfter(n) => DiffMark::DeletedAfter(n + delete_count),
                                _ => DiffMark::DeletedAfter(delete_count),
                            };
                        }
                    }
                }
                DiffTag::Insert => {
                    // Check if this insert is part of a replacement
                    let is_replacement = group.iter().any(|o| o.tag() == DiffTag::Delete);

                    let range = op.new_range();
                    for i in range.clone() {
                        if i < marks.len() {
                            if is_replacement {
                                marks[i] = DiffMark::Modified;
                            } else {
                                marks[i] = DiffMark::Added;
                            }
                        }
                    }
                    current_line_idx += range.len();
                }
                DiffTag::Replace => {
                    // Some diff engines use Replace instead of Delete+Insert
                    let range = op.new_range();
                    for i in range.clone() {
                        if i < marks.len() {
                            marks[i] = DiffMark::Modified;
                        }
                    }
                    current_line_idx += range.len();
                }
            }
        }
    }

    DiffGutter { marks }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_diff() {
        let base = "line 1\nline 2\nline 3\n";
        let current = "line 1\nline 2\nline 3\n";

        let gutter = diff_gutter_from_text(base, current);

        assert_eq!(gutter.marks.len(), 3);
        assert_eq!(gutter.get(0), DiffMark::None);
        assert_eq!(gutter.get(1), DiffMark::None);
        assert_eq!(gutter.get(2), DiffMark::None);
    }

    #[test]
    fn test_added_line() {
        let base = "line 1\nline 2\n";
        let current = "line 1\nline 2\nline 3\n";

        let gutter = diff_gutter_from_text(base, current);

        assert_eq!(gutter.marks.len(), 3);
        assert_eq!(gutter.get(0), DiffMark::None);
        assert_eq!(gutter.get(1), DiffMark::None);
        assert_eq!(gutter.get(2), DiffMark::Added);
    }

    #[test]
    fn test_deleted_line() {
        let base = "line 1\nline 2\nline 3\n";
        let current = "line 1\nline 3\n";

        let gutter = diff_gutter_from_text(base, current);

        assert_eq!(gutter.marks.len(), 2);
        assert_eq!(gutter.get(0), DiffMark::DeletedAfter(1));
        assert_eq!(gutter.get(1), DiffMark::None);
    }

    #[test]
    fn test_modified_line() {
        let base = "line 1\nline 2\nline 3\n";
        let current = "line 1\nmodified line 2\nline 3\n";

        let gutter = diff_gutter_from_text(base, current);

        assert_eq!(gutter.marks.len(), 3);
        assert_eq!(gutter.get(0), DiffMark::None);
        assert_eq!(gutter.get(1), DiffMark::Modified);
        assert_eq!(gutter.get(2), DiffMark::None);
    }

    #[test]
    fn test_multiple_deletions() {
        let base = "line 1\nline 2\nline 3\nline 4\n";
        let current = "line 1\nline 4\n";

        let gutter = diff_gutter_from_text(base, current);

        assert_eq!(gutter.marks.len(), 2);
        assert_eq!(gutter.get(0), DiffMark::DeletedAfter(2));
        assert_eq!(gutter.get(1), DiffMark::None);
    }

    #[test]
    fn test_empty_gutter() {
        let gutter = DiffGutter::empty(5);

        assert_eq!(gutter.marks.len(), 5);
        for i in 0..5 {
            assert_eq!(gutter.get(i), DiffMark::None);
        }
    }

    #[test]
    #[cfg(feature = "git")]
    fn test_new_file_all_added() {
        let base = "";
        let current = "line 1\nline 2\nline 3\n";

        let gutter = diff_gutter_from_text(base, current);

        assert_eq!(gutter.marks.len(), 3);
        assert_eq!(gutter.get(0), DiffMark::Added);
        assert_eq!(gutter.get(1), DiffMark::Added);
        assert_eq!(gutter.get(2), DiffMark::Added);
    }
}

