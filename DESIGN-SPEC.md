# Alacritty — Дизайн-спека: Табы + Сплиты + Графика

> v0.1.0-draft · Цель: один бинарник, KISS, графика без посредников

---

## 1. Архитектура

```
WindowContext
├── tabs: Vec<Tab>
│   ├── Tab 0
│   │   ├── pane_tree: PaneNode          ← рекурсивное дерево сплитов
│   │   │   ├── Leaf: Term + PTY          ← терминал с графикой
│   │   │   └── Split { dir, ratio, a, b }
│   │   └── tab_bar_title, focus_pane, ...
│   └── Tab 1
│       └── pane_tree: PaneNode
├── active_tab: usize
├── display: Display (общий на все табы)
└── renderer: Renderer (общий GPU-контекст)

Каждый Leaf — изолированный Term со своим PTY.
Графика работает в каждом листе, без мультиплексора.
```

## 2. Pane Tree

```rust
enum PaneNode {
    Leaf {
        pane_id: PaneId,
        terminal: Arc<FairMutex<Term<EventProxy>>>,
        pty_handle: PtyHandle,
        last_size: (u32, u32),
    },
    Split {
        split_id: SplitId,
        direction: SplitDir,        // Horizontal | Vertical
        ratio: f32,                 // 0.1 .. 0.9
        a: Box<PaneNode>,
        b: Box<PaneNode>,
    },
}
```

Операции:

- `split(pane_id, direction)` — разбить лист на два
- `remove(pane_id)` — закрыть, схлопнуть дерево к соседу
- `resize(split_id, ratio)` — подвинуть разделитель
- `focus(direction)` — сфокусировать соседнюю панель (←↑↓→)
- `leaf_rects(viewport)` — вычислить Rect для каждого листа
- `zoom(pane_id)` — развернуть на весь таб / свернуть обратно

## 3. Рендеринг

Один `Display::draw()` на кадр. Для каждого таба:

1. Взять активное дерево `pane_tree`
2. `leaf_rects(viewport)` → `Vec<(PaneId, Rect)>`
3. Для каждого листа:
   - `glScissor(rect)` + `glViewport(rect)`
   - Отрендерить Term (текст + графика)
   - Отрендерить разделители между сплитами (1px линия)
4. Отрендерить tab bar (поверх всего)

Графика: `GraphicsRenderer` уже есть. Ключ текстуры = `(tab_id << 32) | pane_id << 16 | graphic_id`. Гарантирует изоляцию между табами и панелями.

## 4. События и ввод

```
Клавиатура/мышь → Event
    ├── tab_id   (из WindowContext.active_tab)
    ├── pane_id  (из hit-test координат мыши / активной панели)
    └── action → роутинг в Term активной панели
```

Фокус панели:

- Клик мыши → hit-test по leaf_rects → сменить active_pane
- `Ctrl+B ←↑↓→` → focus(direction)

## 5. Keybindings

```toml
[keyboard]
bindings = [
  # Табы
  { key = "T",     mods = "Super",       action = "CreateNewTab" },
  { key = "W",     mods = "Super",       action = "CloseTab" },
  { key = "Right", mods = "Super",       action = "SelectNextTab" },
  { key = "Left",  mods = "Super",       action = "SelectPrevTab" },

  # Сплиты
  { key = "D",     mods = "Super",       action = "SplitRight" },
  { key = "D",     mods = "Super|Shift", action = "SplitDown" },
  { key = "W",     mods = "Super|Shift", action = "ClosePane" },
  { key = "Return",mods = "Super|Shift", action = "ToggleZoom" },

  # Фокус
  { key = "H",     mods = "Super",       action = "FocusLeft" },
  { key = "L",     mods = "Super",       action = "FocusRight" },
  { key = "K",     mods = "Super",       action = "FocusUp" },
  { key = "J",     mods = "Super",       action = "FocusDown" },

  # Ресайз
  { key = "H",     mods = "Super|Alt",   action = "ResizeLeft" },
  { key = "L",     mods = "Super|Alt",   action = "ResizeRight" },
  { key = "K",     mods = "Super|Alt",   action = "ResizeUp" },
  { key = "J",     mods = "Super|Alt",   action = "ResizeDown" },
]
```

## 6. Конфиг

```toml
[tabs]
tab_bar_edge = "top"
tab_bar_style = "powerline"
active_tab_foreground = "#cba6f7"
active_tab_background = "#1e1e2e"
# ...

[panes]
border_width = 1
border_color = "#45475a"
active_border_color = "#cba6f7"
split_ratio = 0.5           # default for new splits
min_pane_size = 2           # min cells
```

## 7. Что НЕ делаем (KISS)

- ❌ Перетаскивание панелей мышью — v1 без этого
- ❌ Сохранение/восстановление layout'ов — v2
- ❌ Floating panes — усложняет scissor-логику
- ❌ Табы внутри сплитов (сплиты внутри табов — достаточно)
- ❌ Встроенный SSH — отдельная задача
- ❌ `Ctrl+B` как префикс (как в tmux) — все на Super

## 8. План реализации

| Фаза | Что                                            | Строк | Зависимости               |
| ---- | ---------------------------------------------- | ----- | ------------------------- |
| 0    | `PaneNode` + `leaf_rects()` + тесты            | ~150  | 0                         |
| 1    | Интеграция табов из alacritty-tabs             | ~1400 | конфликт в display/mod.rs |
| 2    | Сплиты внутри таба + scissor-рендеринг         | ~300  | 0                         |
| 3    | Фокус, ресайз, разделители, zoom               | ~200  | 0                         |
| 4    | Конфиг + keybindings + tab bar styling         | ~200  | 0                         |
| 5    | Тестирование графики в сплитах (оба протокола) | ~100  | 0                         |

**Итого: ~2350 строк, 0 новых крейтов.**

## 9. Почему это лучше Zellij

|                    | Zellij                                | Наш подход                         |
| ------------------ | ------------------------------------- | ---------------------------------- |
| Архитектура        | Сервер + клиенты, IPC, WASM           | Один процесс, одно окно            |
| Графика            | Только Sixel, с багами                | Sixel + Kitty, общий ImageRenderer |
| Зависимости        | wasmtime, protobuf, множество крейтов | 0 новых                            |
| Запуск             | `zellij` → терминал                   | `alacritty` — это и есть терминал  |
| Производительность | Двойная эмуляция (zellij + alacritty) | Одинарная (alacritty)              |
| KISS               | ❌                                    | ✅                                 |
