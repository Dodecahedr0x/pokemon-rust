//! Embedded mGBA running the real ROM, plus access to the engine debug region.

use anyhow::{Context, Result, anyhow};
use engine::Buttons;
use engine::debug::{INPUT_RING, MAGIC, OFF_FRAME, OFF_HASH, OFF_INPUTS};
use std::path::Path;

const EWRAM_START: u32 = 0x0200_0000;
const EWRAM_SIZE: u32 = 256 * 1024;

pub fn xbgr8_to_rgb555(px: u32) -> u16 {
    let r = (px & 0xFF) >> 3;
    let g = ((px >> 8) & 0xFF) >> 3;
    let b = ((px >> 16) & 0xFF) >> 3;
    (r | g << 5 | b << 10) as u16
}

#[derive(Clone, Debug)]
pub struct DebugSnapshot {
    pub frame: u32,
    pub hash: u64,
    pub inputs: [u16; INPUT_RING],
}

pub struct Emu {
    core: mgba::Core,
    debug_base: Option<u32>,
}

impl Emu {
    pub fn open(rom: &Path) -> Result<Self> {
        let mut core = mgba::Core::new().map_err(|e| anyhow!("mgba core: {e:?}"))?;
        core.load_rom(rom)
            .map_err(|e| anyhow!("load {}: {e:?}", rom.display()))?;
        core.reset().map_err(|e| anyhow!("reset: {e:?}"))?;
        Ok(Self {
            core,
            debug_base: None,
        })
    }

    pub fn run_frame(&mut self, keys: Buttons) -> Result<()> {
        self.core
            .set_keys(keys.0 as u32)
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
            .step_by(4)
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
        let mut inputs = [0u16; INPUT_RING];
        for (i, slot) in inputs.iter_mut().enumerate() {
            *slot = self.read16(base + OFF_INPUTS + 2 * i as u32);
        }
        Some(DebugSnapshot {
            frame,
            hash,
            inputs,
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
        Err(anyhow!("debug region not found after {max_frames} frames"))
            .context("is the ROM a pokemon-rust build?")
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
}
