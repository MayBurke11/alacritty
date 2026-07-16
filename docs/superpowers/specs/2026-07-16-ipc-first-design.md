# IPC-First Architecture

Date: 2026-07-16
Status: design

## Summary

Refactor Alacritty-Kitty to use a unified `Action` enum as the single dispatch point for all user-initiated and IPC-initiated operations. Keyboard, menu, `alacritty msg` — everything goes through `WindowContext::handle_action(Action) -> ActionResult`.

## Motivation

Currently there are three separate dispatch paths:

- **Keyboard**: `input/keyboard.rs` → direct method calls on `ActionContext`
- **Menu**: `window_context.rs` → `TabAction` enum → `handle_menu_selection()`
- **IPC**: `event.rs` → `EventType::CreateTabIPC`, `SelectTabIPC`, ... → duplicated logic

This causes:
- Duplicated logic (e.g., create-tab is implemented 3 times)
- IPC is second-class — fire-and-forget, no error responses
- Hard to test — need xdotool/keyboard simulation
- Hard to extend — new feature requires changes in 3 places

## Design Decisions

| Decision | Choice |
|----------|--------|
| Transport | Unix socket + TCP (localhost, optional `--token`) |
| Action scope | Everything user-facing (keyboard, menu, IPC); PTY hot path stays direct |
| Backward compat | Breaking change — old `msg` commands removed |
| Config overrides | Part of Action body (`config` field on CreateTab, NewWindow) |
| Wire format | JSON-RPC-light: `{id, action, params}` → `{id, ok, data}` |

## Action Enum

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "action")]
enum Action {
    // Window
    NewWindow { command: Option<Vec<String>>, cwd: Option<String>, config: Option<Value> },
    CloseWindow,

    // Tab
    CreateTab { command: Option<Vec<String>>, cwd: Option<String>, config: Option<Value>, no_switch: bool },
    CloseTab { index: Option<usize> },
    SelectTab { index: usize },
    ListTabs,
    MoveTab { index: usize, delta: i32 },
    TogglePin { index: usize },
    SetTabTitle { title: String },

    // Pane
    SplitPane { direction: SplitDir },
    ClosePane,
    FocusPane { direction: FocusDir },
    ToggleZoom,
    ResizePane { direction: SplitDir, grow: bool },

    // Session
    SaveSession,
    LoadSession { name: String },
    ListSessions,

    // Config
    GetConfig,
    SetConfig { options: HashMap<String, Value>, reset: bool },

    // Shell
    QuickRun { command: Vec<String>, no_switch: bool },
    Scroll { lines: i32 },
    Copy,
    Paste { data: String },

    // UI
    ToggleMenu,
    MenuNavigate { direction: MenuDir },
    MenuSelect,
    MenuBack,
    MenuLetter { ch: char },

    // Internal
    Bell,
    TitleChanged { title: Option<String> },
    CursorBlink,
    Search { direction: SearchDir },
    SearchNext,
}
```

## ActionResult

```rust
struct ActionResult {
    ok: bool,
    data: Option<serde_json::Value>,
    error: Option<String>,
}
```

- `ok: false, error: "tab index out of range"` — client gets error
- `ok: true, data: Some(json_config)` — for GetConfig, ListTabs
- `ok: true, data: None` — fire-and-forget (CreateTab, etc.)

## IPC Wire Format

Request:
```json
{"id":1,"action":"CreateTab","params":{"command":["htop"],"no_switch":false}}
```

Response:
```json
{"id":1,"ok":true}
```

Notification (no id → no reply):
```json
{"action":"Bell","params":{}}
```

## CLI Syntax

```
alacritty msg window create [-e CMD] [--cwd DIR] [-o K=V]
alacritty msg tab create [-e CMD] [--cwd DIR] [--no-switch] [-w IDX]
alacritty msg tab close [INDEX] [-w IDX]
alacritty msg tab select INDEX [-w IDX]
alacritty msg tab list [-w IDX]
alacritty msg tab move INDEX [+N|-N] [-w IDX]
alacritty msg tab pin INDEX [-w IDX]
alacritty msg tab rename TITLE [-w IDX]
alacritty msg pane split right|down [-w IDX]
alacritty msg pane close [-w IDX]
alacritty msg pane focus left|right|up|down [-w IDX]
alacritty msg pane zoom [-w IDX]
alacritty msg config get [-w IDX]
alacritty msg config set K=V... [-w IDX]
alacritty msg config reset [-w IDX]
alacritty msg session save|load|list [-w IDX]
alacritty msg quickrun CMD... [--no-switch] [-w IDX]
```

Global options:
- `-s, --socket PATH` — Unix socket path
- `-t, --tcp HOST:PORT` — TCP endpoint
- `--token TOKEN` — TCP auth token

## Window Indexing

Each window gets a stable 1-based ordinal index assigned at creation. Indices do not compact on close. Used via `-w IDX` on all subcommands.

## Internal Architecture

```
Keyboard (winit) ──┐
Menu (Ctrl+G)    ──┼──→ Action ──→ WindowContext::handle_action() → ActionResult
IPC (socket/TCP) ──┘
```

`WindowContext` gains:
```rust
fn handle_action(&mut self, action: Action) -> ActionResult;
fn queue_action(&mut self, action: Action);  // for keyboard (non-blocking)
```

Event loop processes queued actions in order: keyboard first, then menu, then IPC.

## Implementation Phases

### Phase 1: Action enum + dispatch (core)
- New file: `alacritty/src/action.rs` (Action enum, ActionResult)
- `WindowContext::handle_action()` — single dispatch
- Keyboard and menu emit Actions through `queue_action()`

### Phase 2: New IPC
- Replace `SocketMessage` with `{action, params}` JSON
- JSON-RPC-light parser/responder in `ipc.rs`
- TCP listener (localhost, `--token` auth)

### Phase 3: Cleanup
- Remove `EventType::*IPC` variants
- Remove `TabAction`, `MenuSelection` duplication
- Replace `SocketReply` with `ActionResult`

### Phase 4: CLI migration
- New `alacritty msg` syntax
- Update `scripts/install.sh`, `README.md`

## Non-Goals

- PTY hot path (write, resize) stays direct — not routed through Action
- Live config reload stays separate
- macOS/Windows IPC (Unix-only for now)

## Risk Mitigation

- Each phase compiles and passes tests independently
- `PtyWrite` stays out of Action to avoid hot-path overhead
- TCP requires `--token` auth — no open network exposure
- Borrow checker: `handle_action` dispatches to helper methods that take partial borrows
