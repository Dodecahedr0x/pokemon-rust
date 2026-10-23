//! Embedded mGBA running the real ROM, plus access to the engine debug region.

use anyhow::{Context, Result, anyhow};
use engine::Buttons;
use engine::checkpoint::{BuildHash, FLASH_SIZE};
use engine::data::{MapId, ScriptId};
use engine::debug::{
    INPUT_RING, MAGIC, NO_SCRIPT, OFF_BOOT, OFF_BUILD_CONTENT, OFF_BUILD_ENGINE,
    OFF_EVENTS_DROPPED, OFF_FRAME, OFF_HASH, OFF_HASH_LINES_MAX, OFF_HEAP_MAX, OFF_INPUTS,
    OFF_LAST_SCRIPT, OFF_LATE, OFF_LATE_BLACK, OFF_LATE_SAVING, OFF_LOOP_LINES_MAX, OFF_MAP,
    OFF_PANICKED, OFF_REPORTS, OFF_SAVE_STATUS, OFF_SAVES, OFF_SCAN_PASSES, OFF_SCREENS,
    OFF_STACK_MIN, OFF_STEP_LINES_MAX, SIZE,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const EWRAM_START: u32 = 0x0200_0000;
const EWRAM_SIZE: u32 = 256 * 1024;

pub fn xbgr8_to_rgb555(px: u32) -> u16 {
    let r = (px & 0xFF) >> 3;
    let g = ((px >> 8) & 0xFF) >> 3;
    let b = ((px >> 16) & 0xFF) >> 3;
    (r | g << 5 | b << 10) as u16
}

/// A copy of the ROM's debug region.
///
/// Only consistent when sampled between `run_frame` calls: mGBA returns at the
/// start of vblank, while the ROM is halted after publishing the frame's state.
/// A ROM that overruns a frame could leave `hash` belonging to `frame + 1`.
#[derive(Clone, Debug)]
pub struct DebugSnapshot {
    pub frame: u32,
    pub hash: u64,
    pub inputs: [u16; INPUT_RING],
    /// Frames the ROM applied late (see `engine::debug::DebugRegion::late`).
    pub late: u32,
    /// Late applies while the screen stayed black (`DebugRegion::late_black`).
    pub late_black: u32,
    /// The ROM's `checkpoint::build_hash` (zero until the ROM publishes it at boot).
    pub build: BuildHash,
    /// How the ROM's World started: an `engine::debug::boot` code.
    pub boot: u32,
    /// Successful flash writes.
    pub saves: u32,
    /// The last flash write: an `engine::debug::save` code.
    pub save_status: u32,
    /// Worst scanlines of `World::hash()` on a visible frame.
    pub hash_lines_max: u32,
    /// Worst scanlines from the apply to the end of publish on a visible frame.
    pub step_lines_max: u32,
    /// Worst scanlines of the whole shell loop body on a visible frame (apply, step, publish,
    /// save tail; a flash write excluded), out of the 228-line frame.
    pub loop_lines_max: u32,
    /// Heap high-water in bytes (`DebugRegion::heap_max`).
    pub heap_max: u32,
    /// The last script started (`World::last_script`).
    pub last_script: Option<ScriptId>,
    /// The current map (`World::map`).
    pub map: MapId,
    /// `Report` events since boot (saturating).
    pub reports: u16,
    /// `World::events_dropped` (saturating).
    pub events_dropped: u16,
    /// IWRAM stack bytes never touched (`DebugRegion::stack_min`).
    pub stack_min: u16,
    /// The panic location's line once the ROM panicked, else 0.
    pub panicked: u16,
    /// Complete scan passes of the ROM's memory watch (`DebugRegion::scan_passes`, wrapping).
    pub scan_passes: u32,
    /// Frames over budget while the shell wrote an in-game save (Milestone 9; not `late`).
    pub late_saving: u32,
    /// Screens open (`World::screens_open`) at the last step (Milestone 9).
    pub screens: u16,
}

/// A zeroed region (frame 0, no script, map 0), for tests that build snapshots.
impl Default for DebugSnapshot {
    fn default() -> Self {
        DebugSnapshot {
            frame: 0,
            hash: 0,
            inputs: [0; INPUT_RING],
            late: 0,
            late_black: 0,
            build: BuildHash::default(),
            boot: 0,
            saves: 0,
            save_status: 0,
            hash_lines_max: 0,
            step_lines_max: 0,
            loop_lines_max: 0,
            heap_max: 0,
            last_script: None,
            map: MapId(0),
            reports: 0,
            events_dropped: 0,
            stack_min: 0,
            panicked: 0,
            scan_passes: 0,
            late_saving: 0,
            screens: 0,
        }
    }
}

// `Emu::debug` reads the v4 fields at engine's offsets: inside the region, the u64s on 8-byte
// boundaries (read as two 32-bit halves), the u32s on 4-byte and the u16s on 2-byte ones.
const _: () = {
    assert!(SIZE == 224 && MAGIC[7] == b'4');
    assert!(OFF_HEAP_MAX == 200 && OFF_LAST_SCRIPT == 204 && OFF_MAP == 206);
    assert!(OFF_REPORTS == 208 && OFF_EVENTS_DROPPED == 210 && OFF_STACK_MIN == 212);
    assert!(OFF_PANICKED == 214 && OFF_LATE_SAVING == 216 && OFF_SCREENS == 220);
    assert!(OFF_SCREENS as usize + 4 == SIZE);
    assert!(OFF_FRAME == 8 && OFF_SCAN_PASSES == 12);
    assert!(OFF_BUILD_ENGINE == 160 && OFF_BUILD_CONTENT == 168);
    assert!(OFF_BOOT == 176 && OFF_SAVES == 180 && OFF_SAVE_STATUS == 184);
    assert!(OFF_HASH_LINES_MAX == 188 && OFF_STEP_LINES_MAX == 192);
    assert!(OFF_LOOP_LINES_MAX == 196 && OFF_LOOP_LINES_MAX as usize + 4 <= SIZE);
    assert!(OFF_HASH.is_multiple_of(8) && OFF_BUILD_ENGINE.is_multiple_of(8));
    assert!(OFF_BUILD_CONTENT.is_multiple_of(8));
};

/// Gate on [`DebugSnapshot::heap_max`] (Milestone 8 decision 7): 160 KiB of the 256 KiB of
/// EWRAM, the rest being the statics and headroom.
pub const HEAP_MAX: u32 = 160 << 10;
/// Gate on [`DebugSnapshot::stack_min`]: at least this much of the IWRAM stack never touched.
pub const STACK_UNTOUCHED_MIN: u16 = 1 << 10;

/// The memory gates every parity lane and the map tour apply to the ROM's debug region at the
/// end of a run: [`HEAP_MAX`], [`STACK_UNTOUCHED_MIN`], no dropped events and no panic. Prints
/// the values.
pub fn check_memory(what: &str, end: &DebugSnapshot) -> Result<()> {
    println!(
        "{what}: heap_max {} B (gate {HEAP_MAX}), stack untouched {} B (gate \
         {STACK_UNTOUCHED_MIN}), reports {}, events dropped {}",
        end.heap_max, end.stack_min, end.reports, end.events_dropped
    );
    if end.panicked != 0 {
        return Err(anyhow!("{what}: the ROM panicked (line {})", end.panicked));
    }
    if end.heap_max > HEAP_MAX {
        return Err(anyhow!(
            "{what}: heap high-water {} B over {HEAP_MAX}",
            end.heap_max
        ));
    }
    if end.stack_min < STACK_UNTOUCHED_MIN {
        return Err(anyhow!(
            "{what}: only {} B of stack never touched (gate {STACK_UNTOUCHED_MIN})",
            end.stack_min
        ));
    }
    if end.events_dropped != 0 {
        return Err(anyhow!("{what}: {} events dropped", end.events_dropped));
    }
    Ok(())
}

unsafe extern "C" {
    /// libc's: frees the buffer `mCore::savedataClone` `malloc`ed.
    fn free(ptr: *mut core::ffi::c_void);
}

/// `struct GBACartridgeOverride` (`mgba/gba/interface.h`), not in `mgba-sys`'s bindings.
#[repr(C)]
struct GbaCartridgeOverride {
    id: [core::ffi::c_char; 4],
    savetype: mgba_sys::GBASavedataType,
    hardware: core::ffi::c_int,
    idle_loop: u32,
    vba_bug_compat: bool,
}
/// `HW_NO_OVERRIDE`: keep the cartridge's detected hardware.
const HW_NO_OVERRIDE: core::ffi::c_int = 0x8000;
/// `GBA_IDLE_LOOP_NONE`.
const IDLE_LOOP_NONE: u32 = 0xFFFF_FFFF;

// The C layout: char[4], enum (int), int, uint32_t, bool + padding. The override below is
// FLASH512, the 64 KiB chip `FLASH_SIZE` describes.
const _: () = {
    assert!(core::mem::size_of::<mgba_sys::GBASavedataType>() == 4);
    assert!(core::mem::size_of::<GbaCartridgeOverride>() == 20);
    assert!(FLASH_SIZE == 64 * 1024);
};

/// The RTC time of a retail cartridge ([`Emu::open_retail`]): 2005-01-01 00:00:00 UTC, in ms.
pub const RETAIL_RTC_MS: i64 = 1_104_537_600_000;

/// Private scratch directory removed on drop (errors ignored).
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("pkr-emu-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        Ok(Self(dir))
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct Emu {
    // Field order matters: `core` drops (closing its save file) before `_scratch`.
    core: mgba::Core,
    debug_base: Option<u32>,
    _scratch: ScratchDir,
}

impl Emu {
    /// Loads a private copy of `rom`, so mGBA's `.sav` autoload never touches
    /// (or reads state from) the directory the ROM lives in. The flash starts erased.
    pub fn open(rom: &Path) -> Result<Self> {
        Self::open_with_save(rom, None)
    }

    /// [`Emu::open`] with `sav` (a `FLASH_SIZE` image, see `game_native::flash`) as the
    /// cartridge's flash: it is written to `game.sav` next to the private ROM copy, where
    /// `mCoreAutoloadSave` picks it up.
    ///
    /// The save type is set to 64 KiB flash (FLASH512, Panasonic id), the `FLASH_SIZE` chip.
    /// That is what libmgba picks by itself for a homebrew ROM on its first flash command (it
    /// reads neither agb's `FLASH*_V` markers nor the `.sav` size), so the harness runs what a
    /// stock mGBA runs; setting it up front only makes [`Emu::save_data`] `FLASH_SIZE` bytes
    /// before the ROM touches the flash.
    pub fn open_with_save(rom: &Path, sav: Option<&[u8]>) -> Result<Self> {
        Self::open_inner(rom, sav, true)
    }

    /// Opens a retail cartridge (the vanilla oracle's BPEE): like [`Emu::open`], but the save
    /// type and the hardware come from libmgba's own override table for the game code (BPEE:
    /// 128 KiB flash and an RTC), not our FLASH512 override, and the RTC is pinned to
    /// [`RETAIL_RTC_MS`] so runs do not depend on the host clock.
    pub fn open_retail(rom: &Path) -> Result<Self> {
        Self::open_inner(rom, None, false)
    }

    fn open_inner(rom: &Path, sav: Option<&[u8]>, homebrew: bool) -> Result<Self> {
        if let Some(sav) = sav
            && sav.len() != FLASH_SIZE
        {
            return Err(anyhow!(
                "save image is {} bytes, expected {FLASH_SIZE}",
                sav.len()
            ));
        }
        let scratch = ScratchDir::new()?;
        let copy = scratch.0.join("game.gba");
        std::fs::copy(rom, &copy).with_context(|| format!("copy {}", rom.display()))?;
        if let Some(sav) = sav {
            let path = scratch.0.join("game.sav");
            std::fs::write(&path, sav).with_context(|| format!("write {}", path.display()))?;
        }
        let mut core = mgba::Core::new().map_err(|e| anyhow!("mgba core: {e:?}"))?;
        core.load_rom(&copy)
            .map_err(|e| anyhow!("load {}: {e:?}", rom.display()))?;
        let flash = GbaCartridgeOverride {
            id: [0; 4],
            savetype: mgba_sys::GBA_SAVEDATA_FLASH512,
            hardware: HW_NO_OVERRIDE,
            idle_loop: IDLE_LOOP_NONE,
            // libmgba's value when no override applies (`GBACreate`).
            vba_bug_compat: false,
        };
        // SAFETY: the core is initialised and loaded; setOverride copies the struct, and the
        // reset below applies it (`_GBACoreReset` -> `GBAOverrideApply`). The RTC source is
        // the core's generic one, read on every RTC access.
        unsafe {
            let raw = core.raw_ptr();
            if homebrew {
                let set = (*raw)
                    .setOverride
                    .ok_or_else(|| anyhow!("no setOverride"))?;
                set(raw, (&raw const flash).cast());
            } else {
                (*raw).rtc.override_ = mgba_sys::RTC_FIXED;
                (*raw).rtc.value = RETAIL_RTC_MS;
            }
        }
        core.reset().map_err(|e| anyhow!("reset: {e:?}"))?;
        Ok(Self {
            core,
            debug_base: None,
            _scratch: scratch,
        })
    }

    /// A copy of the cartridge's save memory (`FLASH_SIZE` bytes of flash).
    pub fn save_data(&mut self) -> Result<Vec<u8>> {
        // SAFETY: savedataClone mallocs `len` bytes (or sets NULL and returns 0); they are
        // copied out, then freed with the same allocator.
        unsafe {
            let raw = self.core.raw_ptr();
            let clone = (*raw)
                .savedataClone
                .ok_or_else(|| anyhow!("no savedataClone"))?;
            let mut buf: *mut core::ffi::c_void = core::ptr::null_mut();
            let len = clone(raw, &mut buf);
            if buf.is_null() {
                return Err(anyhow!("savedataClone returned no save data"));
            }
            let out = core::slice::from_raw_parts(buf as *const u8, len).to_vec();
            free(buf);
            Ok(out)
        }
    }

    /// mGBA's whole-machine save state (`mCore::saveState`): a vanilla-oracle position to
    /// replay branches from ([`Emu::load_state`]).
    pub fn save_state(&mut self) -> Result<Vec<u8>> {
        // SAFETY: the core is loaded; `stateSize` bytes are what `saveState` writes.
        unsafe {
            let raw = self.core.raw_ptr();
            let size = (*raw).stateSize.ok_or_else(|| anyhow!("no stateSize"))?(raw);
            let save = (*raw).saveState.ok_or_else(|| anyhow!("no saveState"))?;
            let mut buf = vec![0u8; size];
            if !save(raw, buf.as_mut_ptr().cast()) {
                return Err(anyhow!("saveState failed"));
            }
            Ok(buf)
        }
    }

    /// Restores a [`Emu::save_state`] of the same core.
    pub fn load_state(&mut self, state: &[u8]) -> Result<()> {
        // SAFETY: as `save_state`; the size is checked first.
        unsafe {
            let raw = self.core.raw_ptr();
            let size = (*raw).stateSize.ok_or_else(|| anyhow!("no stateSize"))?(raw);
            if state.len() != size {
                return Err(anyhow!("state is {} bytes, not {size}", state.len()));
            }
            let load = (*raw).loadState.ok_or_else(|| anyhow!("no loadState"))?;
            if !load(raw, state.as_ptr().cast()) {
                return Err(anyhow!("loadState failed"));
            }
            Ok(())
        }
    }

    /// Skips drawing `n` frames out of `n + 1` (mGBA's `frameskip` option: the scanline
    /// renderer is not run, the emulation is unchanged). The vanilla oracle reads only RAM.
    pub fn set_frameskip(&mut self, n: i32) {
        // SAFETY: the core is initialised; `reloadConfigOption` with no option name copies
        // `opts` (frameskip among them) into the board (`_GBACoreReloadConfigOption`).
        unsafe {
            let raw = self.core.raw_ptr();
            (*raw).opts.frameskip = n;
            if let Some(reload) = (*raw).reloadConfigOption {
                reload(raw, core::ptr::null(), core::ptr::null());
            }
        }
    }

    pub fn run_frame(&mut self, keys: Buttons) -> Result<()> {
        self.core
            .set_keys(Buttons::from_raw(keys.0).0 as u32)
            .map_err(|e| anyhow!("set_keys: {e:?}"))?;
        self.core
            .run_frame()
            .map_err(|e| anyhow!("run_frame: {e:?}"))
    }

    pub fn framebuffer_rgb555(&self) -> Vec<u16> {
        self.core
            .video_buffer()
            .iter()
            .map(|&p| xbgr8_to_rgb555(p))
            .collect()
    }

    /// [`Emu::framebuffer_rgb555`] into `out` without allocating; `out` must hold the screen
    /// (`SCREEN_W * SCREEN_H` pixels); extra entries are left untouched.
    pub fn framebuffer_into(&self, out: &mut [u16]) {
        for (o, &p) in out.iter_mut().zip(self.core.video_buffer()) {
            *o = xbgr8_to_rgb555(p);
        }
    }

    /// `len` bytes of the GBA's address space from `addr`, through libmgba's `rawRead8` (a
    /// view of the memory with no side effects: no open-bus or I/O register reads, no wait
    /// states). Meant for RAM and ROM; sample between `run_frame` calls.
    pub fn read_bytes(&mut self, addr: u32, len: usize) -> Vec<u8> {
        let mut out = vec![0; len];
        self.read_into(addr, &mut out);
        out
    }

    /// [`Emu::read_bytes`] into `out`.
    pub fn read_into(&mut self, addr: u32, out: &mut [u8]) {
        // SAFETY: core is initialised; rawRead8 is a pure memory view (segment -1: current).
        unsafe {
            let raw = self.core.raw_ptr();
            let read = (*raw).rawRead8.expect("rawRead8");
            for (i, b) in out.iter_mut().enumerate() {
                *b = read(raw, addr.wrapping_add(i as u32), -1) as u8;
            }
        }
    }

    /// Writes `bytes` at `addr` (`rawWrite8`, the current segment): the vanilla oracle's
    /// stand-ins for what it cannot play (`vanilla::Vanilla::teach`).
    pub fn write_bytes(&mut self, addr: u32, bytes: &[u8]) {
        // SAFETY: core is initialised; rawWrite8 writes the bus without side effects.
        unsafe {
            let raw = self.core.raw_ptr();
            let write = (*raw).rawWrite8.expect("rawWrite8");
            for (i, &b) in bytes.iter().enumerate() {
                write(raw, addr.wrapping_add(i as u32), -1, b);
            }
        }
    }

    pub fn read_u8(&mut self, addr: u32) -> u8 {
        let mut b = [0; 1];
        self.read_into(addr, &mut b);
        b[0]
    }

    /// A little-endian u16 at `addr` (any alignment).
    pub fn read_u16(&mut self, addr: u32) -> u16 {
        let mut b = [0; 2];
        self.read_into(addr, &mut b);
        u16::from_le_bytes(b)
    }

    /// A little-endian u32 at `addr` (any alignment).
    pub fn read_u32(&mut self, addr: u32) -> u32 {
        let mut b = [0; 4];
        self.read_into(addr, &mut b);
        u32::from_le_bytes(b)
    }

    fn read16(&mut self, addr: u32) -> u16 {
        // SAFETY: core is initialised and loaded; busRead16 is a pure memory read.
        // libmgba declares busRead16 as returning uint32_t; only the low half is meaningful.
        unsafe {
            let raw = self.core.raw_ptr();
            ((*raw).busRead16.expect("busRead16"))(raw, addr) as u16
        }
    }

    fn read32(&mut self, addr: u32) -> u32 {
        // SAFETY: as above.
        unsafe {
            let raw = self.core.raw_ptr();
            ((*raw).busRead32.expect("busRead32"))(raw, addr)
        }
    }

    /// A little-endian u64 stored as two 32-bit words.
    fn read64(&mut self, addr: u32) -> u64 {
        self.read32(addr) as u64 | (self.read32(addr + 4) as u64) << 32
    }

    fn find_debug_base(&mut self) -> Option<u32> {
        let m0 = u32::from_le_bytes(MAGIC[0..4].try_into().unwrap());
        let m1 = u32::from_le_bytes(MAGIC[4..8].try_into().unwrap());
        (EWRAM_START..EWRAM_START + EWRAM_SIZE)
            .step_by(8) // DebugRegion is 8-byte aligned.
            .find(|&a| self.read32(a) == m0 && self.read32(a + 4) == m1)
    }

    /// `None` until the ROM has initialised its debug region.
    pub fn debug(&mut self) -> Option<DebugSnapshot> {
        if self.debug_base.is_none() {
            self.debug_base = self.find_debug_base();
        }
        let base = self.debug_base?;
        let frame = self.read32(base + OFF_FRAME);
        let hash = self.read64(base + OFF_HASH);
        let late = self.read32(base + OFF_LATE);
        let late_black = self.read32(base + OFF_LATE_BLACK);
        let mut inputs = [0u16; INPUT_RING];
        for (i, slot) in inputs.iter_mut().enumerate() {
            *slot = self.read16(base + OFF_INPUTS + 2 * i as u32);
        }
        Some(DebugSnapshot {
            frame,
            hash,
            inputs,
            late,
            late_black,
            build: BuildHash {
                engine: self.read64(base + OFF_BUILD_ENGINE),
                content: self.read64(base + OFF_BUILD_CONTENT),
            },
            boot: self.read32(base + OFF_BOOT),
            saves: self.read32(base + OFF_SAVES),
            save_status: self.read32(base + OFF_SAVE_STATUS),
            hash_lines_max: self.read32(base + OFF_HASH_LINES_MAX),
            step_lines_max: self.read32(base + OFF_STEP_LINES_MAX),
            loop_lines_max: self.read32(base + OFF_LOOP_LINES_MAX),
            heap_max: self.read32(base + OFF_HEAP_MAX),
            last_script: match self.read16(base + OFF_LAST_SCRIPT) {
                NO_SCRIPT => None,
                id => Some(ScriptId(id)),
            },
            map: MapId(self.read16(base + OFF_MAP)),
            reports: self.read16(base + OFF_REPORTS),
            events_dropped: self.read16(base + OFF_EVENTS_DROPPED),
            stack_min: self.read16(base + OFF_STACK_MIN),
            panicked: self.read16(base + OFF_PANICKED),
            scan_passes: self.read32(base + OFF_SCAN_PASSES),
            late_saving: self.read32(base + OFF_LATE_SAVING),
            screens: self.read16(base + OFF_SCREENS),
        })
    }

    /// Runs frames without input until the ROM's memory watch completed two scan passes after
    /// `from` (the pass under way at `from` may have started before its last step; the next one
    /// starts after it), within `max_frames` emulator frames. Returns the region then: its
    /// `heap_max` and `stack_min` include everything written up to `from`'s step. The ROM
    /// keeps stepping its World meanwhile (idle).
    pub fn settle(&mut self, from: &DebugSnapshot, max_frames: u32) -> Result<DebugSnapshot> {
        for _ in 0..max_frames {
            self.run_frame(Buttons::NONE)?;
            let s = self
                .debug()
                .ok_or_else(|| anyhow!("debug region vanished"))?;
            if s.panicked != 0 {
                return Err(anyhow!(
                    "ROM panicked (line {}) while settling, engine frame {}",
                    s.panicked,
                    s.frame
                ));
            }
            if s.scan_passes.wrapping_sub(from.scan_passes) >= 2 {
                return Ok(s);
            }
        }
        Err(anyhow!(
            "the ROM's memory watch did not complete two passes in {max_frames} frames"
        ))
    }

    /// Boot until the debug region is visible.
    pub fn boot(&mut self, max_frames: u32) -> Result<DebugSnapshot> {
        for _ in 0..max_frames {
            self.run_frame(Buttons::NONE)?;
            if let Some(s) = self.debug() {
                return Ok(s);
            }
        }
        Err(anyhow!(
            "debug region not found after {max_frames} frames (is the ROM a pokemon-rust build?)"
        ))
    }

    /// Runs frames without input until the debug region is visible and the ROM has published
    /// the World it booted (the region's hash is a placeholder 0 until then; [`run_rom_from`]
    /// waits the same way), within `max_frames` emulator frames. Returns the region at that
    /// point and the emulator frames run.
    pub fn boot_published(&mut self, max_frames: u32) -> Result<(DebugSnapshot, u32)> {
        for ran in 1..=max_frames {
            self.run_frame(Buttons::NONE)?;
            if let Some(s) = self.debug()
                && s.hash != 0
            {
                return Ok((s, ran));
            }
        }
        Err(anyhow!(
            "ROM never published its boot state within {max_frames} frames (is it a pokemon-rust build?)"
        ))
    }
}

/// What the ROM showed when the emulator sampled engine frame `frame`.
pub struct Capture {
    pub frame: u32,
    pub fb: Vec<u16>,
}

/// A scripted ROM run (see [`run_rom_from`]).
pub struct RomRun {
    /// The engine frame the ROM booted at: 0 for a new game, the checkpoint's frame for a
    /// restored save.
    pub start: u32,
    /// Input consumed by step `start + i + 1`.
    pub consumed: Vec<Buttons>,
    /// (engine frame, World hash) at every sample after `start`, including picture frames.
    pub hashes: Vec<(u32, u64)>,
    pub captures: Vec<Capture>,
    /// Emulator frames the run took after boot.
    pub emu_frames: u32,
    /// `DebugSnapshot::late_black` at the end.
    pub late_black: u32,
    /// The engine frame read after each `late_black` increment (one entry per increment).
    pub late_black_frames: Vec<u32>,
    /// The debug region once the ROM published its boot state (frame `start`, its hash, and
    /// the `build` and `boot` fields).
    pub boot: DebugSnapshot,
    /// The debug region at the end (`saves`, `save_status`, the scanline maxima), with the
    /// memory marks (`heap_max`, `stack_min`, `scan_passes`) of [`Emu::settle`] after it.
    pub end: DebugSnapshot,
    /// The cartridge flash at the end ([`Emu::save_data`]).
    pub save: Vec<u8>,
}

/// [`run_rom_from`] on erased flash: a new game, which must boot at frame 0.
pub fn run_rom(
    rom: &Path,
    keys: impl Fn(u32) -> Buttons,
    total: u32,
    picture_frames: &[u32],
) -> Result<RomRun> {
    run_rom_from(rom, None, Some(0), keys, total, picture_frames)
}

/// Emulator frames [`Emu::settle`] may take at the end of a run (a pass takes a few frames).
pub const SETTLE_FRAMES: u32 = 600;

/// Emulator frames [`run_rom_from`] waits for the ROM to publish its boot state.
const BOOT_FRAMES: u32 = 120;

/// Boots the ROM on `sav` (erased flash if `None`, see [`Emu::open_with_save`]) and runs it
/// until engine frame `total`, feeding `keys(f)` while the ROM is at engine frame `f` (so step
/// `f + 1` consumes it). The run starts at the ROM's first published frame (`RomRun::start`).
/// Records the hash at every sample and the picture when the ROM is at each of
/// `picture_frames` (the picture of step `frame - 1`, see `engine::debug`), which must lie in
/// `start + 1..=total`.
///
/// Fails if `expect_start` is given and the ROM booted at another frame (a restore run passes
/// the checkpoint's frame), if the ROM steps past a picture frame between two samples (two engine steps in one
/// emulator frame), reports a late frame apply (`DebugRegion::late`) or a panic, runs more steps than the
/// input ring holds between reads, or stops advancing.
pub fn run_rom_from(
    rom: &Path,
    sav: Option<&[u8]>,
    expect_start: Option<u32>,
    keys: impl Fn(u32) -> Buttons,
    total: u32,
    picture_frames: &[u32],
) -> Result<RomRun> {
    if !picture_frames.windows(2).all(|w| w[0] < w[1]) {
        return Err(anyhow!("picture frames must ascend"));
    }
    let mut emu = Emu::open_with_save(rom, sav)?;
    let mut snap = emu.boot(BOOT_FRAMES)?;
    // The region is visible from the ROM's first instructions (its static initialiser), with a
    // placeholder hash of 0 until the shell publishes the World it booted (new or restored).
    // The shell publishes before its first step, so no input is consumed while waiting.
    let mut waited = 0;
    while snap.hash == 0 {
        waited += 1;
        if waited > BOOT_FRAMES {
            return Err(anyhow!("ROM never published its boot state"));
        }
        emu.run_frame(Buttons::NONE)?;
        snap = emu
            .debug()
            .ok_or_else(|| anyhow!("debug region vanished"))?;
    }
    let boot = snap.clone();
    let start = snap.frame;
    if let Some(want) = expect_start
        && start != want
    {
        return Err(anyhow!(
            "ROM booted at frame {start} (boot code {}), expected {want}",
            snap.boot
        ));
    }
    if total < start {
        return Err(anyhow!("ROM booted at frame {start}, past {total}"));
    }
    if !picture_frames
        .iter()
        .all(|&c| (start + 1..=total).contains(&c))
    {
        return Err(anyhow!(
            "picture frames must lie in {}..={total}",
            start + 1
        ));
    }
    let mut consumed: Vec<Buttons> = Vec::new();
    let mut hashes = Vec::new();
    let mut captures = Vec::new();
    let mut next = picture_frames.iter().peekable();
    let mut emu_frames = 0;
    let mut late_black_frames = Vec::new();
    while snap.frame < total {
        emu.run_frame(keys(snap.frame))?;
        snap = emu
            .debug()
            .ok_or_else(|| anyhow!("debug region vanished"))?;
        let have = start + consumed.len() as u32;
        if snap.frame < have {
            return Err(anyhow!("ROM went back from frame {have} to {}", snap.frame));
        }
        if snap.frame - have > INPUT_RING as u32 {
            return Err(anyhow!("ROM ran >{INPUT_RING} steps between reads"));
        }
        for f in have + 1..=snap.frame {
            consumed.push(Buttons(snap.inputs[engine::debug::input_slot(f)]));
        }
        if snap.panicked != 0 {
            return Err(anyhow!(
                "ROM panicked (line {}) by engine frame {}, map {:#06x}, last script {:?}",
                snap.panicked,
                snap.frame,
                snap.map.0,
                snap.last_script
            ));
        }
        let counted = late_black_frames.len() as u32;
        late_black_frames.extend((counted..snap.late_black).map(|_| snap.frame));
        if snap.late != 0 {
            return Err(anyhow!(
                "ROM applied a frame late by engine frame {}",
                snap.frame
            ));
        }
        if snap.frame > have {
            hashes.push((snap.frame, snap.hash));
        }
        if let Some(&&c) = next.peek() {
            if snap.frame > c {
                return Err(anyhow!(
                    "ROM jumped from frame {have} to {} past picture frame {c}",
                    snap.frame
                ));
            }
            if snap.frame == c {
                captures.push(Capture {
                    frame: c,
                    fb: emu.framebuffer_rgb555(),
                });
                next.next();
            }
        }
        emu_frames += 1;
        if emu_frames > (total - start) * 3 {
            return Err(anyhow!("ROM stopped advancing at frame {}", snap.frame));
        }
    }
    let save = emu.save_data()?;
    // The marks once the watch scanned everything the last step wrote.
    let settled = emu.settle(&snap, SETTLE_FRAMES)?;
    snap.heap_max = settled.heap_max;
    snap.stack_min = settled.stack_min;
    snap.scan_passes = settled.scan_passes;
    Ok(RomRun {
        start,
        consumed,
        hashes,
        captures,
        emu_frames,
        late_black: snap.late_black,
        late_black_frames,
        boot,
        save,
        end: snap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xbgr8_to_rgb555_keeps_top_five_bits() {
        // mGBA expands 5->8 bits; the top 5 bits are always the original channel.
        let r5 = 0x1F_u32;
        let g5 = 0x0A_u32;
        let b5 = 0x03_u32;
        let px = ((r5 * 0x21) >> 2) | (((g5 * 0x21) >> 2) << 8) | (((b5 * 0x21) >> 2) << 16);
        assert_eq!(xbgr8_to_rgb555(px), (r5 | g5 << 5 | b5 << 10) as u16);
        assert_eq!(
            xbgr8_to_rgb555(r5 << 3 | (g5 << 3) << 8 | (b5 << 3) << 16),
            (r5 | g5 << 5 | b5 << 10) as u16
        );
    }

    #[test]
    fn open_missing_rom_errors() {
        assert!(Emu::open(Path::new("/nonexistent/definitely-not-a-rom.gba")).is_err());
    }

    /// A ROM with just enough header for mGBA's GBAIsROM (0xEA at 3, 0x96 at 0xB2), so the
    /// load succeeds and reaches mCoreAutoloadSave. With an all-zero file the load fails
    /// first and the tests below would pass vacuously.
    fn dummy_rom(dir: &ScratchDir) -> PathBuf {
        let rom = dir.0.join("x.gba");
        let mut bytes = [0u8; 512];
        bytes[3] = 0xEA;
        bytes[0xB2] = 0x96;
        std::fs::write(&rom, bytes).unwrap();
        rom
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn open_does_not_create_sav_next_to_rom() {
        let dir = ScratchDir::new().unwrap();
        let rom = dummy_rom(&dir);
        let emu = Emu::open(&rom).expect("dummy header should load");
        assert_eq!(listing(&dir.0), ["x.gba"]);
        let scratch = emu._scratch.0.clone();
        drop(emu);
        assert!(!scratch.exists(), "scratch dir not removed on drop");
    }

    #[test]
    fn open_with_save_injects_game_sav_into_scratch_only() {
        let dir = ScratchDir::new().unwrap();
        let rom = dummy_rom(&dir);
        let sav: Vec<u8> = (0..FLASH_SIZE).map(|i| (i * 7 + i / 4096) as u8).collect();
        let mut emu = Emu::open_with_save(&rom, Some(&sav)).expect("dummy header should load");
        assert_eq!(listing(&dir.0), ["x.gba"]);
        assert_eq!(listing(&emu._scratch.0), ["game.gba", "game.sav"]);
        // The whole image is loaded and read back.
        assert!(
            emu.save_data().unwrap() == sav,
            "save data differs from the injected image"
        );
    }

    #[test]
    fn reads_see_the_cartridge_little_endian() {
        let dir = ScratchDir::new().unwrap();
        let mut emu = Emu::open(&dummy_rom(&dir)).unwrap();
        const ROM: u32 = 0x0800_0000;
        assert_eq!(emu.read_bytes(ROM, 4), [0, 0, 0, 0xEA]);
        assert_eq!(emu.read_u8(ROM + 0xB2), 0x96);
        assert_eq!(emu.read_u32(ROM), 0xEA00_0000);
        assert_eq!(emu.read_u16(ROM + 2), 0xEA00);
        // Unaligned, across the header byte at 0xB2.
        assert_eq!(emu.read_u16(ROM + 0xB1), 0x9600);
    }

    #[test]
    fn retail_open_pins_the_rtc() {
        let dir = ScratchDir::new().unwrap();
        let mut emu = Emu::open_retail(&dummy_rom(&dir)).unwrap();
        // SAFETY: reading a plain field of the initialised core.
        let rtc = unsafe { (*emu.core.raw_ptr()).rtc.value };
        assert_eq!(rtc, RETAIL_RTC_MS);
    }

    #[test]
    fn fresh_save_is_erased_flash() {
        let dir = ScratchDir::new().unwrap();
        let mut emu = Emu::open(&dummy_rom(&dir)).unwrap();
        let data = emu.save_data().unwrap();
        assert_eq!(data.len(), FLASH_SIZE);
        assert!(data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn open_with_wrong_size_save_errors() {
        let dir = ScratchDir::new().unwrap();
        let rom = dummy_rom(&dir);
        assert!(Emu::open_with_save(&rom, Some(&vec![0xFF; FLASH_SIZE + 1])).is_err());
    }
}
