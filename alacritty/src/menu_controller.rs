//! Modal menu state holder. Extracted from WindowContext.
use crate::event::{MenuOp, MenuState};

pub struct MenuController {
    pub state: MenuState,
    pub toggle_pending: bool,
    pub op_pending: Vec<MenuOp>,
}

impl MenuController {
    pub fn new() -> Self {
        Self {
            state: MenuState::default(),
            toggle_pending: false,
            op_pending: Vec::new(),
        }
    }
}
