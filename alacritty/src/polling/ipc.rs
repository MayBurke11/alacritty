//! Alacritty socket IPC.

use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Error as IoError, ErrorKind, Result as IoResult, Write};
use std::net::{Shutdown, TcpListener};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fs};

use log::{error, warn};
use std::result::Result;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

use crate::cli::{Options, SocketMessage};
use crate::event::{Event, EventType};

/// Environment variable name for the IPC socket path.
const ALACRITTY_SOCKET_ENV: &str = "ALACRITTY_SOCKET";

/// IPC socket listener.
pub struct IpcListener {
    pub socket: UnixListener,

    event_proxy: EventLoopProxy<Event>,
    data: String,
}

impl IpcListener {
    pub fn new(
        options: &Options,
        event_proxy: EventLoopProxy<Event>,
        path: &Path,
    ) -> Result<Self, IoError> {
        // Create unix socket in nonblocking mode.
        let socket = UnixListener::bind(path)?;
        socket.set_nonblocking(true)?;

        // Register socket path as environment variable for `alacritty msg`.
        unsafe { env::set_var(ALACRITTY_SOCKET_ENV, path.as_os_str()) };
        if options.daemon {
            println!("ALACRITTY_SOCKET={}; export ALACRITTY_SOCKET", path.display());
        }

        Ok(Self { event_proxy, socket, data: Default::default() })
    }

