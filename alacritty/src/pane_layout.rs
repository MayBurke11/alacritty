//! Edge-adjacency pane layout — rectangles with shared borders.
//! Replaces binary tree. Uses pane_tree types for compatibility.

use serde::{Deserialize, Serialize};
pub use crate::pane_tree::{PaneId, Rect, SplitDir, FocusDir, SplitId};

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

impl Default for PaneLayout {
    fn default() -> Self { Self::new() }
}

impl PaneLayout {
    pub fn new() -> Self {
        Self {
            panes: vec![PaneRect { id: PaneId(0), x: 0.0, y: 0.0, w: 1.0, h: 1.0 }],
            next_pane_id: 1, active_pane: PaneId(0), zoomed_pane: None,
        }
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> { self.panes.iter().map(|p| p.id).collect() }

    pub fn split(&mut self, target: PaneId, new_id: PaneId, dir: SplitDir) -> bool {
        let Some(idx) = self.panes.iter().position(|p| p.id == target) else { return false; };
        let r = self.panes[idx].clone();
        if r.w < 0.1 || r.h < 0.1 { return false; }
        match dir {
            SplitDir::Horizontal => {
                let half = r.w * 0.5;
                self.panes[idx].w = half;
                self.panes.push(PaneRect { id: new_id, x: r.x + half, y: r.y, w: r.w - half, h: r.h });
            },
            SplitDir::Vertical => {
                let half = r.h * 0.5;
                self.panes[idx].h = half;
                self.panes.push(PaneRect { id: new_id, x: r.x, y: r.y + half, w: r.w, h: r.h - half });
            },
        }
        self.next_pane_id = self.next_pane_id.max(new_id.0 + 1);
        true
    }

    pub fn close(&mut self, target: PaneId) -> bool {
        if self.panes.len() <= 1 { return false; }
        let Some(idx) = self.panes.iter().position(|p| p.id == target) else { return false; };
        let removed = self.panes.remove(idx);
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in self.panes.iter().enumerate() {
            let share_right = (p.x - removed.x - removed.w).abs() < 0.001 && p.y < removed.y + removed.h && p.y + p.h > removed.y;
            let share_left = (removed.x - p.x - p.w).abs() < 0.001 && removed.y < p.y + p.h && removed.y + removed.h > p.y;
            let share_bottom = (p.y - removed.y - removed.h).abs() < 0.001 && p.x < removed.x + removed.w && p.x + p.w > removed.x;
            let share_top = (removed.y - p.y - p.h).abs() < 0.001 && removed.x < p.x + p.w && removed.x + removed.w > p.x;
            let overlap = if share_right || share_left {
                let y0 = p.y.max(removed.y); let y1 = (p.y + p.h).min(removed.y + removed.h); y1 - y0
            } else if share_bottom || share_top {
                let x0 = p.x.max(removed.x); let x1 = (p.x + p.w).min(removed.x + removed.w); x1 - x0
            } else { continue; };
            if overlap > 0.0 && (best.is_none() || overlap > best.unwrap().1) { best = Some((i, overlap)); }
        }
        if let Some((nidx, _)) = best {
            let n = &mut self.panes[nidx];
            if (n.x - removed.x - removed.w).abs() < 0.001 { n.w += n.x - removed.x; n.x = removed.x; }
            else if (removed.x - n.x - n.w).abs() < 0.001 { n.w = removed.x + removed.w - n.x; }
            else if (n.y - removed.y - removed.h).abs() < 0.001 { n.h += n.y - removed.y; n.y = removed.y; }
            else { n.h = removed.y + removed.h - n.y; }
            return true;
        }
        false
    }

    pub fn resize(&mut self, target: PaneId, dir: SplitDir, delta: f32) -> bool {
        let Some(tidx) = self.panes.iter().position(|p| p.id == target) else { return false; };
        let r = self.panes[tidx].clone();
        match dir {
            SplitDir::Horizontal => {
                let ex = r.x + r.w; let eps = 0.001;
                let mut left: Vec<usize> = vec![]; let mut right: Vec<usize> = vec![];
                let is_right_edge = (ex - 1.0).abs() < eps;
                let is_left_edge = r.x.abs() < eps;
                for (i, p) in self.panes.iter().enumerate() {
                    if i == tidx { continue; } // exclude self
                    if (p.x + p.w - ex).abs() < eps && p.y <= r.y + r.h && p.y + p.h >= r.y { left.push(i); }
                    if (p.x - ex).abs() < eps && p.y <= r.y + r.h && p.y + p.h >= r.y { right.push(i); }
                }
                if left.is_empty() && right.is_empty() && !is_right_edge && !is_left_edge { return false; }
                // Always adjust both sides of the border.
                if left.is_empty() { left.push(tidx); }
                for &li in &left { self.panes[li].w = (self.panes[li].w + delta).max(0.05); }
                if right.is_empty() { right.push(tidx); }
                for &ri in &right { self.panes[ri].x = (self.panes[ri].x + delta).max(0.0); self.panes[ri].w = (self.panes[ri].w - delta).max(0.05); }
            },
            SplitDir::Vertical => {
                let ey = r.y + r.h; let eps = 0.001;
                let mut top: Vec<usize> = vec![]; let mut bot: Vec<usize> = vec![];
                let is_bottom_edge = (ey - 1.0).abs() < eps;
                let is_top_edge = r.y.abs() < eps;
                for (i, p) in self.panes.iter().enumerate() {
                    if i == tidx { continue; }
                    if (p.y + p.h - ey).abs() < eps && p.x <= r.x + r.w && p.x + p.w >= r.x { top.push(i); }
                    if (p.y - ey).abs() < eps && p.x <= r.x + r.w && p.x + p.w >= r.x { bot.push(i); }
                }
                if top.is_empty() && bot.is_empty() && !is_bottom_edge && !is_top_edge { return false; }
                if top.is_empty() { top.push(tidx); }
                for &ti in &top { self.panes[ti].h = (self.panes[ti].h + delta).max(0.05); }
                if bot.is_empty() { bot.push(tidx); }
                for &bi in &bot { self.panes[bi].y = (self.panes[bi].y + delta).max(0.0); self.panes[bi].h = (self.panes[bi].h - delta).max(0.05); }
            },
        }
        true
    }

    pub fn focus(&self, target: PaneId, dir: FocusDir) -> Option<PaneId> {
        let Some(tidx) = self.panes.iter().position(|p| p.id == target) else { return None; };
        let r = &self.panes[tidx];
        let cx = r.x + r.w * 0.5; let cy = r.y + r.h * 0.5;
        let mut best: Option<(PaneId, f32)> = None;
        for p in &self.panes {
            if p.id == target { continue; }
            let pcx = p.x + p.w * 0.5; let pcy = p.y + p.h * 0.5;
            let (in_dir, dist) = match dir {
                FocusDir::Left if pcx < cx => (true, cx - pcx),
                FocusDir::Right if pcx > cx => (true, pcx - cx),
                FocusDir::Up if pcy < cy => (true, cy - pcy),
                FocusDir::Down if pcy > cy => (true, pcy - cy),
                _ => (false, 0.0),
            };
            if !in_dir { continue; }
            if best.is_none() || dist < best.unwrap().1 { best = Some((p.id, dist)); }
        }
        best.map(|(id, _)| id)
    }

    pub fn layout(&self, viewport: Rect) -> (Vec<(PaneId, Rect)>, Vec<Rect>) {
        let mut pr = Vec::new();
        for p in &self.panes {
            pr.push((p.id, Rect::new(p.x * viewport.width + viewport.x, p.y * viewport.height + viewport.y, p.w * viewport.width, p.h * viewport.height)));
        }
        let mut divs = Vec::new();
        for a in &self.panes {
            for b in &self.panes {
                if a.id.0 >= b.id.0 { continue; }
                let ar = a.x + a.w; let bl = b.x;
                if (ar - bl).abs() < 0.03 {
                    let y0 = a.y.max(b.y) * viewport.height + viewport.y;
                    let y1 = (a.y + a.h).min(b.y + b.h) * viewport.height + viewport.y;
                    if y1 > y0 + 1.0 { divs.push(Rect::new(ar * viewport.width + viewport.x, y0, 1.0, y1 - y0)); }
                }
                let ab = a.y + a.h; let bt = b.y;
                if (ab - bt).abs() < 0.03 {
                    let x0 = a.x.max(b.x) * viewport.width + viewport.x;
                    let x1 = (a.x + a.w).min(b.x + b.w) * viewport.width + viewport.x;
                    if x1 > x0 + 1.0 { divs.push(Rect::new(x0, ab * viewport.height + viewport.y, x1 - x0, 1.0)); }
                }
            }
        }
        (pr, divs)
    }

    pub fn panes(&self) -> &[PaneRect] { &self.panes }
    pub fn active_rect(&self) -> Option<&PaneRect> { self.panes.iter().find(|p| p.id == self.active_pane) }

    // Adapter for old leaf_rects return type
    pub fn leaf_rects_compat(&self, vp: Rect) -> (Vec<(PaneId, Rect)>, Vec<Rect>) { self.layout(vp) }
    pub fn adjust_ratio_compat(&mut self, pid: PaneId, dir: SplitDir, grow: bool, delta: f32) -> bool { self.resize(pid, dir, if grow { delta } else { -delta }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_has_one_pane() { let l = PaneLayout::new(); assert_eq!(l.panes.len(), 1); }
    #[test]
    fn split_horizontal() {
        let mut l = PaneLayout::new(); l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        assert_eq!(l.panes.len(), 2); assert!((l.panes[0].w - 0.5).abs() < 0.01);
    }
    #[test]
    fn close_absorbs_into_neighbor() {
        let mut l = PaneLayout::new(); l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        assert!(l.close(PaneId(1))); assert_eq!(l.panes.len(), 1);
    }
    #[test]
    fn resize_horizontal_edge() {
        let mut l = PaneLayout::new(); l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.resize(PaneId(0), SplitDir::Horizontal, 0.1);
        let p0 = l.panes.iter().find(|p| p.id == PaneId(0)).unwrap();
        assert!((p0.w - 0.6).abs() < 0.02);
    }
    #[test]
    fn layout_covers_viewport() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.split(PaneId(0), PaneId(2), SplitDir::Vertical);
        let (rects, _) = l.layout(Rect::new(0.0, 0.0, 800.0, 600.0));
        let total: f32 = rects.iter().map(|(_, r)| r.width * r.height).sum();
        assert!((total - 480000.0).abs() < 2000.0);
    }
}
