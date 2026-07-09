# Modal Menu — What We Built

> Branch: `tab-dev` · Tags: `modal-menu` → `menu-actions-done` → `menu-list-mode` → `menu-tested`

## Menu Bar as Navigation Hub

One horizontal bar (top or bottom), same rendering mechanism as tab bar via `draw_bar(edge)`. Content changes dynamically based on mode.

### Architecture

```
[LOCKED]                         ← default: all keys → terminal
    ↓ Ctrl+G
[ACTIVE] [TAB] [SESS] [FILE]     ← unlocked, items from config
    ↓ t (letter key)               first letter = hotkey, or click
[ BACK ] [RENAME] [CREATE] [KILL] ← submenu of TAB
    ↓ Esc                           back one level
[ACTIVE] [TAB] [SESS]              ← parent level
    ↓ Ctrl+G
[LOCKED]                         ← locked again, bar clean
```

### List Mode (Dynamic Items)

When a menu item has `list = "sessions"`:
```
    ↓ Ctrl+G, s, l (SESS → LOAD)
[ LOAD ] [sess-1] [sess-2] [sess-3]  ← dynamic list, Alt+←/→ to navigate
    ↓ Enter
→ restores session, auto-locks, bar clean
```

## Key Files

| File | Role |
|---|---|
| `alacritty/src/event.rs` | `MenuState` (active, focus, path, list_mode), `MenuOp`, `MenuSelection`, `ListMode` |
| `alacritty/src/window_context.rs` | `handle_menu_selection()`, sync menu op processing after event loop |
| `alacritty/src/input/keyboard.rs` | Ctrl+G toggle, menu key interception (letter keys, Alt+arrows, Enter, Esc) |
| `alacritty/src/display/mod.rs` | `draw_bar(edge)` renders menu labels from `menu_state.bar_labels()` |
| `alacritty/src/config/menu.rs` | `MenuItem` — `label`, `command`, `action`, `list`, `submenu` fields |
| `alacritty/src/config/bindings.rs` | `Action::ToggleLocked` + `Ctrl+G` default binding |

## Internal Actions (Tab Operations)

Leaf menu items with `action = "..."` dispatch directly via event loop (no IPC):

| Action | Effect |
|---|---|
| `create-tab` | New tab |
| `close-tab` | Close active tab |
| `rename-tab` | Enter tab rename editor |
| `pin-tab` | Toggle pin |
| `move-tab-forward` / `move-tab-backward` | Reorder tabs |
| `save-session` | Write JSON to `~/.config/alacritty/sessions/session-{ts}.json` |

## Sync Processing (No Frame Delay)

Menu ops (toggle, focus, select, back, letter keys) use a flag + Vec mechanism to apply state changes in the same frame, not through `event_proxy` with 1-frame delay.

Flow:
```
key_input() → sets menu_op_pending / menu_toggle_pending (flags)
    ↓
event loop (for event in queued_events)
    ↓
post-loop: process menu_toggle_pending + menu_op_pending Vec
    ↓
draw() — state is fresh, no lag
```

## Bugs Fixed

1. **LOCKED + items visible**: `bar_labels()` condition changed from `self.active || !self.path.is_empty()` → `self.active`. Items only show when unlocked.

2. **Stale submenu after Ctrl+G unlock**: `select()` now clears `path` when:
   - Auto-locking after action execution (`self.active = false` → `path.clear()`)
   - ToggleLocked when deactivating
   - List mode selection (restore session)

3. **Frame delay for menu ops**: Replaced async `event_proxy` events with sync flag-based processing.

## Test Coverage

| Test | Path | Count |
|---|---|---|
| E2E Tab basics | `scripts/e2e-tabs-test.sh` | 6 tests (needs xdotool fix) |
| E2E Tab IPC | `scripts/e2e-tabs-ipc-test.sh` | 7 tests (needs socket fix) |
| E2E Session save/restore | `scripts/e2e-session-test.sh` | 7/7 ✅ |
| E2E Menu state machine | `scripts/e2e-menu-state-test.sh` | **18/18 ✅** |

Menu state machine test verifies: unlock, submenu nav, letter-key matching, Escape back, lock, deep nav + Alt+Right + lock + unlock → clean top-level, CREATE tab, KILL tab. Uses `[menu-sync]` debug logs for state verification.

## Config Example

```toml
[menu]
menu_bar_edge = "bottom"

[[menu.items]]
label = "TAB"
[[menu.items.submenu]]
label = "CREATE"
action = "create-tab"
[[menu.items.submenu]]
label = "KILL"
action = "close-tab"
[[menu.items.submenu]]
label = "RENAME"
action = "rename-tab"
[[menu.items.submenu]]
label = "PIN"
action = "pin-tab"

[[menu.items]]
label = "SESS"
[[menu.items.submenu]]
label = "SAVE"
action = "save-session"
[[menu.items.submenu]]
label = "LOAD"
list = "sessions"          # ← dynamic list mode
```
