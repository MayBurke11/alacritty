# Alacritty IPC: управление через сокет

> Версия: 0.18.0-dev · Только Linux/macOS (Unix domain sockets)

## Архитектура

```
┌─────────────────────────────────────────────────┐
│  alacritty (процесс-сервер)                      │
│  ┌──────────┐    ┌────────────────────────────┐ │
│  │ I/O poll │    │  Event Loop (winit)         │ │
│  │  thread  │───▶│  EventType::CreateWindow    │ │
│  │          │    │  EventType::IpcConfig       │ │
│  │  Unix    │    │  EventType::IpcGetConfig    │ │
│  │  socket  │    └────────────────────────────┘ │
│  └──────────┘                                   │
│       ▲                                         │
└───────┼─────────────────────────────────────────┘
        │  JSON (одна строка)
┌───────┴─────────────────────────────────────────┐
│  alacritty msg <subcommand>                     │
│  ┌──────────────────────────────────────────┐   │
│  │  SocketMessage (serde_json)              │   │
│  │  ├─ CreateWindow(WindowOptions)          │   │
│  │  ├─ Config(IpcConfig)                    │   │
│  │  └─ GetConfig(IpcGetConfig)              │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

Сокет создаётся в `$XDG_RUNTIME_DIR/alacritty/Alacritty-:0-<pid>.sock`. Путь экспортируется в `ALACRITTY_SOCKET`.

---

## Подкоманды `alacritty msg`

### 1. `create-window` — новое окно в том же процессе

```bash
alacritty msg create-window [OPTIONS]
```

Опции:

| Флаг | Описание |
|---|---|
| `--working-directory <DIR>` | Рабочая директория shell |
| `--hold` | Не закрывать окно после выхода child-процесса |
| `-e`, `--command <CMD>...` | Команда и аргументы (должна быть последней) |
| `-T`, `--title <TITLE>` | Заголовок окна |
| `--class <CLASS>` | Window class/app_id (X11/Wayland) |
| `-o`, `--option <OPTION>...` | Переопределение конфига: `'cursor.style="Beam"'` |

```bash
# Открыть htop в новом окне
alacritty msg create-window -e htop

# Открыть с кастомным заголовком
alacritty msg create-window -T "My Server" -e ssh myserver

# Открыть с переопределённым конфигом
alacritty msg create-window -o 'font.size=14' -o 'window.opacity=0.9'
```

**Позиционирование окна через IPC невозможно.** Позиция задаётся только в статическом конфиге:

```toml
[window]
position = { x = 100, y = 200 }
```

### 2. `config` — горячее обновление конфига

```bash
alacritty msg config [OPTIONS] <OPTIONS>...
```

| Флаг | Описание |
|---|---|
| `-w <ID>` | ID окна (или `-1` для всех окон) |
| `-r` | Сбросить все runtime-переопределения |
| `<OPTIONS>` | TOML-ключи: `'font.size=14'` |

```bash
# Изменить размер шрифта во всех окнах
alacritty msg config 'font.size=16'

# Изменить прозрачность в конкретном окне
alacritty msg config -w 12345 'window.opacity=0.8'

# Сбросить все переопределения
alacritty msg config -w -1 -r
```

Переопределения через IPC применяются поверх файлового конфига и живут до перезапуска или `-r`.

### 3. `get-config` — чтение текущего конфига

```bash
alacritty msg get-config [-w <ID>]
```

Возвращает полный конфиг в JSON. `-w -1` — глобальный конфиг.

```bash
# Сохранить текущий конфиг
alacritty msg get-config > runtime-config.json

# Прочитать конкретное окно
alacritty msg get-config -w 12345 | jq '.font.size'
```

---

## Флаги `alacritty`

### `--daemon`

```bash
alacritty --daemon
```

Запускает процесс **без начального окна**. Только создаёт сокет и ждёт команд `create-window`. Выводит путь к сокету для shell-интеграции:

```bash
$ alacritty --daemon &
ALACRITTY_SOCKET=/run/user/1000/alacritty/Alacritty-:0-12345.sock; export ALACRITTY_SOCKET

