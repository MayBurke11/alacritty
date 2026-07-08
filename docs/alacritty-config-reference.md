# Alacritty Configuration Reference

> Версия: v0.18.0-dev · Форк MayBurke11 с Sixel + Kitty graphics

Все опции конфигурационного файла `alacritty.toml` с типами, значениями по умолчанию и описанием.

---

## `[general]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `import` | `string[]` | `[]` | Список путей к файлам конфигурации для импорта |
| `working_directory` | `string?` | `None` | Рабочая директория при запуске shell |
| `live_config_reload` | `bool` | `true` | Автоматически перезагружать конфиг при изменении файла |
| `ipc_socket` | `bool` | `true` | Включить Unix-сокет для `alacritty msg` (только Linux/macOS) |

---

## `[env]`

Словарь `{ "KEY" = "value" }` — дополнительные переменные окружения, передаваемые процессу.

```toml
[env]
TERM = "alacritty"
EDITOR = "nvim"
```

---

## `[window]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `opacity` | `float` (0.0–1.0) | `1.0` | Прозрачность окна. Требует композитор |
| `blur` | `bool` | `false` | Размытие фона под окном (macOS/Windows/KDE) |
| `dynamic_title` | `bool` | `true` | Менять заголовок окна на заголовок терминала |
| `dynamic_padding` | `bool` | `false` | Равномерно распределять дополнительные отступы |
| `resize_increments` | `bool` | `false` | Изменять размер окна кратно размерам ячейки |
| `startup_mode` | `enum` | `"Windowed"` | Режим при запуске: `Windowed`, `Maximized`, `Fullscreen`, `SimpleFullscreen` |
| `decorations` | `enum` | `"Full"` | Оформление окна: `Full`, `Transparent`, `Buttonless`, `None` |
| `decorations_theme_variant` | `enum?` | `None` | Тема рамок: `"Light"`, `"Dark"` |
| `level` | `enum` | `"Normal"` | Уровень окна: `"Normal"`, `"AlwaysOnTop"` |
| `option_as_alt` | `enum` | `"None"` | Трактовка Option как Alt: `"OnlyLeft"`, `"OnlyRight"`, `"Both"`, `"None"` |

### `[window.position]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `x` | `int` | — | Начальная X-координата окна (в пикселях) |
| `y` | `int` | — | Начальная Y-координата окна (в пикселях) |

### `[window.dimensions]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `columns` | `int` | `0` | Ширина окна в символах (0 = авто) |
| `lines` | `int` | `0` | Высота окна в строках (0 = авто) |

### `[window.padding]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `x` | `int` | `0` | Горизонтальный отступ в пикселях |
| `y` | `int` | `0` | Вертикальный отступ в пикселях |

### `[window.identity]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `title` | `string` | `"Alacritty"` | Заголовок окна |
| `class.general` | `string` | `"Alacritty"` | Общий класс окна (X11 WM_CLASS / Wayland app_id) |
| `class.instance` | `string` | `"Alacritty"` | Класс экземпляра окна |

---

## `[scrolling]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `history` | `int` | `10000` | Максимальное количество строк прокрутки (0–100000) |
| `multiplier` | `int` | `3` | Множитель для событий колёсика мыши |

---

## `[font]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `size` | `float` | `11.25` | Размер шрифта в пунктах |
| `builtin_box_drawing` | `bool` | `true` | Использовать встроенный шрифт для box drawing символов |
| `normal.family` | `string` | `"monospace"` (Linux), `"Menlo"` (macOS), `"Consolas"` (Win) | Семейство обычного шрифта |
| `normal.style` | `string?` | `None` | Стиль обычного шрифта |
| `bold.family` | `string?` | `None` | Семейство жирного шрифта (None = наследуется от normal) |
| `bold.style` | `string?` | `None` | Стиль жирного шрифта |
| `italic.family` | `string?` | `None` | Семейство курсивного шрифта |
| `italic.style` | `string?` | `None` | Стиль курсивного шрифта |
| `bold_italic.family` | `string?` | `None` | Семейство жирного курсива |
| `bold_italic.style` | `string?` | `None` | Стиль жирного курсива |

### `[font.offset]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `x` | `int` | `0` | Горизонтальный отступ между символами |
| `y` | `int` | `0` | Вертикальный отступ между символами |

