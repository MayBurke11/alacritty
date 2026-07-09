# Unified Bar Rendering

> Tag: `menu-done` · Commit: `798004e9`

## How it works

Both the tab bar and menu bar use the **same** `draw_bar()` function:

```rust
fn draw_bar(
    &mut self,
    config: &UiConfig,
    titles: &[(String, bool)],   // (label, is_active)
    line: usize,                  // grid line for positioning
    edge: TabBarEdge,             // Top | Bottom — where to stick
)
```

| | Tabs | Menu |
|---|---|---|
| `titles` | Real tabs with active/inactive | All items as `(label, false)` — always inactive |
| `edge` | `config.tabs.tab_bar_edge` | `config.menu.menu_bar_edge` mapped to `TabBarEdge` |
| `line` | Computed from edge + search/message bars | Same formula |
| Colors | Active/inactive from tab config | All use `inactive_tab_*` colors |
| Viewport | `reserve_lines` for `tab_bar_lines` | `reserve_lines` for `menu_bar_lines` |
| Hit boxes | `tab_hit_boxes` | Extracted to `menu_hit_boxes` |

## Viewport reservation

Both bars request lines from `handle_update()`:

```rust
new_size.reserve_lines(
    message_bar_lines + search_lines + tab_bar_lines + menu_bar_lines + tab_title_editor_lines,
);
```

This ensures room for both bars at any window size — no clipping.

## Submenu

When a menu item is expanded, a third `draw_bar()` call renders the submenu on the adjacent line:
- `Bottom` edge: `saturating_sub(1)` (submenu ABOVE menu)
- `Top` edge: `+ 1` (submenu BELOW menu)
