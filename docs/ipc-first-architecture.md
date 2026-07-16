# Alacritty-Kitty v0.20.0: IPC-First Architecture

## До: три раздельных мира

До этой итерации любое действие в Alacritty проходило одним из трёх путей:

```
Клавиатура (Ctrl+Shift+T)
  → input/keyboard.rs
  → ActionContext методы напрямую
  → создание таба

Меню (Ctrl+G, t, c)
  → TabAction enum
  → window_context.rs handle_menu_selection()
  → создание таба

IPC (alacritty msg create-tab)
  → SocketMessage JSON через Unix-сокет
  → EventType::CreateTabIPC
  → event.rs хендлер
  → создание таба
```

**Три пути → три реализации одного и того же.** Добавление новой фичи требовало правок в трёх местах. IPC был второсортным — без ответов, без подтверждений. Клавиатура и меню вообще не имели отношения к IPC.

## После: единый поток

Теперь **все** действия проходят через одну точку входа:

```
Клавиатура ──┐
Меню ────────┼──→ Action ──→ WindowContext::handle_action() → ActionResult
IPC ─────────┘      ↑
                    │
             35 вариантов:
             CreateTab, SplitPane,
             FocusPane, SaveSession,
             GetConfig, SetConfig,
             QuickRun, Bell, Scroll...
```

### Action — сердце системы

```rust
enum Action {
    CreateTab { command: Option<Vec<String>>, no_switch: bool, ... },
    SplitPane { direction: SplitDir },
    FocusPane { direction: FocusDir },
    SaveSession,
    GetConfig,
    SetConfig { options: HashMap<String, Value> },
    Bell,
    Scroll { lines: i32 },
    // ... ещё 26 вариантов
}
```

`Action` сериализуется в JSON — один enum для клавиатуры, меню и IPC:

```json
{"action":"create_tab","command":["htop"],"no_switch":false}
{"action":"split_pane","direction":"right"}
{"action":"focus_pane","direction":"left"}
{"action":"get_config"}
```

## Жизненный цикл команды

### Клавиатура

```
1. Пользователь жмёт Ctrl+Shift+T
2. input/keyboard.rs находит биндинг → TabAction::Create
3. window_context.rs конвертирует TabAction → Action::CreateTab
4. handle_action(Action::CreateTab) → создаёт таб
5. ActionResult::success() → флаг dirty → перерисовка
```

### Меню

```
1. Ctrl+G → TAB → CREATE
2. MenuState::select() → MenuSelection::Action("create-tab")
3. handle_menu_selection() → Action::CreateTab
4. handle_action(...) → создаёт таб
```

### IPC (JSON-RPC-light)

```
1. echo '{"id":1,"action":"create_tab","command":["htop"]}' | socat - UNIX-CONNECT:$SOCK
2. ipc.rs парсит IpcRequest { id: 1, action: CreateTab { ... } }
3. event_proxy.send_event(EventType::IpcAction(action))
4. window_context.rs получает событие → handle_action(action)
5. ActionResult::success()
6. Ответ клиенту: {"id":1,"ok":true}
```

## Что это даёт

### 1. IPC — гражданин первого класса

Раньше: `alacritty msg create-tab -e htop` — и всё, ответа нет.
Сейчас:

```bash
# С ответом
alacritty msg exec '{"id":1,"action":"get_config"}'
# {"id":1,"ok":true,"data":{"font":{"size":11.0},...}}

# Иерархический CLI
alacritty msg tab create -e htop
alacritty msg pane split right
alacritty msg config set font.size=14
```

### 2. TCP — удалённое управление

```bash
alacritty --tcp-addr 127.0.0.1:9090 --token mysecret &
echo 'mysecret' | nc 127.0.0.1 9090
echo '{"action":"create_tab","command":["htop"]}' | nc 127.0.0.1 9090
```

### 3. Скрипты и автоматизация

```python
import socket, json
s = socket.socket(socket.AF_UNIX)
s.connect("/run/user/1000/Alacritty-:0-*.sock")
s.sendall(json.dumps({"action":"split_pane","direction":"right"}).encode() + b"\n")
s.sendall(json.dumps({"action":"save_session"}).encode() + b"\n")
```

### 4. Единое поведение

`Ctrl+Shift+T` и `alacritty msg tab create` — один и тот же код. Меньше багов, легче тестировать.

## Состав изменений

| Файл | Что |
|---|---|
| `alacritty/src/action.rs` | **Новый** — Action enum, ActionResult |
| `alacritty/src/window_context.rs` | handle_action() — единый dispatch |
| `alacritty/src/input/keyboard.rs` | TabAction → Action |
| `alacritty/src/event.rs` | EventType::IpcAction |
| `alacritty/src/cli.rs` | Иерархический CLI (tab/pane/window/session/config) |
| `alacritty/src/polling/ipc.rs` | IpcRequest/IpcResponse, TCP listener |
| `alacritty/src/main.rs` | TCP + --token CLI флаги |

**22 коммита, 149 тестов проходят, 0 регрессий для пользователя.**
