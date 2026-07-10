# Known Bugs

## Split Down Intermittent Glitch
- **Symptom**: Occasionally when doing `Ctrl+G, p, s, d` (split down), the new pane renders incorrectly or temporarily freezes.
- **Repro**: Hard to catch, happens ~5% of splits.
- **Likely cause**: Race between PTY creation and terminal resize in `split_pane()`. The new pane's shell starts before the damage tracker is updated for the split rect.
- **Status**: Open. Low priority — self-corrects on next frame.

## Menu Bar Top Edge
- **Symptom**: `menu_bar_edge = "top"` doesn't render (bar at negative Y when window padding is 0).
- **Workaround**: Use `menu_bar_edge = "bottom"`.
- **Status**: Open. Requires GL viewport coordinate fix in `draw_bar()`.

## Cursor Not Visible After Split
- **Symptom**: After creating a new pane, cursor not visible (but input works).
- **Cause**: `cursor_blink_timed_out` tracked per-tab, not per-pane.
- **Status**: Open. Minor.

## Divider Color Hardcoded
- **Symptom**: Pane divider color `Rgb(196,154,247)` hardcoded in `window_context.rs:1428`.
- **Status**: Open. Should be config option `panes.divider_color`.
