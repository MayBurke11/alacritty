//! IPC types and utilities.

pub mod mcp;

use serde::{Deserialize, Serialize};

/// JSON-RPC-light request wrapper.
#[derive(Serialize, Deserialize, Debug)]
pub struct IpcRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub action: crate::action::Action,
}

/// JSON-RPC-light response wrapper.
#[derive(Serialize, Deserialize, Debug)]
pub struct IpcResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// IPC socket replies (used by daemon for synchronous responses).
#[derive(Serialize, Deserialize, Debug)]
pub enum SocketReply {
    GetConfig(String),
    ListTabs(String),
    SaveTabs(String),
}

/// Format tree JSON as ASCII art.
pub fn format_tree_ascii(data: &serde_json::Value) -> String {
    let mut out = String::new();
    let app = &data["app"];
    out.push_str(&format!("alacritty-kitty v{} (pid {})\n\n",
        app["version"].as_str().unwrap_or("?"),
        app["pid"]));

    if let Some(windows) = data["windows"].as_array() {
        for w in windows {
            out.push_str(&format!("WINDOW {} (id {})\n", w["index"], w["id"]));
            if let Some(tabs) = w["tabs"].as_array() {
                for (ti, t) in tabs.iter().enumerate() {
                    let prefix = if ti + 1 < tabs.len() { "├──" } else { "└──" };
                    let active = if t["active"].as_bool().unwrap_or(false) { " [active]" } else { "" };
                    let pinned = if t["pinned"].as_bool().unwrap_or(false) { " [pinned]" } else { "" };
                    let size = t["config"]["font"]["size"].as_f64().map_or(String::new(), |s| format!("  font={}", s));
                    out.push_str(&format!("{} TAB {}: {}{}{}{}\n",
                        prefix, t["index"], t["title"].as_str().unwrap_or("?"),
                        active, pinned, size));
                    if let Some(panes) = t["panes"].as_array() {
                        let indent = if ti + 1 < tabs.len() { "│   " } else { "    " };
                        for (pi, p) in panes.iter().enumerate() {
                            let pprefix = if pi + 1 < panes.len() { "├──" } else { "└──" };
                            let proc = &p["process"];
                            out.push_str(&format!("{}{} PANE {}: {} (pid {})\n",
                                indent, pprefix, p["id"],
                                proc["command"].as_str().unwrap_or("?"),
                                proc["pid"]));
                        }
                    }
                }
            }
        }
    }
    out
}
