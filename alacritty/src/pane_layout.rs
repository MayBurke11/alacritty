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
        let eps = 0.001;
        match dir {
            SplitDir::Horizontal => {
                // Try right edge first. If no neighbors (viewport boundary), use left edge.
                let (edge_x, use_right) = if (r.x + r.w - 1.0).abs() < eps && delta > 0.0 {
                    (r.x, false) // at right boundary pushing right: use left edge
                } else if r.x.abs() < eps && delta < 0.0 {
                    (r.x + r.w, true) // at left boundary pushing left: use right edge
                } else {
                    (r.x + r.w, true) // default: use right edge
                };

                let mut left: Vec<usize> = vec![]; let mut right: Vec<usize> = vec![];
                for (i, p) in self.panes.iter().enumerate() {
                    if (p.x + p.w - edge_x).abs() < eps && p.y <= r.y + r.h && p.y + p.h >= r.y { left.push(i); }
                    if (p.x - edge_x).abs() < eps && p.y <= r.y + r.h && p.y + p.h >= r.y { right.push(i); }
                }
                if left.is_empty() && right.is_empty() {
                    // Try the other edge
                    let other_edge = if use_right { r.x } else { r.x + r.w };
                    for (i, p) in self.panes.iter().enumerate() {
                        if i == tidx { continue; }
                        if (p.x + p.w - other_edge).abs() < eps && p.y <= r.y + r.h && p.y + p.h >= r.y { left.push(i); }
                        if (p.x - other_edge).abs() < eps && p.y <= r.y + r.h && p.y + p.h >= r.y { right.push(i); }
                    }
                }
                if left.is_empty() && right.is_empty() { return false; }
                if left.is_empty() { left.push(tidx); }
                for &li in &left { self.panes[li].w = (self.panes[li].w + delta).max(0.05); }
                if right.is_empty() { right.push(tidx); }
                for &ri in &right { self.panes[ri].x = (self.panes[ri].x + delta).max(0.0); self.panes[ri].w = (self.panes[ri].w - delta).max(0.05); }
            },
            SplitDir::Vertical => {
                let (edge_y, use_bottom) = if (r.y + r.h - 1.0).abs() < eps && delta > 0.0 {
                    (r.y, false)
                } else if r.y.abs() < eps && delta < 0.0 {
                    (r.y + r.h, true)
                } else {
                    (r.y + r.h, true)
                };

                let mut top: Vec<usize> = vec![]; let mut bot: Vec<usize> = vec![];
                for (i, p) in self.panes.iter().enumerate() {
                    if (p.y + p.h - edge_y).abs() < eps && p.x <= r.x + r.w && p.x + p.w >= r.x { top.push(i); }
                    if (p.y - edge_y).abs() < eps && p.x <= r.x + r.w && p.x + p.w >= r.x { bot.push(i); }
                }
                if top.is_empty() && bot.is_empty() {
                    let other_edge = if use_bottom { r.y } else { r.y + r.h };
                    for (i, p) in self.panes.iter().enumerate() {
                        if (p.y + p.h - other_edge).abs() < eps && p.x <= r.x + r.w && p.x + p.w >= r.x { top.push(i); }
                        if (p.y - other_edge).abs() < eps && p.x <= r.x + r.w && p.x + p.w >= r.x { bot.push(i); }
                    }
                }
                if top.is_empty() && bot.is_empty() { return false; }
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
    fn split_vertical() {
        let mut l = PaneLayout::new(); l.split(PaneId(0), PaneId(1), SplitDir::Vertical);
        assert_eq!(l.panes.len(), 2); assert!((l.panes[0].h - 0.5).abs() < 0.01);
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
    fn resize_right_grows_left_shrinks_right() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.resize(PaneId(0), SplitDir::Horizontal, 0.1);
        let p0 = &l.panes[0]; let p1 = &l.panes[1];
        assert!((p0.w - 0.6).abs() < 0.02, "p0.w={}", p0.w);
        assert!((p1.w - 0.4).abs() < 0.02, "p1.w={}", p1.w);
        assert!((p1.x - 0.6).abs() < 0.02, "p1.x={}", p1.x);
    }

    #[test]
    fn resize_left_shrinks_left_grows_right() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.resize(PaneId(0), SplitDir::Horizontal, -0.1);
        let p0 = &l.panes[0]; let p1 = &l.panes[1];
        assert!((p0.w - 0.4).abs() < 0.02);
        assert!((p1.w - 0.6).abs() < 0.02);
    }

    #[test]
    fn resize_right_from_right_pane() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.resize(PaneId(1), SplitDir::Horizontal, 0.1);
        let p0 = &l.panes[0]; let p1 = &l.panes[1];
        assert!((p0.w - 0.6).abs() < 0.02);
        assert!((p1.w - 0.4).abs() < 0.02);
    }

    #[test]
    fn resize_down_grows_top_shrinks_bottom() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Vertical);
        l.resize(PaneId(0), SplitDir::Vertical, 0.1);
        let p0 = &l.panes[0]; let p1 = &l.panes[1];
        assert!((p0.h - 0.6).abs() < 0.02);
        assert!((p1.h - 0.4).abs() < 0.02);
    }

    #[test]
    fn resize_up_from_bottom_pane() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Vertical);
        l.resize(PaneId(1), SplitDir::Vertical, 0.1);
        let p0 = &l.panes[0]; let p1 = &l.panes[1];
        assert!((p0.h - 0.6).abs() < 0.02);
        assert!((p1.h - 0.4).abs() < 0.02);
    }

    fn make_2x2() -> PaneLayout {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.split(PaneId(1), PaneId(2), SplitDir::Vertical);
        l.split(PaneId(0), PaneId(3), SplitDir::Vertical);
        l.active_pane = PaneId(3);
        l
    }

    #[test]
    fn grid_2x2_resize_right() {
        let mut l = make_2x2();
        eprintln!("Before resize:");
        for p in &l.panes { eprintln!("  id={}: x={:.3} y={:.3} w={:.3} h={:.3}", p.id.0, p.x, p.y, p.w, p.h); }
        l.resize(PaneId(3), SplitDir::Horizontal, 0.1);
        eprintln!("After resize:");
        for p in &l.panes { eprintln!("  id={}: x={:.3} y={:.3} w={:.3} h={:.3}", p.id.0, p.x, p.y, p.w, p.h); }
        let p0 = l.panes.iter().find(|p| p.id == PaneId(0)).unwrap();
        let p3 = l.panes.iter().find(|p| p.id == PaneId(3)).unwrap();
        let p1 = l.panes.iter().find(|p| p.id == PaneId(1)).unwrap();
        let p2 = l.panes.iter().find(|p| p.id == PaneId(2)).unwrap();
        assert!((p0.w - 0.6).abs() < 0.02, "p0.w={}", p0.w);
        assert!((p3.w - 0.6).abs() < 0.02, "p3.w={}", p3.w);
        assert!((p1.w - 0.4).abs() < 0.02, "p1.w={}", p1.w);
        assert!((p2.w - 0.4).abs() < 0.02, "p2.w={}", p2.w);
    }

    #[test]
    fn grid_2x2_focus_left() {
        let l = make_2x2();
        assert_eq!(l.focus(PaneId(3), FocusDir::Left), None);
        assert_eq!(l.focus(PaneId(1), FocusDir::Left), Some(PaneId(0)));
    }

    #[test]
    fn grid_2x2_focus_up() {
        let l = make_2x2();
        assert_eq!(l.focus(PaneId(3), FocusDir::Up), Some(PaneId(0)));
    }

    #[test]
    fn grid_2x2_focus_down() {
        let l = make_2x2();
        let f = l.focus(PaneId(0), FocusDir::Down);
        assert!(f == Some(PaneId(2)) || f == Some(PaneId(3)), "got {:?}", f);
    }

    #[test]
    fn grid_2x2_close_expands_neighbor() {
        let mut l = make_2x2();
        assert!(l.close(PaneId(3)));
        assert_eq!(l.panes.len(), 3);
        let p0 = l.panes.iter().find(|p| p.id == PaneId(0)).unwrap();
        assert!((p0.h - 1.0).abs() < 0.02, "pane 0 should absorb: h={}", p0.h);
    }

    #[test]
    fn cannot_resize_below_min() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.resize(PaneId(0), SplitDir::Horizontal, 0.5);
        let p1 = l.panes.iter().find(|p| p.id == PaneId(1)).unwrap();
        assert!(p1.w >= 0.05, "min width violated: {}", p1.w);
    }

    #[test]
    fn layout_correct_after_split_and_resize() {
        let mut l = PaneLayout::new();
        l.split(PaneId(0), PaneId(1), SplitDir::Horizontal);
        l.split(PaneId(1), PaneId(2), SplitDir::Vertical);
        l.resize(PaneId(0), SplitDir::Horizontal, 0.1);
        let (rects, _) = l.layout(Rect::new(0., 0., 1000., 800.));
        let r0 = rects.iter().find(|(id, _)| *id == PaneId(0)).unwrap().1.clone();
        let r1 = rects.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1.clone();
        let r2 = rects.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1.clone();
        assert!((r0.width - 600.0).abs() < 5.0, "r0.w={}", r0.width);
        assert!((r1.x - 600.0).abs() < 5.0);
        assert!((r2.x - 600.0).abs() < 5.0);
        assert!(r1.y < r2.y);
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
