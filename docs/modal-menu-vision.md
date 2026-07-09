# Modal Menu UX — Vision

> Based on Zellij interaction model · Tag: `menu-done`

## The Idea

The (bottom) menu bar should be **context-sensitive**, not static. It serves triple duty:

| Role | What it shows |
|---|---|
| **Status** | Current mode name (locked / normal / pane / tab / session / scroll / resize) |
| **Help** | Available hotkeys with mnemonics (first letter = activator) |
| **Menu** | Clickable items that execute actions |

## Three-Step Chain

```
1. Ctrl+g         — unlock (enter Normal mode)
2. Ctrl+<object>  — choose what to work with
3. <key>          — execute action
```

After step 3, auto-return to Normal or Locked.

## Modes (from Zellij, adapted)

| Mode | Key | Purpose |
|---|---|---|
| **Locked** | — | All keys → terminal. Default state. |
| **Normal** | `Ctrl+g` | Bar shows objects: `TABS │ PANES │ SCROLL │ SESSION │ RESIZE │ MOVE` |
| **Tab** | `Ctrl+t` | Tab actions: new, close, rename, nav, pin, break |
| **Pane** | `Ctrl+p` | Pane actions: split, close, fullscreen, rename |
| **Scroll** | `Ctrl+s` | Scrollback mode |
| **Session** | `Ctrl+o` | Save, detach, session manager |
| **Resize** | `Ctrl+n` | Resize panes |
| **Move** | `Ctrl+h` | Move between panes |

## What We Have vs What's Needed

| Feature | Status |
|---|---|
| Static menu bar (2-level) | Done (`menu-done`) |
| Menu bar as keyboard-accessible bar | **Need: mode switching, hotkey display** |
| Tab bar (click to switch, close) | Done |
| QuickRun (`Ctrl+Shift+P`) | Done |
| Tab CRUD via keyboard | Partial (IPC only) |
| Pane splitting | Not started (tiling deferred) |
| Session save/restore | Done (via IPC) |
| Modal lock/unlock | **Need: locked mode + `Ctrl+g` binding** |

## First Steps

1. Add `Locked` / `Normal` modes with `Ctrl+g` toggle
2. In Normal mode, bar renders clickable object labels: `[T]ABS │ [P]ANES │ [S]CROLL │ [O]SESSION`
3. `Ctrl+t` enters Tab mode, bar shows actions: `[N]ew │ [C]lose │ [R]ename │ [P]in │ ...`
4. Click/press key → execute → return to Normal or Locked
