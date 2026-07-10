#[cfg(not(windows))]
use std::os::unix::io::RawFd;
use std::sync::Arc;

use alacritty_terminal::event_loop::Notifier;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;

use crate::event::EventProxy;

use crate::pane_tree::PaneId;

pub struct PaneState {
    pub pane_id: PaneId,
    pub terminal: Arc<FairMutex<Term<EventProxy>>>,
    pub notifier: Notifier,
    pub command: Option<Vec<String>>,
    #[cfg(not(windows))]
    pub master_fd: RawFd,
    #[cfg(not(windows))]
    pub shell_pid: u32,
}

impl PaneState {
    pub fn new(
        pane_id: PaneId,
        terminal: Arc<FairMutex<Term<EventProxy>>>,
        notifier: Notifier,
        command: Option<Vec<String>>,
        #[cfg(not(windows))] master_fd: RawFd,
        #[cfg(not(windows))] shell_pid: u32,
    ) -> Self {
        Self {
            pane_id,
            terminal,
            notifier,
            command,
            #[cfg(not(windows))]
            master_fd,
            #[cfg(not(windows))]
            shell_pid,
        }
    }
}

impl Drop for PaneState {
    fn drop(&mut self) {
        let _ = self.notifier.0.send(alacritty_terminal::event_loop::Msg::Shutdown);
        // master_fd is owned by PtyEventLoop, not by us. Don't close it here.
    }
}
