//! Linewise selection model for Visual Line mode

/// Represents a linewise selection in the document
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineSelection {
    pub anchor: usize,
    pub cursor: usize,
}

impl LineSelection {
    /// Create a new selection at a single line
    pub fn new(line: usize) -> Self {
        Self {
            anchor: line,
            cursor: line,
        }
    }

    /// Get the selection range as (min, max) inclusive
    pub fn range(&self) -> (usize, usize) {
        let a = self.anchor.min(self.cursor);
        let b = self.anchor.max(self.cursor);
        (a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_forward_selection() {
        let sel = LineSelection {
            anchor: 5,
            cursor: 10,
        };
        assert_eq!(sel.range(), (5, 10));
    }

    #[test]
    fn test_range_backward_selection() {
        let sel = LineSelection {
            anchor: 10,
            cursor: 5,
        };
        assert_eq!(sel.range(), (5, 10));
    }

    #[test]
    fn test_range_single_line() {
        let sel = LineSelection::new(7);
        assert_eq!(sel.range(), (7, 7));
    }
}
