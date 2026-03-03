//! Kitty graphics protocol response formatting and sending.

use log::trace;

use crate::event::{Event, EventListener};
use crate::graphics::kitty_parser::Quiet;

/// Build a kitty graphics OK response.
pub fn ok_response(image_id: u32) -> String {
    format!("\x1b_Gi={image_id};OK\x1b\\")
}

/// Build a kitty graphics error response.
pub fn error_response(image_id: u32, code: &str, message: &str) -> String {
    format!("\x1b_Gi={image_id};{code}:{message}\x1b\\")
}

/// Send a response to the PTY, respecting quiet mode.
pub fn send_response<L: EventListener>(
    event_proxy: &L,
    quiet: Quiet,
    image_id: u32,
    result: &Result<(), String>,
) {
    match result {
        Ok(()) => {
            if quiet == Quiet::None {
                let resp = ok_response(image_id);
                trace!("[kitty] response: {resp:?}");
                event_proxy.send_event(Event::PtyWrite(resp));
            }
        },
        Err(msg) => {
            if quiet != Quiet::SuppressAll {
                let resp = error_response(image_id, "EINVAL", msg);
                trace!("[kitty] response: {resp:?}");
                event_proxy.send_event(Event::PtyWrite(resp));
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_response_format() {
        assert_eq!(ok_response(42), "\x1b_Gi=42;OK\x1b\\");
    }

    #[test]
    fn error_response_format() {
        assert_eq!(
            error_response(42, "EINVAL", "bad image"),
            "\x1b_Gi=42;EINVAL:bad image\x1b\\"
        );
    }
}