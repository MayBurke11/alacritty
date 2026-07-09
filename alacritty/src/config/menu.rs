use serde::Serialize;

use alacritty_config_derive::ConfigDeserialize;

use crate::config::ui_config::Program;

#[derive(ConfigDeserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Menu {
    /// Where to place the menu bar.
    #[serde(default)]
    pub menu_bar_edge: MenuBarEdge,

    /// Menu items (top-level).
    #[serde(default)]
    pub items: Vec<MenuItem>,
}

#[derive(ConfigDeserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuItem {
    /// Label shown on the menu bar.
    pub label: String,

    /// Command to execute when clicked (for leaf items).
    #[serde(default)]
    pub command: Option<Program>,

    /// Built-in action name (takes priority over command, for leaf items).
    #[serde(default)]
    pub action: Option<String>,

    /// Dynamic list source (takes priority over submenu).
    /// When set, the menu enters list mode and shows items from this source.
    #[serde(default)]
    pub list: Option<String>,

    /// Sub-menu items (shown on click if non-empty).
    #[serde(default)]
    pub submenu: Vec<MenuItem>,
}

#[derive(ConfigDeserialize, Serialize, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MenuBarEdge {
    #[default]
    Top,
    Bottom,
}
