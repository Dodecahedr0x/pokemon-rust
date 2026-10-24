//! Pure save decisions of the GBA shell (no agb imports): the debug save combo, the boot
//! choice and the pending-save timeout. Host-tested by `crates/emu/tests/save_logic.rs`,
//! which includes this file through `#[path]`.

use engine::Buttons;
use engine::checkpoint::{Checkpoint, SaveMeta, schema_readable};
use engine::data::SpecialIds;
use engine::debug::boot;

/// Marks a slot written by the in-game save (Milestone 9 Task S) in `SaveMeta::badges`' high
/// bit (badges never exceed 8): such a slot boots into the main menu. A slot written by the
/// debug combo or the tools (`SaveMeta::of`) restores at once, as before (the parity lanes
/// boot their checkpoints that way).
pub const IN_GAME_SAVE: u8 = 0x80;

/// The metadata of an in-game save: the badges for the CONTINUE window, and the mark.
pub fn in_game_meta(cp: &Checkpoint, ids: &SpecialIds) -> SaveMeta {
    let m = cp.save_meta(ids);
    SaveMeta {
        badges: m.badges | IN_GAME_SAVE,
        ..m
    }
}

/// The CONTINUE window's metadata (the mark dropped).
pub fn shown_meta(m: SaveMeta) -> SaveMeta {
    SaveMeta {
        badges: m.badges & !IN_GAME_SAVE,
        ..m
    }
}

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
    /// The main menu (CONTINUE, NEW GAME, OPTION) over a valid in-game save (Milestone 9 Task
    /// S, [`IN_GAME_SAVE`]).
    MainMenu,
}

/// What agb reports for slot 0 at boot. Blank or foreign flash is reformatted by agb at init
/// and shows up as `Empty`; `Corrupted` is a slot whose data and ghost copy both fail checks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotSeen {
    Empty,
    Valid(SaveMeta),
    Corrupted,
}

/// B held at boot skips any save; then an empty slot is a new game, a corrupted one is
/// reported, and another schema is refused before anything is decoded. An in-game save opens
/// the main menu; any other valid slot is restored at once.
pub fn boot_choice(slot: SlotSeen, held_at_boot: Buttons) -> BootChoice {
    if held_at_boot.contains(Buttons::B) {
        return BootChoice::NewGame(boot::SKIPPED);
    }
    match slot {
        SlotSeen::Empty => BootChoice::NewGame(boot::NEW_GAME),
        SlotSeen::Corrupted => BootChoice::NewGame(boot::CORRUPT),
        SlotSeen::Valid(m) if !schema_readable(m.schema) => BootChoice::NewGame(boot::REJECTED),
        SlotSeen::Valid(m) if m.badges & IN_GAME_SAVE != 0 => BootChoice::MainMenu,
        SlotSeen::Valid(_) => BootChoice::Restore,
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
