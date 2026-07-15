<p align="center">
    <img width="200" alt="Alacritty Logo" src="https://raw.githubusercontent.com/alacritty/alacritty/master/extra/logo/compat/alacritty-term%2Bscanlines.png">
</p>

<h1 align="center">Alacritty-Kitty — GPU terminal with tabs, tiling, modal menu</h1>

## This Fork — Alacritty-Kitty

GPU terminal fork — tabs, modal menu, tiling panes, session
save/restore, and IPC — built alongside AI (OpenCode).

### Features

- **Tabs** — `Ctrl+Shift+T` create, `Ctrl+Shift+W` close, `Ctrl+Tab` switch. Tab bar with 5 styles (Slant, Separator, Fade, Powerline, Hidden). Pin tabs, rename, move, per-tab titles with hysteresis.
- **Modal Menu Bar** — `Ctrl+G` unlocks, letter-key navigation, nested submenus, list mode (session files). Same rendering mechanism as tab bar via `draw_bar(edge)`.
- **Tiling Panes (BSP Tree)** — Split right/down, resize (`Ctrl+G, p, r` + `Alt+arrows`), focus (`Alt+arrows`), zoom (`Alt+F`), close (`Ctrl+Shift+#`). Fixup_subtree preserves far-side pixel positions on resize (19/19 tests).
- **Session Save/Restore** — `Ctrl+G, s, s` saves to `~/.config/alacritty/sessions/`. `--restore` CLI flag. Pane tree + foreground commands preserved.
- **IPC** — `alacritty msg create-tab/close-tab/list-tabs/select-tab/pin-tab/save-tabs/quickrun`. Unix socket at `$XDG_RUNTIME_DIR`.
- **CLI Flags** — `--create-config`, `--create-systemd-unit`, `--restore`.

### One-Line Install

```bash
curl -fsSL https://github.com/MayBurke11/alacritty/releases/download/v0.18.0/alacritty -o ~/.local/bin/alacritty && chmod +x ~/.local/bin/alacritty && mkdir -p ~/.local/share/applications ~/.config/alacritty && cat > ~/.local/share/applications/alacritty.desktop << 'EOF'
[Desktop Entry]
Type=Application
Name=Alacritty-Kitty
Comment=GPU terminal with tabs, tiling, modal menu
Icon=utilities-terminal
Exec=alacritty
Terminal=false
Categories=System;TerminalEmulator;
StartupNotify=true
StartupWMClass=Alacritty
EOF
~/.local/bin/alacritty --create-config > ~/.config/alacritty/alacritty.toml && echo 'Done! Run: alacritty'
```

Downloads the release binary, installs a desktop entry, and generates a default config.

### Quick Start (Build from Source)

```bash
cargo build --release
./target/release/alacritty

# Generate default config
./target/release/alacritty --create-config > ~/.config/alacritty/alacritty.toml
```

### Default Key Bindings

| Key | Action |
|---|---|
| `Ctrl+G` | Toggle menu bar (LOCKED ↔ ACTIVE) |
| `Ctrl+Shift+T` | Create tab |
| `Ctrl+Shift+W` | Close tab |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next/previous tab |
| `Alt+←/→/↑/↓` | Focus pane (add to personal config) |
| `Alt+F` | Toggle pane zoom (add to personal config) |
| `Ctrl+Shift+#` | Close pane |
| `Alt+F4` | Quit |

### Menu Flow

```
Ctrl+G → [ACTIVE] [PAN] [TAB] [SESS]
  p, s, r → PAN → SPLIT → RIGHT (split right)
  p, s, d → PAN → SPLIT → DOWN (split down)
  p, r    → PAN → RESIZE (Alt+arrows resize, Esc exit)
  p, k    → PAN → KILL (close pane)
  p, f    → PAN → FULL (zoom toggle)
  t, c    → TAB → CREATE (new tab)
  t, k    → TAB → KILL (close tab)
  s, s    → SESS → SAVE (save session)
  s, l    → SESS → LOAD (list sessions, Alt+arrows select)
```

### Example Config

