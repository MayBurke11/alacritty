use std::error::Error;
use std::fs::File;
#[cfg(not(windows))]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(not(windows))]
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use alacritty_terminal::event::Event as TerminalEvent;
use alacritty_terminal::event_loop::{EventLoop as PtyEventLoop, Msg, Notifier};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::tty;
use winit::window::WindowId;
use winit::event_loop::EventLoopProxy;

use crate::cli::WindowOptions;
use crate::config::UiConfig;
#[cfg(not(windows))]
use crate::daemon::foreground_process_path;
use crate::event::{
    Event, EventProxy, EventType, InlineSearchState, SearchState, TabId,
};
use crate::message_bar::MessageBuffer;
use crate::pane_manager::PaneManager;
use crate::util;


pub struct TerminalTab {

    pub id: TabId,

    pub terminal: Arc<FairMutex<Term<EventProxy>>>,

    pub notifier: Notifier,

    pub config: Rc<UiConfig>,

    pub pinned: bool,

    /// Command that was used to create this tab, for save/restore.

    pub command: Option<Vec<String>>,

    pub terminal_title: Option<String>,

    pub detected_title: String,

    pub custom_title: Option<String>,

    /// Cache for hysteresis-based title update — only apply after two consecutive matches.

    pub cached_title: Option<String>,

    pub message_buffer: MessageBuffer,

    pub cursor_blink_timed_out: bool,

    pub prev_bell_cmd: Option<Instant>,

    pub inline_search_state: InlineSearchState,

    pub search_state: SearchState,

    // Pane management.

    pub panes: PaneManager,

    #[cfg(not(windows))]

    pub master_fd: RawFd,

    #[cfg(not(windows))]

    pub shell_pid: u32,

}



impl TerminalTab {

    pub fn new(

        id: TabId,

        window_id: WindowId,

        size_info: crate::display::SizeInfo,

        config: Rc<UiConfig>,

        options: &WindowOptions,

        proxy: &EventLoopProxy<Event>,

    ) -> Result<Self, Box<dyn Error>> {

        let mut pty_config = config.pty_config();

        options.terminal_options.override_pty_config(&mut pty_config);



        let event_proxy = EventProxy::new(proxy.clone(), window_id, id, None);



        let terminal = Term::new(config.term_options(), &size_info, event_proxy.clone());

        let terminal = Arc::new(FairMutex::new(terminal));



        let pty = tty::new(&pty_config, size_info.into(), window_id.into())?;



        #[cfg(not(windows))]

        let master_fd = pty.file().as_raw_fd();

        #[cfg(not(windows))]

        let shell_pid = pty.child().id();



        let event_loop = PtyEventLoop::new(

            Arc::clone(&terminal),

            event_proxy.clone(),

            pty,

            pty_config.drain_on_exit,

            config.debug.ref_test,

        )?;



        let loop_tx = event_loop.channel();

        let _io_thread = event_loop.spawn();



        if config.cursor.style().blinking {

            event_proxy.send_event(TerminalEvent::CursorBlinkingChange.into());

        }



        Ok(Self {

            id,

            terminal,

            #[cfg(not(windows))]

            master_fd,

            #[cfg(not(windows))]

            shell_pid,

            config: config.clone(),

            pinned: false,

            command: None,

            notifier: Notifier(loop_tx),

            terminal_title: None,

            detected_title: Self::detected_title(

                #[cfg(not(windows))]

                master_fd,

                #[cfg(not(windows))]

                shell_pid,

                &config.window.identity.title,

            ),

            custom_title: None,

            cached_title: None,

            cursor_blink_timed_out: false,

            prev_bell_cmd: None,

            inline_search_state: Default::default(),

            message_buffer: Default::default(),

            search_state: Default::default(),

            panes: PaneManager::new(),

        })

    }



    pub fn detected_title(

        #[cfg(not(windows))] master_fd: RawFd,

        #[cfg(not(windows))] shell_pid: u32,

        default_title: &str,

    ) -> String {

        #[cfg(not(windows))]

        {

            let process = util::foreground_process_name(master_fd, shell_pid);

            let cwd = foreground_process_path(master_fd, shell_pid)

                .ok()

                .map(|path| util::format_tab_cwd(&path));



            if let Some(process) = process.filter(|process| !util::is_shell_process(process)) {

                process

            } else if let Some(cwd) = cwd {

                cwd

            } else {

                default_title.to_owned()

            }

        }



        #[cfg(windows)]

        {

            default_title.to_owned()

        }

    }



    pub fn refresh_detected_title(&mut self, config: &UiConfig) -> bool {

        let new_title = Self::detected_title(

            #[cfg(not(windows))]

            self.master_fd,

            #[cfg(not(windows))]

            self.shell_pid,

            &config.window.identity.title,

        );



        // Hysteresis: only apply after two consecutive matches.

        // Transient processes (ls, cat) never confirm, so they are filtered out.

        match self.cached_title.take() {

            Some(cached) if cached == new_title => {

                self.detected_title = new_title;

                true

            },

            _ => {

                self.cached_title = Some(new_title);

                false

            },

        }

    }



    pub fn display_title(&self) -> &str {

        displayed_title(

            self.custom_title.as_deref(),

            self.terminal_title.as_deref(),

            &self.detected_title,

        )

    }

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



pub struct TabTitleEditor {

    pub tab_id: TabId,

    pub value: String,

}
