//! Alacritty - The GPU Enhanced Terminal.

#![warn(rust_2018_idioms, future_incompatible)]
#![deny(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]
#![cfg_attr(clippy, deny(warnings))]
// With the default subsystem, 'console', windows creates an additional console
// window for the program.
// This is silently ignored on non-windows systems.
// See https://msdn.microsoft.com/en-us/library/4cc7ya5b.aspx for more details.
#![windows_subsystem = "windows"]

#[cfg(not(any(feature = "x11", feature = "wayland", target_os = "macos", windows)))]
compile_error!(r#"at least one of the "x11"/"wayland" features must be enabled"#);

use std::error::Error;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::PathBuf;
use std::{env, fs};

use log::info;
#[cfg(windows)]
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, FreeConsole};
use winit::event_loop::EventLoop;
#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use alacritty_terminal::tty;

mod action;
mod cli;
mod clipboard;
mod config;
mod daemon;
mod display;
mod event;
mod input;
mod ipc_types;
mod logging;
#[cfg(target_os = "macos")]
mod macos;
mod message_bar;
mod migrate;
mod pane_tree;
mod pane_state;
pub mod pane_manager;
pub mod menu_controller;
#[cfg(windows)]
mod panic;
#[cfg(unix)]
mod polling;
mod renderer;
mod scheduler;
mod string;
mod window_context;

mod gl {
    #![allow(clippy::all, unsafe_op_in_unsafe_fn)]
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

#[cfg(unix)]
use crate::cli::MessageOptions;
#[cfg(not(any(target_os = "macos", windows)))]
use crate::cli::SocketMessage;
use crate::cli::{Options, Subcommands};
use crate::config::UiConfig;
use crate::config::monitor::ConfigMonitor;
use crate::event::{Event, Processor};
#[cfg(target_os = "macos")]
use crate::macos::locale;
#[cfg(unix)]
use crate::polling::{IoListener, ipc};

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    panic::attach_handler();

    // When linked with the windows subsystem windows won't automatically attach
    // to the console of the parent process, so we do it explicitly. This fails
    // silently if the parent has no console.
    #[cfg(windows)]
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    // Load command line options.
    let options = Options::new();

    match options.subcommands {
        #[cfg(unix)]
        Some(Subcommands::Msg(options)) => msg(options)?,
        Some(Subcommands::Migrate(options)) => migrate::migrate(options),
        None => alacritty(options)?,
    }

    Ok(())
}

/// `msg` subcommand entrypoint.
#[cfg(unix)]
#[allow(unused_mut)]
fn msg(mut options: MessageOptions) -> Result<(), Box<dyn Error>> {
    #[cfg(not(any(target_os = "macos", windows)))]
    if let SocketMessage::Window(cmd) = &mut options.message {
        if let crate::cli::WindowAction::Create { .. } = &cmd.action {
            // No activation token needed in unified format
        }
    }
    ipc::send_message(options.socket, options.message).map_err(|err| err.into())
}

/// Temporary files stored for Alacritty.
///
/// This stores temporary files to automate their destruction through its `Drop` implementation.
struct TemporaryFiles {
    #[cfg(unix)]
    socket_path: Option<PathBuf>,
    log_file: Option<PathBuf>,
}

impl Drop for TemporaryFiles {
    fn drop(&mut self) {
        // Clean up the IPC socket file.
        #[cfg(unix)]
        if let Some(socket_path) = self.socket_path.as_deref() {
            let _ = fs::remove_file(socket_path);
        }

        // Clean up logfile.
        if let Some(log_file) = &self.log_file {
            if fs::remove_file(log_file).is_ok() {
                let _ = writeln!(io::stdout(), "Deleted log file at \"{}\"", log_file.display());
            }
        }
    }
}