# Теперь можно запускать окна
alacritty msg create-window -e htop
```

В daemon-режиме event loop **не выходит** при закрытии последнего окна — процесс живёт, пока его не убьют.

### `-e`, `--command`

```bash
alacritty -e <CMD> [ARGS...]
```

Запускает указанную команду вместо shell. Должна быть **последним** аргументом.

```bash
alacritty -e bash -c "echo hello && sleep 5"
alacritty -e nvim
alacritty -e ssh user@host
```

---

## Программное использование

### Поиск сокета

```bash
# Способ 1: переменная окружения (самый надёжный)
echo $ALACRITTY_SOCKET

# Способ 2: сканирование файловой системы
ls $XDG_RUNTIME_DIR/alacritty/Alacritty-*.sock
```

### Отправка сообщений напрямую (без CLI)

Формат — одна строка JSON через Unix-сокет:

```python
import socket, json, os

def find_socket():
    """Найти сокет Alacritty."""
    path = os.environ.get("ALACRITTY_SOCKET")
    if path:
        return path
    runtime = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
    alacritty_dir = os.path.join(runtime, "alacritty")
    for f in os.listdir(alacritty_dir):
        if f.startswith("Alacritty-") and f.endswith(".sock"):
            return os.path.join(alacritty_dir, f)
    raise FileNotFoundError("Alacritty socket not found")

def send_message(message: dict) -> str | None:
    """Отправить сообщение и прочитать ответ."""
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(find_socket())
    payload = json.dumps(message) + "\n"
    sock.sendall(payload.encode())
    sock.shutdown(socket.SHUT_WR)
    reply = b""
    while True:
        chunk = sock.recv(4096)
        if not chunk:
            break
        reply += chunk
    sock.close()
    return reply.decode() if reply else None

# Создать окно
msg = {"CreateWindow": {
    "terminal_options": {
        "command": ["htop"],
        "working_directory": None,
        "hold": False
    },
    "window_identity": {"title": None, "class": None},
    "window_config_overrides": {"font.size": 14}
}}
send_message(msg)

# Прочитать конфиг
msg = {"GetConfig": {"window_id": None}}
config_json = send_message(msg)
print(config_json)

# Изменить конфиг
msg = {"Config": {
    "options": ['window.opacity=0.85'],
    "window_id": -1,
    "reset": False
}}
send_message(msg)
```

### Обёртка на Rust

```rust
use std::os::unix::net::UnixStream;
use std::io::{Write, BufRead, BufReader};

fn send_ipc(socket_path: &str, msg: &str) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(socket_path)?;
    stream.write_all(msg.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut reply = String::new();
    BufReader::new(&stream).read_line(&mut reply)?;
    Ok(reply)
}
```

---

## MCP-сервер на базе IPC

Можно построить MCP-сервер, который через Alacritty IPC:

```
Claude → MCP Server → alacritty msg → Alacritty (окна)
```

**Что можно:**

| MCP Tool | Реализация |
|---|---|
| `create_terminal` | `alacritty msg create-window -e <cmd>` |
| `set_config` | `alacritty msg config 'font.size=<N>'` |
| `get_config` | `alacritty msg get-config` |
| `list_windows` | Сканирование сокетов + `get_config -w <id>` |
| `run_command` | `create-window -e <cmd> --hold` |

**Что нельзя через IPC:**

| Действие | Почему |
|---|---|
| Послать клавиши в окно | IPC только управляет окнами, не вводом |
| Прочитать содержимое терминала | Нет PTY-прокси через IPC |
| Позиционировать окно | Только статический конфиг |
| Закрыть окно | Нет `close-window` в протоколе |
| Сделать скриншот | Нет графического доступа |

**Примерная архитектура MCP:**

```typescript
// mcp-server.ts
import { spawn } from "child_process";

function findSocket(): string { /* как в Python-примере выше */ }

