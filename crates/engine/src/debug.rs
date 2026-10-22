//! Debug mirror the GBA build keeps in EWRAM so the studio/emulator can follow the engine.
//! Write order on the GBA: inputs slot, then hash, then late, then frame (frame last = commit).
//! The GBA writer must use `write_volatile` for every field, and readers must sample between
//! frames (`hash` is a u64 written as two 32-bit stores).
//!
//! Display lag: the GBA applies the *previous* step's `video::Frame` at vblank, so the picture
//! visible when the emulator samples after `run_frame` shows engine frame `frame - 1`, not
//! `frame`. Pixel comparisons must use a stable (idle) tail or compare against the native render
//! of `frame - 1`. Harnesses must key on `frame` and the input ring,
//! never on emulator frame counts (boot adds an offset).

pub const MAGIC: [u8; 8] = *b"PKRDBG02";
pub const INPUT_RING: usize = 64;
pub const OFF_FRAME: u32 = 8;
pub const OFF_HASH: u32 = 16;
pub const OFF_INPUTS: u32 = 24;
pub const OFF_LATE: u32 = 152;
pub const OFF_LATE_BLACK: u32 = 156;
pub const OFF_BUILD_ENGINE: u32 = 160;
pub const OFF_BUILD_CONTENT: u32 = 168;
pub const OFF_BOOT: u32 = 176;
pub const OFF_SAVES: u32 = 180;
pub const OFF_SAVE_STATUS: u32 = 184;
pub const OFF_HASH_LINES_MAX: u32 = 188;
pub const OFF_STEP_LINES_MAX: u32 = 192;
/// `size_of::<DebugRegion>()`.
pub const SIZE: usize = 200;

/// How the ROM's World started (`DebugRegion::boot`).
pub mod boot {
    /// No save: `World::new`.
    pub const NEW_GAME: u32 = 0;
    /// `World::restore` from the flash checkpoint.
    pub const RESTORED: u32 = 1;
    /// B held at boot: the save was ignored.
    pub const SKIPPED: u32 = 2;
    /// The save has another schema, or `World::restore` refused it.
    pub const REJECTED: u32 = 3;
    /// The save could not be read or decoded.
    pub const CORRUPT: u32 = 4;
    /// No flash chip answered.
    pub const NO_MEDIA: u32 = 5;
}

/// Result of the last flash write (`DebugRegion::save_status`).
pub mod save {
    /// No write attempted.
    pub const NONE: u32 = 0;
    pub const OK: u32 = 1;
    /// The world did not become capturable in time.
    pub const TIMEOUT: u32 = 2;
    /// The encoded checkpoint exceeds `checkpoint::MAX_ENCODED`.
    pub const TOO_LARGE: u32 = 3;
    /// The flash write failed.
    pub const STORAGE: u32 = 4;
}

#[repr(C)]
pub struct DebugRegion {
    pub magic: [u8; 8],
    /// Number of engine steps completed.
    pub frame: u32,
    pub _pad: u32,
    /// `World::hash()` after step `frame`.
    pub hash: u64,
    /// Input consumed by step `f` lives at `input_slot(f)`.
    pub inputs: [u16; INPUT_RING],
    /// Number of frames the GBA shell applied late: the previous step overran into the next
    /// frame (vblank catch-up), or applying a `Frame` ran past vblank, while the screen was not
    /// fully black (see `late_black`). Pixels shown while late may differ from native; the
    /// parity harness requires 0.
    pub late: u32,
    /// Late applies that changed nothing visible: the screen was fully black before and after
    /// (map loads upload whole tilesets while faded out). Informational, not a parity error.
    pub late_black: u32,
    // ---- v2 (Milestone 5) ----
    /// Engine half of `checkpoint::BuildHash`.
    pub build_engine: u64,
    /// Content half of `checkpoint::BuildHash`.
    pub build_content: u64,
    /// How the World started: a [`boot`] code.
    pub boot: u32,
    /// Successful flash writes.
    pub saves: u32,
    /// The last flash write: a [`save`] code.
    pub save_status: u32,
    /// Worst scanlines of `World::hash()` on a visible frame.
    pub hash_lines_max: u32,
    /// Worst scanlines from the apply to the end of publish on a visible frame.
    pub step_lines_max: u32,
}

