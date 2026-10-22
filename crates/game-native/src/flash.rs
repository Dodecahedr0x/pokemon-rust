//! GBA flash images on the host (Milestone 5 Task E): `agb_save` over a 128 KiB buffer, with the
//! layout `game-gba` writes (`engine::checkpoint::{FLASH_MAGIC, FLASH_SLOTS, FLASH_SIZE}`), so
//! images are byte-identical to the ROM's.

use engine::checkpoint::{Checkpoint, SaveMeta};

/// A `FLASH_SIZE`-byte `.sav` image holding `cp` in slot 0.
pub fn encode_sav(cp: &Checkpoint) -> Result<Vec<u8>, String> {
    let _ = cp;
    todo!("Milestone 5 Task E")
}

/// Slot 0's metadata and checkpoint.
pub fn decode_sav(sav: &[u8]) -> Result<(SaveMeta, Checkpoint), String> {
    let _ = sav;
    todo!("Milestone 5 Task E")
}
