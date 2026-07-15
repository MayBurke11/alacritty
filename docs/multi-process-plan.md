# Multi-Process Windows — Implementation Plan

## Goal
Every alacritty window = separate process. Daemon manages them. `msg config -w <id>` works for all.

## Architecture

```
alacritty --daemon                    ← PID 1000, only IPC socket, no windows
│
├── alacritty --child-of 1000        ← PID 1001, one window
│   ├── opens IPC socket: Alacritty-:0-1001.sock
│   └── responds to msg config -w <pid>
│
└── alacritty --child-of 1000        ← PID 1002, one window
    └── same
```

## Implementation Steps

### Phase 1: Child Mode (today)
- `--child-of <pid>` flag ✅
- Child skips daemon startup
- Child creates window normally
- Child opens own IPC socket

### Phase 2: Daemon Spawn (today)  
- `create_window` handler calls `Command::new("alacritty").arg("--child-of").arg(daemon_pid).spawn()`
- Daemon tracks: `HashMap<WindowId, ChildPid>`

### Phase 3: Config Routing (this week)
- Daemon has child_pid → computes socket path
- `msg config -w <pid>` → forwards to child's socket via `alacritty msg -s <child_sock> config`

## Changed Files
| File | Changes |
|---|---|
| `cli.rs` | `--child-of` flag ✅ |
| `main.rs` | child mode: skip daemon, create window |
| `event.rs` | fork() instead of in-process; track children; route config |
| `polling/ipc.rs` | no changes (existing `msg config` works on child side) |

## Lines of Code
~80 lines total. Phase 1+2: 50 lines. Phase 3: 30 lines.