const _: () = {
    assert!(core::mem::offset_of!(DebugRegion, frame) == OFF_FRAME as usize);
    assert!(core::mem::offset_of!(DebugRegion, hash) == OFF_HASH as usize);
    assert!(core::mem::offset_of!(DebugRegion, inputs) == OFF_INPUTS as usize);
    assert!(core::mem::offset_of!(DebugRegion, late) == OFF_LATE as usize);
    assert!(core::mem::offset_of!(DebugRegion, late_black) == OFF_LATE_BLACK as usize);
    assert!(core::mem::offset_of!(DebugRegion, build_engine) == OFF_BUILD_ENGINE as usize);
    assert!(core::mem::offset_of!(DebugRegion, build_content) == OFF_BUILD_CONTENT as usize);
    assert!(core::mem::offset_of!(DebugRegion, boot) == OFF_BOOT as usize);
    assert!(core::mem::offset_of!(DebugRegion, saves) == OFF_SAVES as usize);
    assert!(core::mem::offset_of!(DebugRegion, save_status) == OFF_SAVE_STATUS as usize);
    assert!(core::mem::offset_of!(DebugRegion, hash_lines_max) == OFF_HASH_LINES_MAX as usize);
    assert!(core::mem::offset_of!(DebugRegion, step_lines_max) == OFF_STEP_LINES_MAX as usize);
    assert!(core::mem::size_of::<DebugRegion>() == SIZE);
    // emu scans EWRAM in 8-byte steps.
    assert!(core::mem::align_of::<DebugRegion>() == 8);
};

#[allow(clippy::new_without_default)]
impl DebugRegion {
    pub const fn new() -> Self {
        Self {
            magic: MAGIC,
            frame: 0,
            _pad: 0,
            hash: 0,
            inputs: [0; INPUT_RING],
            late: 0,
            late_black: 0,
            build_engine: 0,
            build_content: 0,
            boot: boot::NEW_GAME,
            saves: 0,
            save_status: save::NONE,
            hash_lines_max: 0,
            step_lines_max: 0,
        }
    }
}

/// Ring index for the input consumed by the step that produced engine frame `frame` (>= 1).
pub const fn input_slot(frame: u32) -> usize {
    ((frame.wrapping_sub(1)) as usize) % INPUT_RING
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    #[test]
    fn layout_is_stable() {
        assert_eq!(offset_of!(DebugRegion, magic), 0);
        assert_eq!(offset_of!(DebugRegion, frame), OFF_FRAME as usize);
        assert_eq!(offset_of!(DebugRegion, hash), OFF_HASH as usize);
        assert_eq!(offset_of!(DebugRegion, inputs), OFF_INPUTS as usize);
        assert_eq!(offset_of!(DebugRegion, late), OFF_LATE as usize);
        assert_eq!(offset_of!(DebugRegion, late_black), OFF_LATE_BLACK as usize);
        assert_eq!(offset_of!(DebugRegion, build_engine), 160);
        assert_eq!(offset_of!(DebugRegion, build_content), 168);
        assert_eq!(offset_of!(DebugRegion, boot), 176);
        assert_eq!(offset_of!(DebugRegion, saves), 180);
        assert_eq!(offset_of!(DebugRegion, save_status), 184);
        assert_eq!(offset_of!(DebugRegion, hash_lines_max), 188);
        assert_eq!(offset_of!(DebugRegion, step_lines_max), 192);
        assert_eq!(size_of::<DebugRegion>(), 200);
        assert_eq!(core::mem::align_of::<DebugRegion>(), 8);
        assert_eq!(&DebugRegion::new().magic, b"PKRDBG02");
    }

    #[test]
    fn input_slot_for_frame() {
        assert_eq!(input_slot(1), 0);
        assert_eq!(input_slot(64), 63);
        assert_eq!(input_slot(65), 0);
    }
}
