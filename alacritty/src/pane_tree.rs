use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SplitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDir {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub enum PaneNode {
    Leaf { pane_id: PaneId, last_size: (u32, u32) },
    Split { split_id: SplitId, direction: SplitDir, ratio: f32, a: Box<PaneNode>, b: Box<PaneNode> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

#[derive(Serialize, Deserialize)]
pub enum FocusDir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
enum SearchResult {
    NotFound,
    Found,
    Adjusted,
}

#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub enum RemoveResult {
    CollapseToSibling(PaneNode),
    TreeEmptied,
    NotFound,
}

impl Default for PaneNode {
    fn default() -> Self {
        PaneNode::Leaf { pane_id: PaneId(0), last_size: (80, 24) }
    }
}

impl PaneNode {
    
    pub fn leaf(pane_id: PaneId) -> Self {
        PaneNode::Leaf { pane_id, last_size: (80, 24) }
    }

    pub fn split(
        &mut self,
        target_pane: PaneId,
        new_pane_id: PaneId,
        new_split_id: SplitId,
        direction: SplitDir,
    ) -> bool {
        match self {
            PaneNode::Leaf { pane_id, last_size } if *pane_id == target_pane => {
                let old = PaneNode::Leaf { pane_id: *pane_id, last_size: *last_size };
                let new = PaneNode::Leaf { pane_id: new_pane_id, last_size: *last_size };
                *self = PaneNode::Split {
                    split_id: new_split_id,
                    direction,
                    ratio: 0.5,
                    a: Box::new(old),
                    b: Box::new(new),
                };
                true
            },
            PaneNode::Split { a, b, .. } => {
                a.split(target_pane, new_pane_id, new_split_id, direction)
                    || b.split(target_pane, new_pane_id, new_split_id, direction)
            },
            _ => false,
        }
    }

    pub fn remove(&mut self, target_pane: PaneId) -> RemoveResult {
        if self.is_leaf() {
            return if self.leaf_id() == Some(target_pane) {
                RemoveResult::TreeEmptied
            } else {
                RemoveResult::NotFound
            };
        }

        if let PaneNode::Split { a, b, .. } = self {
            if a.is_leaf() && a.leaf_id() == Some(target_pane) {
                return RemoveResult::CollapseToSibling((**b).clone());
            }
            if b.is_leaf() && b.leaf_id() == Some(target_pane) {
                return RemoveResult::CollapseToSibling((**a).clone());
            }

            match a.remove(target_pane) {
                RemoveResult::CollapseToSibling(sibling) => {
                    *a = Box::new(sibling);
                    return RemoveResult::CollapseToSibling(self.clone());
                },
                RemoveResult::TreeEmptied => {
                    *a = Box::new(PaneNode::Leaf { pane_id: PaneId(0), last_size: (80, 24) });
                    return RemoveResult::CollapseToSibling(self.clone());
                },
                RemoveResult::NotFound => match b.remove(target_pane) {
                    RemoveResult::CollapseToSibling(sibling) => {
                        *b = Box::new(sibling);
                        return RemoveResult::CollapseToSibling(self.clone());
                    },
                    RemoveResult::TreeEmptied => {
                        *b = Box::new(PaneNode::Leaf { pane_id: PaneId(0), last_size: (80, 24) });
                        return RemoveResult::CollapseToSibling(self.clone());
                    },
                    other => return other,
                },
            }
        }

        RemoveResult::NotFound
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Leaf { pane_id, .. } => vec![*pane_id],
            PaneNode::Split { a, b, .. } => {
                let mut ids = a.leaf_ids();
                ids.extend(b.leaf_ids());
                ids
            },
        }
    }

    pub fn find_split_ratio_mut(&mut self, target: SplitId) -> Option<&mut f32> {
        match self {
            PaneNode::Split { split_id, ratio, a, b, .. } => {
                if *split_id == target {
                    return Some(ratio);
                }
                a.find_split_ratio_mut(target).or_else(|| b.find_split_ratio_mut(target))
            },
            PaneNode::Leaf { .. } => None,
        }
    }

    /// Adjust the nearest ancestor split ratio in the given direction.
    ///
    /// `grow` = true expands the pane (makes it larger), `grow` = false shrinks it.
    /// For Horizontal splits: `grow` moves the divider right (A grows) or left (B grows).
    /// For Vertical splits: `grow` moves the divider down (A grows) or up (B grows).
    pub fn adjust_ratio(&mut self, pane_id: PaneId, dir: SplitDir, grow: bool, delta: f32) -> bool {
        self.adjust_ratio_impl(pane_id, dir, grow, delta) != SearchResult::NotFound
    }

    fn adjust_ratio_impl(
        &mut self,
        pane_id: PaneId,
        dir: SplitDir,
        grow: bool,
        delta: f32,
    ) -> SearchResult {
        match self {
            PaneNode::Leaf { pane_id: id, .. } => {
                if *id == pane_id { SearchResult::Found } else { SearchResult::NotFound }
            },
            PaneNode::Split { direction, ratio, a, b, .. } => {
                let result_a = a.adjust_ratio_impl(pane_id, dir, grow, delta);
                match result_a {
                    SearchResult::Found => {
                        if *direction == dir {
                            let old_r = *ratio;
                            let sign = if grow { 1.0 } else { -1.0 };
                            *ratio = (*ratio + sign * delta).clamp(0.1, 0.9);
                            // Focus in A: A's right side (near divider) absorbs. B keeps far side.
                            a.fixup_subtree(dir, old_r, *ratio, true);  // A: keep left (far from divider)
                            b.fixup_subtree(dir, 1.0 - old_r, 1.0 - *ratio, false); // B: keep right (far from divider)
                            return SearchResult::Adjusted;
                        }
                        return SearchResult::Found;
                    },
                    SearchResult::Adjusted => return SearchResult::Adjusted,
                    SearchResult::NotFound => (),
                }

                let result_b = b.adjust_ratio_impl(pane_id, dir, grow, delta);
                match result_b {
                    SearchResult::Found => {
                        if *direction == dir {
                            let old_r = *ratio;
                            let sign = if grow { 1.0 } else { -1.0 };
                            *ratio = (*ratio + sign * delta).clamp(0.1, 0.9);
                            // Focus in B: B's left side (near divider) absorbs. A keeps far side.
                            a.fixup_subtree(dir, old_r, *ratio, true);  // A: keep left (far from divider)
                            b.fixup_subtree(dir, 1.0 - old_r, 1.0 - *ratio, false); // B: keep right (far from divider)
                            return SearchResult::Adjusted;
                        }
                        return SearchResult::Found;
                    },
                    SearchResult::Adjusted => return SearchResult::Adjusted,
                    SearchResult::NotFound => (),
                }

                SearchResult::NotFound
            },
        }
    }

    /// After a parent ratio change, keep far-side children at their pixel width.
    /// Only adjusts splits whose direction matches `resize_dir`.
    fn fixup_subtree(&mut self, resize_dir: SplitDir, old_parent_frac: f32, new_parent_frac: f32, keep_left: bool) {
        if new_parent_frac <= 0.0 || old_parent_frac <= 0.0 { return; }
        match self {
            PaneNode::Split { direction, ratio, a, b, .. } => {
                if *direction == resize_dir {
                    let new_r = if keep_left {
                        let old_left = *ratio * old_parent_frac;
                        (old_left / new_parent_frac).clamp(0.1, 0.9)
                    } else {
                        let old_right = (1.0 - *ratio) * old_parent_frac;
                        let new_right = (old_right / new_parent_frac).clamp(0.1, 0.9);
                        1.0 - new_right
                    };
                    *ratio = new_r;
                }
                // Recurse into children.
                let child_old = if *direction == resize_dir { *ratio * old_parent_frac } else { old_parent_frac };
                let child_new = if *direction == resize_dir { *ratio * new_parent_frac } else { new_parent_frac };
                a.fixup_subtree(resize_dir, child_old, child_new, keep_left);
                let child_old_b = if *direction == resize_dir { (1.0 - *ratio) * old_parent_frac } else { old_parent_frac };
                let child_new_b = if *direction == resize_dir { (1.0 - *ratio) * new_parent_frac } else { new_parent_frac };
                b.fixup_subtree(resize_dir, child_old_b, child_new_b, keep_left);
            },
            PaneNode::Leaf { .. } => {},
        }
    }

    /// Get the direction of a split by its ID.
    pub fn split_direction(&self, target: SplitId) -> Option<SplitDir> {
        match self {
            PaneNode::Split { split_id, direction, a, b, .. } => {
                if *split_id == target {
                    Some(*direction)
                } else {
                    a.split_direction(target).or_else(|| b.split_direction(target))
                }
            },
            _ => None,
        }
    }

    /// Find the SplitId of the Nth divider (in leaf_rects order).
    pub fn find_split_by_divider_index(&self, target: usize) -> Option<SplitId> {
        let mut index = 0;
        self.find_split_index_impl(target, &mut index)
    }

    fn find_split_index_impl(&self, target: usize, index: &mut usize) -> Option<SplitId> {
        if let PaneNode::Split { split_id, a, b, .. } = self {
            if *index == target {
                return Some(*split_id);
            }
            *index += 1;
            a.find_split_index_impl(target, index)
                .or_else(|| b.find_split_index_impl(target, index))
        } else {
            None
        }
    }

    pub fn update_size(&mut self, target_pane: PaneId, size: (u32, u32)) {
        match self {
            PaneNode::Leaf { pane_id, last_size } if *pane_id == target_pane => {
                *last_size = size;
            },
            PaneNode::Split { a, b, .. } => {
                a.update_size(target_pane, size);
                b.update_size(target_pane, size);
            },
            _ => {},
        }
    }

    pub fn leaf_rects(&self, viewport: Rect) -> (Vec<(PaneId, Rect)>, Vec<Rect>) {
        let mut panes = Vec::new();
        let mut dividers = Vec::new();
        self.compute_rects(viewport, &mut panes, &mut dividers);
        (panes, dividers)
    }

    fn compute_rects(&self, rect: Rect, panes: &mut Vec<(PaneId, Rect)>, dividers: &mut Vec<Rect>) {
        match self {
            PaneNode::Leaf { pane_id, .. } => {
                panes.push((*pane_id, rect));
            },
            PaneNode::Split { direction, ratio, a, b, .. } => {
                let ratio = ratio.clamp(0.1, 0.9);
                let (ra, divider, rb) = split_rect(rect, *direction, ratio);
                dividers.push(divider);
                a.compute_rects(ra, panes, dividers);
                b.compute_rects(rb, panes, dividers);
            },
        }
    }

    fn is_leaf(&self) -> bool {
        matches!(self, PaneNode::Leaf { .. })
    }

    fn leaf_id(&self) -> Option<PaneId> {
        match self {
            PaneNode::Leaf { pane_id, .. } => Some(*pane_id),
            _ => None,
        }
    }
}

pub fn split_rect(rect: Rect, direction: SplitDir, ratio: f32) -> (Rect, Rect, Rect) {
    let ratio = ratio.clamp(0.1, 0.9);
    let divider_size = 1.0;

    match direction {
        SplitDir::Horizontal => {
            let left_width = ((rect.width - divider_size) * ratio).floor();
            let right_width = rect.width - left_width - divider_size;
            let a = Rect::new(rect.x, rect.y, left_width, rect.height);
            let divider = Rect::new(rect.x + left_width, rect.y, divider_size, rect.height);
            let b = Rect::new(rect.x + left_width + divider_size, rect.y, right_width, rect.height);
            (a, divider, b)
        },
        SplitDir::Vertical => {
            let top_height = ((rect.height - divider_size) * ratio).floor();
            let bottom_height = rect.height - top_height - divider_size;
            let a = Rect::new(rect.x, rect.y, rect.width, top_height);
            let divider = Rect::new(rect.x, rect.y + top_height, rect.width, divider_size);
            let b =
                Rect::new(rect.x, rect.y + top_height + divider_size, rect.width, bottom_height);
            (a, divider, b)
        },
    }
}

#[macro_export]
macro_rules! pane_id {
    ($n:expr) => {
        PaneId($n)
    };
}

#[macro_export]
macro_rules! split_id {
    ($n:expr) => {
        SplitId($n)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: u64) -> PaneNode {
        PaneNode::Leaf { pane_id: PaneId(id), last_size: (80, 24) }
    }

    fn root_with_first_split() -> PaneNode {
        let mut root = leaf(1);
        assert!(root.split(PaneId(1), PaneId(2), SplitId(10), SplitDir::Horizontal));
        root
    }

    #[test]
    fn leaf_ids_single() {
        assert_eq!(leaf(1).leaf_ids(), vec![PaneId(1)]);
    }

    #[test]
    fn split_creates_two_leaves() {
        let root = root_with_first_split();
        assert_eq!(root.leaf_ids(), vec![PaneId(1), PaneId(2)]);
    }

    #[test]
    fn nested_split_three_leaves() {
        let mut root = leaf(1);
        root.split(PaneId(1), PaneId(2), SplitId(10), SplitDir::Horizontal);
        root.split(PaneId(2), PaneId(3), SplitId(11), SplitDir::Vertical);
        assert_eq!(root.leaf_ids(), vec![PaneId(1), PaneId(2), PaneId(3)]);
    }

    #[test]
    fn split_unknown_pane_fails() {
        let mut root = leaf(1);
        assert!(!root.split(PaneId(99), PaneId(2), SplitId(10), SplitDir::Horizontal));
        assert_eq!(root.leaf_ids(), vec![PaneId(1)]);
    }

    #[test]
    fn remove_left_collapse_to_sibling() {
        let mut root = root_with_first_split();
        let result = root.remove(PaneId(1));
        assert!(matches!(result, RemoveResult::CollapseToSibling(_)));
    }

    #[test]
    fn remove_right_apply_collapse() {
        let mut root = root_with_first_split();
        if let RemoveResult::CollapseToSibling(replacement) = root.remove(PaneId(2)) {
            root = replacement;
        }
        assert_eq!(root.leaf_ids(), vec![PaneId(1)]);
    }

    #[test]
    fn remove_last_leaf_empties_tree() {
        let mut root = leaf(1);
        assert_eq!(root.remove(PaneId(1)), RemoveResult::TreeEmptied);
    }

    #[test]
    fn remove_unknown_pane_not_found() {
        let mut root = root_with_first_split();
        assert_eq!(root.remove(PaneId(99)), RemoveResult::NotFound);
    }

    #[test]
    fn resize_split_ratio() {
        let mut root = root_with_first_split();
        let ratio = root.find_split_ratio_mut(SplitId(10)).unwrap();
        *ratio = 0.3;
        if let PaneNode::Split { ratio, .. } = &root {
            assert!((*ratio - 0.3).abs() < 1e-6);
        } else {
            panic!("expected split");
        }
    }

    #[test]
    fn update_size_leaf() {
        let mut root = leaf(1);
        root.update_size(PaneId(1), (40, 12));
        if let PaneNode::Leaf { last_size, .. } = &root {
            assert_eq!(*last_size, (40, 12));
        }
    }

    #[test]
    fn leaf_rects_split_produces_two_rects() {
        let root = root_with_first_split();
        let viewport = Rect::new(0.0, 0.0, 100.0, 50.0);
        let (panes, dividers) = root.leaf_rects(viewport);
        assert_eq!(panes.len(), 2);
        assert_eq!(dividers.len(), 1);
    }

    #[test]
    fn split_rect_horizontal_widths() {
        let rect = Rect::new(0.0, 0.0, 100.0, 50.0);
        let (a, div, b) = split_rect(rect, SplitDir::Horizontal, 0.5);
        assert!(a.width > 0.0);
        assert!(b.width > 0.0);
        assert!(div.width > 0.0);
        assert!((a.width + div.width + b.width - 100.0).abs() < 1.0);
        assert!(a.x < div.x && div.x < b.x);
    }

    #[test]
    fn split_rect_vertical_heights() {
        let rect = Rect::new(0.0, 0.0, 100.0, 50.0);
        let (a, div, b) = split_rect(rect, SplitDir::Vertical, 0.5);
        assert!(a.height > 0.0);
        assert!(b.height > 0.0);
        assert!(div.height > 0.0);
        assert!((a.height + div.height + b.height - 50.0).abs() < 1.0);
    }

    #[test]
    fn leaf_rects_nested_split() {
        let mut root = leaf(1);
        root.split(PaneId(1), PaneId(2), SplitId(10), SplitDir::Horizontal);
        root.split(PaneId(2), PaneId(3), SplitId(11), SplitDir::Vertical);
        let viewport = Rect::new(0.0, 0.0, 100.0, 50.0);
        let (panes, dividers) = root.leaf_rects(viewport);
        assert_eq!(panes.len(), 3);
        assert_eq!(dividers.len(), 2);
    }

    // --- fixup_subtree tests ---

    fn leaf_rect_width(pane_id: u64, root: &PaneNode, vp: &Rect) -> f32 {
        root.leaf_rects(vp.clone()).0.iter().find(|(id,_)| id.0 == pane_id).unwrap().1.width
    }

    fn leaf_rect_height(pane_id: u64, root: &PaneNode, vp: &Rect) -> f32 {
        root.leaf_rects(vp.clone()).0.iter().find(|(id,_)| id.0 == pane_id).unwrap().1.height
    }

    #[test]
    fn fixup_two_pane_horizontal_resize() {
        // [1 | 2] → resize 1 right → [1(wider) | 2(narrower)]
        let mut root = leaf(1);
        root.split(PaneId(1), PaneId(2), SplitId(1), SplitDir::Horizontal);
        let vp = Rect::new(0., 0., 100., 100.);
        let w1 = leaf_rect_width(1, &root, &vp);
        let w2 = leaf_rect_width(2, &root, &vp);
        assert!((w1 - 49.0).abs() < 3.0, "w1={}", w1); // 100 - 1px divider = 99, split at 0.5: 49/50
        assert!((w2 - 50.0).abs() < 3.0, "w2={}", w2);

        root.adjust_ratio(PaneId(1), SplitDir::Horizontal, true, 0.1);
        let w1 = leaf_rect_width(1, &root, &vp);
        let w2 = leaf_rect_width(2, &root, &vp);
        // ratio 0.5 → 0.6. A (left) gets 60%, B gets 40%.
        // 99 * 0.6 ≈ 59, 99 * 0.4 ≈ 39 (with 1px divider)
        assert!(w1 > 55.0 && w1 < 62.0, "w1={}", w1);
        assert!(w2 > 37.0 && w2 < 44.0, "w2={}", w2);
    }

    #[test]
    fn fixup_nested_split_preserves_far_side() {
        // Split(H) { Split(V){1,4}, Split(V){2,3} } — 2x2 grid
        // Focus on pane 3 (bottom-right), resize right → far-side HEIGHTS preserved
        let mut root = leaf(1);
        root.split(PaneId(1), PaneId(2), SplitId(1), SplitDir::Horizontal); // [1|2]
        root.split(PaneId(2), PaneId(3), SplitId(2), SplitDir::Vertical);   // [1|2/3]
        root.split(PaneId(1), PaneId(4), SplitId(3), SplitDir::Vertical);   // [1/4|2/3]
        let vp = Rect::new(0., 0., 200., 200.);

        let h1_before = leaf_rect_height(1, &root, &vp);
        let h2_before = leaf_rect_height(2, &root, &vp);
        let h3_before = leaf_rect_height(3, &root, &vp);
        let h4_before = leaf_rect_height(4, &root, &vp);

        // Resize pane 3 right (Horizontal). Heights should NOT change.
        root.adjust_ratio(PaneId(3), SplitDir::Horizontal, true, 0.1);

        let h1_after = leaf_rect_height(1, &root, &vp);
        let h2_after = leaf_rect_height(2, &root, &vp);
        let h3_after = leaf_rect_height(3, &root, &vp);
        let h4_after = leaf_rect_height(4, &root, &vp);

        // All heights should stay the same — only widths changed
        assert!((h1_after - h1_before).abs() < 3.0,
            "pane 1 height should not change: {} → {}", h1_before, h1_after);
        assert!((h2_after - h2_before).abs() < 3.0,
            "pane 2 height should not change: {} → {}", h2_before, h2_after);
        assert!((h3_after - h3_before).abs() < 3.0,
            "pane 3 height should not change: {} → {}", h3_before, h3_after);
        assert!((h4_after - h4_before).abs() < 3.0,
            "pane 4 height should not change: {} → {}", h4_before, h4_after);
    }

    #[test]
    fn fixup_vertical_nested_preserves_far_side() {
        // Split(V) { Split(H){1,4}, Split(H){2,3} } — 2x2 grid (vertical root)
        // Focus on pane 2 (top-right), resize down → far-side WIDTHS preserved
        let mut root = leaf(1);
        root.split(PaneId(1), PaneId(2), SplitId(1), SplitDir::Vertical);
        root.split(PaneId(2), PaneId(3), SplitId(2), SplitDir::Horizontal);
        root.split(PaneId(1), PaneId(4), SplitId(3), SplitDir::Horizontal);
        let vp = Rect::new(0., 0., 200., 200.);

        let w1_before = leaf_rect_width(1, &root, &vp);
        let w2_before = leaf_rect_width(2, &root, &vp);

        root.adjust_ratio(PaneId(2), SplitDir::Vertical, true, 0.1);

        let w1_after = leaf_rect_width(1, &root, &vp);
        let w2_after = leaf_rect_width(2, &root, &vp);

        // Widths should NOT change — vertical resize affects heights only
        assert!((w1_after - w1_before).abs() < 3.0,
            "pane 1 width should not change: {} → {}", w1_before, w1_after);
        assert!((w2_after - w2_before).abs() < 3.0,
            "pane 2 width should not change: {} → {}", w2_before, w2_after);
    }

    #[test]
    fn deep_nested_split() {
        let mut root = leaf(1);
        root.split(PaneId(1), PaneId(2), SplitId(10), SplitDir::Horizontal);
        root.split(PaneId(2), PaneId(3), SplitId(11), SplitDir::Vertical);
        root.split(PaneId(3), PaneId(4), SplitId(12), SplitDir::Horizontal);
        root.split(PaneId(1), PaneId(5), SplitId(13), SplitDir::Vertical);
        let viewport = Rect::new(0.0, 0.0, 200.0, 100.0);
        let (panes, dividers) = root.leaf_rects(viewport);
        assert_eq!(panes.len(), 5);
        assert_eq!(dividers.len(), 4);
    }

    #[test]
    fn remove_from_nested_collapses_correctly() {
        let mut root = leaf(1);
        root.split(PaneId(1), PaneId(2), SplitId(10), SplitDir::Horizontal);
        root.split(PaneId(2), PaneId(3), SplitId(11), SplitDir::Vertical);
        assert_eq!(root.leaf_ids().len(), 3);

        let result = root.remove(PaneId(2));
        assert!(matches!(result, RemoveResult::CollapseToSibling(_)));
    }
}
