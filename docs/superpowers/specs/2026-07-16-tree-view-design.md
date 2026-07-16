# Process Tree View (alacritty msg tree)

Date: 2026-07-16
Status: design

## Summary

Add `Action::Tree` that returns a complete snapshot of the alacritty process hierarchy: windows → tabs → panes → processes, with full config for each window and tab. Accessible via `alacritty msg tree` with JSON, NUON, and ASCII-tree output formats.

## Motivation

Currently there is no way to see the full process tree of alacritty. Users need to use external tools (`pstree`, `ps`) to understand what's running. The `msg tree` command provides a task-manager-like view with process info and config properties in a single command.

## Design

### Action

```rust
// No params — always returns full tree.
Action::Tree
```

### CLI

```bash
alacritty msg tree                  # ASCII tree (default)
alacritty msg tree --format json    # JSON output
alacritty msg tree --format nuon    # NUON output (nushell)
```

### JSON Response Structure

```json
{
  "app": {
    "version": "0.20.1",
    "pid": 12345,
    "socket": "/run/user/1000/Alacritty-:0-12345.sock"
  },
  "windows": [{
    "index": 1,
    "id": 88080389,
    "config": { /* full UiConfig as JSON */ },
    "tabs": [{
      "index": 1,
      "id": 1,
      "title": "zsh",
      "active": true,
      "pinned": false,
      "cwd": "/home/user/projects",
      "config": { /* per-tab UiConfig (includes overrides) */ },
      "panes": [{
        "id": 0,
        "active": true,
        "zoomed": false,
        "process": {
          "pid": 12350,
          "command": "zsh",
          "cwd": "/home/user/projects"
        }
      }, {
        "id": 1,
        "direction": "right",
        "process": {
          "pid": 12355,
          "command": "htop"
        }
      }]
    }]
  }]
}
```

### ASCII Output Format

```
alacritty-kitty v0.20.1 (pid 12345)

WINDOW 1 (id 88080389)
├── TAB 1: zsh [active]  font=11.0
│   ├── PANE 0: zsh (pid 12350)
│   └── PANE 1: htop (pid 12355) [right]
└── TAB 2: bash
    └── PANE 0: bash (pid 12360)
```

Config properties shown per-tab: font.size, cursor.style, opacity.
Full config available via `--format json`.

### Implementation

1. **`alacritty/src/action.rs`**: Add `Tree` variant to Action enum
2. **`alacritty/src/window_context.rs`**: Add `handle_action` arm that builds the tree JSON
3. **`alacritty/src/window_context.rs`**: Add helper `build_tree_json()` collecting windows/tabs/panes/configs
4. **`alacritty/src/event.rs`**: Add `EventType::Tree` or reuse `IpcAction`
5. **`alacritty/src/cli.rs`**: Add `SocketMessage::Tree` variant with `--format` flag
6. **`alacritty/src/polling/ipc.rs`**: Handle Tree action with reply
7. **`alacritty/src/main.rs`**: Format output as ASCII tree when no `--format` flag

### Process Info Collection

- **PID**: `tab.shell_pid` for pane 0, `pane.shell_pid` for additional panes
- **Command**: `util::foreground_process_name(master_fd, shell_pid)` (already exists)
- **CWD**: `daemon::foreground_process_path(master_fd, shell_pid)` (already exists)
- **Config**: `serde_json::to_value(&*tab.config)` for per-tab config

### Output Format Selection

- Default (no flag): ASCII tree printed to stdout
- `--format json`: Raw JSON to stdout (pipe to jq/nu/python)
- `--format nuon`: JSON converted to NUON format (TOML-like)

## Non-Goals

- Real-time updates (snapshot only)
- Process CPU/memory stats (use htop/btm for that)
- Tree for non-current windows (only current window's tabs/panes shown)
- Inter-process relationships (parent PID chains beyond direct children)

## Testing

- `cargo test -p alacritty` — no regressions
- Manual: `alacritty msg tree` → ASCII output visible
- Manual: `alacritty msg tree --format json | jq .windows[0].tabs[0].panes` → process info correct
