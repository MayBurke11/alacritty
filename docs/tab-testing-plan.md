# Tab Features Testing Plan

> Branch: `tab-dev` · Tag candidate: `tab-ipc-done`+ · Binary: `./target/release/alacritty`

Пройди по каждому пункту, ставь ✅ или ❌.

---

## 1. Базовые табы

### 1.1 Создание
```bash
# Запусти alacritty
./target/release/alacritty
```
- [ ] **Ctrl+Shift+T** — создать вкладку. Появляется вкладка с таб-баром
- [ ] **Ctrl+Shift+T** ещё раз — 3 вкладки, все отображаются

### 1.2 Переключение
- [ ] **Ctrl+Shift+→** — следующая вкладка
- [ ] **Ctrl+Shift+←** — предыдущая вкладка
- [ ] **Клик мышью** по вкладке — переключение

### 1.3 Закрытие
- [ ] **Ctrl+Shift+W** — закрыть активную вкладку
- [ ] Закрыть все кроме одной — таб-бар скрывается (при `tab_bar_min_tabs = 2`)
- [ ] Закрыть последнюю — окно закрывается

### 1.4 Заголовок вкладки
- [ ] Изначально показывает имя shell или CWD (без дёрганья)
- [ ] `cd ~/somewhere` → заголовок обновляется
- [ ] `ls` / `cat` — заголовок **НЕ** дёргается (hysteresis)

---

## 2. Настройки таб-бара

```bash
# Применяй по одному и смотри изменения
alacritty msg config 'tabs.tab_bar_edge="Top"'
alacritty msg config 'tabs.tab_bar_style="Slant"'
alacritty msg config 'tabs.tab_bar_style="Separator"'
alacritty msg config 'tabs.tab_bar_style="Fade"'
alacritty msg config 'tabs.tab_bar_style="Powerline"'
```

- [ ] **Top/Bottom** — таб-бар сверху/снизу
- [ ] **Slant** — диагональные разделители, табы не дёргаются при переключении
- [ ] **Separator** — вертикальные разделители
- [ ] **Fade** — плавное затухание
- [ ] **Powerline** — powerline-разделители
- [ ] Цвета активного/неактивного таба меняются через `active_tab_foreground` и `inactive_tab_background`

---

## 3. Переименование вкладок

- [ ] **Ctrl+Shift+Alt+T** — открывается инлайн-редактор в футере
- [ ] Ввод имени → **Enter** — вкладка переименована
- [ ] **Esc** — отмена, имя не меняется
- [ ] **Backspace** — удаление символов
- [ ] **Ctrl+W** — удалить слово

---

## 4. QuickRun (Ctrl+Shift+P)

- [ ] **Ctrl+Shift+P** — появляется `run: ` в футере
- [ ] Ввод `htop` → **Enter** — новая вкладка с htop, авто-переключение
- [ ] **Ctrl+Shift+P** → `bash` → **Shift+Enter** — новая вкладка в фоне, без переключения
- [ ] **Esc** — отмена, вкладка не создаётся

---

## 5. Pin (закрепление вкладок)

- [ ] **Ctrl+Shift+Alt+P** — вкладка закреплена, перед именем появилась `*`
- [ ] **Ctrl+Shift+W** на закреплённой вкладке — не закрывается
- [ ] **Ctrl+Shift+Alt+P** ещё раз — открепление, `*` пропала

---

## 6. IPC команды

Сначала найди сокет:
```bash
ls /run/user/1000/Alacritty-*.sock | head -1
# или
echo $ALACRITTY_SOCKET
```

### 6.1 create-tab / list-tabs
```bash
SOCK=$(ls /run/user/1000/Alacritty-*.sock | head -1)

alacritty msg -s "$SOCK" create-tab -e htop
alacritty msg -s "$SOCK" list-tabs
```

- [ ] **create-tab -e htop** — вкладка создана
- [ ] **list-tabs** — JSON со всеми вкладками: `[{index, id, title, active, pinned}]`
- [ ] **create-tab --no-switch -e bash** — вкладка создана в фоне, активная не изменилась

### 6.2 select-tab / close-tab
```bash
alacritty msg -s "$SOCK" select-tab 1
alacritty msg -s "$SOCK" close-tab 2
```

- [ ] **select-tab 1** — переключение на вкладку 1
- [ ] **close-tab 2** — вкладка 2 закрыта

### 6.3 pin-tab
```bash
alacritty msg -s "$SOCK" pin-tab 1
alacritty msg -s "$SOCK" list-tabs  # проверить pinned=true
```

- [ ] **pin-tab 1** — вкладка закреплена, в JSON `pinned: true`
- [ ] Повторный **pin-tab 1** — открепление, `pinned: false`

### 6.4 quickrun
```bash
alacritty msg -s "$SOCK" quickrun -e bash --no-switch
```

- [ ] **quickrun -e bash** — вкладка создана + переключение
- [ ] **quickrun --no-switch -e bash** — вкладка в фоне

---

## 7. Per-tab config (переопределение конфига)

```bash
# Создай вкладку с тёмным фоном
alacritty msg -s "$SOCK" create-tab \
  -o 'colors.primary.background="#000000"' \
  -o 'colors.primary.foreground="#00ff00"'

# Переключайся между вкладками
```

- [ ] **Тёмная вкладка** отличается от обычной (цвет фона/текста)
- [ ] **Обычная вкладка** не изменилась
- [ ] Переключение туда-обратно — цвета корректно переключаются

---

## 8. [[tabs.presets]] — стартовые вкладки

Добавь в конфиг `~/.config/alacritty/alacritty.toml`:

```toml
[[tabs.presets]]
command = "htop"

[[tabs.presets]]
command = "bash"
no_switch = true
```

- [ ] При запуске создаются 3 вкладки (1 начальная + 2 пресета)
- [ ] **htop** — активная (первый без no_switch)
- [ ] **bash** — в фоне (no_switch = true)

---

## 9. Save/Restore

### 9.1 Сохранение
```bash
alacritty msg -s "$SOCK" save-tabs
```

- [ ] Вывод — валидный JSON с `tabs`, `active_tab`

### 9.2 Восстановление
```bash
alacritty msg -s "$SOCK" save-tabs > /tmp/session.json
# Закрой alacritty
alacritty --restore /tmp/session.json
```

- [ ] Восстановилось правильное количество вкладок
- [ ] **Команды** восстановлены (htop → запущен htop)
- [ ] **Активная вкладка** та же, что была при сохранении
- [ ] **Закреплённые** вкладки остались закреплены

---

## 10. E2E авто-тесты

```bash
bash scripts/e2e-tabs-test.sh        # 6 тестов: create, switch, close
bash scripts/e2e-tabs-ipc-test.sh    # 7 тестов: IPC команды
```

- [ ] **Все тесты зелёные** (pass, ни одного fail)

---

## Результат

Если все 50+ пунктов ✅ — можно тегать релиз. Если что-то ❌ — пиши что именно сломалось.