```toml
[menu]
menu_bar_edge = "bottom"

[[menu.items]]
label = "PAN"

[[menu.items.submenu]]
label = "SPLIT"
[[menu.items.submenu.submenu]]
label = "RIGHT"
action = "split-right"
[[menu.items.submenu.submenu]]
label = "DOWN"
action = "split-down"

[[menu.items.submenu]]
label = "RESIZE"
action = "resize-mode"

[[menu.items.submenu]]
label = "KILL"
action = "close-pane"

[[menu.items]]
label = "TAB"
[[menu.items.submenu]]
label = "CREATE"
action = "create-tab"
[[menu.items.submenu]]
label = "KILL"
action = "close-tab"

[[menu.items]]
label = "SESS"
[[menu.items.submenu]]
label = "SAVE"
action = "save-session"
[[menu.items.submenu]]
label = "LOAD"
list = "sessions"

[tabs]
tab_bar_edge = "top"
tab_bar_style = "Slant"
tab_bar_min_tabs = 1
```

### Known Issues

- Menu bar `top` edge may not render correctly. Use `menu_bar_edge = "bottom"` as workaround.
- `Alt+arrows` focus bindings disabled by default — add to personal config.
- See `docs/known-bugs.md` for more.

## Original Alacritty README
## About

Alacritty is a modern terminal emulator that comes with sensible defaults, but
allows for extensive [configuration](#configuration). By integrating with other
applications, rather than reimplementing their functionality, it manages to
provide a flexible set of [features](./docs/features.md) with high performance.
The supported platforms currently consist of BSD, Linux, macOS and Windows.

The software is considered to be at a **beta** level of readiness; there are
a few missing features and bugs to be fixed, but it is already used by many as
a daily driver.

Precompiled binaries are available from the [GitHub releases page](https://github.com/alacritty/alacritty/releases).

Join [`#alacritty`] on libera.chat if you have questions or looking for a quick help.

[`#alacritty`]: https://web.libera.chat/gamja/?channels=#alacritty

## Features

You can find an overview over the features available in Alacritty [here](./docs/features.md).

## Further information

- [Announcing Alacritty, a GPU-Accelerated Terminal Emulator](https://jwilm.io/blog/announcing-alacritty/) January 6, 2017
- [A talk about Alacritty at the Rust Meetup January 2017](https://www.youtube.com/watch?v=qHOdYO3WUTk) January 19, 2017
- [Alacritty Lands Scrollback, Publishes Benchmarks](https://jwilm.io/blog/alacritty-lands-scrollback/) September 17, 2018

## Installation

Alacritty can be installed by using various package managers on Linux, BSD,
macOS and Windows.

Prebuilt binaries for macOS and Windows can also be downloaded from the
[GitHub releases page](https://github.com/alacritty/alacritty/releases).

For everyone else, the detailed instructions to install Alacritty can be found
[here](INSTALL.md).

### Requirements

- At least OpenGL ES 2.0
- [Windows] ConPTY support (Windows 10 version 1809 or higher)

## Configuration

You can find the documentation for Alacritty's configuration in `man 5
alacritty`, or by looking at [the website] if you do not have the manpages
installed.

[the website]: https://alacritty.org/config-alacritty.html

Alacritty doesn't create the config file for you, but it looks for one in the
following locations:

1. `$XDG_CONFIG_HOME/alacritty/alacritty.toml`
2. `$XDG_CONFIG_HOME/alacritty.toml`
3. `$HOME/.config/alacritty/alacritty.toml`
4. `$HOME/.alacritty.toml`
5. `/etc/alacritty/alacritty.toml`

On Windows, the config file will be looked for in:

* `%APPDATA%\alacritty\alacritty.toml`

## Contributing

A guideline about contributing to Alacritty can be found in the
[`CONTRIBUTING.md`](CONTRIBUTING.md) file.

## FAQ

**_Is it really the fastest terminal emulator?_**

Benchmarking terminal emulators is complicated. Alacritty uses
[vtebench](https://github.com/alacritty/vtebench) to quantify terminal emulator
throughput and manages to consistently score better than the competition using
it. If you have found an example where this is not the case, please report a
bug.

Other aspects like latency or framerate and frame consistency are more difficult
to quantify. Some terminal emulators also intentionally slow down to save
resources, which might be preferred by some users.

If you have doubts about Alacritty's performance or usability, the best way to
quantify terminal emulators is always to test them with **your** specific
usecases.

**_Why isn't feature X implemented?_**

Alacritty has many great features, but not every feature from every other
terminal. This could be for a number of reasons, but sometimes it's just not a
good fit for Alacritty. This means you won't find things like splits (which are
best left to a window manager or [terminal multiplexer][tmux]) nor niceties
like a GUI config editor.

[tmux]: https://github.com/tmux/tmux

## License

Alacritty is released under the [Apache License, Version 2.0].

[Apache License, Version 2.0]: https://github.com/alacritty/alacritty/blob/master/LICENSE-APACHE
