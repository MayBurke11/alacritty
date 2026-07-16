//! Terminal window context.

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::mem;
#[cfg(not(windows))]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(not(windows))]
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use glutin::config::Config as GlutinConfig;
use glutin::display::GetGlDisplay;
#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use glutin::platform::x11::X11GlConfigExt;
use log::info;
use serde_json as json;
use toml::Value as TomlValue;
use winit::event::{Event as WinitEvent, Modifiers, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowId;

use alacritty_terminal::event::Event as TerminalEvent;
use alacritty_terminal::event_loop::{EventLoop as PtyEventLoop, Msg, Notifier};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::Direction;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::tty;

use crate::cli::{ParsedOptions, WindowOptions};
use crate::clipboard::Clipboard;
use crate::config::UiConfig;
use crate::config::tabs::{TabBarEdge, TabSwitchStrategy};
#[cfg(not(windows))]
use crate::daemon::foreground_process_path;
use crate::display::Display;
use crate::display::window::Window;
use crate::event::{
    ActionContext, Event, EventProxy, EventType, InlineSearchState, MenuOp, MenuSelection,
    MenuState, Mouse, SearchState, TabAction, TabId, TouchPurpose,
};
use crate::pane_manager::PaneManager;
use crate::menu_controller::MenuController;
use crate::tab::{TerminalTab, TabTitleEditor};
use crate::pane_state::PaneState;
use crate::pane_tree::{FocusDir, PaneId, PaneNode, SplitDir};
#[cfg(unix)]
use crate::logging::LOG_TARGET_IPC_CONFIG;
use crate::message_bar::{Message, MessageBuffer, MessageType};
use crate::scheduler::Scheduler;
use crate::{input, renderer};

#[cfg(not(windows))]
fn process_cwd(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(not(windows))]
fn format_tab_cwd(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    if home.as_deref().is_some_and(|home| home == path) {
        return String::from("~");
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) const WINDOW_CLOSE_CONFIRMATION_TARGET: &str = "window_close_confirmation";

pub struct WindowContext {
    pub display: Display,
    pub dirty: bool,
    event_queue: Vec<WinitEvent<Event>>,
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    next_tab_id: u64,
    last_active_tab_id: Option<TabId>,
    tab_title_editor: Option<TabTitleEditor>,
    run_editor: Option<String>,
    /// Modal menu controller.
    menu: MenuController,
    /// Divider drag state: (tab_idx, split_id, direction, start_x, start_y, start_ratio).
    divider_drag: Option<(usize, crate::pane_tree::SplitId, SplitDir, f32, f32, f32)>,
    /// Alt+arrows resize panes instead of focusing.
    pane_resize_mode: bool,
    window_close_confirmation_pending: bool,
    focused: bool,
    modifiers: Modifiers,
    mouse: Mouse,
    touch: TouchPurpose,
    occluded: bool,
    preserve_title: bool,
    window_config: ParsedOptions,
    config: Rc<UiConfig>,
    event_proxy: EventLoopProxy<Event>,
}

impl WindowContext {
    fn active_tab(&self) -> &TerminalTab {
        &self.tabs[self.active_tab]
    }

    fn active_tab_mut(&mut self) -> &mut TerminalTab {
        &mut self.tabs[self.active_tab]
    }

    fn tab_index(&self, tab_id: Option<TabId>) -> Option<usize> {
        match tab_id {
            Some(tab_id) => self.tabs.iter().position(|tab| tab.id == tab_id),
            None => Some(self.active_tab),
        }
    }

    fn tab_bar_lines(&self) -> usize {
        usize::from(self.config.tabs.display_tab_bar(self.tabs.len()))
    }

    fn tab_bar_at_top(&self) -> bool {
        self.tab_bar_lines() != 0 && self.config.tabs.tab_bar_edge == TabBarEdge::Top
    }

    fn refresh_window_title(&mut self) {
        let title = self.active_tab().display_title().to_owned();
        self.display.window.set_title(title);
    }

    fn start_tab_title_editor(&mut self) {
        let tab = self.active_tab();
        self.tab_title_editor = Some(TabTitleEditor {
            tab_id: tab.id,
            value: tab.custom_title.clone().unwrap_or_default(),
        });
        self.mark_dirty();
    }

    fn confirm_tab_title_editor(&mut self) {
        let Some(editor) = self.tab_title_editor.take() else {
            return;
        };
        let Some(index) = self.tabs.iter().position(|tab| tab.id == editor.tab_id) else {
            return;
        };

        let title = editor.value.trim().to_owned();
        self.tabs[index].custom_title = (!title.is_empty()).then_some(title);
        if index == self.active_tab {
            self.refresh_window_title();
        }
        self.mark_dirty();
    }

    fn cancel_tab_title_editor(&mut self) {
        if self.tab_title_editor.take().is_some() {
            self.mark_dirty();
        }
    }

    fn start_run_editor(&mut self) {
        self.run_editor = Some(String::new());
        self.mark_dirty();
    }

    fn confirm_run_editor(&mut self, no_switch: bool) {
        let Some(cmd) = self.run_editor.take() else { return };
        let cmd = cmd.trim().to_owned();
        if !cmd.is_empty() {
            let args: Vec<String> = cmd.split_whitespace().map(|s| s.to_owned()).collect();
            let _ = self.create_tab_inner(Some(args), None, no_switch, None);
        }
        self.mark_dirty();
    }

    fn cancel_run_editor(&mut self) {
        if self.run_editor.take().is_some() {
            self.mark_dirty();
        }
    }

    fn run_editor_input(&mut self, c: char) {
        let Some(ref mut value) = self.run_editor else { return };
        match c {
            '\x08' | '\x7f' => { value.pop(); }
            _ => value.push(c),
        }
        self.dirty = true;
    }

    fn run_editor_pop_word(&mut self) {
        let Some(ref mut value) = self.run_editor else { return };
        while value.pop().is_some_and(|c| c != ' ') {}
        self.dirty = true;
    }

    fn window_close_confirmation_message(&self) -> Message {
        let tab_count = self.tabs.len();
        let tab_label = if tab_count == 1 { "tab" } else { "tabs" };
        let mut message = Message::new(
            format!(
                "Close window and all {tab_count} {tab_label}? Press Enter or close the window \
                 again to confirm. Press Escape to cancel."
            ),
            MessageType::Warning,
        );
        message.set_target(WINDOW_CLOSE_CONFIRMATION_TARGET.to_owned());
        message
    }

    fn show_window_close_confirmation(&mut self) {
        let message = self.window_close_confirmation_message();
        for tab in &mut self.tabs {
            tab.message_buffer.remove_target(WINDOW_CLOSE_CONFIRMATION_TARGET);
            tab.message_buffer.push(message.clone());
        }

        self.window_close_confirmation_pending = true;
        self.mark_dirty();
    }

    fn cancel_window_close_confirmation(&mut self) {
        if !self.window_close_confirmation_pending {
            return;
        }

        self.window_close_confirmation_pending = false;
        for tab in &mut self.tabs {
            tab.message_buffer.remove_target(WINDOW_CLOSE_CONFIRMATION_TARGET);
        }

        self.mark_dirty();
    }

    fn confirm_window_close(&mut self) {
        self.window_close_confirmation_pending = false;
        for tab in &mut self.tabs {
            tab.message_buffer.remove_target(WINDOW_CLOSE_CONFIRMATION_TARGET);
        }

        // Auto-save session if enabled.
        if self.config.general.auto_save_session {
            if let Ok(home) = std::env::var("HOME") {
                let dir = std::path::PathBuf::from(home).join(".config").join("alacritty").join("sessions");
                std::fs::create_dir_all(&dir).ok();
                let path = dir.join("last.json");
                let data = self.tabs_save_json();
                std::fs::write(&path, data).ok();
                log::info!("[session] auto-saved to {}", path.display());
            }
        }

        self.display.window.hold = false;
        for tab in &mut self.tabs {
            tab.terminal.lock().exit();
        }
    }

    fn request_window_close(&mut self) {
        if self.tabs.len() > 1 && !self.window_close_confirmation_pending {
            self.cancel_tab_title_editor();
            self.show_window_close_confirmation();
        } else {
            self.confirm_window_close();
        }
    }

    fn tab_title_input(&mut self, c: char) {
        let Some(editor) = self.tab_title_editor.as_mut() else {
            return;
        };

        match c {
            '\x08' | '\x7f' => {
                let _ = editor.value.pop();
            },
            ' '..='~' | '\u{a0}'..='\u{10ffff}' => editor.value.push(c),
            _ => return,
        }

        self.mark_dirty();
    }

    fn tab_title_pop_word(&mut self) {
        let Some(editor) = self.tab_title_editor.as_mut() else {
            return;
        };

        editor.value = editor.value.trim_end().to_owned();
        editor.value.truncate(editor.value.rfind(' ').map_or(0, |index| index + 1));
        self.mark_dirty();
    }

    fn render_tab_title(&self, index: usize, tab: &TerminalTab, active: bool) -> String {
        let template =
            if active { self.config.tabs.active_tab_title_template.as_deref() } else { None }
                .unwrap_or(&self.config.tabs.tab_title_template);

        let title = tab.display_title();
        let rendered = template
            .replace("{title}", title)
            .replace("{index}", &(index + 1).to_string())
            .replace("{num_windows}", "1");

        let mut title = if rendered.is_empty() { title.to_owned() } else { rendered };
        if tab.bell_pending {
            title = self.config.tabs.tab_bell_indicator.replace("{title}", &title);
        }
        if tab.pinned { format!("*{}", title) } else { title }
    }

    fn mark_dirty(&mut self) {
        self.display.pending_update.dirty = true;
        self.dirty = true;
    }

    fn process_menu_ops(&mut self) {
        if std::mem::take(&mut self.menu.toggle_pending) {
            self.menu.state.active = !self.menu.state.active;
            if !self.menu.state.active {
                self.menu.state.path.clear();
                self.menu.state.list_mode = None;
                self.menu.state.focus = 0;
            }
            log::debug!("[menu-sync] op=ToggleLocked AFTER: active={} path={:?} list={:?} focus={} labels={:?}",
                self.menu.state.active, self.menu.state.path,
                self.menu.state.list_mode.is_some(), self.menu.state.focus,
                self.menu.state.bar_labels(&self.config.menu, self.active_tab().panes.tree.leaf_ids().len(), self.pane_resize_mode).iter().map(|(l,_)| l.as_str()).collect::<Vec<_>>());
            self.dirty = true;
        }

        for op in std::mem::take(&mut self.menu.op_pending) {
            log::debug!("[menu-sync] op={:?} BEFORE: active={} path={:?} list={:?} focus={}",
                op, self.menu.state.active, self.menu.state.path,
                self.menu.state.list_mode.is_some(), self.menu.state.focus);
            match op {
                MenuOp::FocusLeft => {
                    if self.menu.state.focus > 0 { self.menu.state.focus -= 1; }
                    self.dirty = true;
                },
                MenuOp::FocusRight => {
                    let count = self.menu.state.item_count(&self.config.menu);
                    if self.menu.state.focus + 1 <= count { self.menu.state.focus += 1; }
                    self.dirty = true;
                },
                MenuOp::Select => {
                    let sel = self.menu.state.select(&self.config.menu);
                    self.handle_menu_selection(sel);
                    self.dirty = true;
                },
                MenuOp::Back => { self.menu.state.back(); self.dirty = true; },
                MenuOp::Click(idx) => {
                    self.menu.state.focus = idx;
                    let sel = self.menu.state.select(&self.config.menu);
                    self.handle_menu_selection(sel);
                    self.dirty = true;
                },
                MenuOp::LetterKey(ch) => {
                    let labels = self.menu.state.bar_labels(&self.config.menu, self.active_tab().panes.tree.leaf_ids().len(), self.pane_resize_mode);
                    let ch_lower: char = ch.to_lowercase().next().unwrap_or(ch);
                    for (i, (label, _)) in labels.iter().enumerate() {
                        if i == 0 { continue; }
                        let first_char = label.chars().next().map(|c| c.to_lowercase().next().unwrap_or(c));
                        if first_char == Some(ch_lower) {
                            log::debug!("[menu-sync] LetterKey '{}' matched label '{}' at idx={}", ch, label, i);
                            self.menu.state.focus = i;
                            let sel = self.menu.state.select(&self.config.menu);
                            self.handle_menu_selection(sel);
                            break;
                        }
                    }
                    self.dirty = true;
                },
            }
            log::debug!("[menu-sync] op={:?} AFTER: active={} path={:?} list={:?} focus={} labels={:?}",
                op, self.menu.state.active, self.menu.state.path,
                self.menu.state.list_mode.is_some(), self.menu.state.focus,
                self.menu.state.bar_labels(&self.config.menu, self.active_tab().panes.tree.leaf_ids().len(), self.pane_resize_mode).iter().map(|(l,_)| l.as_str()).collect::<Vec<_>>());
        }
    }

    /// Returns true if the caller should `continue` to the next event.
    fn dispatch_tab_action(&mut self, action: &TabAction) -> bool {
        use crate::action::Action as AppAction;
        use crate::action::{FocusDir, MenuDir, SplitDir};
        let app_action = match action {
            TabAction::Create => AppAction::CreateTab { command: None, cwd: None, config: None, no_switch: false },
            TabAction::Close => AppAction::CloseTab { index: None },
            TabAction::ConfirmWindowClose => { self.confirm_window_close(); return true; },
            TabAction::CancelWindowClose => { self.cancel_window_close_confirmation(); return true; },
            TabAction::SelectNext => AppAction::SelectNextTab,
            TabAction::SelectPrevious => AppAction::SelectPreviousTab,
            TabAction::Select(index) => AppAction::SelectTab { index: *index },
            TabAction::SelectLast => AppAction::SelectLastTab,
            TabAction::MoveForward => AppAction::MoveTab { index: self.active_tab, delta: 1 },
            TabAction::MoveBackward => AppAction::MoveTab { index: self.active_tab, delta: -1 },
            TabAction::SetTitle => { self.start_tab_title_editor(); return true; },
            TabAction::ConfirmTitle => { self.confirm_tab_title_editor(); return true; },
            TabAction::CancelTitle => { self.cancel_tab_title_editor(); return true; },
            TabAction::TitleInput(c) => { self.tab_title_input(*c); return true; },
            TabAction::TitlePopWord => { self.tab_title_pop_word(); return true; },
            TabAction::Run => { self.start_run_editor(); return true; },
            TabAction::ConfirmRun => { self.confirm_run_editor(false); return true; },
            TabAction::ConfirmRunNoSwitch => { self.confirm_run_editor(true); return true; },
            TabAction::CancelRun => { self.cancel_run_editor(); return true; },
            TabAction::RunInput(c) => { self.run_editor_input(*c); return true; },
            TabAction::RunPopWord => { self.run_editor_pop_word(); return true; },
            TabAction::TogglePin => AppAction::TogglePin { index: self.active_tab },
            TabAction::ToggleMenu(idx) => {
                self.menu.state.focus = *idx;
                let sel = self.menu.state.select(&self.config.menu);
                self.handle_menu_selection(sel);
                self.dirty = true;
                return true;
            },
            TabAction::MenuCommand(_, _) => { self.dirty = true; return true; },
            TabAction::ToggleLocked => AppAction::ToggleMenu,
            TabAction::MenuFocusLeft => AppAction::MenuNavigate { direction: MenuDir::Left },
            TabAction::MenuFocusRight => AppAction::MenuNavigate { direction: MenuDir::Right },
            TabAction::MenuSelect => AppAction::MenuSelect,
            TabAction::MenuBack => AppAction::MenuBack,
            TabAction::MenuClick(idx) => AppAction::MenuClick { index: *idx },
            TabAction::MenuLetterKey(ch) => AppAction::MenuLetter { ch: *ch },
            TabAction::SplitRight => AppAction::SplitPane { direction: SplitDir::Right },
            TabAction::SplitDown => AppAction::SplitPane { direction: SplitDir::Down },
            TabAction::ClosePane => AppAction::ClosePane,
            TabAction::FocusLeft => AppAction::FocusPane { direction: FocusDir::Left },
            TabAction::FocusRight => AppAction::FocusPane { direction: FocusDir::Right },
            TabAction::FocusUp => AppAction::FocusPane { direction: FocusDir::Up },
            TabAction::FocusDown => AppAction::FocusPane { direction: FocusDir::Down },
            TabAction::ToggleZoom => AppAction::ToggleZoom,
            TabAction::ResizeRight => AppAction::ResizePane { direction: SplitDir::Right, grow: true },
            TabAction::ResizeLeft => AppAction::ResizePane { direction: SplitDir::Right, grow: false },
            TabAction::ResizeUp => AppAction::ResizePane { direction: SplitDir::Down, grow: false },
            TabAction::ResizeDown => AppAction::ResizePane { direction: SplitDir::Down, grow: true },
        };
        let _ = self.handle_action(app_action);
        false
    }

    fn sync_focus(&mut self) {
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            let is_active_tab = self.focused && index == self.active_tab;
            tab.terminal.lock().is_focused = is_active_tab && tab.panes.active == PaneId(0);
            for (_, pane) in &tab.panes.additional {
                pane.terminal.lock().is_focused = is_active_tab && tab.panes.active == pane.pane_id;
            }
        }
    }

    pub fn handle_action(&mut self, action: crate::action::Action) -> crate::action::ActionResult {
        use crate::action::{ActionResult, FocusDir, MenuDir, SplitDir};
        use crate::pane_tree::FocusDir as PaneFocusDir;
        use crate::pane_tree::SplitDir as PaneSplitDir;

        match action {
            crate::action::Action::NewWindow { .. } => ActionResult::err("use msg window create"),
            crate::action::Action::CloseWindow => ActionResult::err("not yet implemented"),
            crate::action::Action::CreateTab { command, cwd, config, no_switch } => {
                let options = crate::cli::TabCreateOptions {
                    command: command.unwrap_or_default(),
                    working_directory: cwd.map(std::path::PathBuf::from),
                    no_switch,
                    config_overrides: translate_config_overrides(config),
                    window_id: None,
                };
                self.create_tab_ipc(options);
                ActionResult::success()
            },
            crate::action::Action::CloseTab { index } => {
                let idx = index.unwrap_or(self.active_tab);
                if idx < self.tabs.len() && !self.tabs[idx].pinned {
                    let closing_id = self.tabs[idx].id;
                    self.handle_tab_exit(Some(closing_id));
                }
                ActionResult::success()
            },
            crate::action::Action::SelectTab { index } => {
                if index < self.tabs.len() {
                    self.set_active_tab(index);
                }
                ActionResult::success()
            },
            crate::action::Action::SelectNextTab => {
                if !self.tabs.is_empty() {
                    self.set_active_tab((self.active_tab + 1) % self.tabs.len());
                }
                ActionResult::success()
            },
            crate::action::Action::SelectPreviousTab => {
                if !self.tabs.is_empty() {
                    let prev = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
                    self.set_active_tab(prev);
                }
                ActionResult::success()
            },
            crate::action::Action::SelectLastTab => {
                if !self.tabs.is_empty() {
                    self.set_active_tab(self.tabs.len() - 1);
                }
                ActionResult::success()
            },
            crate::action::Action::ListTabs => {
                let tabs: Vec<serde_json::Value> = self.tabs.iter().enumerate().map(|(i, t)| {
                    serde_json::json!({
                        "index": i + 1,
                        "id": t.id.0,
                        "title": t.display_title(),
                        "active": i == self.active_tab,
                        "pinned": t.pinned,
                    })
                }).collect();
                ActionResult::data(serde_json::Value::Array(tabs))
            },
            crate::action::Action::MoveTab { index, delta } => {
                if index < self.tabs.len() {
                    let new = if delta < 0 {
                        index.saturating_sub(delta.unsigned_abs() as usize)
                    } else {
                        (index + delta as usize).min(self.tabs.len() - 1)
                    };
                    self.tabs.swap(index, new);
                    if self.active_tab == index { self.active_tab = new; }
                    else if self.active_tab == new { self.active_tab = index; }
                    self.dirty = true;
                }
                ActionResult::success()
            },
            crate::action::Action::TogglePin { index } => {
                if index < self.tabs.len() {
                    self.tabs[index].pinned = !self.tabs[index].pinned;
                    self.dirty = true;
                }
                ActionResult::success()
            },
            crate::action::Action::SetTabTitle { title } => {
                self.active_tab_mut().custom_title = if title.is_empty() { None } else { Some(title) };
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::SplitPane { direction } => {
                match direction {
                    SplitDir::Right => self.split_pane(PaneSplitDir::Horizontal),
                    SplitDir::Down => self.split_pane(PaneSplitDir::Vertical),
                }
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::ClosePane => {
                self.close_pane();
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::FocusPane { direction } => {
                match direction {
                    FocusDir::Left => self.focus_pane(PaneFocusDir::Left),
                    FocusDir::Right => self.focus_pane(PaneFocusDir::Right),
                    FocusDir::Up => self.focus_pane(PaneFocusDir::Up),
                    FocusDir::Down => self.focus_pane(PaneFocusDir::Down),
                }
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::ToggleZoom => {
                self.toggle_zoom();
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::ResizePane { direction, grow } => {
                let sd = match direction {
                    SplitDir::Right => PaneSplitDir::Horizontal,
                    SplitDir::Down => PaneSplitDir::Vertical,
                };
                self.resize_pane(sd, grow);
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::SaveSession => {
                let dir = std::env::var("HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(".config").join("alacritty").join("sessions");
                std::fs::create_dir_all(&dir).ok();
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let path = dir.join(format!("session-{ts}.json"));
                let data = self.tabs_save_json();
                std::fs::write(&path, &data).ok();
                ActionResult::success()
            },
            crate::action::Action::LoadSession { name } => {
                let dir = std::env::var("HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(".config").join("alacritty").join("sessions");
                let path = dir.join(format!("{name}.json"));
                match std::fs::read(&path) {
                    Ok(data) => {
                        if let Err(err) = self.restore_tabs(&data) {
                            ActionResult::err(format!("failed to restore: {err}"))
                        } else {
                            ActionResult::success()
                        }
                    },
                    Err(err) => ActionResult::err(format!("cannot load {name}: {err}")),
                }
            },
            crate::action::Action::ListSessions => {
                let dir = std::env::var("HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join(".config").join("alacritty").join("sessions");
                let mut sessions = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for entry in rd.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".json") {
                            sessions.push(serde_json::Value::String(name.trim_end_matches(".json").to_string()));
                        }
                    }
                    sessions.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                }
                ActionResult::data(serde_json::Value::Array(sessions))
            },
            crate::action::Action::GetConfig => {
                let config = serde_json::to_value(&*self.config).unwrap_or_default();
                ActionResult::data(config)
            },
            crate::action::Action::SetConfig { options, reset: _reset } => {
                for (key, value) in &options {
                    let toml_val = match value {
                        serde_json::Value::String(s) => TomlValue::String(s.clone()),
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() {
                                TomlValue::Integer(i)
                            } else if let Some(f) = n.as_f64() {
                                TomlValue::Float(f)
                            } else {
                                TomlValue::String(n.to_string())
                            }
                        },
                        serde_json::Value::Bool(b) => TomlValue::Boolean(*b),
                        _ => TomlValue::String(value.to_string()),
                    };
                    self.window_config.push((key.clone(), toml_val));
                }
                let overridden = self.window_config.override_config_rc(self.config.clone());
                self.update_config(overridden);
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::QuickRun { command, no_switch } => {
                let options = crate::cli::TabCreateOptions {
                    command,
                    working_directory: None,
                    no_switch,
                    config_overrides: Vec::new(),
                    window_id: None,
                };
                self.create_tab_ipc(options);
                ActionResult::success()
            },
            crate::action::Action::Scroll { lines } => {
                self.active_terminal().lock().scroll_display(Scroll::Delta(lines));
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::Copy => ActionResult::err("copy via keyboard binding only"),
            crate::action::Action::Paste { data: _ } => ActionResult::success(),
            crate::action::Action::ToggleMenu => {
                self.menu.toggle_pending = true;
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::MenuNavigate { direction } => {
                match direction {
                    MenuDir::Left => self.menu.op_pending.push(crate::event::MenuOp::FocusLeft),
                    MenuDir::Right => self.menu.op_pending.push(crate::event::MenuOp::FocusRight),
                }
                ActionResult::success()
            },
            crate::action::Action::MenuSelect => {
                self.menu.op_pending.push(crate::event::MenuOp::Select);
                ActionResult::success()
            },
            crate::action::Action::MenuBack => {
                self.menu.op_pending.push(crate::event::MenuOp::Back);
                ActionResult::success()
            },
            crate::action::Action::MenuLetter { ch } => {
                self.menu.op_pending.push(crate::event::MenuOp::LetterKey(ch));
                ActionResult::success()
            },
            crate::action::Action::MenuClick { index } => {
                self.menu.op_pending.push(crate::event::MenuOp::Click(index));
                ActionResult::success()
            },
            crate::action::Action::Bell => {
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::Tree => {
                ActionResult::data(self.build_tree_json())
            },
            crate::action::Action::SearchForward => {
                let tab = self.active_tab_mut();
                tab.search_state.direction = alacritty_terminal::index::Direction::Right;
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::SearchBackward => {
                let tab = self.active_tab_mut();
                tab.search_state.direction = alacritty_terminal::index::Direction::Left;
                self.dirty = true;
                ActionResult::success()
            },
            crate::action::Action::SearchNext => {
                self.dirty = true;
                ActionResult::success()
            },
        }
    }

    fn set_active_tab(&mut self, index: usize) {
        if self.tabs.is_empty() {
            return;
        }

        let index = index.min(self.tabs.len() - 1);
        if self.active_tab == index {
            return;
        }

        self.last_active_tab_id = Some(self.tabs[self.active_tab].id);
        self.active_tab = index;
        self.tabs[index].bell_pending = false;
        info!("[tabs] switch to tab {} (of {})", index + 1, self.tabs.len());
        let config = self.config.clone();
        self.active_tab_mut().refresh_detected_title(&config);
        self.cancel_tab_title_editor();
        self.display.clear_hint_highlights();
        self.sync_focus();
        self.refresh_window_title();
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.mark_dirty();
    }

    fn create_tab(&mut self) -> Result<(), Box<dyn Error>> {
        self.create_tab_inner(None, None, false, None)
    }

    /// Create a new tab via IPC with optional command, working directory, and no-switch flag.
    pub fn create_tab_ipc(&mut self, options: crate::cli::TabCreateOptions) {
        let cmd = (!options.command.is_empty()).then(|| options.command.clone());
        let cwd = options.working_directory.clone();
        let no_switch = options.no_switch;
        let overrides = (!options.config_overrides.is_empty()).then(|| options.config_overrides.clone());
        if let Err(err) = self.create_tab_inner(cmd, cwd, no_switch, overrides) {
            log::warn!("Failed to create tab via IPC: {err:?}");
        }
    }

    /// QuickRun via IPC — create a tab with the given command.
    pub fn quick_run_ipc(&mut self, options: crate::cli::TabQuickRun) {
        let cmd = (!options.command.is_empty()).then(|| options.command.clone());
        let no_switch = options.no_switch;
        let overrides = (!options.config_overrides.is_empty()).then(|| options.config_overrides.clone());
        if let Err(err) = self.create_tab_inner(cmd, None, no_switch, overrides) {
            log::warn!("Failed to create tab via QuickRun IPC: {err:?}");
        }
    }

    fn create_tab_inner(
        &mut self,
        command: Option<Vec<String>>,
        working_directory: Option<PathBuf>,
        no_switch: bool,
        config_overrides: Option<Vec<String>>,
    ) -> Result<(), Box<dyn Error>> {
        self.cancel_window_close_confirmation();

        let tab_id = TabId(self.next_tab_id);
        self.next_tab_id += 1;

        let mut options = WindowOptions::default();

        // IPC-provided working directory takes priority over inheritance.
        if let Some(ref wd) = working_directory {
            options.terminal_options.working_directory = Some(wd.clone());
        } else {
            #[cfg(not(windows))]
            if !self.tabs.is_empty() {
                if let Some(wd) =
                    process_cwd(self.active_tab().shell_pid).filter(|path| path.is_dir()).or_else(|| {
                        foreground_process_path(self.active_tab().master_fd, self.active_tab().shell_pid)
                            .ok()
                            .filter(|path| path.is_dir())
                    })
                {
                    options.terminal_options.working_directory = Some(wd);
                }
            }
        }

        // Per-tab config: if overrides provided, clone global and apply.
        let tab_config = if let Some(ref overrides) = config_overrides {
            if !overrides.is_empty() {
                let mut overridden = (*self.config).clone();
                let mut parsed = crate::cli::ParsedOptions::from_options(overrides);
                parsed.override_config(&mut overridden);
                Rc::new(overridden)
            } else {
                self.config.clone()
            }
        } else {
            self.config.clone()
        };

        // IPC-provided command (save for restore before moving into options).
        let saved_command = command.clone();
        if let Some(cmd) = command {
            options.terminal_options.command = cmd;
        }

        let tab = match TerminalTab::new(
            tab_id,
            self.display.window.id(),
            self.display.size_info,
            tab_config,
            &options,
            &self.event_proxy,
        ) {
            Ok(tab) => tab,
            Err(err) if options.terminal_options.working_directory.is_some() => {
                log::warn!(
                    "Retrying tab creation without inherited working directory after error: {err:?}"
                );
                let fallback_options = WindowOptions::default();
                TerminalTab::new(
                    tab_id,
                    self.display.window.id(),
                    self.display.size_info,
                    self.config.clone(),
                    &fallback_options,
                    &self.event_proxy,
                )?
            },
            Err(err) => return Err(err),
        };

        self.tabs.push(tab);
        // Store the command used to create this tab for save/restore.
        if let Some(ref cmd) = saved_command {
            self.tabs.last_mut().unwrap().command = Some(cmd.clone());
        }
        info!("[tabs] created tab {}, total={}", tab_id.0, self.tabs.len());
        if !no_switch {
            self.set_active_tab(self.tabs.len() - 1);
        }
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.mark_dirty();

        Ok(())
    }

    /// Close a tab by index via IPC.
    pub fn close_tab_at(&mut self, index: usize) {
        if index >= self.tabs.len() || self.tabs[index].pinned {
            return;
        }
        self.cancel_window_close_confirmation();
        self.tabs[index].terminal.lock().exit();
    }

    /// Toggle pin on a tab by index.
    pub fn toggle_pin_at(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs[index].pinned = !self.tabs[index].pinned;
        self.dirty = true;
    }

    fn handle_menu_selection(&mut self, selection: Option<MenuSelection>) {
        let sel = match selection {
            Some(s) => s,
            None => return,
        };
        match sel {
            MenuSelection::Command(program, args) => {
                let _ = std::process::Command::new(&program).args(&args).spawn();
            },
            MenuSelection::Action(ref action) => {
                match action.as_str() {
                    "create-tab" => {
                        let _ = self.event_proxy.send_event(Event::new(
                            EventType::Tab(TabAction::Create),
                            self.display.window.id(),
                        ));
                    },
                    "close-tab" => {
                        let _ = self.event_proxy.send_event(Event::new(
                            EventType::Tab(TabAction::Close),
                            self.display.window.id(),
                        ));
                    },
                    "rename-tab" => {
                        let _ = self.event_proxy.send_event(Event::new(
                            EventType::Tab(TabAction::SetTitle),
                            self.display.window.id(),
                        ));
                    },
                    "pin-tab" => {
                        self.toggle_pin_at(self.active_tab);
                    },
                    "move-tab-forward" => {
                        let _ = self.event_proxy.send_event(Event::new(
                            EventType::Tab(TabAction::MoveForward),
                            self.display.window.id(),
                        ));
                    },
                    "move-tab-backward" => {
                        let _ = self.event_proxy.send_event(Event::new(
                            EventType::Tab(TabAction::MoveBackward),
                            self.display.window.id(),
                        ));
                    },
                    "save-session" => {
                        let json = self.tabs_save_json();
                        let dir = std::env::var("HOME")
                            .map(std::path::PathBuf::from)
                            .unwrap_or_else(|_| std::path::PathBuf::from("."))
                            .join(".config").join("alacritty").join("sessions");
                        let _ = std::fs::create_dir_all(&dir);
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let path = dir.join(format!("session-{timestamp}.json"));
                        if let Err(err) = std::fs::write(&path, &json) {
                            log::warn!("Failed to save session: {err}");
                        } else {
                            log::info!("Session saved to {}", path.display());
                        }
                    },
                    "split-right" => self.split_pane(SplitDir::Horizontal),
                    "split-down" => self.split_pane(SplitDir::Vertical),
                    "close-pane" => self.close_pane(),
                    "toggle-zoom" => self.toggle_zoom(),
                    "focus-left" => self.focus_pane(FocusDir::Left),
                    "focus-right" => self.focus_pane(FocusDir::Right),
                    "focus-up" => self.focus_pane(FocusDir::Up),
                    "focus-down" => self.focus_pane(FocusDir::Down),
                    "resize-mode" => {
                        self.pane_resize_mode = !self.pane_resize_mode;
                        log::info!("[panes] resize mode: {}", self.pane_resize_mode);
                    },
                    _ => {},
                }
            },
            MenuSelection::List { ref list_type, ref value } => {
                match list_type.as_str() {
                    "sessions" => {
                        let dir = std::env::var("HOME")
                            .map(std::path::PathBuf::from)
                            .unwrap_or_else(|_| std::path::PathBuf::from("."))
                            .join(".config").join("alacritty").join("sessions");
                        let path = dir.join(format!("{value}.json"));
                        match std::fs::read(&path) {
                            Ok(data) => {
                                if let Err(err) = self.restore_tabs(&data) {
                                    log::warn!("Failed to restore session: {err}");
                                }
                            },
                            Err(err) => log::warn!("Failed to load session {}: {err}", path.display()),
                        }
                    },
                    "tabs" => {
                        if let Ok(index) = value.parse::<usize>() {
                            if index < self.tabs.len() {
                                self.set_active_tab(index);
                            }
                        }
                    },
                    _ => {},
                }
            },
        }
    }

    /// Select a tab by index via IPC.
    pub fn select_tab_at(&mut self, index: usize) {
        self.set_active_tab(index);
    }

    /// Return JSON string with all tabs' info.
    pub fn tabs_info_json(&self) -> String {
        use serde::Serialize;
        #[derive(Serialize)]
        struct TabInfo {
            index: usize,
            id: u64,
            title: String,
            active: bool,
            pinned: bool,
        }
        let tabs: Vec<TabInfo> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| TabInfo {
                index: i + 1,
                id: tab.id.0,
                title: tab.display_title().to_owned(),
                active: i == self.active_tab,
                pinned: tab.pinned,
            })
            .collect();
        serde_json::to_string(&tabs).unwrap_or_default()
    }

    /// Return JSON session file for save/restore.
    pub fn tabs_save_json(&self) -> String {
        use serde::Serialize;
        #[derive(Serialize)]
        struct SavedPane {
            pane_id: u64,
            command: Option<Vec<String>>,
        }
        #[derive(Serialize)]
        struct SavedTab {
            command: Option<Vec<String>>,
            cwd: Option<String>,
            pinned: bool,
            title: Option<String>,
            active_pane: u64,
            zoomed_pane: Option<u64>,
            pane_tree: crate::pane_tree::PaneNode,
            additional_panes: Vec<SavedPane>,
        }
        #[derive(Serialize)]
        struct Session {
            active_tab: usize,
            tabs: Vec<SavedTab>,
        }
        let tabs: Vec<SavedTab> = self.tabs.iter().map(|tab| {
            let cwd = std::fs::read_link(format!("/proc/{}/cwd", tab.shell_pid)).ok()
                .map(|p| p.to_string_lossy().to_string());
            let additional: Vec<SavedPane> = tab.panes.additional.iter().map(|(id, pane)| {
                let cmd = crate::util::foreground_process_cmdline(pane.master_fd, pane.shell_pid);
                SavedPane { pane_id: id.0, command: cmd }
            }).collect();
            SavedTab {
                command: crate::util::foreground_process_cmdline(tab.master_fd, tab.shell_pid)
                    .or_else(|| tab.command.clone()),
                cwd,
                pinned: tab.pinned,
                title: tab.custom_title.clone(),
                active_pane: tab.panes.active.0,
                zoomed_pane: tab.panes.zoomed.map(|id| id.0),
                pane_tree: tab.panes.tree.clone(),
                additional_panes: additional,
            }
        }).collect();
        let session = Session { active_tab: self.active_tab, tabs };
        serde_json::to_string(&session).unwrap_or_default()
    }

    /// Restore tabs from a saved session JSON.
    pub fn restore_tabs(&mut self, json: &[u8]) -> Result<(), Box<dyn Error>> {
        use serde::Deserialize;
        #[derive(Deserialize)]
        struct SavedPane {
            pane_id: u64,
            command: Option<Vec<String>>,
        }
        #[derive(Deserialize)]
        struct SavedTab {
            command: Option<Vec<String>>,
            cwd: Option<String>,
            pinned: bool,
            #[serde(default)]
            title: Option<String>,
            #[serde(default)]
            active_pane: u64,
            #[serde(default)]
            zoomed_pane: Option<u64>,
            #[serde(default)]
            pane_tree: crate::pane_tree::PaneNode,
            #[serde(default)]
            additional_panes: Vec<SavedPane>,
        }
        #[derive(Deserialize)]
        struct Session {
            active_tab: usize,
            tabs: Vec<SavedTab>,
        }
        let session: Session = serde_json::from_slice(json)?;
        if session.tabs.is_empty() { return Ok(()); }

        for (i, saved) in session.tabs.iter().enumerate() {
            let wd = saved.cwd.as_ref().and_then(|c| std::path::PathBuf::from(c).canonicalize().ok());
            if i == 0 {
                self.tabs[0].terminal.lock().exit();
                self.tabs.remove(0);
                let _ = self.create_tab_inner(
                    saved.command.clone(), wd.map(|p| p), true, None,
                );
            } else {
                let _ = self.create_tab_inner(
                    saved.command.clone(), wd.map(|p| p), true, None,
                );
            }
            if saved.pinned && i < self.tabs.len() {
                self.tabs[i].pinned = true;
            }
            if let Some(ref title) = saved.title {
                if !title.is_empty() && i < self.tabs.len() {
                    self.tabs[i].custom_title = Some(title.clone());
                }
            }

            // Restore pane tree.
            if saved.pane_tree.leaf_ids().len() > 1 {
                let tab = &mut self.tabs[i];
                tab.panes.tree = saved.pane_tree.clone();
                tab.panes.next_id = saved.additional_panes.iter()
                    .map(|p| p.pane_id).max().unwrap_or(0) + 1;
                tab.panes.active = crate::pane_tree::PaneId(saved.active_pane);
                tab.panes.zoomed = saved.zoomed_pane.map(crate::pane_tree::PaneId);

                // Create additional pane terminals.
                let config = Rc::clone(&self.config);
                let window_id = self.display.window.id();
                let proxy = self.event_proxy.clone();
                let tab_id = tab.id;
                let size_info = self.display.size_info;

                for saved_pane in &saved.additional_panes {
                    if saved_pane.pane_id == 0 { continue; }
                    let pane_id = crate::pane_tree::PaneId(saved_pane.pane_id);
                    if let Ok(pane_state) = Self::create_pane(
                        &config, window_id, size_info, &proxy, tab_id, pane_id,
                        saved_pane.command.clone(),
                    ) {
                        tab.panes.additional.insert(pane_id, pane_state);
                    }
                }
            }
        }

        let active = session.active_tab.min(self.tabs.len().saturating_sub(1));
        self.set_active_tab(active);

        // Full redraw after pane restoration to avoid damage tracker OOB.
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.mark_dirty();
        Ok(())
    }

    fn close_active_tab(&mut self) {
        self.cancel_window_close_confirmation();
        if self.active_tab().pinned && self.tabs.len() > 1 {
            return;
        }
        let tab = self.active_tab_mut();
        tab.terminal.lock().exit();
    }

    fn move_active_tab(&mut self, delta: isize) {
        if self.tabs.len() < 2 {
            return;
        }

        let old_index = self.active_tab;
        let new_index = if delta < 0 {
            old_index.saturating_sub(delta.unsigned_abs())
        } else {
            (old_index + delta as usize).min(self.tabs.len() - 1)
        };

        if new_index == old_index {
            return;
        }

        self.tabs.swap(old_index, new_index);
        self.active_tab = new_index;
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.dirty = true;
    }

    pub fn handle_tab_wakeup(&mut self, tab_id: Option<TabId>) {
        if let Some(index) = self.tab_index(tab_id) {
            let title_changed = self.tabs[index].refresh_detected_title(&self.config);
            if index == self.active_tab && title_changed {
                self.refresh_window_title();
            }
        }

        if self.tab_index(tab_id) == Some(self.active_tab) {
            self.dirty = true;
        }
    }

    pub fn handle_tab_exit(&mut self, tab_id: Option<TabId>) -> bool {
        if self.display.window.hold {
            return false;
        }

        self.cancel_window_close_confirmation();

        let Some(index) = self.tab_index(tab_id) else {
            return false;
        };
        let closing_tab_id = self.tabs[index].id;
        let next_active_id = match self.config.tabs.tab_switch_strategy {
            TabSwitchStrategy::Previous => self.last_active_tab_id.filter(|tab_id| {
                self.tabs.iter().any(|tab| tab.id == *tab_id && tab.id != closing_tab_id)
            }),
            TabSwitchStrategy::Left => {
                index.checked_sub(1).and_then(|index| self.tabs.get(index)).map(|tab| tab.id)
            },
            TabSwitchStrategy::Right => self.tabs.get(index + 1).map(|tab| tab.id),
            TabSwitchStrategy::Last => self.tabs.last().map(|tab| tab.id),
        };

        self.tabs.remove(index);
        info!("[tabs] closed tab, remaining={}", self.tabs.len());
        if self.tab_title_editor.as_ref().is_some_and(|editor| editor.tab_id == closing_tab_id) {
            self.tab_title_editor = None;
        }

        if self.tabs.is_empty() {
            return true;
        }

        self.active_tab = next_active_id
            .and_then(|tab_id| self.tabs.iter().position(|tab| tab.id == tab_id))
            .unwrap_or_else(|| {
                self.active_tab
                    .min(self.tabs.len() - 1)
                    .saturating_sub(usize::from(index < self.active_tab))
            });

        self.tabs[self.active_tab].bell_pending = false;

        let config = self.config.clone();
        self.active_tab_mut().refresh_detected_title(&config);

        self.sync_focus();
        self.refresh_window_title();
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.mark_dirty();

        false
    }

    pub fn clear_config_messages(&mut self, target: &str) {
        for tab in &mut self.tabs {
            tab.message_buffer.remove_target(target);
        }
    }

    /// Create initial window context that does bootstrapping the graphics API we're going to use.
    pub fn initial(
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<Event>,
        config: Rc<UiConfig>,
        mut options: WindowOptions,
    ) -> Result<Self, Box<dyn Error>> {
        let raw_display_handle = event_loop.display_handle().unwrap().as_raw();

        let mut identity = config.window.identity.clone();
        options.window_identity.override_identity_config(&mut identity);

        // Windows has different order of GL platform initialization compared to any other platform;
        // it requires the window first.
        #[cfg(windows)]
        let window = Window::new(event_loop, &config, &identity, &mut options)?;
        #[cfg(windows)]
        let raw_window_handle = Some(window.raw_window_handle());

        #[cfg(not(windows))]
        let raw_window_handle = None;

        let gl_display = renderer::platform::create_gl_display(
            raw_display_handle,
            raw_window_handle,
            config.debug.prefer_egl,
        )?;
        let gl_config = renderer::platform::pick_gl_config(&gl_display, raw_window_handle)?;

        #[cfg(not(windows))]
        let window = Window::new(
            event_loop,
            &config,
            &identity,
            &mut options,
            #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
            gl_config.x11_visual(),
        )?;

        // Create context.
        let gl_context =
            renderer::platform::create_gl_context(&gl_display, &gl_config, raw_window_handle)?;

        let display = Display::new(window, gl_context, &config, false)?;

        Self::new(display, config, options, proxy)
    }

    /// Create additional context with the graphics platform other windows are using.
    pub fn additional(
        gl_config: &GlutinConfig,
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<Event>,
        config: Rc<UiConfig>,
        mut options: WindowOptions,
        config_overrides: ParsedOptions,
    ) -> Result<Self, Box<dyn Error>> {
        let gl_display = gl_config.display();

        let mut identity = config.window.identity.clone();
        options.window_identity.override_identity_config(&mut identity);

        // Check if new window will be opened as a tab.
        // This must be done before `Window::new()`, which unsets `window_tabbing_id`.
        #[cfg(target_os = "macos")]
        let tabbed = options.window_tabbing_id.is_some();
        #[cfg(not(target_os = "macos"))]
        let tabbed = false;

        let window = Window::new(
            event_loop,
            &config,
            &identity,
            &mut options,
            #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
            gl_config.x11_visual(),
        )?;

        // Create context.
        let raw_window_handle = window.raw_window_handle();
        let gl_context =
            renderer::platform::create_gl_context(&gl_display, gl_config, Some(raw_window_handle))?;

        let display = Display::new(window, gl_context, &config, tabbed)?;

        let mut window_context = Self::new(display, config, options, proxy)?;

        // Set the config overrides at startup.
        //
        // These are already applied to `config`, so no update is necessary.
        window_context.window_config = config_overrides;

        Ok(window_context)
    }

    /// Create a new terminal window context.
    fn new(
        display: Display,
        config: Rc<UiConfig>,
        options: WindowOptions,
        proxy: EventLoopProxy<Event>,
    ) -> Result<Self, Box<dyn Error>> {
        let preserve_title = options.window_identity.title.is_some();

        info!(
            "PTY dimensions: {:?} x {:?}",
            display.size_info.screen_lines(),
            display.size_info.columns()
        );

        let first_tab = TerminalTab::new(
            TabId(0),
            display.window.id(),
            display.size_info,
            config.clone(),
            &options,
            &proxy,
        )?;

        // Create context for the Alacritty window.
        let mut wc = WindowContext {
            preserve_title,
            display,
            config: config.clone(),
            window_config: Default::default(),
            event_queue: Default::default(),
            modifiers: Default::default(),
            occluded: Default::default(),
            mouse: Default::default(),
            touch: Default::default(),
            dirty: Default::default(),
            tabs: vec![first_tab],
            active_tab: 0,
            next_tab_id: 1,
            last_active_tab_id: None,
            tab_title_editor: None,
            run_editor: None,
            menu: MenuController::new(),
            divider_drag: None,
            pane_resize_mode: false,
            window_close_confirmation_pending: false,
            focused: false,
            event_proxy: proxy,
        };

        // Process tab presets from config.
        let presets: Vec<_> = wc.config.tabs.presets.clone();
        for preset in presets {
            let cmd = preset.command.as_ref().map(|p| {
                let mut args = vec![p.program().to_owned()];
                args.extend(p.args().iter().cloned());
                args
            });
            let _ = wc.create_tab_inner(cmd, None, preset.no_switch, None);
        }

        Ok(wc)
    }

    /// Update the terminal window to the latest config.
    pub fn update_config(&mut self, new_config: Rc<UiConfig>) {
        let old_config = mem::replace(&mut self.config, new_config);

        // Apply ipc config if there are overrides.
        self.config = self.window_config.override_config_rc(self.config.clone());

        self.display.update_config(&self.config);
        for tab in &mut self.tabs {
            tab.terminal.lock().set_options(self.config.term_options());
        }

        // Reload cursor if its thickness has changed.
        if (old_config.cursor.thickness() - self.config.cursor.thickness()).abs() > f32::EPSILON {
            self.display.pending_update.set_cursor_dirty();
        }

        if old_config.font != self.config.font {
            let scale_factor = self.display.window.scale_factor as f32;
            // Do not update font size if it has been changed at runtime.
            if self.display.font_size == old_config.font.size().scale(scale_factor) {
                self.display.font_size = self.config.font.size().scale(scale_factor);
            }

            let font = self.config.font.clone().with_size(self.display.font_size);
            self.display.pending_update.set_font(font);
        }

        // Always reload the theme to account for auto-theme switching.
        self.display.window.set_theme(self.config.window.theme());

        // Update display if either padding options or resize increments were changed.
        let window_config = &old_config.window;
        if window_config.padding(1.) != self.config.window.padding(1.)
            || window_config.dynamic_padding != self.config.window.dynamic_padding
            || window_config.resize_increments != self.config.window.resize_increments
        {
            self.display.pending_update.dirty = true;
        }

        // Update title on config reload according to the following table.
        //
        // │cli │ dynamic_title │ current_title == old_config ││ set_title │
        // │ Y  │       _       │              _              ││     N     │
        // │ N  │       Y       │              Y              ││     Y     │
        // │ N  │       Y       │              N              ││     N     │
        // │ N  │       N       │              _              ││     Y     │
        if !self.preserve_title
            && (!self.config.window.dynamic_title
                || self.display.window.title() == old_config.window.identity.title)
        {
            self.refresh_window_title();
        }

        let opaque = self.config.window_opacity() >= 1.;

        // Disable shadows for transparent windows on macOS.
        #[cfg(target_os = "macos")]
        self.display.window.set_has_shadow(opaque);

        #[cfg(target_os = "macos")]
        self.display.window.set_option_as_alt(self.config.window.option_as_alt());

        // Change opacity and blur state.
        self.display.window.set_transparent(!opaque);
        self.display.window.set_blur(self.config.window.blur);

        // Update hint keys.
        self.display.hint_state.update_alphabet(self.config.hints.alphabet());

        // Update cursor blinking.
        let event =
            Event::new(TerminalEvent::CursorBlinkingChange.into(), self.display.window.id());
        self.event_queue.push(event.into());

        self.dirty = true;
    }

    /// Get reference to the window's configuration.
    #[cfg(unix)]
    pub fn config(&self) -> &UiConfig {
        &self.config
    }

    /// Clear the window config overrides.
    #[cfg(unix)]
    pub fn reset_window_config(&mut self, config: Rc<UiConfig>) {
        // Clear previous window errors.
        for tab in &mut self.tabs {
            tab.message_buffer.remove_target(LOG_TARGET_IPC_CONFIG);
        }

        self.window_config.clear();

        // Reload current config to pull new IPC config.
        self.update_config(config);
    }

    /// Add new window config overrides.
    #[cfg(unix)]
    pub fn add_window_config(&mut self, config: Rc<UiConfig>, options: &ParsedOptions) {
        // Clear previous window errors.
        for tab in &mut self.tabs {
            tab.message_buffer.remove_target(LOG_TARGET_IPC_CONFIG);
        }

        self.window_config.extend_from_slice(options);

        // Reload current config to pull new IPC config.
        self.update_config(config);
    }

    /// Draw the window.
    pub fn draw(&mut self, scheduler: &mut Scheduler) {
        self.display.window.requested_redraw = false;

        if self.occluded {
            return;
        }

        self.dirty = false;

        // Force the display to process any pending display update.
        self.display.process_renderer_update();

        // Request immediate re-draw if visual bell animation is not finished yet.
        if !self.display.visual_bell.completed() {
            // We can get an OS redraw which bypasses alacritty's frame throttling, thus
            // marking the window as dirty when we don't have frame yet.
            if self.display.window.has_frame {
                self.display.window.request_redraw();
            } else {
                self.dirty = true;
            }
        }

        // Redraw the window.
        let tab_titles: Vec<_> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let active = index == self.active_tab;
                (self.render_tab_title(index, tab, active), active)
            })
            .collect();
        let active_tab_idx = self.active_tab;

        // Populate tab list into menu state if in tab list mode (before active_tab borrow).
        if let Some(crate::event::ListMode::Tabs(_)) = self.menu.state.list_mode {
            let entries: Vec<(usize, String)> = self.tabs.iter().enumerate()
                .map(|(i, t)| (i, t.display_title().to_owned()))
                .collect();
            self.menu.state.set_tab_list(entries);
        }

        let tab_config = self.tabs[active_tab_idx].config.clone();
        let (display, tabs) = (&mut self.display, &mut self.tabs);
        let active_tab = &mut tabs[active_tab_idx];

        // Zoomed pane: render single pane full-viewport.
        if let Some(zoomed_id) = active_tab.panes.zoomed {
            display.pane_dividers.clear();
            let terminal_lock = if zoomed_id == PaneId(0) {
                Arc::clone(&active_tab.terminal)
            } else if let Some(pane) = active_tab.panes.additional.get(&zoomed_id) {
                Arc::clone(&pane.terminal)
            } else {
                Arc::clone(&active_tab.terminal)
            };
            let terminal = terminal_lock.lock();
            display.draw(
                terminal, scheduler, &active_tab.message_buffer, &tab_config,
                &mut active_tab.search_state, &tab_titles,
                self.tab_title_editor.as_ref().map(|e| e.value.as_str()),
                self.run_editor.as_deref(), &self.menu.state, None, true, false,
            );
            return;
        }

        // Compute pane rects and divider rects for multi-pane layout.
        display.pane_dividers.clear();
        let leaves = active_tab.panes.tree.leaf_ids();
        if leaves.len() > 1 {
            use crate::pane_tree::Rect as PRect;
            let viewport = PRect::new(0.0, 0.0, display.size_info.width() as f32, display.size_info.height() as f32);
            let (pane_rects, divider_rects) = active_tab.panes.tree.leaf_rects(viewport);
            for divider in &divider_rects {
                let rr = crate::renderer::rects::RenderRect::new(
                    divider.x, divider.y, divider.width.max(2.0), divider.height.max(2.0),
                    crate::display::color::Rgb(alacritty_terminal::vte::ansi::Rgb { r: 196, g: 154, b: 247 }), 0.6,
                );
                display.pane_dividers.push(rr);
            }

            let active_pane = active_tab.panes.active;
            // Draw all panes with scissor clips.
            for (pane_id, rect) in &pane_rects {
                let terminal_lock = if *pane_id == PaneId(0) {
                    Arc::clone(&active_tab.terminal)
                } else if let Some(pane) = active_tab.panes.additional.get(pane_id) {
                    Arc::clone(&pane.terminal)
                } else {
                    Arc::clone(&active_tab.terminal)
                };
                let terminal_guard = terminal_lock.lock();
                let clip = (rect.x, rect.y, rect.width, rect.height);
                display.draw(
                    terminal_guard, scheduler, &active_tab.message_buffer, &tab_config,
                    &mut active_tab.search_state,
                    if *pane_id == active_pane { &tab_titles } else { &[] },
                    if *pane_id == active_pane { self.tab_title_editor.as_ref().map(|e| e.value.as_str()) } else { None },
                    self.run_editor.as_deref(), &self.menu.state,
                    Some(clip), *pane_id == active_pane, true,
                );
            }

            unsafe { crate::gl::Disable(crate::gl::SCISSOR_TEST); }

            // Draw tab bar and menu bar on top of panes.
            let tab_title_editor_offset =
                usize::from(self.tab_title_editor.is_some() || self.run_editor.is_some());
            let search_lines =
                usize::from(active_tab.search_state.regex().is_some()) + tab_title_editor_offset;
            let message_lines = active_tab.message_buffer.message()
                .map_or(0, |m| m.text(&display.size_info).len());

            if tab_config.tabs.display_tab_bar(tab_titles.len()) {
                let tab_line = match tab_config.tabs.tab_bar_edge {
                    TabBarEdge::Top => 0,
                    TabBarEdge::Bottom => display.size_info.screen_lines() + search_lines + message_lines,
                };
                display.draw_tab_bar(&tab_config, &tab_titles.iter().map(|(t, a)| (t.clone(), *a)).collect::<Vec<_>>(), tab_line);
            }

            if !tab_config.menu.items.is_empty() {
                let menu_edge = match tab_config.menu.menu_bar_edge {
                    crate::config::menu::MenuBarEdge::Top => TabBarEdge::Top,
                    crate::config::menu::MenuBarEdge::Bottom => TabBarEdge::Bottom,
                };
                let menu_line = match menu_edge {
                    TabBarEdge::Top => {
                        usize::from(tab_config.tabs.display_tab_bar(tab_titles.len())
                            && tab_config.tabs.tab_bar_edge == TabBarEdge::Top)
                    },
                    TabBarEdge::Bottom => {
                        let base = display.size_info.screen_lines() + search_lines + message_lines;
                        if tab_config.tabs.display_tab_bar(tab_titles.len())
                            && tab_config.tabs.tab_bar_edge == TabBarEdge::Bottom
                        {
                            base + 1
                        } else {
                            base
                        }
                    },
                };
                let pc = active_tab.panes.tree.leaf_ids().len();
                let bar_labels = self.menu.state.bar_labels(&tab_config.menu, pc, self.pane_resize_mode);
                let labels: Vec<(String, bool)> = bar_labels.iter().map(|(l, f)| (l.clone(), *f)).collect();
                display.draw_bar(&tab_config, &labels, menu_line, menu_edge);
            }

            display.draw_pane_dividers();

            // Draw editor bars on top.
            display.draw_editor_footer(
                &tab_config,
                self.tab_title_editor.as_ref().map(|e| e.value.as_str()),
                self.run_editor.as_deref(),
                active_tab.search_state.regex().is_some(),
            );

            display.present(scheduler);
        } else {
            let terminal_lock = if active_tab.panes.active == PaneId(0) {
                Arc::clone(&active_tab.terminal)
            } else if let Some(pane) = active_tab.panes.additional.get(&active_tab.panes.active) {
                Arc::clone(&pane.terminal)
            } else {
                Arc::clone(&active_tab.terminal)
            };
            let terminal = terminal_lock.lock();
            display.draw(
                terminal, scheduler, &active_tab.message_buffer, &tab_config,
                &mut active_tab.search_state, &tab_titles,
                self.tab_title_editor.as_ref().map(|e| e.value.as_str()),
                self.run_editor.as_deref(), &self.menu.state, None, true, false,
            );
        }
    }

    /// Process events for this terminal window.
    pub fn handle_event(
        &mut self,
        #[cfg(target_os = "macos")] event_loop: &ActiveEventLoop,
        _event_proxy: &EventLoopProxy<Event>,
        clipboard: &mut Clipboard,
        scheduler: &mut Scheduler,
        event: WinitEvent<Event>,
    ) {
        match event {
            WinitEvent::AboutToWait
            | WinitEvent::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                // Skip further event handling with no staged updates.
                if self.event_queue.is_empty() {
                    return;
                }

                // Continue to process all pending events.
            },
            event => {
                self.event_queue.push(event);
                return;
            },
        }

        let old_is_searching = self.active_tab().search_state.history_index.is_some();
        let queued_events: Vec<_> = self.event_queue.drain(..).collect();

        for event in queued_events {
            match &event {
                WinitEvent::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    self.request_window_close();
                    continue;
                },
                WinitEvent::WindowEvent { event: WindowEvent::Focused(is_focused), .. } => {
                    self.focused = *is_focused;
                },
                #[cfg(unix)]
                WinitEvent::UserEvent(Event { payload: EventType::IpcAction(action), .. }) => {
                    let _ = self.handle_action(action.clone());
                },
                WinitEvent::UserEvent(Event { payload: EventType::Tab(action), .. }) => {
                    if self.dispatch_tab_action(action) { continue; }
                },
                _ => (),
            }

            let tab_index = match &event {
                WinitEvent::UserEvent(Event { tab_id, .. }) => {
                    self.tab_index(*tab_id).unwrap_or(self.active_tab)
                },
                _ => self.active_tab,
            };
            let is_active_tab = tab_index == self.active_tab;
            let tab = &mut self.tabs[tab_index];
            let active_pid = tab.panes.active;

            // Lock the active pane's terminal, get notifier/fd/pid (no unsafe needed).
            let mut terminal;
            let notifier: &Notifier;
            #[cfg(not(windows))]
            let fd: RawFd;
            #[cfg(not(windows))]
            let pid: u32;
            if active_pid == PaneId(0) {
                terminal = tab.terminal.lock();
                notifier = &tab.notifier;
                #[cfg(not(windows))] { fd = tab.master_fd; pid = tab.shell_pid; }
            } else if let Some(pane) = tab.panes.additional.get(&active_pid) {
                terminal = pane.terminal.lock();
                notifier = &pane.notifier;
                #[cfg(not(windows))] { fd = pane.master_fd; pid = pane.shell_pid; }
            } else {
                terminal = tab.terminal.lock();
                notifier = &tab.notifier;
                #[cfg(not(windows))] { fd = tab.master_fd; pid = tab.shell_pid; }
            }

            let context = ActionContext {
                notifier,
                terminal: &mut terminal,
                tab_terminal_title: &mut tab.terminal_title,
                tab_detected_title: &tab.detected_title,
                tab_custom_title: &mut tab.custom_title,
                tab_title_editor_active: self.tab_title_editor.is_some(),
                run_editor_active: self.run_editor.is_some(),
                is_active_tab,
                bell_pending: &mut tab.bell_pending,
                clipboard,
                mouse: &mut self.mouse,
                touch: &mut self.touch,
                modifiers: &mut self.modifiers,
                display: &mut self.display,
                message_buffer: &mut tab.message_buffer,
                config: &*tab.config,
                cursor_blink_timed_out: &mut tab.cursor_blink_timed_out,
                prev_bell_cmd: &mut tab.prev_bell_cmd,
                event_proxy: &self.event_proxy,
                scheduler,
                search_state: &mut tab.search_state,
                inline_search_state: &mut tab.inline_search_state,
                dirty: &mut self.dirty,
                occluded: &mut self.occluded,
                preserve_title: self.preserve_title,
                menu_active: self.menu.state.active,
                menu_toggle_pending: &mut self.menu.toggle_pending,
                menu_op_pending: &mut self.menu.op_pending,
                pane_resize_mode: &mut self.pane_resize_mode,
                #[cfg(not(windows))]
                master_fd: fd,
                #[cfg(not(windows))]
                shell_pid: pid,
                #[cfg(target_os = "macos")]
                event_loop,
            };
            let mut processor = input::Processor::new(context);
            processor.handle_event(event);
        }

        self.process_menu_ops();

        self.sync_focus();

        // Process DisplayUpdate events.
        if self.display.pending_update.dirty {
            let tab_bar_lines = self.tab_bar_lines();
            let tab_bar_at_top = usize::from(self.tab_bar_at_top());
            let tab_title_editor_lines =
                usize::from(self.tab_title_editor.is_some() || self.run_editor.is_some());
            let active_index = self.active_tab;
            let menu_bar_lines =
                usize::from(!self.tabs[active_index].config.menu.items.is_empty());
            let tab_config = self.tabs[active_index].config.clone();
            let (display, tabs) = (&mut self.display, &mut self.tabs);
            let active_tab = &mut tabs[active_index];
            let is_multi_pane = active_tab.panes.tree.leaf_ids().len() > 1;

            // Always resize pane 0 (submit_display_update handles reserved lines).
            {
                let mut terminal = active_tab.terminal.lock();
                Self::submit_display_update(
                    &mut terminal, display, &mut active_tab.notifier,
                    &active_tab.message_buffer, &mut active_tab.search_state,
                    old_is_searching, &tab_config, tab_bar_lines,
                    tab_bar_at_top, menu_bar_lines, tab_title_editor_lines,
                );
            }

            // Multi-pane: resize ALL panes to their split rects.
            if is_multi_pane {
                let size_info = display.size_info;
                let cell_w = size_info.cell_width();
                let cell_h = size_info.cell_height();
                let padding_x = size_info.padding_x();
                let padding_y = size_info.padding_y();
                let viewport = crate::pane_tree::Rect::new(0.0, 0.0, size_info.width() as f32, size_info.height() as f32);
                let (pane_rects, _) = active_tab.panes.tree.leaf_rects(viewport);
                for (pid, rect) in &pane_rects {
                    if active_tab.panes.zoomed == Some(*pid) { continue; }
                    let ps = crate::display::SizeInfo::new(rect.width.max(1.), rect.height.max(1.), cell_w, cell_h, padding_x, padding_y, false);
                    if *pid == PaneId(0) {
                        let mut t = active_tab.terminal.lock();
                        t.resize(ps);
                        let _ = active_tab.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(ps.into()));
                    } else if let Some(pane) = active_tab.panes.additional.get_mut(pid) {
                        let mut t = pane.terminal.lock();
                        t.resize(ps);
                        let _ = pane.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(ps.into()));
                    }
                }
            }
            self.dirty = true;
        }

        if self.dirty || self.mouse.hint_highlight_dirty {
            let active_index = self.active_tab;
            let tab_config = self.tabs[active_index].config.clone();
            let (display, tabs) = (&mut self.display, &mut self.tabs);
            let active_tab = &mut tabs[active_index];
            let terminal = active_tab.terminal.lock();
            self.dirty |= display.update_highlighted_hints(
                &terminal,
                &tab_config,
                &self.mouse,
                self.modifiers.state(),
            );
            self.mouse.hint_highlight_dirty = false;
        }

        // Don't call `request_redraw` when event is `RedrawRequested` since the `dirty` flag
        // represents the current frame, but redraw is for the next frame.
        if self.dirty
            && self.display.window.has_frame
            && !self.occluded
            && !matches!(event, WinitEvent::WindowEvent { event: WindowEvent::RedrawRequested, .. })
        {
            self.display.window.request_redraw();
        }
    }

    /// ID of this terminal context.
    pub fn id(&self) -> WindowId {
        self.display.window.id()
    }

    /// Write the ref test results to the disk.
    pub fn write_ref_test_results(&self) {
        // Dump grid state.
        let mut grid = self.active_tab().terminal.lock().grid().clone();
        grid.initialize_all();
        grid.truncate();

        let serialized_grid = json::to_string(&grid).expect("serialize grid");

        let size_info = &self.display.size_info;
        let size = TermSize::new(size_info.columns(), size_info.screen_lines());
        let serialized_size = json::to_string(&size).expect("serialize size");

        let serialized_config = format!("{{\"history_size\":{}}}", grid.history_size());

        File::create("./grid.json")
            .and_then(|mut f| f.write_all(serialized_grid.as_bytes()))
            .expect("write grid.json");

        File::create("./size.json")
            .and_then(|mut f| f.write_all(serialized_size.as_bytes()))
            .expect("write size.json");

        File::create("./config.json")
            .and_then(|mut f| f.write_all(serialized_config.as_bytes()))
            .expect("write config.json");
    }

    /// Submit the pending changes to the `Display`.
    fn submit_display_update(
        terminal: &mut Term<EventProxy>,
        display: &mut Display,
        notifier: &mut Notifier,
        message_buffer: &MessageBuffer,
        search_state: &mut SearchState,
        old_is_searching: bool,
        config: &UiConfig,
        tab_bar_lines: usize,
        top_tab_bar_lines: usize,
        menu_bar_lines: usize,
        tab_title_editor_lines: usize,
    ) {
        // Compute cursor positions before resize.
        let num_lines = terminal.screen_lines();
        let cursor_at_bottom = terminal.grid().cursor.point.line + 1 == num_lines;
        let origin_at_bottom = if terminal.mode().contains(TermMode::VI) {
            terminal.vi_mode_cursor.point.line == num_lines - 1
        } else {
            search_state.direction == Direction::Left
        };

        display.handle_update(
            terminal,
            notifier,
            message_buffer,
            search_state,
            config,
            tab_bar_lines,
            top_tab_bar_lines,
            menu_bar_lines,
            tab_title_editor_lines,
        );

        let new_is_searching = search_state.history_index.is_some();
        if !old_is_searching && new_is_searching {
            // Scroll on search start to make sure origin is visible with minimal viewport motion.
            let display_offset = terminal.grid().display_offset();
            if display_offset == 0 && cursor_at_bottom && !origin_at_bottom {
                terminal.scroll_display(Scroll::Delta(1));
            } else if display_offset != 0 && origin_at_bottom {
                terminal.scroll_display(Scroll::Delta(-1));
            }
        }
    }

    // ----- Pane helpers -----

    fn active_terminal(&self) -> Arc<FairMutex<Term<EventProxy>>> {
        let tab = self.active_tab();
        if tab.panes.active == PaneId(0) {
            Arc::clone(&tab.terminal)
        } else if let Some(pane) = tab.panes.additional.get(&tab.panes.active) {
            Arc::clone(&pane.terminal)
        } else {
            Arc::clone(&tab.terminal)
        }
    }

    fn active_terminal_for(&self, tab: &TerminalTab, pane_id: PaneId) -> Arc<FairMutex<Term<EventProxy>>> {
        if pane_id == PaneId(0) {
            Arc::clone(&tab.terminal)
        } else if let Some(pane) = tab.panes.additional.get(&pane_id) {
            Arc::clone(&pane.terminal)
        } else {
            Arc::clone(&tab.terminal)
        }
    }

    fn pane_at_position(&self, tab: &TerminalTab, size_info: &crate::display::SizeInfo, x: f32, y: f32) -> Option<PaneId> {
        use crate::pane_tree::Rect;
        if tab.panes.tree.leaf_ids().len() <= 1 { return Some(tab.panes.active); }
        let viewport = Rect::new(0.0, 0.0, size_info.width() as f32, size_info.height() as f32);
        let (pane_rects, _) = tab.panes.tree.leaf_rects(viewport);
        for (pane_id, rect) in &pane_rects {
            if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
                return Some(*pane_id);
            }
        }
        None
    }

    fn divider_at_position(&self, tab: &TerminalTab, size_info: &crate::display::SizeInfo, x: f32, y: f32) -> Option<crate::pane_tree::SplitId> {
        use crate::pane_tree::Rect;
        let viewport = Rect::new(0.0, 0.0, size_info.width() as f32, size_info.height() as f32);
        let (_, divider_rects) = tab.panes.tree.leaf_rects(viewport);
        for (i, rect) in divider_rects.iter().enumerate() {
            let margin = 1.0;
            if x >= rect.x - margin && x < rect.x + rect.width + margin
                && y >= rect.y - margin && y < rect.y + rect.height + margin
            {
                return tab.panes.tree.find_split_by_divider_index(i);
            }
        }
        None
    }

    fn split_pane(&mut self, direction: SplitDir) {
        log::info!("[panes] split {:?}", direction);
        self.display.cursor_hidden = false;
        self.active_tab_mut().cursor_blink_timed_out = false;
        use crate::pane_tree::Rect;
        let new_pane_id = PaneId(self.active_tab().panes.next_id);
        let full_size_info = self.display.size_info;
        let cell_w = full_size_info.cell_width();
        let cell_h = full_size_info.cell_height();
        let padding_x = full_size_info.padding_x();
        let padding_y = full_size_info.padding_y();
        let viewport_w = full_size_info.width() as f32;
        let viewport_h = full_size_info.height() as f32;
        let config = Rc::clone(&self.config);
        let window_id = self.display.window.id();
        let event_loop_proxy = self.event_proxy.clone();
        let tab_id = self.active_tab().id;
        let tab = self.active_tab_mut();
        tab.panes.next_id += 1;
        let split_id = crate::pane_tree::SplitId(tab.panes.next_id);
        tab.panes.next_id += 1;
        tab.panes.tree.split(tab.panes.active, new_pane_id, split_id, direction);
        let full_viewport = Rect::new(0.0, 0.0, viewport_w, viewport_h);
        let (pane_rects, _) = tab.panes.tree.leaf_rects(full_viewport);
        for (pane_id, rect) in &pane_rects {
            if *pane_id == new_pane_id { continue; }
            let ps = crate::display::SizeInfo::new(rect.width.max(1.), rect.height.max(1.), cell_w, cell_h, padding_x, padding_y, false);
            if *pane_id == PaneId(0) {
                let mut t = tab.terminal.lock(); t.resize(ps);
                let _ = tab.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(ps.into()));
            } else if let Some(pane) = tab.panes.additional.get_mut(pane_id) {
                let mut t = pane.terminal.lock(); t.resize(ps);
                let _ = pane.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(ps.into()));
            }
        }
        let new_pane_rect = pane_rects.iter().find(|(id, _)| *id == new_pane_id).map(|(_, r)| r);
        let pane_size = if let Some(rect) = new_pane_rect {
            crate::display::SizeInfo::new(rect.width.max(1.), rect.height.max(1.), cell_w, cell_h, padding_x, padding_y, false)
        } else { full_size_info };
        if let Ok(pane_state) = Self::create_pane(&config, window_id, pane_size, &event_loop_proxy, tab_id, new_pane_id, None) {
            tab.panes.additional.insert(new_pane_id, pane_state);
        }
        tab.panes.active = new_pane_id;
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.mark_dirty();
    }

    fn create_pane(config: &UiConfig, window_id: WindowId, size_info: crate::display::SizeInfo, proxy: &EventLoopProxy<Event>, tab_id: TabId, pane_id: PaneId, command: Option<Vec<String>>) -> Result<PaneState, Box<dyn Error>> {
        let mut pty_config = config.pty_config();
        if let Some(ref cmd) = command {
            if !cmd.is_empty() {
                let program = cmd[0].clone();
                let args: Vec<String> = cmd[1..].to_vec();
                let prog = crate::config::ui_config::Program::WithArgs { program, args };
                pty_config.shell = Some(prog.into());
            }
        }
        let event_proxy = EventProxy::new(proxy.clone(), window_id, tab_id, Some(pane_id));
        let terminal = Term::new(config.term_options(), &size_info, event_proxy.clone());
        let terminal = Arc::new(FairMutex::new(terminal));
        let pty = tty::new(&pty_config, size_info.into(), window_id.into())?;
        #[cfg(not(windows))] let master_fd = pty.file().as_raw_fd();
        #[cfg(not(windows))] let shell_pid = pty.child().id();
        let event_loop = PtyEventLoop::new(Arc::clone(&terminal), event_proxy.clone(), pty, pty_config.drain_on_exit, config.debug.ref_test)?;
        let loop_tx = event_loop.channel();
        let _io_thread = event_loop.spawn();
        if config.cursor.style().blinking { event_proxy.send_event(TerminalEvent::CursorBlinkingChange.into()); }
        Ok(PaneState::new(pane_id, terminal, Notifier(loop_tx), command, master_fd, shell_pid))
    }

    fn close_pane(&mut self) {
        log::info!("[panes] close");
        self.display.cursor_hidden = false;
        self.active_tab_mut().cursor_blink_timed_out = false;
        let leaves_before = self.active_tab().panes.tree.leaf_ids().len();
        if leaves_before <= 1 { return; }
        let pane_to_close = self.active_tab().panes.active;
        // Find nearest sibling to focus after close.
        let leaves = self.active_tab().panes.tree.leaf_ids();
        let close_pos = leaves.iter().position(|&id| id == pane_to_close).unwrap_or(0);
        let sibling = if close_pos > 0 { leaves[close_pos - 1] } else { leaves[1] };
        let result = self.active_tab_mut().panes.tree.remove(pane_to_close);
        if let crate::pane_tree::RemoveResult::CollapseToSibling(replacement) = result {
            self.active_tab_mut().panes.tree = replacement;
        }
        self.active_tab_mut().panes.additional.remove(&pane_to_close);
        self.active_tab_mut().panes.active = sibling;
        // Resize remaining pane(s) to full viewport.
        let remaining = self.active_tab().panes.tree.leaf_ids().len();
        if remaining == 1 {
            let size_info = self.display.size_info;
            let tab = self.active_tab_mut();
            if tab.panes.active == PaneId(0) {
                let mut t = tab.terminal.lock();
                t.resize(size_info);
                let _ = tab.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(size_info.into()));
            } else if let Some(pane) = tab.panes.additional.get_mut(&tab.panes.active) {
                let mut t = pane.terminal.lock();
                t.resize(size_info);
                let _ = pane.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(size_info.into()));
            }
        }
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.mark_dirty();
    }

    fn focus_pane(&mut self, direction: FocusDir) {
        log::info!("[panes] focus {:?}", direction);
        self.display.cursor_hidden = false;
        self.active_tab_mut().cursor_blink_timed_out = false;
        use crate::pane_tree::Rect;
        let tab = self.active_tab(); let leaves = tab.panes.tree.leaf_ids();
        if leaves.len() <= 1 { return; }
        let active = tab.panes.active;
        let viewport = Rect::new(0.0, 0.0, self.display.size_info.width() as f32, self.display.size_info.height() as f32);
        let (pane_rects, _) = tab.panes.tree.leaf_rects(viewport);
        let active_rect = pane_rects.iter().find(|(id, _)| *id == active).map(|(_, r)| r.clone());
        let Some(active_rect) = active_rect else { return };
        let active_cx = active_rect.x + active_rect.width * 0.5; let active_cy = active_rect.y + active_rect.height * 0.5;
        let candidates: Vec<_> = pane_rects.iter().filter(|(id, _)| *id != active).collect();
        let mut best: Option<(PaneId, f32)> = None;
        for (id, r) in &candidates {
            let cx = r.x + r.width * 0.5; let cy = r.y + r.height * 0.5;
            let (in_dir, primary_dist) = match direction {
                FocusDir::Left if cx < active_cx => (true, active_cx - cx),
                FocusDir::Right if cx > active_cx => (true, cx - active_cx),
                FocusDir::Up if cy < active_cy => (true, active_cy - cy),
                FocusDir::Down if cy > active_cy => (true, cy - active_cy),
                _ => (false, 0.0),
            };
            if !in_dir { continue; }
            let overlaps = match direction {
                FocusDir::Left | FocusDir::Right => r.y < active_rect.y + active_rect.height && r.y + r.height > active_rect.y,
                FocusDir::Up | FocusDir::Down => r.x < active_rect.x + active_rect.width && r.x + r.width > active_rect.x,
            };
            let score = if overlaps { primary_dist } else { 1000.0 + primary_dist };
            match best { None => best = Some((*id, score)), Some((_, bs)) if score < bs => best = Some((*id, score)), _ => {} }
        }
        let new_active = if let Some((id, _)) = best { id } else { leaves[(leaves.iter().position(|&i| i == active).unwrap_or(0) + 1) % leaves.len()] };
        self.active_tab_mut().panes.active = new_active;
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.mark_dirty();
    }

    fn toggle_zoom(&mut self) {
        log::info!("[panes] zoom toggle");
        use crate::pane_tree::Rect;
        let size_info = self.display.size_info;
        let viewport_w = size_info.width() as f32; let viewport_h = size_info.height() as f32;
        let cell_w = size_info.cell_width(); let cell_h = size_info.cell_height();
        let padding_x = size_info.padding_x(); let padding_y = size_info.padding_y();
        let tab = self.active_tab_mut();
        let was_zoomed = tab.panes.zoomed.is_some();
        tab.panes.zoomed = if was_zoomed { None } else { Some(tab.panes.active) };
        if tab.panes.zoomed.is_some() {
            let zoomed = tab.panes.zoomed.unwrap();
            if zoomed == PaneId(0) { let mut t = tab.terminal.lock(); t.resize(size_info);
                let _ = tab.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(size_info.into())); }
            else if let Some(pane) = tab.panes.additional.get_mut(&zoomed) { let mut t = pane.terminal.lock(); t.resize(size_info);
                let _ = pane.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(size_info.into())); }
        } else {
            let full_viewport = Rect::new(0.0, 0.0, viewport_w, viewport_h);
            let (pane_rects, _) = tab.panes.tree.leaf_rects(full_viewport);
            for (pid, rect) in &pane_rects {
                let ps = crate::display::SizeInfo::new(rect.width.max(1.), rect.height.max(1.), cell_w, cell_h, padding_x, padding_y, false);
                if *pid == PaneId(0) { let mut t = tab.terminal.lock(); t.resize(ps);
                    let _ = tab.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(ps.into())); }
                else if let Some(pane) = tab.panes.additional.get_mut(pid) { let mut t = pane.terminal.lock(); t.resize(ps);
                    let _ = pane.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(ps.into())); }
            }
        }
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.dirty = true;
    }

    fn resize_pane(&mut self, dir: SplitDir, grow: bool) {
        use crate::pane_tree::Rect;
        let size_info = self.display.size_info;
        let viewport_w = size_info.width() as f32; let viewport_h = size_info.height() as f32;
        let cell_w = size_info.cell_width(); let cell_h = size_info.cell_height();
        let padding_x = size_info.padding_x(); let padding_y = size_info.padding_y();
        let tab = self.active_tab_mut();
        let pane_id = tab.panes.active;
        let delta = 0.05;
        if !tab.panes.tree.adjust_ratio(pane_id, dir, grow, delta) { return; }
        let full_viewport = Rect::new(0.0, 0.0, viewport_w, viewport_h);
        let (pane_rects, _) = tab.panes.tree.leaf_rects(full_viewport);
        for (pid, rect) in &pane_rects {
            let ps = crate::display::SizeInfo::new(rect.width.max(1.), rect.height.max(1.), cell_w, cell_h, padding_x, padding_y, false);
            if *pid == PaneId(0) { let mut t = tab.terminal.lock(); t.resize(ps);
                let _ = tab.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(ps.into())); }
            else if let Some(pane) = tab.panes.additional.get_mut(pid) { let mut t = pane.terminal.lock(); t.resize(ps);
                let _ = pane.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(ps.into())); }
        }
        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.damage_tracker.next_frame().mark_fully_damaged();
        self.mark_dirty();
    }

    pub fn handle_pane_exit(&mut self, tab_id: Option<TabId>, pane_id: Option<PaneId>) -> bool {
        let Some(tab_idx) = self.tab_index(tab_id) else { return false; };
        let leaves = self.tabs[tab_idx].panes.tree.leaf_ids().len();
        if let Some(pid) = pane_id {
            if pid != PaneId(0) || leaves > 1 {
                self.display.cursor_hidden = false;
                self.tabs[tab_idx].cursor_blink_timed_out = false;
                self.tabs[tab_idx].panes.active = pid;
                let leaves_before = self.tabs[tab_idx].panes.tree.leaf_ids().len();
                if leaves_before > 1 {
                    // Find nearest sibling to focus after removal.
                    let sibling = {
                        let leaves = self.tabs[tab_idx].panes.tree.leaf_ids();
                        let close_pos = leaves.iter().position(|&id| id == pid).unwrap_or(0);
                        if close_pos > 0 { leaves[close_pos - 1] } else { leaves[1] }
                    };
                    let result = self.tabs[tab_idx].panes.tree.remove(pid);
                    if let crate::pane_tree::RemoveResult::CollapseToSibling(replacement) = result {
                        self.tabs[tab_idx].panes.tree = replacement;
                    }
                    self.tabs[tab_idx].panes.additional.remove(&pid);
                    self.tabs[tab_idx].panes.active = sibling;
                    // If only one pane remains, resize it to full viewport.
                    if self.tabs[tab_idx].panes.tree.leaf_ids().len() == 1 {
                        let size_info = self.display.size_info;
                        let tab = &mut self.tabs[tab_idx];
                        if tab.panes.active == PaneId(0) {
                            let mut t = tab.terminal.lock();
                            t.resize(size_info);
                            let _ = tab.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(size_info.into()));
                        } else if let Some(pane) = tab.panes.additional.get_mut(&tab.panes.active) {
                            let mut t = pane.terminal.lock();
                            t.resize(size_info);
                            let _ = pane.notifier.0.send(alacritty_terminal::event_loop::Msg::Resize(size_info.into()));
                        }
                    }
                    self.display.damage_tracker.next_frame().mark_fully_damaged();
                    self.mark_dirty();
                    return false;
                }
            }
        }
        self.handle_tab_exit(tab_id)
    }

    pub fn build_tree_json(&self) -> serde_json::Value {
        let window_id = format!("{:?}", self.display.window.id());
        serde_json::json!({
            "app": {
                "version": env!("CARGO_PKG_VERSION"),
                "pid": std::process::id(),
            },
            "windows": [{
                "index": 1,
                "id": window_id,
                "config": serde_json::to_value(&*self.config).unwrap_or_default(),
                "tabs": self.tabs.iter().enumerate().map(|(i, tab)| {
                    let config = serde_json::to_value(&*tab.config).unwrap_or_default();
                    let panes: Vec<serde_json::Value> = {
                        let mut v = Vec::new();
                        v.push(serde_json::json!({
                            "id": 0,
                            "active": tab.panes.active == PaneId(0),
                            "zoomed": tab.panes.zoomed == Some(PaneId(0)),
                            "process": self.pane_process_info(tab, PaneId(0)),
                        }));
                        for (pane_id, _pane) in &tab.panes.additional {
                            v.push(serde_json::json!({
                                "id": pane_id.0,
                                "active": tab.panes.active == *pane_id,
                                "zoomed": tab.panes.zoomed == Some(*pane_id),
                                "process": self.pane_process_info(tab, *pane_id),
                            }));
                        }
                        v
                    };
                    let cwd_val = {
                        #[cfg(not(windows))]
                        let c = crate::daemon::foreground_process_path(tab.master_fd, tab.shell_pid)
                            .ok().map(|p| p.display().to_string());
                        #[cfg(windows)]
                        let c: Option<String> = None;
                        c
                    };
                    serde_json::json!({
                        "index": i + 1,
                        "id": tab.id.0,
                        "title": tab.display_title(),
                        "active": i == self.active_tab,
                        "pinned": tab.pinned,
                        "cwd": cwd_val,
                        "config": config,
                        "panes": panes,
                    })
                }).collect::<Vec<_>>(),
            }],
        })
    }

    fn pane_process_info(&self, tab: &TerminalTab, pane_id: PaneId) -> serde_json::Value {
        #[cfg(not(windows))]
        {
            let (fd, pid) = if pane_id == PaneId(0) {
                (tab.master_fd, tab.shell_pid)
            } else if let Some(pane) = tab.panes.additional.get(&pane_id) {
                (pane.master_fd, pane.shell_pid)
            } else {
                return serde_json::json!({"pid": 0, "command": "?"});
            };
            let cmd = crate::util::foreground_process_name(fd, pid);
            let cwd = crate::daemon::foreground_process_path(fd, pid).ok()
                .map(|p| p.display().to_string());
            serde_json::json!({
                "pid": pid,
                "command": cmd.unwrap_or_else(|| "?".into()),
                "cwd": cwd,
            })
        }
        #[cfg(windows)]
        {
            serde_json::json!({"pid": 0, "command": "?"})
        }
    }

}
impl Drop for WindowContext {
    fn drop(&mut self) {
        for tab in &mut self.tabs {
            let _ = tab.notifier.0.send(Msg::Shutdown);
        }
    }
}

fn translate_config_overrides(config: Option<serde_json::Value>) -> Vec<String> {
    let Some(config) = config else { return Vec::new() };
    let obj = match config {
        serde_json::Value::Object(map) => map,
        _ => return Vec::new(),
    };
    obj.into_iter().map(|(k, v)| {
        match v {
            serde_json::Value::String(s) => format!("{k}={s}"),
            other => format!("{k}={other}"),
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use crate::util::displayed_title;

    #[test]
    fn displayed_title_uses_detected_when_terminal_title_is_empty() {
        assert_eq!(displayed_title(None, Some(""), "zsh"), "zsh");
    }

    #[test]
    fn displayed_title_prefers_custom_title() {
        assert_eq!(displayed_title(Some("build"), Some("spinner"), "zsh"), "build");
    }
}