### `[font.glyph_offset]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `x` | `int` | `0` | Горизонтальное смещение глифа внутри ячейки |
| `y` | `int` | `0` | Вертикальное смещение глифа внутри ячейки |

---

## `[colors]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `transparent_background_colors` | `bool` | `false` | Фоновые цвета учитывают opacity окна |
| `draw_bold_text_with_bright_colors` | `bool` | `false` | Жирный текст рисуется яркими (bright) цветами |

### `[colors.primary]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `foreground` | `color` | `#d8d8d8` | Основной цвет текста |
| `background` | `color` | `#181818` | Основной цвет фона |
| `bright_foreground` | `color?` | `None` | Яркий цвет текста (None = авто-яркость) |
| `dim_foreground` | `color?` | `None` | Тусклый цвет текста (None = 2/3 яркости) |

### `[colors.cursor]` / `[colors.vi_mode_cursor]` / `[colors.selection]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `text` (или `foreground`) | `color\|string` | `CellBackground` | Цвет текста (инвертированных ячеек). Специальные: `CellForeground`, `CellBackground` |
| `cursor` (или `background`) | `color\|string` | `CellForeground` | Цвет фона (инвертированных ячеек) |

### `[colors.normal]` / `[colors.bright]` / `[colors.dim]`

16 стандартных ANSI-цветов. `dim` — тусклые версии (опционально, по умолчанию 2/3 яркости normal).

| Поле | normal (default) | bright (default) | dim (default) |
|---|---|---|---|
| `black` | `#181818` | `#6b6b6b` | `#0f0f0f` |
| `red` | `#ac4242` | `#c55555` | `#712b2b` |
| `green` | `#90a959` | `#aac474` | `#5f6f3a` |
| `yellow` | `#f4bf75` | `#feca88` | `#a17e4d` |
| `blue` | `#6a9fb5` | `#82b8c8` | `#456877` |
| `magenta` | `#aa759f` | `#c28cb8` | `#704d68` |
| `cyan` | `#75b5aa` | `#93d3c3` | `#4d7770` |
| `white` | `#d8d8d8` | `#f8f8f8` | `#8e8e8e` |

### `[colors.indexed_colors]`

Массив `[{ index = 16, color = "#..." }]` — переопределение 256-цветной палитры. `index`: 16–255.

### `[colors.search]`

| Подсекция | Поле | Тип | По умолчанию | Описание |
|---|---|---|---|---|
| `matches` | `foreground` | `color\|string` | `#181818` | Цвет текста совпадений |
| `matches` | `background` | `color\|string` | `#ac4242` | Цвет фона совпадений |
| `focused_match` | `foreground` | `color\|string` | `#181818` | Цвет текста активного совпадения |
| `focused_match` | `background` | `color\|string` | `#f4bf75` | Цвет фона активного совпадения |

### `[colors.line_indicator]`

Индикатор позиции (правый верхний угол) в vi-режиме и при поиске.

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `foreground` | `color?` | `None` | Цвет текста |
| `background` | `color?` | `None` | Цвет фона |

### `[colors.hints]`

Подсветка подсказок (hints).

| Подсекция | Поле | Тип | По умолчанию | Описание |
|---|---|---|---|---|
| `start` | `foreground` | `color\|string` | `#181818` | Цвет текста начала подсказки |
| `start` | `background` | `color\|string` | `#f4bf75` | Цвет фона начала подсказки |
| `end` | `foreground` | `color\|string` | `#181818` | Цвет текста конца подсказки |
| `end` | `background` | `color\|string` | `#ac4242` | Цвет фона конца подсказки |

### `[colors.footer_bar]`

Строка состояния (поиск, заголовок вкладки, сообщения).

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `foreground` | `color?` | `None` | Цвет текста |
| `background` | `color?` | `None` | Цвет фона |

---

## `[bell]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `animation` | `enum` | `"Linear"` | Анимация визуального звонка: `Ease`, `EaseOut`, `EaseOutSine`, `EaseOutQuad`, `EaseOutCubic`, `EaseOutQuart`, `EaseOutQuint`, `EaseOutExpo`, `EaseOutCirc`, `Linear` |
| `color` | `color` | `#ffffff` | Цвет визуального звонка |
| `duration` | `int` (ms) | `0` | Длительность визуального звонка. `0` — отключён |
| `command` | `string\|object?` | `None` | Команда при звонке. `"notify-send"` или `{ program = "notify-send", args = ["Alacritty", "Bell!"] }` |

