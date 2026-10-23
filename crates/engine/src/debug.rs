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

pub const MAGIC: [u8; 8] = *b"PKRDBG04";
pub const INPUT_RING: usize = 64;
pub const OFF_FRAME: u32 = 8;
pub const OFF_SCAN_PASSES: u32 = 12;
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
pub const OFF_LOOP_LINES_MAX: u32 = 196;
pub const OFF_HEAP_MAX: u32 = 200;
pub const OFF_LAST_SCRIPT: u32 = 204;
pub const OFF_MAP: u32 = 206;
pub const OFF_REPORTS: u32 = 208;
pub const OFF_EVENTS_DROPPED: u32 = 210;
pub const OFF_STACK_MIN: u32 = 212;
pub const OFF_PANICKED: u32 = 214;
pub const OFF_LATE_SAVING: u32 = 216;
pub const OFF_SCREENS: u32 = 220;
/// `size_of::<DebugRegion>()`.
pub const SIZE: usize = 224;
/// [`DebugRegion::last_script`] before any script started.
pub const NO_SCRIPT: u16 = 0xFFFF;

/// How the ROM's World started (`DebugRegion::boot`).
pub mod boot {
    /// No save (an empty slot; blank or foreign flash is reformatted at init and lands here):
    /// `World::new`.
    pub const NEW_GAME: u32 = 0;
    /// `World::restore` from the flash checkpoint.
    pub const RESTORED: u32 = 1;
    /// B held at boot: the save was ignored.
    pub const SKIPPED: u32 = 2;
    /// The save has another schema, or `World::restore` refused it.
    pub const REJECTED: u32 = 3;
    /// The slot (and agb's ghost copy) failed its checks, or could not be read or decoded.
    pub const CORRUPT: u32 = 4;
    /// Flash init failed: no chip answered, or agb could not format it.
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
    /// v3 (Milestone 8 Task M; v2's padding): complete scan passes of the memory watch (a
    /// stack pass then a heap pass, see `heap_max` and `stack_min`), wrapping. Marks read
    /// after this grew twice since a step include everything that step wrote.
    pub scan_passes: u32,
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
    /// Worst scanlines of `World::hash()` on a visible frame (the applied frame not black).
    ///
    /// All three `*_lines_max` are measured with `VCOUNT`, mod one frame (228 lines): the
    /// maxima are meaningful only when `late == 0`.
    pub hash_lines_max: u32,
    /// Worst scanlines from the apply to the end of publish on a visible frame.
    pub step_lines_max: u32,
    /// Worst scanlines of the whole shell loop body (apply, step, publish, save tail; a flash
    /// write excluded) on a visible frame, from the vblank wake to the next wait.
    pub loop_lines_max: u32,
    // ---- v3 (Milestone 8 Task M) ----
    /// High-water of the heap in bytes, from the heap start (agb's `__ewram_data_end`) to the
    /// highest EWRAM word ever written. The shell paints the free heap at boot and scans it in
    /// each frame's slack (agb owns the `#[global_allocator]`, so it cannot be wrapped).
    pub heap_max: u32,
    /// `World::last_script()`, [`NO_SCRIPT`] for none.
    pub last_script: u16,
    /// `World::map()`.
    pub map: u16,
    /// `Event::Report`s seen since boot (saturates at `u16::MAX`).
    pub reports: u16,
    /// `World::events_dropped()` (saturates at `u16::MAX`).
    pub events_dropped: u16,
    /// IWRAM stack bytes never touched since boot: from agb's IWRAM heap tip at boot (the end
    /// of the IWRAM statics) to the lowest word written. The shell paints the free stack at boot and scans it in each frame's
    /// slack.
    pub stack_min: u16,
    /// The line of the panic location (saturates at `u16::MAX`) once the ROM panicked, else 0.
    pub panicked: u16,
    // ---- v4 (Milestone 9 Wave 0) ----
    /// Late applies while the shell wrote the in-game save to flash (`Event::Saved`): flash
    /// time stalls the shell, not the engine, so they are counted apart from `late` and not
    /// gated (decision 6).
    pub late_saving: u32,
    /// `World::screens_open()`: screens open after step `frame`.
    pub screens: u16,
    /// Padding to the region's 8-byte alignment.
    pub _pad: u16,
}

const _: () = {
    assert!(core::mem::offset_of!(DebugRegion, frame) == OFF_FRAME as usize);
    assert!(core::mem::offset_of!(DebugRegion, scan_passes) == OFF_SCAN_PASSES as usize);
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
    assert!(core::mem::offset_of!(DebugRegion, loop_lines_max) == OFF_LOOP_LINES_MAX as usize);
    assert!(core::mem::offset_of!(DebugRegion, heap_max) == OFF_HEAP_MAX as usize);
    assert!(core::mem::offset_of!(DebugRegion, last_script) == OFF_LAST_SCRIPT as usize);
    assert!(core::mem::offset_of!(DebugRegion, map) == OFF_MAP as usize);
    assert!(core::mem::offset_of!(DebugRegion, reports) == OFF_REPORTS as usize);
    assert!(core::mem::offset_of!(DebugRegion, events_dropped) == OFF_EVENTS_DROPPED as usize);
    assert!(core::mem::offset_of!(DebugRegion, stack_min) == OFF_STACK_MIN as usize);
    assert!(core::mem::offset_of!(DebugRegion, panicked) == OFF_PANICKED as usize);
    assert!(core::mem::offset_of!(DebugRegion, late_saving) == OFF_LATE_SAVING as usize);
    assert!(core::mem::offset_of!(DebugRegion, screens) == OFF_SCREENS as usize);
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
            scan_passes: 0,
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
            loop_lines_max: 0,
            heap_max: 0,
            last_script: NO_SCRIPT,
            map: 0,
            reports: 0,
            events_dropped: 0,
            stack_min: 0,
            panicked: 0,
            late_saving: 0,
            screens: 0,
            _pad: 0,
        }
    }
}

/// Ring index for the input consumed by the step that produced engine frame `frame` (>= 1).
pub const fn input_slot(frame: u32) -> usize {
    ((frame.wrapping_sub(1)) as usize) % INPUT_RING
}

#[cfg(test)]
mod tests;
