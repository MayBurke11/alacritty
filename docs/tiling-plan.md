# Tiling (Pane Tree) — Implementation Plan

> Branch: `tab-dev` · Source: `graphics` branch · Goal: merge tiling into tab-dev with menu integration

## Architecture

### Pane Tree (Binary Space Partition)

```
PaneNode::Leaf { pane_id, last_size }          ← terminal pane
PaneNode::Split { split_id, direction, ratio, a, b }  ← container

SplitDir::Horizontal  → children left-to-right
SplitDir::Vertical    → children top-to-bottom
```

Each tab has a `pane_tree: PaneNode`. Default: single Leaf (no splits).

### Per-Pane State

`PaneState { terminal, pty_master_fd, notifier }` — one per pane.

- `additional_panes: HashMap<PaneId, PaneState>` — all panes beyond the first
- `active_pane: PaneId` — which pane has keyboard focus
- `zoomed_pane: Option<PaneId>` — zoomed pane (full viewport)

### Rendering (Multi-Pane)

Uses existing single-terminal renderer with:
1. `glScissor` → clip to pane rect
2. Cell offset translation → map pane-local cells to viewport
3. Only active pane gets cursor
4. Divider rects drawn on top

### Events

`Event` struct gains `pane_id: Option<PaneId>` — routes keyboard/mouse to specific pane.
`EventProxy` includes `pane_id` in its context.

### Menu Integration

New `[PANE]` menu item:
```
[[menu.items]]
label = "PANE"
[[menu.items.submenu]]
label = "SPLIT ↓"
action = "split-down"
[[menu.items.submenu]]
label = "SPLIT →"
action = "split-right"
[[menu.items.submenu]]
label = "KILL"
action = "close-pane"
[[menu.items.submenu]]
label = "FULL"
action = "toggle-zoom"
```

## Steps

1. **Copy** `pane_tree.rs` + `pane_state.rs` from `graphics`
2. **Port** `TerminalTab` pane fields + methods
3. **Port** `display/mod.rs` multi-pane rendering (scissor, clip, dividers)
4. **Port** `event.rs` — pane_id on Event, EventProxy, TabAction variants
5. **Port** `config/bindings.rs` — pane actions + bindings
6. **Port** `input/keyboard.rs` — pane key handling (Alt+arrows, etc.)
7. **Add** `[PANE]` menu + internal actions (`split-down`, `close-pane`, etc.)
8. **Add** `ResizeMode` to menu (list mode for resize with Alt+arrows)
9. **Test** E2E
