//! Pane management and split tree

use crate::app::ViewState;
use ratatui::layout::Rect;
use std::collections::HashMap;

/// Unique identifier for a pane
pub type PaneId = usize;

/// Direction of a split
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDir {
    Horizontal,
    Vertical,
}

/// Pane tree node - either a leaf (single pane) or a split
#[derive(Debug, Clone)]
pub enum PaneNode {
    Leaf(PaneId),
    Split {
        dir: SplitDir,
        left: Box<PaneNode>,
        right: Box<PaneNode>,
        ratio: f32, // 0.0 to 1.0, portion allocated to left/top
    },
}

impl PaneNode {
    /// Create a new leaf node
    pub fn leaf(id: PaneId) -> Self {
        PaneNode::Leaf(id)
    }

    /// Create a new split node
    pub fn split(dir: SplitDir, left: PaneNode, right: PaneNode, ratio: f32) -> Self {
        PaneNode::Split {
            dir,
            left: Box::new(left),
            right: Box::new(right),
            ratio,
        }
    }

    /// Find the leaf containing the given pane ID
    pub fn find_leaf(&self, target_id: PaneId) -> Option<&PaneNode> {
        match self {
            PaneNode::Leaf(id) if *id == target_id => Some(self),
            PaneNode::Leaf(_) => None,
            PaneNode::Split { left, right, .. } => {
                left.find_leaf(target_id).or_else(|| right.find_leaf(target_id))
            }
        }
    }

    /// Replace a leaf node with a new node
    pub fn replace_leaf(&mut self, target_id: PaneId, new_node: PaneNode) -> bool {
        match self {
            PaneNode::Leaf(id) if *id == target_id => {
                *self = new_node;
                true
            }
            PaneNode::Leaf(_) => false,
            PaneNode::Split { left, right, .. } => {
                left.replace_leaf(target_id, new_node.clone())
                    || right.replace_leaf(target_id, new_node)
            }
        }
    }

    /// Get all leaf pane IDs in order
    pub fn leaf_ids(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Leaf(id) => vec![*id],
            PaneNode::Split { left, right, .. } => {
                let mut ids = left.leaf_ids();
                ids.extend(right.leaf_ids());
                ids
            }
        }
    }
}

/// Individual pane state
#[derive(Debug, Clone)]
pub struct Pane {
    pub id: PaneId,
    pub doc_id: usize, // Index into documents array (for multi-file support later)
    pub view: ViewState,
}

impl Pane {
    /// Create a new pane
    pub fn new(id: PaneId, doc_id: usize) -> Self {
        Self {
            id,
            doc_id,
            view: ViewState::new(),
        }
    }
}

/// Pane manager - manages the pane tree and pane storage
pub struct PaneManager {
    pub root: PaneNode,
    pub panes: HashMap<PaneId, Pane>,
    pub focused: PaneId,
    next_id: PaneId,
}

impl PaneManager {
    /// Create a new pane manager with a single pane
    pub fn new(doc_id: usize) -> Self {
        let pane_id = 0;
        let pane = Pane::new(pane_id, doc_id);
        let mut panes = HashMap::new();
        panes.insert(pane_id, pane);

        Self {
            root: PaneNode::leaf(pane_id),
            panes,
            focused: pane_id,
            next_id: 1,
        }
    }

    /// Get the focused pane
    pub fn focused_pane(&self) -> Option<&Pane> {
        self.panes.get(&self.focused)
    }

    /// Get the focused pane mutably
    pub fn focused_pane_mut(&mut self) -> Option<&mut Pane> {
        self.panes.get_mut(&self.focused)
    }

    /// Split the focused pane
    pub fn split_focused(&mut self, dir: SplitDir, doc_id: usize) {
        let new_pane_id = self.next_id;
        self.next_id += 1;

        // Create new pane with same doc_id
        let new_pane = Pane::new(new_pane_id, doc_id);
        self.panes.insert(new_pane_id, new_pane);

        // Create split node
        let left = PaneNode::leaf(self.focused);
        let right = PaneNode::leaf(new_pane_id);
        let split_node = PaneNode::split(dir, left, right, 0.5);

        // Replace the focused leaf with the split
        self.root.replace_leaf(self.focused, split_node);

        // Focus the new pane
        self.focused = new_pane_id;
    }

    /// Compute the rectangle for each pane given the total area
    pub fn compute_layout(&self, area: Rect) -> HashMap<PaneId, Rect> {
        let mut rects = HashMap::new();
        self.compute_layout_recursive(&self.root, area, &mut rects);
        rects
    }

    fn compute_layout_recursive(
        &self,
        node: &PaneNode,
        area: Rect,
        rects: &mut HashMap<PaneId, Rect>,
    ) {
        match node {
            PaneNode::Leaf(id) => {
                rects.insert(*id, area);
            }
            PaneNode::Split { dir, left, right, ratio } => {
                let (left_rect, right_rect) = match dir {
                    SplitDir::Horizontal => {
                        // Split top/bottom
                        let split_y = area.y + (area.height as f32 * ratio) as u16;
                        let top_height = split_y.saturating_sub(area.y);
                        let bottom_height = area.height.saturating_sub(top_height);

                        let top = Rect {
                            x: area.x,
                            y: area.y,
                            width: area.width,
                            height: top_height,
                        };
                        let bottom = Rect {
                            x: area.x,
                            y: split_y,
                            width: area.width,
                            height: bottom_height,
                        };
                        (top, bottom)
                    }
                    SplitDir::Vertical => {
                        // Split left/right
                        let split_x = area.x + (area.width as f32 * ratio) as u16;
                        let left_width = split_x.saturating_sub(area.x);
                        let right_width = area.width.saturating_sub(left_width);

                        let left = Rect {
                            x: area.x,
                            y: area.y,
                            width: left_width,
                            height: area.height,
                        };
                        let right = Rect {
                            x: split_x,
                            y: area.y,
                            width: right_width,
                            height: area.height,
                        };
                        (left, right)
                    }
                };

                self.compute_layout_recursive(left, left_rect, rects);
                self.compute_layout_recursive(right, right_rect, rects);
            }
        }
    }

