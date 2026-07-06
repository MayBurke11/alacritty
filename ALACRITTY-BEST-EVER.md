# Alacritty Best Ever — План интеграции Sixel + Kitty Graphics

> Цель: собрать воедино Sixel-протокол (уже есть в ветке `graphics`) и Kitty graphics protocol (PR #24 от dsturnbull), обновить до актуального upstream (v0.18.0-dev), пройти полный цикл тестирования.

---

## Текущее состояние

```
upstream/alacritty (v0.17.0 + ~5 post-release коммитов → v0.18.0-dev)
    │
    ├── ayosec/graphics (наш репозиторий, v0.17.0-dev)
    │     ├── 1 большой коммит: Sixel protocol (08ceb315)
    │     ├── ~5 follow-up коммитов (clippy, rustfmt, fixes)
    │     └── регулярные merge из upstream (v0.14 → v0.17)
    │
    └── dsturnbull/alacritty-kitty (PR #24 в ayosec/graphics)
          ├── Phase 1: APC-роутинг + статичные изображения
          │     (~2400 LOC, 43 теста)
          └── Phase 2: файлы/shm, delete, scaling, animation
                (~3400 LOC, 347 тестов total)
```

**Ключевая архитектурная особенность:** `GraphicData` протоколо-независим. Sixel (`DCS q`) и Kitty (`APC G`) декодируются в одинаковый пиксельный буфер, а GPU-пайплайн общий. Это значит, что интеграция Kitty — это добавление нового парсера, а не переписывание рендерера.

---

## Файлы, которые затрагивает интеграция

### Sixel (уже в `graphics`)

| Файл                                        | Строк          | Назначение                                      |
| ------------------------------------------- | -------------- | ----------------------------------------------- |
| `alacritty_terminal/src/graphics/mod.rs`    | 666            | Ядро: GraphicData, insert_graphic, XTSMGRAPHICS |
| `alacritty_terminal/src/graphics/sixel.rs`  | 809            | Sixel-парсер (DEC reference)                    |
| `alacritty_terminal/src/term/mod.rs`        | ~200 изменений | DCS-хуки, TermMode bits, vte handler            |
| `alacritty_terminal/src/term/cell.rs`       | ~30 изменений  | Flags::GRAPHICS, CellExtra.graphics             |
| `alacritty_terminal/src/event.rs`           | ~10 изменений  | TextAreaSizeRequest                             |
| `alacritty/src/renderer/graphics/mod.rs`    | 302            | GPU-текстуры, upload/remove/clear               |
| `alacritty/src/renderer/graphics/shader.rs` | 202            | Шейдерная программа, VAO/VBO                    |
| `alacritty/src/renderer/graphics/draw.rs`   | 299            | RenderList, батчевая отрисовка                  |
| `alacritty/src/renderer/mod.rs`             | ~20 изменений  | GraphicsRenderer в Renderer                     |
| `alacritty/src/display/mod.rs`              | ~100 изменений | dispatch update + draw                          |
| `alacritty/src/display/content.rs`          | ~120 изменений | RenderableGraphicCell                           |
| `alacritty/src/config/ui_config.rs`         | 1 строка       | `kitty_keyboard: true`                          |
| `res/graphics.v.glsl`                       | 101            | Vertex shader (dual GLSL3/GLES2)                |
| `res/graphics.f.glsl`                       | 73             | Fragment shader (16 текстур)                    |
| `alacritty_terminal/Cargo.toml`             | 1 строка       | `vte-graphics` вместо `vte`                     |

### Kitty (из PR #24, НЕ в текущей ветке)

| Файл                                                 | Строк          | Назначение                           |
| ---------------------------------------------------- | -------------- | ------------------------------------ |
| `alacritty_terminal/src/graphics/kitty/mod.rs`       | ~300           | Точка входа, dispatch APC-команд     |
| `alacritty_terminal/src/graphics/kitty/parser.rs`    | ~800           | Потоковый KV-парсер APC G            |
| `alacritty_terminal/src/graphics/kitty/decode.rs`    | ~1100          | base64→zlib→PNG/RGB/RGBA, все media  |
| `alacritty_terminal/src/graphics/kitty/state.rs`     | ~1060          | HashMap<u32,KittyImage>, LRU, delete |
| `alacritty_terminal/src/graphics/kitty/placement.rs` | ~600           | Crop, scale, pixel offsets           |
| `alacritty_terminal/src/graphics/kitty/animation.rs` | ~1120          | Frames, composition, control         |
| `alacritty_terminal/src/graphics/kitty/response.rs`  | ~200           | OK/EINVAL ответы                     |
| `alacritty_terminal/src/term/mod.rs`                 | ~100 изменений | APC-хуки (apc_start/put/end)         |
| `patched-vte-graphics`                               | ~50 изменений  | APC state machine в vte              |
| `alacritty_terminal/Cargo.toml`                      | ~5 строк       | flate2, png, image (новые deps)      |

---

## Этап 0: Подготовка окружения

### 0.1. Добавить upstream remote и обновить

```bash
git remote add upstream https://github.com/alacritty/alacritty.git
git fetch upstream --tags
git fetch upstream master
```

### 0.2. Добавить remote для Kitty PR

```bash
git remote add dsturnbull https://github.com/dsturnbull/alacritty.git
git fetch dsturnbull alacritty-kitty
```

### 0.3. Создать резервную ветку

```bash
git branch backup-graphics-$(date +%Y%m%d) graphics
```

### 0.4. Проверить текущее состояние

```bash
cargo build --release          # должен собраться без ошибок
cargo test --all               # все тесты должны пройти
cargo clippy --all-targets     # без warnings
```

**Критерий готовности:** clean build, все тесты зелёные, clippy чистый.

---

## Этап 1: Обновление до upstream v0.18.0-dev

### 1.1. Слияние с upstream master

```bash
git checkout graphics
git merge upstream/master --no-ff
```

### 1.2. Разрешение конфликтов (ожидаемые зоны)

Наиболее вероятные конфликты — в файлах, которые менялись и в upstream, и в `graphics`:

| Файл                                 | Причина конфликта                                       |
| ------------------------------------ | ------------------------------------------------------- |
| `alacritty_terminal/src/term/mod.rs` | DCS-хуки vs upstream изменения в term handler           |
| `alacritty_terminal/Cargo.toml`      | `vte-graphics` vs возможное обновление `vte` в upstream |
| `alacritty/src/renderer/mod.rs`      | GraphicsRenderer vs новый рендерер-код                  |
| `alacritty/src/display/mod.rs`       | Graphics dispatch vs upstream изменения                 |
| `alacritty/src/config/ui_config.rs`  | `kitty_keyboard` vs новые конфиг-опции                  |

**Стратегия разрешения:** при конфликтах приоритет у upstream-кода (keep theirs), затем вручную добавляем graphics-изменения. Используем `git diff upstream/master...graphics -- <файл>` для понимания что именно добавляет `graphics`.

### 1.3. Компиляция и тесты

```bash
cargo build --release
cargo test --all
cargo clippy --all-targets
```

**Критерий готовности:** чистый build, все тесты проходят.

### Тестирование на этапе 1

#### A. Юнит-тесты Sixel

```bash
cargo test --package alacritty_terminal -- graphics
cargo test --package alacritty_terminal -- sixel
```

Проверяем что Sixel-парсер всё ещё работает:

- `parse_command_parameters` — параметры команд
- `set_color_registers` — HLS→RGB конвертация
- `convert_hls_colors` — 20 assert-проверок против libsixel
- `resize_picture` — ресайз с прозрачностью
- `sixel_height` / `sixel_positions` — побитовые операции
- `load_sixel_files` — 3 эталонных .sixel → .rgba (ImageMagick, libsixel, ppmtosixel)

#### B. Smoke-тест Sixel в реальном терминале

```bash
# Запустить alacritty и выполнить:
printf '\ePq\n#0;2;100;100;100#1;2;0;100;0#2;2;100;0;0#3;2;0;0;100~-o.?!N \e\\'
# Должен отобразиться маленький цветной прямоугольник
```

#### C. Проверка базовой функциональности upstream

```bash
# Проверить что терминал не сломан:
cargo run -- -e bash -c "echo 'Terminal works'; sleep 2"
# Проверить kitty keyboard protocol:
cargo run -- -e bash -c "printf '\e[?1u' && echo 'Kitty keyboard mode set' && sleep 2"
```

---

## Этап 2: Интеграция Kitty Graphics Protocol (PR #24)

### 2.1. Создание feature-ветки

```bash
git checkout -b feature/kitty-graphics graphics
```

### 2.2. Cherry-pick или merge коммитов PR #24

PR содержит 2 коммита:

```bash
# Коммит 1: Phase 0+1 (APC routing + static images)
git cherry-pick 64bbbe5dff2665d5e10676d92a44256bff383a8b

# Разрешить конфликты, закоммитить

# Коммит 2: Phase 2 (file/shm, delete, scaling, animation)
git cherry-pick 8781676feb2e8ecd3cd7c55865bc0e9c671fac5b

# Разрешить конфликты, закоммитить
```

### 2.3. Ожидаемые конфликты и их разрешение

| Файл                                     | Причина                                                                | Стратегия                                                              |
| ---------------------------------------- | ---------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `alacritty_terminal/src/term/mod.rs`     | APC-хуки пересекаются с v0.18 изменениями handler'а                    | Добавить APC-хуки в актуальный handler                                 |
| `alacritty_terminal/src/graphics/mod.rs` | Возможные изменения в `insert_graphic()` для поддержки kitty placement | Аккуратный merge                                                       |
| `alacritty_terminal/Cargo.toml`          | Новые зависимости (flate2, png, image, tempfile)                       | Добавить в актуальный Cargo.toml                                       |
| `Cargo.lock`                             | Конфликт lock-файла                                                    | Удалить и перегенерировать: `rm Cargo.lock && cargo generate-lockfile` |
| Патч `vte-graphics`                      | APC state machine                                                      | Убедиться что vte-graphics обновлён до версии с APC-поддержкой         |

### 2.4. Обновление vte-graphics

PR #24 использует пропатченный `vte-graphics` с APC-хуками. Нужно:

```bash
# Проверить текущую версию vte-graphics в Cargo.toml
grep "vte-graphics\|^vte " alacritty_terminal/Cargo.toml

# Обновить до версии с APC-поддержкой
# Либо указать git-зависимость на форк dsturnbull:
# vte = { git = "https://github.com/dsturnbull/vte", branch = "kitty", package = "vte-graphics", ... }
```

### 2.5. Компиляция

```bash
cargo build --release 2>&1 | tee build.log
# Фиксим ошибки компиляции
cargo build --release     # Должен собраться чисто
```

**Критическая проверка:** в проекте НЕ должно быть `unused import`, `dead_code`, `unused_variable` и других warnings.

### Тестирование на этапе 2

#### A. Юнит-тесты Kitty (347 тестов)

```bash
cargo test --package alacritty_terminal -- kitty
cargo test --package alacritty_terminal -- graphics
```

Ожидаем все 347+ тестов зелёными.

#### B. Интеграционные тесты (13 E2E тестов)

```bash
cargo test --package alacritty_terminal --test '*' -- kitty
```

#### C. Ручное тестирование — статичные изображения

```bash
# Установить инструменты если нет:
# sudo apt install chafa timg

# 1. PNG через chafa
chafa -f kitty /path/to/image.png

# 2. PNG через timg
timg -p kitty /path/to/image.png

# 3. Kitty kitten (chunked PNG)
kitty +kitten icat /path/to/image.png

# 4. Прямая передача RGBA (тестовый скрипт из PR)
python3 scripts/test_kitty_graphics.py
```

#### D. Ручное тестирование — анимации

```bash
timg -p kitty /path/to/animation.gif
```

#### E. Ручное тестирование — разные media

```bash
# File transmission
python3 -c "
import base64, subprocess
with open('/tmp/test.png', 'rb') as f:
    data = base64.b64encode(f.read()).decode()
print(f'\033_Gf=100,t=f,;L3RtcC90ZXN0LnBuZw==\033\\\\')
print(f'\033_Gf=100,t=f,;{data}\033\\\\')
"
```

#### F. Тестирование delete-операций

Скрипт `scripts/test_kitty_graphics.py` (25 тестов) — запустить и убедиться что все ✓.

#### G. Smoke-тест Sixel (регрессия)

```bash
# Убедиться что Sixel не сломан:
printf '\ePq\n#0;2;100;100;100#1;2;0;100;0#2;2;100;0;0#3;2;0;0;100~-o.?!N \e\\'
```

#### H. Проверка совместимости с TMUX

```bash
tmux new -d 'chafa -f kitty /path/to/image.png; sleep 5' && tmux attach
# Известная проблема: TMUX искажает изображения (reported в PR #24)
```

---

## Этап 3: Стабилизация и баг-фиксы

### 3.1. Известные проблемы из PR #24

| Проблема                                    | Статус                        | Действие                                |
| ------------------------------------------- | ----------------------------- | --------------------------------------- |
| **Segfault при `mpv --vo=kitty`** (30+ fps) | Известен, также в ghostty     | Задокументировать, не блокирует         |
| **TMUX искажает картинки**                  | Reported by @aurora0x27       | Исследовать, возможно нужен passthrough |
| **Абсолютные пути в temp-файлах** (ranger)  | @gohellp + fix от @dsturnbull | Применить фикс                          |
| **Анимация: нет автотика**                  | Не реализован event loop tick | Задокументировать как known limitation  |

### 3.2. Clippy + форматирование

```bash
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
```

### 3.3. Полный прогон всех тестов

```bash
cargo test --all
cargo test --all --release  # Дополнительно в release mode
```

**Критерий готовности:** все тесты зелёные, clippy без ошибок, fmt без изменений.

---

## Этап 4: Финальная интеграция

### 4.1. Слияние в `graphics`

```bash
git checkout graphics
git merge feature/kitty-graphics --no-ff -m "feat: integrate Kitty graphics protocol

Merge PR #24 from dsturnbull/alacritty-kitty adding full Kitty graphics
protocol support alongside existing Sixel implementation.

Co-authored-by: David Turnbull <dsturnbull@github>"
```

### 4.2. Обновление документации

Обновить `docs/features.md` — добавить информацию о Kitty graphics protocol.

### 4.3. Подготовка release-тега (опционально)

```bash
git tag -a v0.18.0-graphics -m "Alacritty v0.18.0 with Sixel + Kitty graphics protocols"
```

### 4.4. Финальный build

```bash
cargo build --release
# Проверить размер бинарника
ls -lh target/release/alacritty
```

---

## Стратегия тестирования: сводная таблица

| Этап | Что тестируем         | Как                                                | Ожидаемый результат         |
| ---- | --------------------- | -------------------------------------------------- | --------------------------- |
| 0    | Исходное состояние    | `cargo test --all` + `cargo build --release`       | Всё зелёное                 |
| 1    | Merge upstream        | `cargo test --all` + Sixel smoke-test              | Все тесты, Sixel работает   |
| 1    | Регрессия терминала   | `echo`, цвета, ESC-последовательности              | Терминал не сломан          |
| 2    | Kitty unit-тесты      | `cargo test --package alacritty_terminal -- kitty` | 347+ тестов OK              |
| 2    | Статичные изображения | `chafa -f kitty`, `timg -p kitty`, `icat`          | Картинка отображается       |
| 2    | Разные media          | file, tempfile, shared memory                      | Все способы передачи        |
| 2    | Delete-операции       | `test_kitty_graphics.py` (25 тестов)               | Все ✓                       |
| 2    | Анимации              | `timg -p kitty anim.gif`                           | Кадры загружены             |
| 2    | Регрессия Sixel       | Sixel smoke-test                                   | Не сломан                   |
| 2    | Масштабирование/crop  | `c=`/`r=`/`x=`/`y=`/`w=`/`h=`                      | Корректное позиционирование |
| 3    | Clippy + fmt          | `cargo clippy` + `cargo fmt --check`               | Чисто                       |
| 3    | TMUX                  | `tmux` + `chafa`                                   | Документировать баг         |
| 4    | Полный прогон         | `cargo test --all --release`                       | Всё зелёное                 |

---

## Оценка трудозатрат

| Этап | Операция                   | Время     | Риски                                |
| ---- | -------------------------- | --------- | ------------------------------------ |
| 0    | Подготовка                 | 10 мин    | Низкие                               |
| 1    | Merge upstream v0.18.0-dev | 30-60 мин | Конфликты в term/mod.rs, renderer    |
| 2    | Cherry-pick Kitty PR       | 1-2 часа  | Конфликты в vte-graphics, Cargo.toml |
| 2    | Компиляция и фиксы         | 2-4 часа  | Новые зависимости, API changes       |
| 2    | Тестирование Kitty         | 1-2 часа  | Зависит от доступности chafa/timg    |
| 3    | Стабилизация               | 1-3 часа  | Баги, edge cases                     |
| 4    | Финальная сборка           | 30 мин    | Низкие                               |

**Итого: 6-12 часов** в зависимости от количества конфликтов.

---

## Ключевые риски

1. **vte-graphics API**: если upstream vte обновился, может потребоваться адаптация форка
2. **Новые зависимости**: `flate2`, `png`, `image` — проверить совместимость с MSRV
3. **API изменения в upstream**: v0.18.0-dev мог изменить сигнатуры методов в term/mod.rs или renderer/mod.rs
4. **Segfault при высокой частоте кадров**: не блокирует, но требует документирования
5. **TMUX-совместимость**: требует passthrough, может не работать из коробки

---

## Команды быстрой проверки (cheatsheet)

```bash
# Sixel (работает в любом alacritty с поддержкой):
printf '\ePq\n#0;2;100;100;100#1;2;0;100;0#2;2;100;0;0#3;2;0;0;100~-o.?!N \e\\'

# Kitty (после интеграции):
chafa -f kitty /path/to/image.png
timg -p kitty /path/to/image.png

# Регрессия терминала:
printf '\e[31mRED\e[32mGREEN\e[0m\n'
printf '\e[5 q'  # blinking cursor
```
