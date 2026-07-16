//! MCP (Model Context Protocol) server — JSON-RPC over stdio.
//! Translates MCP tool calls to Action IPC messages.
//! No external dependencies beyond serde_json.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use serde_json::{json, Value};

use crate::action::Action;
use crate::cli::SocketMessage;

pub fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = line?;
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = req["method"].as_str().unwrap_or("");
        let id = &req["id"];

        let response = match method {
            "initialize" => json!({"jsonrpc":"2.0","id":id,"result":{
                "protocolVersion":"2024-11-05",
                "capabilities":{"tools":{}},
                "serverInfo":{"name":"alacritty-kitty","version":env!("CARGO_PKG_VERSION")}
            }}),
            "tools/list" => json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools_list()}}),
            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let args = &req["params"]["arguments"];
                json!({"jsonrpc":"2.0","id":id,"result":handle_tool_call(name, args)})
            },
            _ => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":format!("unknown method: {method}")}}),
        };

        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}

fn handle_tool_call(name: &str, args: &Value) -> Value {
    let action = match name {
        "tab.create" => Action::CreateTab {
            command: args["command"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()),
            cwd: args["cwd"].as_str().map(String::from),
            config: None,
            no_switch: args["no_switch"].as_bool().unwrap_or(false),
        },
        "tab.list" => Action::ListTabs,
        "tab.close" => Action::CloseTab {
            index: args["index"].as_u64().map(|i| i as usize),
        },
        "tab.select" => Action::SelectTab {
            index: args["index"].as_u64().unwrap_or(0) as usize,
        },
        "tab.next" => Action::SelectNextTab,
        "tab.previous" => Action::SelectPreviousTab,
        "pane.split" => Action::SplitPane {
            direction: match args["direction"].as_str().unwrap_or("right") {
                "right" => crate::action::SplitDir::Right,
                _ => crate::action::SplitDir::Down,
            },
        },
        "pane.close" => Action::ClosePane,
        "pane.focus" => Action::FocusPane {
            direction: match args["direction"].as_str().unwrap_or("right") {
                "left" => crate::action::FocusDir::Left,
                "right" => crate::action::FocusDir::Right,
                "up" => crate::action::FocusDir::Up,
                _ => crate::action::FocusDir::Down,
            },
        },
        "pane.zoom" => Action::ToggleZoom,
        "session.save" => Action::SaveSession,
        "session.list" => Action::ListSessions,
        "config.get" => Action::GetConfig,
        "tree" => Action::Tree,
        "bell" => Action::Bell,
        _ => return json!({"ok": false, "error": format!("unknown tool: {name}")}),
    };

    match send_action(&action) {
        Ok(data) => json!({"ok": true, "data": data}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}

fn send_action(action: &Action) -> io::Result<Option<Value>> {
    let socket_path = find_socket()?;
    
    // For data-returning commands, use synchronous SocketMessage format.
    let msg = match action {
        Action::ListTabs => json!({"Tab":{"action":{"List":{"window_id":null}}}}),
        Action::GetConfig => json!({"Config":{"action":"Get"}}),
        Action::ListSessions => json!({"Session":{"action":"List"}}),
        Action::Tree => json!({"Tree":{"format":"json"}}),
        _ => return send_async_action(action, &socket_path),
    };
    
    let mut stream = UnixStream::connect(&socket_path)?;
    let mut req = serde_json::to_vec(&msg)?;
    req.push(b'\n');
    stream.write_all(&req)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut buf = String::new();
    BufReader::new(&stream).read_line(&mut buf)?;
    // Parse SocketReply or plain JSON response
    if let Ok(reply) = serde_json::from_str::<crate::ipc::SocketReply>(&buf) {
        match reply {
            crate::ipc::SocketReply::ListTabs(tabs) => Ok(Some(serde_json::from_str(&tabs).unwrap_or(Value::Null))),
            crate::ipc::SocketReply::GetConfig(config) => Ok(Some(serde_json::from_str(&config).unwrap_or(Value::String(config)))),
            crate::ipc::SocketReply::SaveTabs(data) => Ok(Some(serde_json::from_str(&data).unwrap_or(Value::Null))),
        }
    } else {
        // TreeIPC sends IpcResponse format
        if let Ok(resp) = serde_json::from_str::<crate::ipc::IpcResponse>(&buf) {
            Ok(resp.data)
        } else {
            Ok(None)
        }
    }
}

fn send_async_action(action: &Action, socket_path: &std::path::Path) -> io::Result<Option<Value>> {
    let mut stream = UnixStream::connect(socket_path)?;
    let mut req = serde_json::to_vec(&crate::ipc::IpcRequest { id: Some(1), action: action.clone() })?;
    req.push(b'\n');
    stream.write_all(&req)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut buf = String::new();
    BufReader::new(&stream).read_line(&mut buf)?;
    if let Ok(resp) = serde_json::from_str::<crate::ipc::IpcResponse>(&buf) {
        Ok(resp.data)
    } else {
        Ok(None)
    }
}

fn find_socket() -> io::Result<std::path::PathBuf> {
    if let Ok(path) = std::env::var("ALACRITTY_SOCKET") {
        return Ok(path.into());
    }
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("Alacritty-") && name.ends_with(".sock") {
            return Ok(entry.path());
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "No alacritty socket found"))
}

fn tools_list() -> Vec<Value> {
    vec![
        json!({"name":"tab.create","description":"Create a new tab","inputSchema":{"type":"object","properties":{"command":{"type":"array","items":{"type":"string"},"description":"Command to run"},"cwd":{"type":"string"},"no_switch":{"type":"boolean"}}}}),
        json!({"name":"tab.list","description":"List all tabs"}),
        json!({"name":"tab.close","description":"Close a tab by index","inputSchema":{"type":"object","properties":{"index":{"type":"integer"}}}}),
        json!({"name":"tab.select","description":"Select a tab by index","inputSchema":{"type":"object","properties":{"index":{"type":"integer"}}}}),
        json!({"name":"tab.next","description":"Switch to next tab"}),
        json!({"name":"tab.previous","description":"Switch to previous tab"}),
        json!({"name":"pane.split","description":"Split the active pane","inputSchema":{"type":"object","properties":{"direction":{"type":"string","enum":["right","down"]}}}}),
        json!({"name":"pane.close","description":"Close the active pane"}),
        json!({"name":"pane.focus","description":"Focus a pane by direction","inputSchema":{"type":"object","properties":{"direction":{"type":"string","enum":["left","right","up","down"]}}}}),
        json!({"name":"pane.zoom","description":"Toggle pane zoom"}),
        json!({"name":"session.save","description":"Save current session"}),
        json!({"name":"session.list","description":"List saved sessions"}),
        json!({"name":"config.get","description":"Get current configuration"}),
        json!({"name":"tree","description":"Show process tree"}),
        json!({"name":"bell","description":"Ring the terminal bell"}),
    ]
}
