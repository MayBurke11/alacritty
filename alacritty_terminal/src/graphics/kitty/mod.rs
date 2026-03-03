//! Kitty graphics protocol implementation.
//!
//! This module is split into submodules for parallel development:
//! - `state`: Image storage, quota management, chunked transfer state
//! - `decode`: Payload decode pipeline (base64 → zlib → format → GraphicData)
//! - `placement`: Image placement on the terminal grid, cropping, ID resolution
//! - `response`: Protocol response formatting and PTY writing
//!
//! See: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>

pub mod decode;
pub mod placement;
pub mod response;
pub mod state;

use log::{debug, trace};

use crate::event::EventListener;
use crate::graphics::kitty_parser::{Action, DeleteTarget, KittyCommand};
use crate::term::Term;

pub use self::state::{KittyImage, KittyLoadingImage, KittyState};

use self::decode::decode_payload;
use self::placement::{place_image, resolve_image_id, resolve_or_assign_id};
use self::response::{error_response, ok_response, send_response};

/// Process a fully parsed kitty graphics command.
///
/// This is the main entry point called from `apc_end` in `term/mod.rs`.
pub fn dispatch_command<L: EventListener>(term: &mut Term<L>, cmd: KittyCommand) {
    // Handle chunked transfer accumulation.
    if cmd.more_chunks {
        handle_chunk_start(term, cmd);
        return;
    }

    // Check if this is the final chunk of a multi-chunk transfer.
    let cmd = if let Some(loading) = term.graphics.kitty_state.loading.take() {
        finalize_chunked(loading, cmd)
    } else {
        cmd
    };

    let quiet = cmd.quiet;
    let image_id = cmd.image_id;

    match cmd.action {
        Action::Transmit => {
            let result = handle_transmit(term, &cmd);
            send_response(term.event_proxy(), quiet, image_id, &result);
        },
        Action::TransmitAndDisplay => {
            let result = handle_transmit_and_display(term, &cmd);
            let resolved_id =
                if image_id != 0 { image_id } else { cmd.image_number };
            send_response(term.event_proxy(), quiet, resolved_id, &result);
        },
        Action::Query => {
            handle_query(term, &cmd);
        },
        Action::Display => {
            let result = handle_display(term, &cmd);
            send_response(term.event_proxy(), quiet, image_id, &result);
        },
        Action::Delete => {
            handle_delete(term, &cmd);
        },
        // Phase 3: Animation (stub).
        Action::TransmitFrame => {
            debug!("[kitty] TransmitFrame not yet implemented");
            send_response(
                term.event_proxy(),
                quiet,
                image_id,
                &Err("animation frames not yet supported".into()),
            );
        },
        Action::AnimationControl => {
            debug!("[kitty] AnimationControl not yet implemented");
        },
        Action::ComposeFrames => {
            debug!("[kitty] ComposeFrames not yet implemented");
            send_response(
                term.event_proxy(),
                quiet,
                image_id,
                &Err("frame composition not yet supported".into()),
            );
        },
    }
}

// ── Chunked Transfer ───────────────────────────────────────────────────

/// Start or continue a chunked transfer (m=1).
fn handle_chunk_start<L: EventListener>(term: &mut Term<L>, cmd: KittyCommand) {
    let payload = cmd.payload.clone();

    match &mut term.graphics.kitty_state.loading {
        Some(loading) => {
            loading.payload.extend_from_slice(&payload);
            trace!(
                "[kitty] chunk appended, total payload: {} bytes",
                loading.payload.len()
            );
        },
        None => {
            trace!("[kitty] starting chunked transfer, first chunk: {} bytes", payload.len());
            term.graphics.kitty_state.loading = Some(KittyLoadingImage {
                command: cmd,
                payload,
            });
        },
    }
}

