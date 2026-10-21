//! Debug mirror the GBA build keeps in EWRAM so the studio/emulator can follow the engine.
//! Write order on the GBA: inputs slot, then hash, then frame (frame last = commit).
//! The GBA writer must use `write_volatile` for every field, and readers must sample between
//! frames (`hash` is a u64 written as two 32-bit stores).
//!
//! Display lag: the GBA writes scroll registers at vblank from the *previous* step's
//! `FrameOut`, so the picture visible when the emulator samples after `run_frame` shows engine
//! frame `frame - 1`, not `frame`. Pixel comparisons must use a stable (idle) tail or compare
//! against the native render of `frame - 1`. Harnesses must key on `frame` and the input ring,
//! never on emulator frame counts (boot adds an offset).

pub const MAGIC: [u8; 8] = *b"PKRDBG01";
pub const INPUT_RING: usize = 64;
pub const OFF_FRAME: u32 = 8;
pub const OFF_HASH: u32 = 16;
pub const OFF_INPUTS: u32 = 24;

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
}

const _: () = {
    assert!(core::mem::offset_of!(DebugRegion, frame) == OFF_FRAME as usize);
    assert!(core::mem::offset_of!(DebugRegion, hash) == OFF_HASH as usize);
    assert!(core::mem::offset_of!(DebugRegion, inputs) == OFF_INPUTS as usize);
    assert!(core::mem::size_of::<DebugRegion>() == 152);
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
        assert_eq!(size_of::<DebugRegion>(), 152);
    }

    #[test]
    fn input_slot_for_frame() {
        assert_eq!(input_slot(1), 0);
        assert_eq!(input_slot(64), 63);
        assert_eq!(input_slot(65), 0);
    }
}