fn default_config_string() -> String {
    "\
# Alacritty-Kitty Configuration
# Auto-generated on first run.
# Theme: ~/.config/alacritty/themes/focus_nova.toml
# Menu:  ~/.config/alacritty/menu.toml

[general]
import = [\"~/.config/alacritty/themes/focus_nova.toml\", \"~/.config/alacritty/menu.toml\"]
live_config_reload = true
ipc_socket = true
auto_save_session = true

[env]
TERM = \"alacritty\"

[window]
startup_mode = \"Maximized\"
decorations = \"None\"
option_as_alt = \"None\"

[font]
size = 11

[terminal]
shell = { program = \"/usr/bin/zsh\", args = [\"-l\"] }

[tabs]
tab_bar_edge = \"top\"
tab_bar_style = \"Slant\"
tab_bar_min_tabs = 1
tab_bell_indicator = \"! {title}\"

[tabs.mouse]
enabled = true

# Alt+arrows for pane focus
[[keyboard.bindings]]
key = \"ArrowRight\"
mods = \"Alt\"
action = \"FocusRight\"

[[keyboard.bindings]]
key = \"ArrowLeft\"
mods = \"Alt\"
action = \"FocusLeft\"

[[keyboard.bindings]]
key = \"ArrowUp\"
mods = \"Alt\"
action = \"FocusUp\"

[[keyboard.bindings]]
key = \"ArrowDown\"
mods = \"Alt\"
action = \"FocusDown\"
".to_string()
}

fn default_menu_string() -> String {
    "\
# Alacritty-Kitty Menu
# Auto-generated on first run.

[menu]
menu_bar_edge = \"bottom\"

[[menu.items]]
label = \"PAN\"
[[menu.items.submenu]]
label = \"SPLIT\"
[[menu.items.submenu.submenu]]
label = \"RIGHT\"
action = \"split-right\"
[[menu.items.submenu.submenu]]
label = \"DOWN\"
action = \"split-down\"
[[menu.items.submenu]]
label = \"RESIZE\"
action = \"resize-mode\"
[[menu.items.submenu]]
label = \"KILL\"
action = \"close-pane\"
[[menu.items.submenu]]
label = \"FULL\"
action = \"toggle-zoom\"

[[menu.items]]
label = \"TAB\"
[[menu.items.submenu]]
label = \"CREATE\"
action = \"create-tab\"
[[menu.items.submenu]]
label = \"KILL\"
action = \"close-tab\"
[[menu.items.submenu]]
label = \"RENAME\"
action = \"rename-tab\"
[[menu.items.submenu]]
label = \"PIN\"
action = \"pin-tab\"
[[menu.items.submenu]]
label = \"FOCUS\"
list = \"tabs\"

[[menu.items]]
label = \"SESS\"
[[menu.items.submenu]]
label = \"SAVE\"
action = \"save-session\"
[[menu.items.submenu]]
label = \"LOAD\"
list = \"sessions\"
".to_string()
}

fn default_theme_string() -> String {
    "\
# Alacritty-Kitty Focus Nova Theme
# Auto-generated on first run.

[colors]
draw_bold_text_with_bright_colors = false

[colors.primary]
background = \"#1e1e2e\"
foreground = \"#dcdcdc\"

[colors.normal]
black   = \"#1e1e2e\"
red     = \"#ff6e7f\"
green   = \"#c0fca0\"
yellow  = \"#f9e2af\"
blue    = \"#89b4fa\"
magenta = \"#cba6f7\"
cyan    = \"#94e2d5\"
white   = \"#dcdcdc\"

[colors.bright]
black   = \"#2e2e3e\"
red     = \"#ff6e7f\"
green   = \"#c0fca0\"
yellow  = \"#f9e2af\"
blue    = \"#89b4fa\"
magenta = \"#cba6f7\"
cyan    = \"#94e2d5\"
white   = \"#f4f4f5\"

[colors.cursor]
text   = \"#1e1e2e\"
cursor = \"#dcdcdc\"

[colors.selection]
text       = \"#1e1e2e\"
background = \"#f9e2af\"
".to_string()
}

fn create_default_config_inner(conf_dir: &PathBuf) -> Result<(), Box<dyn Error>> {
    let path = conf_dir.join("alacritty.toml");
    std::fs::write(&path, default_config_string())?;
    eprintln!("alacritty: created config at {}", path.display());

    let themes_dir = conf_dir.join("themes");
    std::fs::create_dir_all(&themes_dir)?;
    let theme_path = themes_dir.join("focus_nova.toml");
    std::fs::write(&theme_path, default_theme_string())?;
    eprintln!("alacritty: created theme at {}", theme_path.display());

    let menu_path = conf_dir.join("menu.toml");
    std::fs::write(&menu_path, default_menu_string())?;
    eprintln!("alacritty: created menu at {}", menu_path.display());

    Ok(())
}

/// Create default config files on first run.
fn create_default_config() -> Result<(), Box<dyn Error>> {
    let home = std::env::var("HOME").map(std::path::PathBuf::from)
        .unwrap_or_else(|_| ".".into());
    let conf_dir = home.join(".config").join("alacritty");
    std::fs::create_dir_all(&conf_dir)?;
    create_default_config_inner(&conf_dir)
}

/// Run main Alacritty entrypoint.
///
/// Creates a window, the terminal state, PTY, I/O event loop, input processor,
/// config change monitor, and runs the main display loop.
fn alacritty(mut options: Options) -> Result<(), Box<dyn Error>> {
    // Print default config to stdout or write to file.
    if options.create_config {
        let home = std::env::var("HOME").map(std::path::PathBuf::from)
            .unwrap_or_else(|_| ".".into());
        let conf_dir = home.join(".config").join("alacritty");

        if options.overwrite {
            std::fs::create_dir_all(&conf_dir)?;
            create_default_config_inner(&conf_dir)?;
        } else {
            let config_str = default_config_string();
            println!("{config_str}");
        }
        return Ok(());
    }

    // Print systemd user unit to stdout and exit.
    if options.create_systemd_unit {
        let exe = std::env::current_exe().unwrap_or_else(|_| "alacritty".into());
        let exe_path = exe.display();
        println!(concat!(
            "[Unit]\n",
            "Description=Alacritty-Kitty terminal emulator (IPC daemon)\n",
            "Documentation=man:alacritty(1)\n",
            "After=graphical-session.target\n",
            "PartOf=graphical-session.target\n",
            "\n",
            "[Service]\n",
            "Type=simple\n",
            "ExecStart={exe_path} --daemon\n",
            "Restart=no\n",
            "\n",
            "[Install]\n",
            "WantedBy=graphical-session.target\n",
        ), exe_path=exe_path);
        return Ok(());
    }

    // Install desktop entry and icons.
    if options.create_links {
        let home = std::env::var("HOME").map(std::path::PathBuf::from)
            .unwrap_or_else(|_| ".".into());
        let apps_dir = home.join(".local/share/applications");
        std::fs::create_dir_all(&apps_dir)?;
        let desktop_path = apps_dir.join("alacritty.desktop");
        let exe = std::env::current_exe().unwrap_or_else(|_| "alacritty".into());
        let exe_path = exe.display();
        let content = format!(concat!(
            "[Desktop Entry]\n",
            "Type=Application\n",
            "Name=Alacritty-Kitty\n",
            "Comment=GPU-accelerated terminal with tabs, tiling, menu\n",
            "Icon=utilities-terminal\n",
            "Exec={exe_path}\n",
            "Terminal=false\n",
            "Categories=System;TerminalEmulator;\n",
            "StartupNotify=true\n",
            "StartupWMClass=Alacritty\n",
            "Actions=NewWindow;\n",
            "\n",
            "[Desktop Action NewWindow]\n",
            "Name=New Window\n",
            "Exec={exe_path} msg window create\n",
        ), exe_path=exe_path);
        std::fs::write(&desktop_path, content)?;
        info!("Installed desktop entry: {}", desktop_path.display());
        println!("Installed: {}", desktop_path.display());
        println!("Run 'update-desktop-database ~/.local/share/applications' to refresh menu.");
        return Ok(());
    }

    // Setup winit event loop.
    let window_event_loop = EventLoop::<Event>::with_user_event().build()?;

    // Initialize the logger as soon as possible as to capture output from other subsystems.
    let log_file = logging::initialize(&options, window_event_loop.create_proxy())
        .expect("Unable to initialize logger");

    info!("Welcome to Alacritty");
    info!("Version {}", env!("VERSION"));

    #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
    info!(
        "Running on {}",
        if matches!(
            window_event_loop.display_handle().unwrap().as_raw(),
            RawDisplayHandle::Wayland(_)
        ) {
            "Wayland"
        } else {
            "X11"
        }
    );
    #[cfg(not(any(feature = "x11", target_os = "macos", windows)))]
    info!("Running on Wayland");

    // Load configuration file.
    let mut config = config::load(&mut options);

    // First run: no config file found — create one and reload.
    if config.config_paths.is_empty() && !options.create_config {
        if let Err(e) = create_default_config() {
            eprintln!("alacritty: failed to create default config: {e}");
        }
        config = config::load(&mut options);
    }

    log_config_path(&config);

    // Update the log level from config.
    log::set_max_level(config.debug.log_level);

    // Set tty environment variables.
    tty::setup_env();

    // Set env vars from config.
    for (key, value) in config.env.iter() {
        unsafe { env::set_var(key, value) };
    }

    // Switch to home directory.
    #[cfg(target_os = "macos")]
    env::set_current_dir(home::home_dir().unwrap()).unwrap();

    // Set macOS locale.
    #[cfg(target_os = "macos")]
    locale::set_locale_environment();

    #[cfg(target_os = "macos")]
    macos::disable_autofill();

    // Spawn the Unix I/O event polling thread.
    #[cfg(unix)]
    let socket_path = match IoListener::spawn(&config, &options, window_event_loop.create_proxy()) {
        Ok(handle) => handle.ipc_socket_path,
        Err(err) if options.daemon => return Err(err.into()),
        Err(err) => {
            log::warn!("Unable to create socket: {err:?}");
            None
        },
    };

    // Start TCP listener if --tcp-addr is set.
    #[cfg(unix)]
    if let Some(ref tcp_addr) = options.tcp_addr {
        if let Err(err) = ipc::start_tcp_listener(tcp_addr, options.token.clone(), window_event_loop.create_proxy()) {
            log::warn!("Unable to start TCP listener on {tcp_addr}: {err}");
        }
    }

    // Setup automatic RAII cleanup for our files.
    let log_cleanup = log_file.filter(|_| !config.debug.persistent_logging);
    let _files = TemporaryFiles {
        #[cfg(unix)]
        socket_path,
        log_file: log_cleanup,
    };

    // Event processor.
    let mut processor = Processor::new(config, options, &window_event_loop);

    // Start event loop and block until shutdown.
    let result = processor.run(window_event_loop);

    // `Processor` must be dropped before calling `FreeConsole`.
    //
    // This is needed for ConPTY backend. Otherwise a deadlock can occur.
    // The cause:
    //   - Drop for ConPTY will deadlock if the conout pipe has already been dropped
    //   - ConPTY is dropped when the last of processor and window context are dropped, because both
    //     of them own an Arc<ConPTY>
    //
    // The fix is to ensure that processor is dropped first. That way, when window context (i.e.
    // PTY) is dropped, it can ensure ConPTY is dropped before the conout pipe in the PTY drop
    // order.
    //
    // FIXME: Change PTY API to enforce the correct drop order with the typesystem.

    // Terminate the config monitor.
    if let Some(config_monitor) = processor.config_monitor.take() {
        config_monitor.shutdown();
    }

    // Without explicitly detaching the console cmd won't redraw it's prompt.
    #[cfg(windows)]
    unsafe {
        FreeConsole();
    }

    info!("Goodbye");

    result
}

fn log_config_path(config: &UiConfig) {
    if config.config_paths.is_empty() {
        return;
    }

    let mut msg = String::from("Configuration files loaded from:");
    for path in &config.config_paths {
        let _ = write!(msg, "\n  {:?}", path.display());
    }

    info!("{msg}");
}
pub mod util;
pub mod tab;