---

## `[selection]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `semantic_escape_chars` | `string` | `",│\`\|:\"' ()[]{}<>\t"` | Символы, по которым двойной клик расширяет выделение |
| `save_to_clipboard` | `bool` | `false` | Автоматически копировать выделение в системный буфер обмена |

---

## `[cursor]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `style` | `string\|object` | `"Block"` | Форма: `"Block"`, `"Underline"`, `"Beam"`. С миганием: `{ shape = "Block", blinking = "On" }` |
| `vi_mode_style` | `string\|object?` | `None` | Форма курсора в vi-режиме (если None — наследует `style`) |
| `unfocused_hollow` | `bool` | `false` | Полый курсор когда окно не в фокусе |
| `thickness` | `float` (0.0–1.0) | `0.15` | Толщина курсора (доля от ширины ячейки) |
| `blink_interval` | `int` (ms) | `750` | Интервал мигания курсора |
| `blink_timeout` | `int` (s) | `5` | Таймаут бездействия до остановки мигания. `0` — всегда мигает |

### Режимы мигания (`blinking`)
| Значение | Описание |
|---|---|
| `"Never"` | Никогда не мигает |
| `"Off"` | Не мигает по умолчанию, включается по escape-последовательности |
| `"On"` | Мигает по умолчанию, выключается по escape-последовательности |
| `"Always"` | Всегда мигает |

---

## `[terminal]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `shell` | `string\|object?` | `None` | Команда shell. `"fish"` или `{ program = "fish", args = ["-l"] }` |
| `osc52` | `enum` | `"OnlyCopy"` | Режим OSC52 (работа с буфером обмена): `"Disabled"`, `"OnlyCopy"`, `"OnlyPaste"`, `"CopyPaste"` |

---

## `[mouse]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `hide_when_typing` | `bool` | `false` | Прятать курсор мыши при вводе с клавиатуры |

### `[[mouse.bindings]]`

Биндинги мыши. Формат:

| Поле | Тип | Описание |
|---|---|---|
| `mouse` | `string` | Кнопка: `"Left"`, `"Right"`, `"Middle"`, `"Forward"`, `"Back"`, `"WheelUp"`, `"WheelDown"` |
| `action` | `string` | Действие (см. Actions Reference ниже) |
| `mods` | `string` | Модификаторы: `"Control"`, `"Shift"`, `"Alt"`, `"Super"` (через `\|`) |
| `mode` | `string` | Режимы, в которых активно (см. Binding Modes) |
| `notmode` | `string` | Режимы, в которых НЕ активно |

---

## `[hints]`

Подсказки для быстрой навигации по содержимому терминала с клавиатуры.

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `alphabet` | `string` | `"asdfghjklqwertyuiopzxcvbnm"` | Символы для меток подсказок |

### `[[hints.enabled]]`

Каждая подсказка — отдельный элемент массива.

| Поле | Тип | Описание |
|---|---|---|
| `regex` | `string?` | Регулярное выражение для поиска |
| `hyperlinks` | `bool` | Искать OSC 8 hyperlinks (если regex не указан) |
| `action` | `string` | Встроенное действие: `"Copy"`, `"Paste"`, `"Select"`, `"MoveViModeCursor"` |
| `command` | `string\|object` | Внешняя команда (вместо action) |
| `post_processing` | `bool` | Обрабатывать текст перед передачей (убирать префиксы) |
| `persist` | `bool` | Сохранять подсказки после выбора |
| `mouse.enabled` | `bool` | Включить подсветку подсказок мышью |
| `mouse.mods` | `string` | Модификаторы для подсветки мышью |

```toml
# Пример: копирование ссылок по Ctrl+Shift+U
[[hints.enabled]]
regex = "(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https?://|news:|file:|git://|ssh:|ftp://)[^\u0000-\u001F\u007F-\u009F<>\"\\s{-}\\^⟨⟩`]+"
binding = { key = "u", mods = "Control|Shift" }
action = "Copy"
post_processing = true

