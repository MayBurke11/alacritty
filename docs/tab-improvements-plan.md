# Tab Improvements Plan

> Branch: `tab-dev` · Based on pre-tiling-stable + xlab-tabs · Tag: `tab-ipc-done`

## Phase 0: Stability ✅

### Hysteresis tab title ✅
- `cached_title: Option<String>` in TerminalTab
- Only apply detected_title after two consecutive matches
- Filters out ls/cat/echo automatically — no timers, no perf impact
- Commit: `56b6406f`

## Phase 1: IPC Commands ✅

### `msg create-tab -e <CMD>` ✅
### `msg create-tab --no-switch` ✅
### `msg list-tabs` ✅ ← JSON `[{index, id, title, active}]`
### `msg select-tab <INDEX>` ✅
### `msg close-tab <INDEX>` ✅

- `create_tab_inner(command, wd, no_switch)` — unified creation
- `TabCreateOptions`, `TabSelect`, `TabTarget` — new CLI types
- `CreateTabIPC`, `ListTabsIPC`, `SelectTabIPC`, `CloseTabIPC` — event pipeline
- `SocketReply::ListTabs(String)` — JSON reply over IPC
- 7/7 E2E IPC tests passing
- Tag: `tab-ipc-done`

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
