//! Shared utility functions — process info, tab helpers.

use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use std::os::unix::io::RawFd;

#[cfg(not(windows))]
pub fn foreground_process_name(master_fd: RawFd, shell_pid: u32) -> Option<String> {
    let mut pid = unsafe { libc::tcgetpgrp(master_fd) };
    if pid < 0 { pid = shell_pid as i32; }
    let process_name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let process_name = process_name.trim();
    (!process_name.is_empty()).then(|| process_name.to_owned())
}

#[cfg(not(windows))]
pub fn foreground_process_cmdline(master_fd: RawFd, shell_pid: u32) -> Option<Vec<String>> {
    let mut pid = unsafe { libc::tcgetpgrp(master_fd) };
    if pid < 0 { pid = shell_pid as i32; }
    let data = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if data.is_empty() { return None; }
    let args: Vec<String> = data.split(|&b| b == 0)
        .filter_map(|chunk| std::str::from_utf8(chunk).ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect();
    if args.is_empty() { None } else { Some(args) }
}

#[cfg(not(windows))]
pub fn process_cwd(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

pub fn format_tab_cwd(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    if home.as_deref().is_some_and(|home| home == path) {
        return String::from("~");
    }
    path.file_name().and_then(|name| name.to_str()).unwrap_or("?").to_owned()
}

pub fn is_shell_process(process: &str) -> bool {
    matches!(process, "bash" | "dash" | "fish" | "nu" | "sh" | "zsh" | "tmux")
}

pub fn displayed_title<'a>(
    custom_title: Option<&'a str>,
    terminal_title: Option<&'a str>,
    detected_title: &'a str,
) -> &'a str {
    custom_title
        .filter(|title| !title.is_empty())
        .or_else(|| terminal_title.filter(|title| !title.is_empty()))
        .unwrap_or(detected_title)
}
