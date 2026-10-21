//! Embedded mGBA running the real ROM, plus access to the engine debug region.

use anyhow::{Context, Result, anyhow};
use engine::Buttons;
use engine::debug::{INPUT_RING, MAGIC, OFF_FRAME, OFF_HASH, OFF_INPUTS, OFF_LATE};
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
}

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
    /// (or reads state from) the directory the ROM lives in.
    pub fn open(rom: &Path) -> Result<Self> {
        let scratch = ScratchDir::new()?;
        let copy = scratch.0.join("game.gba");
        std::fs::copy(rom, &copy).with_context(|| format!("copy {}", rom.display()))?;
        let mut core = mgba::Core::new().map_err(|e| anyhow!("mgba core: {e:?}"))?;
        core.load_rom(&copy)
            .map_err(|e| anyhow!("load {}: {e:?}", rom.display()))?;
        core.reset().map_err(|e| anyhow!("reset: {e:?}"))?;
        Ok(Self {
            core,
            debug_base: None,
            _scratch: scratch,
        })
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
        let hash =
            self.read32(base + OFF_HASH) as u64 | (self.read32(base + OFF_HASH + 4) as u64) << 32;
        let late = self.read32(base + OFF_LATE);
        let mut inputs = [0u16; INPUT_RING];
        for (i, slot) in inputs.iter_mut().enumerate() {
            *slot = self.read16(base + OFF_INPUTS + 2 * i as u32);
        }
        Some(DebugSnapshot {
            frame,
            hash,
            inputs,
            late,
        })
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

    #[test]
    fn open_does_not_create_sav_next_to_rom() {
        let dir = ScratchDir::new().unwrap();
        let rom = dir.0.join("x.gba");
        // Just enough header for mGBA's GBAIsROM (0xEA at 3, 0x96 at 0xB2), so the
        // load succeeds and reaches mCoreAutoloadSave. With an all-zero file the
        // load fails first and this test would pass vacuously.
        let mut bytes = [0u8; 512];
        bytes[3] = 0xEA;
        bytes[0xB2] = 0x96;
        std::fs::write(&rom, bytes).unwrap();
        let emu = Emu::open(&rom).expect("dummy header should load");
        assert!(!dir.0.join("x.sav").exists());
        let scratch = emu._scratch.0.clone();
        drop(emu);
        assert!(!scratch.exists(), "scratch dir not removed on drop");
    }
}
