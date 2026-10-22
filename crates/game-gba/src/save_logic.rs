//! Pure save decisions of the GBA shell (no agb imports): the debug save combo, the boot
//! choice and the pending-save timeout. Host-tested by `crates/emu/tests/save_logic.rs`,
//! which includes this file through `#[path]`.

use engine::Buttons;
use engine::checkpoint::SaveMeta;

/// Frames a combo waits for a capturable World before giving up (`debug::save::TIMEOUT`).
pub const SAVE_WAIT_FRAMES: u16 = 600;

/// L and R held, and SELECT newly pressed.
pub fn combo_pressed(prev: Buttons, cur: Buttons) -> bool {
    let _ = (prev, cur);
    todo!()
}

/// What the shell boots into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BootChoice {
    /// Read slot 0 and `World::restore` it.
    Restore,
    /// `World::new`, with the `debug::boot` code saying why.
    NewGame(u32),
}

pub fn boot_choice(meta: Option<SaveMeta>, held_at_boot: Buttons) -> BootChoice {
    let _ = (meta, held_at_boot);
    todo!()
}

/// A combo seen while the World was not capturable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PendingSave {
    pub frames_left: u16,
}

impl PendingSave {
    pub const fn new() -> Self {
        Self {
            frames_left: SAVE_WAIT_FRAMES,
        }
    }

    pub fn tick(&mut self) -> bool {
        todo!()
    }
}