/// Finalize a chunked transfer by merging the last chunk.
fn finalize_chunked(mut loading: KittyLoadingImage, final_cmd: KittyCommand) -> KittyCommand {
    loading.payload.extend_from_slice(&final_cmd.payload);

    let mut cmd = loading.command;
    cmd.payload = loading.payload;
    cmd.more_chunks = false;
    cmd
}

// ── Action Handlers ────────────────────────────────────────────────────

/// Handle `a=t` (transmit only, don't display).
fn handle_transmit<L: EventListener>(
    term: &mut Term<L>,
    cmd: &KittyCommand,
) -> Result<(), String> {
    let graphic_data = decode_payload(cmd, &cmd.payload).map_err(|e| e.to_string())?;
    let image_id = resolve_or_assign_id(term, cmd);
    term.graphics.kitty_state.store_image(image_id, graphic_data);
    debug!("[kitty] stored image id={image_id}");
    Ok(())
}

/// Handle `a=T` (transmit and display).
fn handle_transmit_and_display<L: EventListener>(
    term: &mut Term<L>,
    cmd: &KittyCommand,
) -> Result<(), String> {
    let graphic_data = decode_payload(cmd, &cmd.payload).map_err(|e| e.to_string())?;
    let image_id = resolve_or_assign_id(term, cmd);

    term.graphics.kitty_state.store_image(image_id, graphic_data);
    debug!("[kitty] stored image id={image_id}, placing on grid");

    place_image(term, image_id, cmd)
}

/// Handle `a=q` (query).
fn handle_query<L: EventListener>(term: &mut Term<L>, cmd: &KittyCommand) {
    use crate::event::Event;

    let image_id = if cmd.image_id != 0 { cmd.image_id } else { 1 };

    let result = if cmd.payload.is_empty() {
        Ok(())
    } else {
        decode_payload(cmd, &cmd.payload).map(|_| ())
    };

    match result {
        Ok(()) => {
            let resp = ok_response(image_id);
            trace!("[kitty] query response: {resp:?}");
            term.event_proxy().send_event(Event::PtyWrite(resp));
        },
        Err(e) => {
            let resp = error_response(image_id, "EINVAL", &e.to_string());
            trace!("[kitty] query error response: {resp:?}");
            term.event_proxy().send_event(Event::PtyWrite(resp));
        },
    }
}

/// Handle `a=p` (display a previously transmitted image).
fn handle_display<L: EventListener>(
    term: &mut Term<L>,
    cmd: &KittyCommand,
) -> Result<(), String> {
    let image_id = resolve_image_id(&term.graphics.kitty_state, cmd)
        .ok_or_else(|| "image not found".to_string())?;

    if !term.graphics.kitty_state.images.contains_key(&image_id) {
        return Err(format!("image id={image_id} not found in storage"));
    }

    place_image(term, image_id, cmd)
}

/// Handle `a=d` (delete).
fn handle_delete<L: EventListener>(term: &mut Term<L>, cmd: &KittyCommand) {
    let target = cmd.delete.unwrap_or(DeleteTarget::All);
    debug!("[kitty] delete: {target:?}, image_id={}, image_number={}", cmd.image_id, cmd.image_number);
    term.graphics.kitty_state.delete(target, cmd);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::kitty_parser::{Action, Format};

    #[test]
    fn finalize_chunked_merges_payloads() {
        let loading = KittyLoadingImage {
            command: KittyCommand {
                action: Action::TransmitAndDisplay,
                format: Format::Png,
                image_id: 5,
                ..Default::default()
            },
            payload: b"AAAA".to_vec(),
        };

        let final_cmd = KittyCommand {
            payload: b"BBBB".to_vec(),
            ..Default::default()
        };

        let merged = finalize_chunked(loading, final_cmd);
        assert_eq!(merged.action, Action::TransmitAndDisplay);
        assert_eq!(merged.format, Format::Png);
        assert_eq!(merged.image_id, 5);
        assert_eq!(merged.payload, b"AAAABBBB");
        assert!(!merged.more_chunks);
    }
}