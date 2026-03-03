//! Kitty graphics image storage and state management.

use std::collections::HashMap;

use log::debug;

use crate::graphics::kitty_parser::{DeleteTarget, KittyCommand};
use crate::graphics::GraphicData;

/// Maximum total memory for kitty image storage (320 MiB).
const STORAGE_QUOTA: usize = 320 * 1024 * 1024;

/// A stored kitty image, potentially with multiple placements on screen.
#[derive(Debug)]
pub struct KittyImage {
    /// The decoded pixel data. Retained for re-placement and animation.
    pub data: GraphicData,
    /// Size of the pixel data in bytes (for quota tracking).
    pub total_bytes: usize,
}

/// Accumulator for chunked image transfers.
#[derive(Debug)]
pub struct KittyLoadingImage {
    /// The command from the first chunk (carries format, medium, etc.).
    pub command: KittyCommand,
    /// Accumulated raw base64 payload bytes across chunks.
    pub payload: Vec<u8>,
}

/// Kitty graphics protocol state, stored inside `Graphics`.
#[derive(Debug, Default)]
pub struct KittyState {
    /// Stored images keyed by image ID.
    pub images: HashMap<u32, KittyImage>,
    /// Mapping from image number (`I=`) to image ID (`i=`).
    pub number_to_id: HashMap<u32, u32>,
    /// Image currently being loaded via chunked transfer.
    pub loading: Option<KittyLoadingImage>,
    /// Total bytes used by stored images (for quota enforcement).
    pub used_memory: usize,
    /// Next auto-assigned image ID (when client doesn't provide one).
    next_image_id: u32,
}

impl KittyState {
    /// Allocate a new unique image ID.
    pub fn next_id(&mut self) -> u32 {
        self.next_image_id = self.next_image_id.wrapping_add(1);
        if self.next_image_id == 0 {
            self.next_image_id = 1;
        }
        self.next_image_id
    }

    /// Evict unreferenced images until we're under the storage quota.
    pub fn enforce_quota(&mut self) {
        if self.used_memory <= STORAGE_QUOTA {
            return;
        }

        let target_free = self.used_memory - STORAGE_QUOTA;
        let mut freed = 0usize;

        let ids_to_remove: Vec<u32> = self
            .images
            .iter()
            .filter_map(|(&id, img)| {
                if freed < target_free {
                    freed += img.total_bytes;
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        for id in ids_to_remove {
            if let Some(img) = self.images.remove(&id) {
                self.used_memory = self.used_memory.saturating_sub(img.total_bytes);
            }
        }

        self.number_to_id.retain(|_, img_id| self.images.contains_key(img_id));
    }

    /// Store an image. Enforces quota before inserting.
    pub fn store_image(&mut self, image_id: u32, data: GraphicData) -> u32 {
        let total_bytes = data.pixels.len();

        if let Some(old) = self.images.remove(&image_id) {
            self.used_memory = self.used_memory.saturating_sub(old.total_bytes);
        }

        self.used_memory += total_bytes;
        self.enforce_quota();

        self.images.insert(image_id, KittyImage { data, total_bytes });

        image_id
    }

    /// Look up an image by ID.
    pub fn get_image(&self, image_id: u32) -> Option<&KittyImage> {
        self.images.get(&image_id)
    }

    /// Resolve an image number to an image ID, if mapped.
    pub fn resolve_number(&self, image_number: u32) -> Option<u32> {
        self.number_to_id.get(&image_number).copied()
    }

    /// Delete images according to the given target.
    pub fn delete(&mut self, target: DeleteTarget, cmd: &KittyCommand) {
        match target {
            DeleteTarget::All | DeleteTarget::AllIncludingScrollback => {
                self.used_memory = 0;
                self.images.clear();
                self.number_to_id.clear();
            },
            DeleteTarget::ById | DeleteTarget::ByIdIncludingScrollback => {
                if cmd.image_id != 0 {
                    if let Some(img) = self.images.remove(&cmd.image_id) {
                        self.used_memory = self.used_memory.saturating_sub(img.total_bytes);
                    }
                    self.number_to_id.retain(|_, &mut id| id != cmd.image_id);
                }
            },
            DeleteTarget::ByNumber | DeleteTarget::ByNumberIncludingScrollback => {
                if cmd.image_number != 0 {
                    if let Some(image_id) = self.number_to_id.remove(&cmd.image_number) {
                        if let Some(img) = self.images.remove(&image_id) {
                            self.used_memory = self.used_memory.saturating_sub(img.total_bytes);
                        }
                    }
                }
            },
            _ => {
                debug!("[kitty] unimplemented delete target: {target:?}");
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::{ColorType, GraphicId};

    fn make_graphic(size: usize) -> GraphicData {
        GraphicData {
            id: GraphicId(0),
            width: 1,
            height: 1,
            color_type: ColorType::Rgba,
            pixels: vec![0; size],
            is_opaque: false,
        }
    }

    #[test]
    fn store_and_retrieve() {
        let mut state = KittyState::default();
        state.store_image(42, make_graphic(16));
        assert!(state.get_image(42).is_some());
        assert_eq!(state.used_memory, 16);
    }

    #[test]
    fn number_to_id() {
        let mut state = KittyState::default();
        state.number_to_id.insert(100, 42);
        assert_eq!(state.resolve_number(100), Some(42));
        assert_eq!(state.resolve_number(999), None);
    }

    #[test]
    fn delete_by_id() {
        let mut state = KittyState::default();
        state.store_image(10, make_graphic(4));
        assert!(state.get_image(10).is_some());

        let cmd = KittyCommand { image_id: 10, ..Default::default() };
        state.delete(DeleteTarget::ById, &cmd);
        assert!(state.get_image(10).is_none());
        assert_eq!(state.used_memory, 0);
    }

    #[test]
    fn delete_all() {
        let mut state = KittyState::default();
        for id in 1..=5 {
            state.store_image(id, make_graphic(4));
        }
        assert_eq!(state.images.len(), 5);

        let cmd = KittyCommand::default();
        state.delete(DeleteTarget::All, &cmd);
        assert!(state.images.is_empty());
        assert_eq!(state.used_memory, 0);
    }

    #[test]
    fn auto_id() {
        let mut state = KittyState::default();
        let id1 = state.next_id();
        let id2 = state.next_id();
        assert_ne!(id1, id2);
        assert_ne!(id1, 0);
        assert_ne!(id2, 0);
    }
}