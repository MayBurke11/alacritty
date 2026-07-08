# QuickRun: inline command runner for new tabs

> Branch: `tab-dev` · Tag: `tab-ipc-done`

`Ctrl+Shift+P` → opens inline field `"run: "` in footer → type command → Enter spawns new tab with that command.

## What exists (reuse)

- `TabTitleEditor` — inline input in footer with keyboard intercept (Backspace, Enter, Esc)
- `start_tab_title_editor()` / `confirm_tab_title_editor()` / `cancel_tab_title_editor()` / `tab_title_input()` / `tab_title_pop_word()`
- `WindowContext.tab_title_editor: Option<TabTitleEditor>`
- `tab_title_editor_active` flag in ActionContext — routes keys to editor instead of terminal
- `draw_footer_text()` in display/mod.rs

## Changes needed

### 1. New `TabAction` variants in `event.rs` (2 lines)
```rust
pub enum TabAction {
    // ...
    Run,            // Open run: editor
    ConfirmRun,     // Execute command in new tab
    CancelRun,      // Close run: editor
    RunInput(char), // Type in run: editor
    RunPopWord,     // Ctrl+W in run: editor
}
```

### 2. New binding in `config/bindings.rs` (2 lines)
```rust
"p", ModifiersState::CONTROL | ModifiersState::SHIFT; Action::QuickRun;
```

### 3. `WindowContext` methods (~30 lines)

```rust
struct RunEditor {
    value: String,
}

fn start_run_editor(&mut self) {
    self.run_editor = Some(RunEditor { value: String::new() });
}

fn confirm_run_editor(&mut self) {
    let Some(editor) = self.run_editor.take() else { return };
    let cmd = editor.value.trim().to_owned();
    if !cmd.is_empty() {
        let _ = self.create_tab_with_command(&cmd);
    }
}

fn cancel_run_editor(&mut self) { self.run_editor.take(); }

fn run_editor_input(&mut self, c: char) {
    // same as tab_title_input but on self.run_editor
}

fn run_editor_pop_word(&mut self) {
    // same as tab_title_pop_word but on self.run_editor
}
```

### 4. `create_tab_with_command(&mut self, command: &str)` (~15 lines)

Modify `create_tab()` to accept optional command:
```rust
fn create_tab(&mut self) -> Result<(), Box<dyn Error>> {
    self.create_tab_inner(WindowOptions::default())
}

fn create_tab_with_command(&mut self, command: &str) -> Result<(), Box<dyn Error>> {
    let mut opts = WindowOptions::default();
    opts.terminal_options.command = Some(command.to_owned());
    self.create_tab_inner(opts)
}
```

### 5. Footer rendering in `display/mod.rs` (~5 lines)

Check `run_editor.is_some()` and draw `"run: {value}"` instead of tab title editor.

### 6. Input routing — existing `tab_title_editor_active` already works

The input processor checks `tab_title_editor_active()` before routing to terminal. We just add `run_editor.is_some()` to that check.

### 7. E2E test (~30 lines)

```bash
xdotool key ctrl+shift+p
xdotool type "htop"
xdotool key Return
# assert: new tab created with htop
```

## Scope: ~80 lines, one evening.