# Пример: запуск команды по ссылке
[[hints.enabled]]
regex = "(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https?://|news:|file:|git://|ssh:|ftp://)[^\u0000-\u001F\u007F-\u009F<>\"\\s{-}\\^⟨⟩`]+"
binding = { key = "o", mods = "Control|Shift" }
command = "xdg-open"
```

---

## `[keyboard]`

### `[[keyboard.bindings]]`

| Поле | Тип | Описание |
|---|---|---|
| `key` | `string` | Клавиша: символ (`"a"`, `"!"`), именованная (`"Return"`, `"Space"`, `"ArrowRight"`), или сканкод (`123`) |
| `mods` | `string` | Модификаторы: `"Control"`, `"Shift"`, `"Alt"`, `"Super"` (через `\|`). `"Control\|Shift"` |
| `mode` | `string` | Режимы, в которых биндинг активен |
| `notmode` | `string` | Режимы, в которых биндинг НЕ активен |
| `action` | `string` | Действие (см. Actions Reference) |
| `chars` | `string` | Послать символы напрямую в терминал |

```toml
[[keyboard.bindings]]
key = "n"
mods = "Control|Shift"
action = "CreateNewTab"

[[keyboard.bindings]]
key = "v"
mods = "Control|Shift"
action = "Paste"

