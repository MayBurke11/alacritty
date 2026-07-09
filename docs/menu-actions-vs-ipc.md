# Menu Actions vs IPC — Architecture

## Two Ways to Execute Menu Items

When the user selects a leaf menu item (click, Enter, or hotkey letter), the item must execute something. There are two mechanisms:

### 1. External Command (`command` field)

```toml
[[menu.items.submenu]]
label = "New Tab"
command = { program = "alacritty", args = ["msg", "create-tab"] }
```

**Flow:**
```
Menu item selected
  → std::process::Command::new("alacritty").args(["msg", "create-tab"]).spawn()
    → New OS process starts
      → Connects to $XDG_RUNTIME_DIR/Alacritty-:0-<pid>.sock (Unix socket)
        → Sends JSON: {"message": "CreateTab", ...}
          → Alacritty daemon reads socket, deserializes, creates Event
            → Event loop processes: new tab created
```

**Pros:** Works for any external tool/script, fully config-based, no code changes needed.
**Cons:** Extra process, socket round-trip, JSON serialization overhead. Needs `--window-id` or socket discovery. Fragile — socket path depends on PID.

### 2. Internal Action (`action` field)

```toml
[[menu.items.submenu]]
label = "KILL"
action = "close-tab"
```

**Flow:**
```
Menu item selected
  → MenuState::select() returns MenuSelection::Action("close-tab")
    → WindowContext::handle_menu_selection() matches "close-tab"
      → event_proxy.send_event(Event::new(EventType::Tab(TabAction::Close), window_id))
        → Same event loop picks it up immediately
          → WindowContext processes TabAction::Close
            → Tab is closed
```

No external process. No socket. No JSON. An `Event` is pushed into the **same** winit event loop's internal queue and processed on the next iteration.

**Pros:** Instant, zero-overhead, no socket discovery. Lives entirely inside the running process.
**Cons:** Requires code to be added (new action string + match arm in `handle_menu_selection`).

## Registered Internal Actions

| Action string | What it does | Equivalent binding |
|---|---|---|
| `create-tab` | Create a new tab (same shell) | `Ctrl+Shift+T` |
| `close-tab` | Close the active tab | `Ctrl+Shift+W` |
| `rename-tab` | Enter tab rename editor | `Ctrl+Alt+Shift+T` |
| `pin-tab` | Toggle pin on active tab | `Ctrl+Shift+Alt+P` |
| `move-tab-forward` | Move active tab right | `Ctrl+Shift+.` |
| `move-tab-backward` | Move active tab left | `Ctrl+Shift+,` |

## Architecture: The Event Bridge

```
                        ┌─────────────────┐
                        │  MenuItem::action │
                        │  "close-tab"      │
                        └────────┬────────┘
                                 │ select()
                                 ▼
                    ┌────────────────────┐
                    │  MenuSelection::    │
                    │  Action("close-tab")│
                    └────────┬───────────┘
                             │ handle_menu_selection()
                             ▼
                    ┌────────────────────┐
                    │  TabAction::Close   │  ← existing event variant
                    └────────┬───────────┘
                             │ event_proxy.send_event()
                             ▼
                    ┌────────────────────┐
                    │  WindowContext      │
                    │  handle_event()     │
                    │  → self.close_tab() │
                    └────────────────────┘
```

The menu is just a **new input surface** that dispatches the same `TabAction` events that keyboard bindings already produce. No new logic in tab management — the menu piggybacks on existing infrastructure.

## When to Use Which

| Use `command` for... | Use `action` for... |
|---|---|
| Launching external tools (editor, browser) | Tab operations (create, close, rename, pin, move) |
| Running arbitrary scripts | Built-in Alacritty functionality |
| Anything not Alacritty-specific | Anything that already has a keyboard binding |

## Adding a New Internal Action

1. Add the action string to the match in `WindowContext::handle_menu_selection()`
2. Send the appropriate `TabAction::*` event (or handle directly)
3. That's it — no serialization, no socket, no new event types