    /// Process the next IPC message.
    pub fn process_message(&mut self) -> Result<(), IoError> {
        let (stream, _) = self.socket.accept()?;

        self.data.clear();
        let mut reader = BufReader::new(&stream);

        match reader.read_line(&mut self.data) {
            Ok(0) | Err(_) => return Ok(()),
            Ok(_) => (),
        };

        let message: SocketMessage = match serde_json::from_str(&self.data) {
            Ok(message) => message,
            Err(_) => {
                // Fallback: try unified IpcRequest format.
                if let Ok(req) = serde_json::from_str::<IpcRequest>(&self.data) {
                    let event = Event::new(EventType::IpcAction(req.action), None);
                    let _ = self.event_proxy.send_event(event);
                    // Send reply if id present.
                    if let Some(id) = req.id {
                        let resp = IpcResponse { id: Some(id), ok: true, data: None, error: None };
                        let mut json = serde_json::to_string(&resp).unwrap_or_default();
                        json.push('\n');
                        let _ = std::io::Write::write_all(&mut &stream, json.as_bytes());
                    }
                    return Ok(());
                }
                // Try bare Action format.
                match serde_json::from_str::<crate::action::Action>(&self.data) {
                    Ok(action) => {
                        let event = Event::new(EventType::IpcAction(action), None);
                        let _ = self.event_proxy.send_event(event);
                        return Ok(());
                    },
                    Err(err) => {
                        warn!("Failed to parse IPC message: {err}");
                        return Ok(());
                    },
                }
            },
        };

        // Handle IPC events.
        match message {
            SocketMessage::CreateWindow(_) => {
                let action = crate::action::Action::NewWindow { command: None, cwd: None, config: None };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::Config(ipc_config) => {
                let window_id =
                    ipc_config.window_id.and_then(|id| u64::try_from(id).ok()).map(WindowId::from);
                let event = Event::new(EventType::IpcConfig(ipc_config), window_id);
                let _ = self.event_proxy.send_event(event);
            },
            SocketMessage::GetConfig(config) => {
                let window_id =
                    config.window_id.and_then(|id| u64::try_from(id).ok()).map(WindowId::from);
                let event = Event::new(EventType::IpcGetConfig(Arc::new(stream)), window_id);
                let _ = self.event_proxy.send_event(event);
            },
            SocketMessage::CreateTab(options) => {
                let action = crate::action::Action::CreateTab {
                    command: if options.command.is_empty() { None } else { Some(options.command.clone()) },
                    cwd: options.working_directory.map(|p| p.to_string_lossy().to_string()),
                    config: None,
                    no_switch: options.no_switch,
                };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::ListTabs(options) => {
                let event = Event::new(EventType::ListTabsIPC(Arc::new(stream), options.window_id), None);
                let _ = self.event_proxy.send_event(event);
            },
            SocketMessage::SelectTab(options) => {
                let action = crate::action::Action::SelectTab { index: options.index.saturating_sub(1) };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::CloseTab(options) => {
                let action = crate::action::Action::CloseTab { index: Some(options.index.saturating_sub(1)) };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::PinTab(options) => {
                let action = crate::action::Action::TogglePin { index: options.index.saturating_sub(1) };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::SaveTabs(options) => {
                let event = Event::new(EventType::SaveTabsIPC(Arc::new(stream), options.window_id), None);
                let _ = self.event_proxy.send_event(event);
            },
            SocketMessage::QuickRun(options) => {
                let action = crate::action::Action::QuickRun {
                    command: options.command.clone(),
                    no_switch: options.no_switch,
                };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::Exec(exec) => {
                match serde_json::from_str::<crate::action::Action>(&exec.json) {
                    Ok(action) => {
                        let event = Event::new(EventType::IpcAction(action), None);
                        let _ = self.event_proxy.send_event(event);
                    },
                    Err(err) => {
                        warn!("Failed to parse Action JSON: {err}");
                    },
                }
            },
            SocketMessage::Pane(cmd) => {
                use crate::action::{Action as AppAction, FocusDir, SplitDir};
                use crate::cli::PaneAction;
                let action = match &cmd.action {
                    PaneAction::Split { direction } => AppAction::SplitPane {
                        direction: match direction.as_str() {
                            "right" => SplitDir::Right,
                            _ => SplitDir::Down,
                        },
                    },
                    PaneAction::Close => AppAction::ClosePane,
                    PaneAction::Focus { direction } => AppAction::FocusPane {
                        direction: match direction.as_str() {
                            "left" => FocusDir::Left,
                            "right" => FocusDir::Right,
                            "up" => FocusDir::Up,
                            _ => FocusDir::Down,
                        },
                    },
                    PaneAction::Zoom => AppAction::ToggleZoom,
                    PaneAction::Resize { direction, action: resize_action } => AppAction::ResizePane {
                        direction: match direction.as_str() {
                            "right" => SplitDir::Right,
                            _ => SplitDir::Down,
                        },
                        grow: resize_action == "grow",
                    },
                };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::Session(cmd) => {
                use crate::action::Action as AppAction;
                use crate::cli::SessionAction;
                let action = match &cmd.action {
                    SessionAction::Save => AppAction::SaveSession,
                    SessionAction::Load { name } => AppAction::LoadSession { name: name.clone() },
                    SessionAction::List => AppAction::ListSessions,
                };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::TabNav(cmd) => {
                use crate::action::Action as AppAction;
                use crate::cli::TabNavAction;
                let action = match &cmd.action {
                    TabNavAction::Next => AppAction::SelectNextTab,
                    TabNavAction::Previous => AppAction::SelectPreviousTab,
                    TabNavAction::Last => AppAction::SelectLastTab,
                };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::ScrollView(cmd) => {
                let action = crate::action::Action::Scroll { lines: cmd.lines };
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
            SocketMessage::Bell => {
                let action = crate::action::Action::Bell;
                let _ = self.event_proxy.send_event(Event::new(EventType::IpcAction(action), None));
            },
        }

        Ok(())
    }
}

/// Send a message to the active Alacritty socket.
pub fn send_message(socket: Option<PathBuf>, message: SocketMessage) -> IoResult<()> {
    let mut socket = find_socket(socket)?;

    // For Exec, send raw JSON string directly.
    if let SocketMessage::Exec(ref exec) = message {
        let mut json = exec.json.clone();
        if !json.ends_with('\n') {
            json.push('\n');
        }
        socket.write_all(json.as_bytes())?;
    } else {
        let message_json = serde_json::to_string(&message)?;
        socket.write_all(message_json.as_bytes())?;
    }
    let _ = socket.flush();

    // Shutdown write end, to allow reading.
    socket.shutdown(Shutdown::Write)?;

    // Get matching IPC reply.
    handle_reply(&socket, &message)?;

    Ok(())
}

/// Process IPC responses.
fn handle_reply(stream: &UnixStream, message: &SocketMessage) -> IoResult<()> {
    // Read reply, returning early if there is none.
    let mut buffer = String::new();
    let mut reader = BufReader::new(stream);
    if let Ok(0) | Err(_) = reader.read_line(&mut buffer) {
        return Ok(());
    }

    // Parse IPC reply — try SocketReply first, then IpcResponse.
    if let Ok(reply) = serde_json::from_str::<SocketReply>(&buffer) {
        return handle_socket_reply(message, &reply);
    }
    if let Ok(resp) = serde_json::from_str::<IpcResponse>(&buffer) {
        if let Some(data) = resp.data {
            println!("{data}");
        } else if let Some(err) = resp.error {
            eprintln!("Error: {err}");
        } else {
            println!("{}", if resp.ok { "ok" } else { "error" });
        }
        return Ok(());
    }
    Ok(())
}

fn handle_socket_reply(message: &SocketMessage, reply: &SocketReply) -> IoResult<()> {
    match (message, reply) {
        (SocketMessage::GetConfig(..), SocketReply::GetConfig(config)) => {
            println!("{config}");
            Ok(())
        },
        (SocketMessage::ListTabs(..), SocketReply::ListTabs(tabs)) => {
            println!("{tabs}");
            Ok(())
        },
        (SocketMessage::SaveTabs(..), SocketReply::SaveTabs(tabs)) => {
            println!("{tabs}");
            Ok(())
        },
        _ => Ok(()),
    }
}

/// Send IPC message reply.
pub fn send_reply(stream: &mut UnixStream, message: SocketReply) {
    if let Err(err) = send_reply_fallible(stream, message) {
        error!("Failed to send IPC reply: {err}");
    }
}

/// Send IPC message reply, returning possible errors.
fn send_reply_fallible(stream: &mut UnixStream, message: SocketReply) -> IoResult<()> {
    let json = serde_json::to_string(&message).map_err(IoError::other)?;
    stream.write_all(json.as_bytes())?;
    stream.flush()?;
    Ok(())
}

/// Directory for the IPC socket file.
#[cfg(not(target_os = "macos"))]
pub fn socket_dir() -> PathBuf {
    xdg::BaseDirectories::with_prefix("alacritty")
        .get_runtime_directory()
        .map(ToOwned::to_owned)
        .ok()
        .and_then(|path| fs::create_dir_all(&path).map(|_| path).ok())
        .unwrap_or_else(env::temp_dir)
}

/// Directory for the IPC socket file.
#[cfg(target_os = "macos")]
pub fn socket_dir() -> PathBuf {
    env::temp_dir()
}

/// Find the IPC socket path.
fn find_socket(socket_path: Option<PathBuf>) -> IoResult<UnixStream> {
    // Handle --socket CLI override.
    if let Some(socket_path) = socket_path {
        // Ensure we inform the user about an invalid path.
        return UnixStream::connect(&socket_path).map_err(|err| {
            let message = format!("invalid socket path {socket_path:?}");
            IoError::new(err.kind(), message)
        });
    }

    // Handle environment variable.
    if let Ok(path) = env::var(ALACRITTY_SOCKET_ENV) {
        let socket_path = PathBuf::from(path);
        if let Ok(socket) = UnixStream::connect(socket_path) {
            return Ok(socket);
        }
    }

    // Search for sockets files.
    for entry in fs::read_dir(socket_dir())?.filter_map(|entry| entry.ok()) {
        let path = entry.path();

        // Skip files that aren't Alacritty sockets.
        let socket_prefix = socket_prefix();
        if path
            .file_name()
            .and_then(OsStr::to_str)
            .filter(|file| file.starts_with(&socket_prefix) && file.ends_with(".sock"))
            .is_none()
        {
            continue;
        }

        // Attempt to connect to the socket.
        match UnixStream::connect(&path) {
            Ok(socket) => return Ok(socket),
            // Delete orphan sockets.
            Err(error) if error.kind() == ErrorKind::ConnectionRefused => {
                let _ = fs::remove_file(&path);
            },
            // Ignore other errors like permission issues.
            Err(_) => (),
        }
    }

    Err(IoError::new(ErrorKind::NotFound, "no socket found"))
}

/// File prefix matching all available sockets.
///
/// This prefix will include display server information to allow for environments with multiple
/// display servers running for the same user.
#[cfg(not(target_os = "macos"))]
pub fn socket_prefix() -> String {
    let display = env::var("WAYLAND_DISPLAY").or_else(|_| env::var("DISPLAY")).unwrap_or_default();
    format!("Alacritty-{}", display.replace('/', "-"))
}

/// File prefix matching all available sockets.
#[cfg(target_os = "macos")]
pub fn socket_prefix() -> String {
    String::from("Alacritty")
}

/// Start a TCP listener for remote IPC (localhost by default).
pub fn start_tcp_listener(addr: &str, proxy: EventLoopProxy<Event>) -> IoResult<()> {
    let listener = TcpListener::bind(addr)?;
    log::info!("TCP IPC listening on {addr}");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            match serde_json::from_str::<crate::action::Action>(&line) {
                Ok(action) => {
                    let _ = proxy.send_event(Event::new(EventType::IpcAction(action), None));
                },
                Err(err) => {
                    warn!("TCP: failed to parse Action: {err}");
                },
            }
        }
    });
    Ok(())
}

/// IPC socket replies.
#[derive(Serialize, Deserialize, Debug)]
pub enum SocketReply {
    GetConfig(String),
    ListTabs(String),
    SaveTabs(String),
}

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
