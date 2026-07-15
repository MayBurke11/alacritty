//! Pane tree manager — holds pane data structures.
//! Extracted from TerminalTab for cleaner architecture.

use crate::pane_tree::{PaneId, PaneNode, SplitDir};
use crate::pane_state::PaneState;

/// Holds the pane tree and all pane-related state.
pub struct PaneManager {
    pub tree: PaneNode,
    pub active: PaneId,
    pub additional: std::collections::HashMap<PaneId, PaneState>,
    pub next_id: u64,
    pub zoomed: Option<PaneId>,
}

impl PaneManager {
    pub fn new() -> Self {
        Self {
            tree: PaneNode::leaf(PaneId(0)),
            active: PaneId(0),
            additional: Default::default(),
            next_id: 1,
            zoomed: None,
        }
    }

    pub fn leaf_ids(&self) -> Vec<PaneId> { self.tree.leaf_ids() }
    pub fn is_multi_pane(&self) -> bool { self.leaf_ids().len() > 1 }
}
