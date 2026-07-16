use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    NewWindow {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        config: Option<Value>,
    },
    CloseWindow,

    CreateTab {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        config: Option<Value>,
        #[serde(default)]
        no_switch: bool,
    },
    CloseTab {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    SelectTab { index: usize },
    SelectNextTab,
    SelectPreviousTab,
    SelectLastTab,
    ListTabs,
    MoveTab { index: usize, delta: i32 },
    TogglePin { index: usize },
    SetTabTitle { title: String },

    SplitPane { direction: SplitDir },
    ClosePane,
    FocusPane { direction: FocusDir },
    ToggleZoom,
    ResizePane { direction: SplitDir, grow: bool },

    SaveSession,
    LoadSession { name: String },
    ListSessions,

    GetConfig,
    SetConfig {
        options: HashMap<String, Value>,
        #[serde(default)]
        reset: bool,
    },

    QuickRun {
        command: Vec<String>,
        #[serde(default)]
        no_switch: bool,
    },
    Scroll { lines: i32 },
    Copy,
    Paste { data: String },

    ToggleMenu,
    MenuNavigate { direction: MenuDir },
    MenuSelect,
    MenuBack,
    MenuLetter { ch: char },
    MenuClick { index: usize },

    Bell,

    SearchForward,
    SearchBackward,
    SearchNext,
    Tree,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SplitDir {
    Right,
    Down,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FocusDir {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum MenuDir {
    Left,
    Right,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActionResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ActionResult {
    pub fn success() -> Self {
        Self { ok: true, data: None, error: None }
    }

    pub fn data(value: Value) -> Self {
        Self { ok: true, data: Some(value), error: None }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, data: None, error: Some(msg.into()) }
    }
}