const tools = {
  create_terminal: async (args: { command: string; title?: string }) => {
    const cmd = ["msg", "create-window", "-e", ...args.command.split(" ")];
    if (args.title) cmd.push("-T", args.title);
    spawn("alacritty", cmd);
    return { success: true };
  },
  set_config: async (args: { key: string; value: string }) => {
    spawn("alacritty", ["msg", "config", `${args.key}=${args.value}`]);
    return { success: true };
  },
  get_config: async () => {
    const { stdout } = await exec("alacritty msg get-config");
    return JSON.parse(stdout);
  },
};
```

---

## Оптимизации и трюки

### 1. Быстрый запуск через daemon

```bash
# В ~/.profile или ~/.bashrc
if ! pgrep -f "alacritty --daemon" > /dev/null; then
    eval "$(alacritty --daemon)"
fi

# Теперь открытие окна мгновенное
alias ht='alacritty msg create-window -e htop'
alias nv='alacritty msg create-window -e nvim'
```

Окна в daemon-режиме разделяют один процесс → экономят память (~30-50 MB на окно вместо ~80 MB каждое).

### 2. Shell-интеграция

```bash
# Открыть файл в редакторе в новом окне
edit() { alacritty msg create-window -e nvim "$@"; }

# Открыть проект в tmux-подобном workflow
dev() {
    alacritty msg create-window -T "dev: server" -e npm run dev
    sleep 1
    alacritty msg create-window -T "dev: editor" -e nvim
}

# Переключение темы на лету
theme() {
    alacritty msg config "import=['~/.config/alacritty/themes/${1}.toml']"
}
```

### 3. Горячее переключение профилей

```bash
# Презентация: крупный шрифт + минимализм
present() {
    alacritty msg config -w -1 'font.size=24' 'window.padding.x=0' 'window.padding.y=0'
}

# Кодинг: шрифт поменьше + паддинги
code() {
    alacritty msg config -w -1 'font.size=12' 'window.padding.x=5' 'window.padding.y=5'
}
```

### 4. Динамическая прозрачность

```bash
# Уменьшить прозрачность (больше непрозрачности)
focus() { alacritty msg config -w -1 'window.opacity=1.0'; }

# Увеличить прозрачность (для фона)
blur() { alacritty msg config -w -1 'window.opacity=0.7'; }
```

### 5. Автоматизация рабочих пространств

```bash
#!/bin/bash
# workspace.sh — открыть набор окон для проекта
PROJECT="$1"

alacritty msg create-window -T "$PROJECT: editor" -e bash -c "cd ~/code/$PROJECT && nvim"
alacritty msg create-window -T "$PROJECT: shell"  -e bash -c "cd ~/code/$PROJECT && fish"
alacritty msg create-window -T "$PROJECT: logs"   -e bash -c "cd ~/code/$PROJECT && docker-compose logs -f"
```

---

## Ограничения

| Ограничение | Детали |
|---|---|
| **Только Unix** | IPC сокеты только на Linux/macOS, не на Windows |
| **JSON, одна строка** | Сообщение должно умещаться в одну строку |
| **Нет управления окнами** | Нельзя закрыть, переместить, изменить размер окна через IPC |
| **Нет ввода** | Нельзя послать клавиши/текст в терминал |
| **Нет PTY-доступа** | Нельзя читать вывод терминала |
| **Нет per-window информации** | `get-config` возвращает только конфиг, не состояние окна |
| **Один процесс** | Все окна в одном процессе — креш одного потенциально роняет все |

---

## Итог: что IPC умеет и не умеет

**Умеет:**
- Создавать окна с кастомной командой, заголовком, классом, конфигом
- Менять конфиг на лету (шрифт, цвета, прозрачность, паддинги — всё что в TOML)
- Читать текущий конфиг
- Работать в daemon-режиме (нет начального окна, ждёт команд)

**Для MCP нужно дополнить:**
- `close-window` — закрыть окно по ID
- `send-keys` — послать клавиши в окно
- `read-output` — прочитать содержимое терминала (через PTY)
- `set-position` — задать позицию окна

Последние два — значительные архитектурные изменения (нужен доступ к PTY из IPC-потока + winit-позиционирование).
