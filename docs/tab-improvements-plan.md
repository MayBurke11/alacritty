# Tab Improvements Plan

> Branch: `graphics-tabs` · Based on pre-tiling-stable + xlab-tabs

## Phase 1: IPC Commands (minimal effort, max impact)

### `msg create-tab -e <CMD>`
- Add `command: Option<Program>` to `create_tab()`
- `WindowOptions` already supports `TerminalOptions.command`
- ~3 lines changed in `window_context.rs`

### `msg create-tab --no-switch`
- Add bool flag, skip `set_active_tab()` when set
- ~2 lines

### `msg close-tab <INDEX>`
- `self.tabs.remove(index)` + tab switch strategy
- ~10 lines

### `msg select-tab <INDEX>`
- `self.set_active_tab(index)` wrapper
- ~5 lines

### `msg list-tabs`
- Return JSON array of `{ id, title, pid, active }`
- ~15 lines, new `SocketMessage::ListTabs` + reply

## Phase 2: QuickRun (inline command runner)

### `Ctrl+Shift+P` — Run command in new tab
```toml
[[keyboard.bindings]]
key = "p"
mods = "Control|Shift"
action = "QuickRun"
```
- Opens inline `"run: "` field in footer
- Same infrastructure as `TabTitleEditor` (keyboard intercept, Enter/Esc)
- On Enter: `create_tab_with_command("htop")` — new tab with command

## Phase 3: Config Presets (medium)

### `[[tabs.presets]]` — startup commands
```toml
[[tabs.presets]]
command = "nvim"

[[tabs.presets]]
command = { program = "cargo", args = ["watch", "-x", "run"] }

[[tabs.presets]]
command = "htop"
```
- Process `presets` in `WindowContext::new()` after first tab created
- Loop: `create_tab()` with each preset's command

### Per-tab working directory override
- `msg create-tab --cwd ~/project`

## Phase 4: Deeper Features (architecture)

### Per-tab config overrides
- `msg create-tab -o 'colors.primary.background=#000'`
- Each tab has its own `Rc<UiConfig>` instead of shared
- Important for: dark editor tab, light shell tab

### Tab pinning
- `tab.pinned: bool` — can't close by accident
- Visual indicator in tab bar (pin icon)

### Save/restore
- `msg save-tabs > layout.json`
- `alacritty --restore layout.json`
- JSON: `[{ command, cwd, title }]`

### Per-tab `search_state` and `message_buffer`
- Already isolated per tab in current implementation — nothing to do here

---

## Priority Order

1. `create-tab -e <CMD>` — unlocks automation
2. `create-tab --no-switch` — background tabs
3. `list-tabs` — foundation for scripting
4. `select-tab` / `close-tab` — completes IPC CRUD
5. QuickRun (`Ctrl+Shift+P`) — inline `run:` → new tab with command
6. `[[tabs.presets]]` — startup tabs in config
7. Per-tab config overrides — dark editor, light shell
8. Save/restore — session management
