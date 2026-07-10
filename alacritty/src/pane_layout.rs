//! Edge-adjacency pane layout — rectangles with shared borders.
//!
//! Replaces the binary tree model. Each pane is an independent rectangle.
//! Resizing moves exactly one border segment; only adjacent panes change.

use serde::{Deserialize, Serialize};

// For API compatibility with the old PaneNode-based code.
use crate::pane_tree::SplitId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDir {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusDir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneRect {
    pub id: PaneId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneLayout {
    panes: Vec<PaneRect>,
    pub next_pane_id: u64,
    pub active_pane: PaneId,
    pub zoomed_pane: Option<PaneId>,
}

impl Default for PaneLayout { fn default() -> Self { Self::new() } }

impl PaneLayout {
    pub fn new() -> Self {
        Self {
            panes: vec![PaneRect { id: PaneId(0), x: 0.0, y: 0.0, w: 1.0, h: 1.0 }],
            next_pane_id: 1,
            active_pane: PaneId(0),
            zoomed_pane: None,
        }
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> {
        self.panes.iter().map(|p| p.id).collect()
    }

    // ---------------------------------------------------------------
    // Split
    // ---------------------------------------------------------------

    pub fn split(&mut self, target: PaneId, new_id: PaneId, dir: SplitDir) -> bool {
        let Some(idx) = self.panes.iter().position(|p| p.id == target) else { return false; };
        let r = self.panes[idx].clone();
        if r.w < 0.1 || r.h < 0.1 { return false; }

        match dir {
            SplitDir::Horizontal => {
                let half = r.w * 0.5;
                self.panes[idx].w = half;
                self.panes.push(PaneRect {
                    id: new_id,
                    x: r.x + half,
                    y: r.y,
                    w: r.w - half,
                    h: r.h,
                });
            },
            SplitDir::Vertical => {
                let half = r.h * 0.5;
                self.panes[idx].h = half;
                self.panes.push(PaneRect {
                    id: new_id,
                    x: r.x,
                    y: r.y + half,
                    w: r.w,
                    h: r.h - half,
                });
            },
        }
        self.next_pane_id = self.next_pane_id.max(new_id.0 + 1);
        true
    }

    // ---------------------------------------------------------------
    // Close
    // ---------------------------------------------------------------

    pub fn close(&mut self, target: PaneId) -> bool {
        if self.panes.len() <= 1 { return false; }
        let Some(idx) = self.panes.iter().position(|p| p.id == target) else { return false; };
        let removed = self.panes.remove(idx);

        // Find neighbor sharing the longest edge.
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in self.panes.iter().enumerate() {
            // Shared vertical edge
            let share_right = (p.x - removed.x - removed.w).abs() < 0.001
                && p.y < removed.y + removed.h && p.y + p.h > removed.y;
            let share_left = (removed.x - p.x - p.w).abs() < 0.001
                && removed.y < p.y + p.h && removed.y + removed.h > p.y;
            // Shared horizontal edge
            let share_bottom = (p.y - removed.y - removed.h).abs() < 0.001
                && p.x < removed.x + removed.w && p.x + p.w > removed.x;
            let share_top = (removed.y - p.y - p.h).abs() < 0.001
                && removed.x < p.x + p.w && removed.x + removed.w > p.x;

            let overlap = if share_right || share_left {
                let y0 = p.y.max(removed.y);
                let y1 = (p.y + p.h).min(removed.y + removed.h);
                y1 - y0
            } else if share_bottom || share_top {
                let x0 = p.x.max(removed.x);
                let x1 = (p.x + p.w).min(removed.x + removed.w);
                x1 - x0
            } else {
                continue;
            };

            if overlap > 0.0 && (best.is_none() || overlap > best.unwrap().1) {
                best = Some((i, overlap));
            }
        }

        if let Some((nidx, _)) = best {
            let neighbor = &mut self.panes[nidx];
            // Expand neighbor to cover removed pane's space.
            if (neighbor.x - removed.x - removed.w).abs() < 0.001 {
                // neighbor is to the right, expand leftward
                neighbor.w += neighbor.x - removed.x;
                neighbor.x = removed.x;
            } else if (removed.x - neighbor.x - neighbor.w).abs() < 0.001 {
                // neighbor is to the left, expand rightward
                neighbor.w = removed.x + removed.w - neighbor.x;
            } else if (neighbor.y - removed.y - removed.h).abs() < 0.001 {
                // neighbor is below, expand upward
                neighbor.h += neighbor.y - removed.y;
                neighbor.y = removed.y;
            } else {
                // neighbor is above, expand downward
                neighbor.h = removed.y + removed.h - neighbor.y;
            }
            return true;
        }

        false
    }

    // ---------------------------------------------------------------
    // Resize
    // ---------------------------------------------------------------

    /// Move the border adjacent to `target` in `dir` by `delta` (fraction of viewport).
    /// Positive delta = grow (border moves right/down from the perspective of `target`).
    pub fn resize(&mut self, target: PaneId, dir: SplitDir, delta: f32) -> bool {
        let Some(tidx) = self.panes.iter().position(|p| p.id == target) else { return false; };
        let r = &self.panes[tidx].clone();

        match dir {
            SplitDir::Horizontal => {
                let edge_x = r.x + r.w;
                let edge_y0 = r.y;
                let edge_y1 = r.y + r.h;
                let eps = 0.001;
                let mut left_group: Vec<usize> = Vec::new();
                let mut right_group: Vec<usize> = Vec::new();
                for (i, p) in self.panes.iter().enumerate() {
                    if (p.x + p.w - edge_x).abs() < eps
                        && p.y < edge_y1 && p.y + p.h > edge_y0
                    {
                        left_group.push(i);
                    }
                    if (p.x - edge_x).abs() < eps
                        && p.y < edge_y1 && p.y + p.h > edge_y0
                    {
                        right_group.push(i);
                    }
                }
                if left_group.is_empty() && right_group.is_empty() { return false; }
                for &li in &left_group {
                    self.panes[li].w = (self.panes[li].w + delta).max(0.05);
                }
                for &ri in &right_group {
                    self.panes[ri].x = (self.panes[ri].x + delta).max(0.0);
                    self.panes[ri].w = (self.panes[ri].w - delta).max(0.05);
                }
            },
            SplitDir::Vertical => {
                let edge_y = r.y + r.h;
                let edge_x0 = r.x;
                let edge_x1 = r.x + r.w;
                let eps = 0.001;
                let mut top_group: Vec<usize> = Vec::new();
                let mut bottom_group: Vec<usize> = Vec::new();
                for (i, p) in self.panes.iter().enumerate() {
                    if (p.y + p.h - edge_y).abs() < eps
                        && p.x < edge_x1 && p.x + p.w > edge_x0
                    {
                        top_group.push(i);
                    }
                    if (p.y - edge_y).abs() < eps
                        && p.x < edge_x1 && p.x + p.w > edge_x0
                    {
                        bottom_group.push(i);
                    }
                }
                if top_group.is_empty() && bottom_group.is_empty() { return false; }
                for &ti in &top_group {
                    self.panes[ti].h = (self.panes[ti].h + delta).max(0.05);
                }
                for &bi in &bottom_group {
                    self.panes[bi].y = (self.panes[bi].y + delta).max(0.0);
                    self.panes[bi].h = (self.panes[bi].h - delta).max(0.05);
                }
            },
        }
        true
    }

    // ---------------------------------------------------------------
    // Focus
    // ---------------------------------------------------------------

    pub fn focus(&mut self, target: PaneId, dir: FocusDir) -> Option<PaneId> {
        let Some(tidx) = self.panes.iter().position(|p| p.id == target) else { return None; };
        let r = &self.panes[tidx];
        let cx = r.x + r.w * 0.5;
        let cy = r.y + r.h * 0.5;

        let mut best: Option<(PaneId, f32)> = None;
        for p in &self.panes {
            if p.id == target { continue; }
            let pcx = p.x + p.w * 0.5;
            let pcy = p.y + p.h * 0.5;
            let (in_dir, dist) = match dir {
                FocusDir::Left if pcx < cx => (true, cx - pcx),
                FocusDir::Right if pcx > cx => (true, pcx - cx),
                FocusDir::Up if pcy < cy => (true, cy - pcy),
                FocusDir::Down if pcy > cy => (true, pcy - cy),
                _ => (false, 0.0),
            };
            if !in_dir { continue; }
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((p.id, dist));
            }
        }
        best.map(|(id, _)| id)
    }

    // ---------------------------------------------------------------
    // Geometry
    // ---------------------------------------------------------------

    /// Compute pixel-space pane rects + divider rects from [0,1]-space layout.
    pub fn layout(&self, viewport: Rect) -> (Vec<(PaneId, Rect)>, Vec<Rect>) {
        let mut pane_rects = Vec::new();
        for p in &self.panes {
            let pr = Rect {
                x: p.x * viewport.width + viewport.x,
                y: p.y * viewport.height + viewport.y,
                width: p.w * viewport.width,
                height: p.h * viewport.height,
            };
            pane_rects.push((p.id, pr));
        }

        // Divider rects: any gap between adjacent panes.
        let mut divider_rects = Vec::new();
        let eps = 1.0; // pixel tolerance
        for a in &self.panes {
            for b in &self.panes {
                if a.id.0 >= b.id.0 { continue; }
                // Vertical divider: a's right edge ≈ b's left edge
                let a_right = (a.x + a.w) * viewport.width + viewport.x;
                let b_left = b.x * viewport.width + viewport.x;
                if (a_right - b_left).abs() < 2.0 {
                    let y0 = (a.y.max(b.y) * viewport.height + viewport.y).max(0.0);
                    let y1 = ((a.y + a.h).min(b.y + b.h) * viewport.height + viewport.y).max(0.0);
                    if y1 > y0 + eps {
                        divider_rects.push(Rect {
                            x: a_right,
                            y: y0,
                            width: 1.0,
                            height: y1 - y0,
                        });
                    }
                }
                // Horizontal divider: a's bottom ≈ b's top
                let a_bottom = (a.y + a.h) * viewport.height + viewport.y;
                let b_top = b.y * viewport.height + viewport.y;
                if (a_bottom - b_top).abs() < 2.0 {
                    let x0 = (a.x.max(b.x) * viewport.width + viewport.x).max(0.0);
                    let x1 = ((a.x + a.w).min(b.x + b.w) * viewport.width + viewport.x).max(0.0);
                    if x1 > x0 + eps {
                        divider_rects.push(Rect {
                            x: x0,
                            y: a_bottom,
                            width: x1 - x0,
                            height: 1.0,
                        });
                    }
                }
            }
        }

        (pane_rects, divider_rects)
    }

    // ---------------------------------------------------------------
    // Adapter API — mirrors old PaneNode signatures
    // ---------------------------------------------------------------

    pub fn leaf_rects(&self, viewport: crate::pane_tree::Rect) -> (Vec<(PaneId, crate::pane_tree::Rect)>, Vec<crate::pane_tree::Rect>) {
        let vp = Rect { x: viewport.x, y: viewport.y, width: viewport.width, height: viewport.height };
        let (panes, dividers) = self.layout(vp);
        let panes: Vec<_> = panes.into_iter().map(|(id, r)| {
            (id, crate::pane_tree::Rect { x: r.x, y: r.y, width: r.width, height: r.height })
        }).collect();
        let dividers: Vec<_> = dividers.into_iter().map(|d| {
            crate::pane_tree::Rect { x: d.x, y: d.y, width: d.width, height: d.height }
        }).collect();
        (panes, dividers)
    }

    pub fn adjust_ratio(&mut self, pane_id: PaneId, dir: crate::pane_tree::SplitDir, grow: bool, delta: f32) -> bool {
        let sdir = match dir {
            crate::pane_tree::SplitDir::Horizontal => SplitDir::Horizontal,
            crate::pane_tree::SplitDir::Vertical => SplitDir::Vertical,
        };
        self.resize(pane_id, sdir, if grow { delta } else { -delta })
    }

    pub fn find_split_by_divider_index(&self, _idx: usize) -> Option<SplitId> {
        None // Dividers computed from adjacency, not from a split tree
    }

    pub fn split_direction(&self, _target: SplitId) -> Option<crate::pane_tree::SplitDir> {
        None
    }

    pub fn find_split_ratio_mut(&mut self, _target: SplitId) -> Option<&mut f32> {
        None // Ratios no longer stored per-split
    }

    /// Backward-compat: split that takes SplitId (ignored).
    pub fn split_compat(&mut self, target: PaneId, new_id: PaneId, _split_id: SplitId, dir: crate::pane_tree::SplitDir) -> bool {
        let sdir = match dir {
            crate::pane_tree::SplitDir::Horizontal => SplitDir::Horizontal,
            crate::pane_tree::SplitDir::Vertical => SplitDir::Vertical,
        };
        self.split(target, new_id, sdir)
    }

    /// Backward-compat: remove → close.
    pub fn remove_compat(&mut self, target: PaneId) -> Option<PaneNodeCompat> {
        if self.close(target) { Some(PaneNodeCompat::Collapsed) } else { None }
    }

    // ---------------------------------------------------------------
    // Accessors for compatibility with old API
    // ---------------------------------------------------------------

    pub fn panes(&self) -> &[PaneRect] {
        &self.panes
    }

    pub fn active_rect(&self) -> Option<&PaneRect> {
        self.panes.iter().find(|p| p.id == self.active_pane)
    }
}

/// Compatibility type for old PaneNode::remove return value.
pub enum PaneNodeCompat {
    Collapsed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_one_pane() {
        let l = PaneLayout::new();
        assert_eq!(l.panes.len(), 1);
        assert_eq!(l.panes[0].w, 1.0);
        assert_eq!(l.panes[0].h, 1.0);
    }

    #[test]
    fn split_horizontal() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        assert_eq!(l.panes.len(), 2);
        let p0 = &l.panes[0];
        let p1 = &l.panes.iter().find(|p| p.id == PaneId(1)).unwrap();
        assert!((p0.w - 0.5).abs() < 0.01);
        assert!((p1.w - 0.5).abs() < 0.01);
        assert!(p0.x < p1.x);
    }

    #[test]
    fn split_vertical() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Vertical);
        assert_eq!(l.panes.len(), 2);
        let p0 = &l.panes[0];
        let p1 = &l.panes.iter().find(|p| p.id == PaneId(1)).unwrap();
        assert!((p0.h - 0.5).abs() < 0.01);
        assert!(p0.y < p1.y);
    }

    #[test]
    fn close_absorbs_into_neighbor() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        assert!(l.close(PaneId(1)));
        assert_eq!(l.panes.len(), 1);
        assert!((l.panes[0].w - 1.0).abs() < 0.01);
    }

    #[test]
    fn resize_horizontal_edge() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        // Pane 0: left half, Pane 1: right half.
        // Resize Pane 0 rightward by +0.1.
        l.resize(PaneId(0), SplitDir::Horizontal, 0.1);
        let p0 = l.panes.iter().find(|p| p.id == PaneId(0)).unwrap();
        let p1 = l.panes.iter().find(|p| p.id == PaneId(1)).unwrap();
        assert!((p0.w - 0.6).abs() < 0.02);
        assert!((p1.w - 0.4).abs() < 0.02);
    }

    #[test]
    fn layout_covers_viewport() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.split(PaneId(0), PaneId(2), SplitDir::Vertical);
        let vp = Rect::new(0.0, 0.0, 800.0, 600.0);
        let (rects, _) = l.layout(vp);
        assert_eq!(rects.len(), 3);
        // Check coverage: sum of areas ≈ viewport area.
        let total: f32 = rects.iter().map(|(_, r)| r.width * r.height).sum();
        assert!((total - 480000.0).abs() < 2000.0);
    }
}