[[keyboard.bindings]]
key = "c"
mods = "Control|Shift"
action = "Copy"
```

### Именованные клавиши

Специальные клавиши, доступные в `key`:

`Return`, `Space`, `Tab`, `Backspace`, `Escape`, `Enter`, `Home`, `End`, `PageUp`, `PageDown`, `Insert`, `Delete`, `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`, `F1`–`F24`, `Numpad0`–`Numpad9`, `NumpadAdd`, `NumpadSubtract`, `NumpadDivide`, `NumpadMultiply`, `NumpadEnter`, `NumpadDecimal`

---

## `[debug]`

| Поле | Тип | По умолчанию | Описание |
|---|---|---|---|
| `log_level` | `enum` | `"Warn"` | Уровень логирования: `"Off"`, `"Error"`, `"Warn"`, `"Info"`, `"Debug"`, `"Trace"` |
| `print_events` | `bool` | `false` | Выводить все события в STDOUT |
| `persistent_logging` | `bool` | `false` | Сохранять лог-файл после выхода |
| `render_timer` | `bool` | `false` | Показывать время рендеринга |
| `highlight_damage` | `bool` | `false` | Подсвечивать изменённые области (отладка рендеринга) |
| `prefer_egl` | `bool` | `false` | Использовать EGL как display API |
| `renderer` | `enum?` | `None` | Выбор рендерера: `"Glsl3"`, `"Gles2"`, `"Gles2Pure"` |

---

## Actions Reference

Все значения поля `action` в биндингах клавиатуры и мыши.

### Управление окнами

| Action | Описание |
|---|---|
| `Hide` | Скрыть окно Alacritty |
| `Minimize` | Свернуть окно |
| `Quit` | Выйти из Alacritty |
| `SpawnNewInstance` | Запустить новый экземпляр Alacritty |
| `CreateNewWindow` | Создать новое окно в том же процессе |

### Управление окном/фуллскрином

| Action | Описание |
|---|---|
| `ToggleFullscreen` | Полноэкранный режим |
| `ToggleMaximized` | Развернуть на весь экран |

### Работа с буфером обмена

| Action | Описание |
|---|---|
| `Copy` | Копировать выделение в системный буфер |
| `CopySelection` | Копировать выделение в буфер выделения |
| `Paste` | Вставить из системного буфера |
| `PasteSelection` | Вставить из буфера выделения |

### Скроллинг

| Action | Описание |
|---|---|
| `ScrollPageUp` | На страницу вверх |
| `ScrollPageDown` | На страницу вниз |
| `ScrollHalfPageUp` | На полстраницы вверх |
| `ScrollHalfPageDown` | На полстраницы вниз |
| `ScrollLineUp` | На строку вверх |
| `ScrollLineDown` | На строку вниз |
| `ScrollToTop` | В начало истории |
| `ScrollToBottom` | В конец (к промпту) |
| `ClearHistory` | Очистить историю скроллинга |

### Шрифт

| Action | Описание |
|---|---|
| `IncreaseFontSize` | Увеличить шрифт |
| `DecreaseFontSize` | Уменьшить шрифт |
| `ResetFontSize` | Сбросить размер шрифта |

### VI-режим

| Action | Описание |
|---|---|
| `ToggleViMode` | Вкл/выкл vi-режим |
| `ClearSelection` | Снять выделение |
| `SearchForward` | Поиск вперёд |
| `SearchBackward` | Поиск назад |
| `ClearLogNotice` | Скрыть сообщения в status bar |

### VI-действия (`Vi` action)

| Действие | Описание |
|---|---|
| `ToggleNormalSelection` | Переключить обычное выделение |
| `ToggleLineSelection` | Переключить линейное выделение |
| `ToggleBlockSelection` | Переключить блочное выделение |
| `ToggleSemanticSelection` | Переключить семантическое выделение |
| `SearchNext` | К следующему совпадению |
| `SearchPrevious` | К предыдущему совпадению |
| `SearchStart` | К началу совпадения слева |
| `SearchEnd` | К концу совпадения справа |
| `Open` | Открыть URL под курсором |
| `CenterAroundViCursor` | Центрировать экран вокруг курсора |
| `InlineSearchForward` | Быстрый поиск вперёд по строке |
| `InlineSearchBackward` | Быстрый поиск назад по строке |
| `InlineSearchForwardShort` | Быстрый поиск вперёд (до символа) |
| `InlineSearchBackwardShort` | Быстрый поиск назад (до символа) |
| `InlineSearchNext` | Следующее inline-совпадение |
| `InlineSearchPrevious` | Предыдущее inline-совпадение |
| `SemanticSearchForward` | Семантический поиск вперёд |
| `SemanticSearchBackward` | Семантический поиск назад |

### Поиск (`Search` action)

| Действие | Описание |
|---|---|
| `SearchFocusNext` | Фокус на следующее совпадение |
| `SearchFocusPrevious` | Фокус на предыдущее совпадение |
| `SearchConfirm` | Подтвердить поиск (выйти из режима) |
| `SearchCancel` | Отменить поиск |
| `SearchClear` | Очистить строку поиска |
| `SearchDeleteWord` | Удалить последнее слово в поиске |
| `SearchHistoryPrevious` | Предыдущий запрос в истории |
| `SearchHistoryNext` | Следующий запрос в истории |

### Мышь (`Mouse` action)

| Действие | Описание |
|---|---|
| `ExpandSelection` | Расширить выделение до позиции мыши |

### Прочее

| Action | Описание |
|---|---|
| `ReceiveChar` | Принять ввод символов (используется в hint-режиме) |
| `None` | Ничего не делать |

---

## Binding Modes

Флаги режимов для `mode` / `notmode` в биндингах (комбинируются через `|`):

| Режим | Синтаксис | Описание |
|---|---|---|
| App Cursor | `"AppCursor"` | Приложение включило режим курсора |
| App Keypad | `"AppKeypad"` | Приложение включило режим клавиатуры |
| Alt Screen | `"Alt"` | Приложение переключилось на альтернативный экран (nvim, less) |
| Vi | `"Vi"` | Активирован vi-режим Alacritty |
| Search | `"Search"` | Активен поиск по буферу |
| Disambiguate ESC | `"~Alt"` | Режим различения Escape-кодов |
| Report All Keys | `"~Alt"` | Режим передачи всех клавиш как Escape |

```toml
# Биндинг только в vi-режиме, но не в поиске
mode = "Vi"
notmode = "Search"

# Биндинг в любом режиме
mode = "~Alt"   # или просто не указывать mode
```

---

## Программы (`Program`)

Поля типа `shell` и `command` принимают строку или объект:

```toml
# Простая форма
command = "xdg-open"

# С аргументами
command = { program = "nvim", args = ["--listen", "/tmp/nvim.sock"] }
```

---

## Цвета

Форматы: `"#RGB"`, `"#RRGGBB"`, `"#RRRGGGBBB"`, `"0xRRGGBB"`. Специальные (для cursor/selection/hints): `"CellForeground"`, `"CellBackground"`.