    /// Close the focused pane. Returns false if this was the last pane.
    pub fn close_focused(&mut self) -> bool {
        let pane_id = self.focused;

        // Get all leaf IDs before closing
        let all_ids = self.root.leaf_ids();

        // If this is the only pane, return false to signal quit
        if all_ids.len() == 1 {
            return false;
        }

        // Remove from storage
        self.panes.remove(&pane_id);

        // Remove from tree by collapsing the parent split
        self.root = self.remove_leaf_from_tree(self.root.clone(), pane_id);

        // Focus on the first remaining pane
        let remaining_ids = self.root.leaf_ids();
        if let Some(first_id) = remaining_ids.first() {
            self.focused = *first_id;
        }

        true
    }

    /// Remove a leaf from the tree, collapsing its parent split
    fn remove_leaf_from_tree(&self, node: PaneNode, target_id: PaneId) -> PaneNode {
        match node {
            PaneNode::Leaf(id) if id == target_id => {
                // This shouldn't happen if called correctly
                node
            }
            PaneNode::Leaf(_) => node,
            PaneNode::Split { dir, left, right, ratio } => {
                // Check if target is in left or right
                let left_ids = left.leaf_ids();
                let right_ids = right.leaf_ids();

                if left_ids.contains(&target_id) && left_ids.len() == 1 {
                    // Left is the target leaf, promote right
                    *right
                } else if right_ids.contains(&target_id) && right_ids.len() == 1 {
                    // Right is the target leaf, promote left
                    *left
                } else {
                    // Target is deeper in the tree, recurse
                    let new_left = self.remove_leaf_from_tree(*left, target_id);
                    let new_right = self.remove_leaf_from_tree(*right, target_id);
                    PaneNode::Split {
                        dir,
                        left: Box::new(new_left),
                        right: Box::new(new_right),
                        ratio,
                    }
                }
            }
        }
    }

    /// Move focus to the next pane in the given direction
    pub fn move_focus(&mut self, direction: Direction, layout: &HashMap<PaneId, Rect>) {
        let current_rect = match layout.get(&self.focused) {
            Some(r) => r,
            None => return,
        };

        // Find the pane center
        let current_center = (
            current_rect.x + current_rect.width / 2,
            current_rect.y + current_rect.height / 2,
        );

        // Find the closest pane in the given direction
        let mut best_pane = None;
        let mut best_distance = u32::MAX;

        for (pane_id, rect) in layout.iter() {
            if *pane_id == self.focused {
                continue;
            }

            let center = (rect.x + rect.width / 2, rect.y + rect.height / 2);

            // Check if this pane is in the right direction
            let in_direction = match direction {
                Direction::Up => center.1 < current_center.1,
                Direction::Down => center.1 > current_center.1,
                Direction::Left => center.0 < current_center.0,
                Direction::Right => center.0 > current_center.0,
            };

            if !in_direction {
                continue;
            }

            // Calculate distance
            let dx = (center.0 as i32 - current_center.0 as i32).abs() as u32;
            let dy = (center.1 as i32 - current_center.1 as i32).abs() as u32;
            let distance = dx * dx + dy * dy;

            if distance < best_distance {
                best_distance = distance;
                best_pane = Some(*pane_id);
            }
        }

        if let Some(new_focus) = best_pane {
            self.focused = new_focus;
        }
    }
}

/// Direction for focus movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_manager_new() {
        let pm = PaneManager::new(0);
        assert_eq!(pm.focused, 0);
        assert_eq!(pm.panes.len(), 1);
        assert!(matches!(pm.root, PaneNode::Leaf(0)));
    }

    #[test]
    fn test_split_pane() {
        let mut pm = PaneManager::new(0);

        // Split vertically
        pm.split_focused(SplitDir::Vertical, 0);

        // Should have 2 panes now
        assert_eq!(pm.panes.len(), 2);
        // Focus should be on new pane (id 1)
        assert_eq!(pm.focused, 1);

        // Root should be a split
        match &pm.root {
            PaneNode::Split { dir, .. } => {
                assert_eq!(*dir, SplitDir::Vertical);
            }
            _ => panic!("Expected split node"),
        }
    }

    #[test]
    fn test_compute_layout() {
        let mut pm = PaneManager::new(0);
        pm.split_focused(SplitDir::Vertical, 0);

        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };

        let layout = pm.compute_layout(area);
        assert_eq!(layout.len(), 2);

        // Should have roughly half width each
        let pane0 = layout.get(&0).unwrap();
        let pane1 = layout.get(&1).unwrap();

        assert_eq!(pane0.width + pane1.width, 100);
        assert_eq!(pane0.height, 50);
        assert_eq!(pane1.height, 50);
    }

    #[test]
    fn test_leaf_ids() {
        let mut pm = PaneManager::new(0);
        pm.split_focused(SplitDir::Vertical, 0);
        pm.split_focused(SplitDir::Horizontal, 0);

        let ids = pm.root.leaf_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&0));
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }
}
