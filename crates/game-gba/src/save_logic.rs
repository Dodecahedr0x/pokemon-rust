//! Pure save decisions of the GBA shell (no agb imports): the debug save combo, the boot
//! choice and the pending-save timeout. Host-tested by `crates/emu/tests/save_logic.rs`,
//! which includes this file through `#[path]`.

use engine::Buttons;
use engine::checkpoint::{SCHEMA, SaveMeta};
use engine::debug::boot;

/// Frames a combo waits for a capturable World before giving up (`debug::save::TIMEOUT`).
pub const SAVE_WAIT_FRAMES: u16 = 600;

/// L and R held, and SELECT newly pressed.
pub fn combo_pressed(prev: Buttons, cur: Buttons) -> bool {
    const LR: Buttons = Buttons(Buttons::L.0 | Buttons::R.0);
    cur.contains(LR) && cur.contains(Buttons::SELECT) && !prev.contains(Buttons::SELECT)
}

/// What the shell boots into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BootChoice {
    /// Read slot 0 and `World::restore` it.
    Restore,
    /// `World::new`, with the `debug::boot` code saying why.
    NewGame(u32),
}

/// B held at boot skips any save; then no slot is a new game, and another schema is refused
/// before anything is decoded.
pub fn boot_choice(meta: Option<SaveMeta>, held_at_boot: Buttons) -> BootChoice {
    if held_at_boot.contains(Buttons::B) {
        return BootChoice::NewGame(boot::SKIPPED);
    }
    match meta {
        None => BootChoice::NewGame(boot::NEW_GAME),
        Some(m) if m.schema != SCHEMA => BootChoice::NewGame(boot::REJECTED),
        Some(_) => BootChoice::Restore,
    }
}

/// A combo seen while the World was not capturable: the shell retries after every step.
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

    /// One more frame without a capturable World; `true` once the wait is over.
    pub fn tick(&mut self) -> bool {
        self.frames_left = self.frames_left.saturating_sub(1);
        self.frames_left == 0
    }
}

impl Default for PendingSave {
    fn default() -> Self {
        Self::new()
    }
}

/// Scanlines in a frame (`VCOUNT` runs 0..228).
pub const LINES_PER_FRAME: u16 = 228;

/// Scanlines from `VCOUNT == start` to `VCOUNT == end`, mod one frame (a span of a whole frame
/// or more wraps; `late` reports those).
pub const fn scanlines(start: u16, end: u16) -> u32 {
    ((end + LINES_PER_FRAME - start) % LINES_PER_FRAME) as u32
}
