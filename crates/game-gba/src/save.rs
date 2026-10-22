//! Flash checkpoint of the GBA shell: one agb_save slot on 64 KiB flash, laid out exactly as
//! `game-native::flash` builds host images (`engine::checkpoint::FLASH_*`). 64 KiB because
//! mGBA autodetects FLASH512 and ignores agb's `FLASH1M_V` marker, so 128 KiB images read back
//! corrupted. Decisions live in `save_logic` (host-tested); this module only talks to agb.

use agb::save::{SaveSlotManager, Slot};
use engine::checkpoint::{
    Checkpoint, DecodeError, FLASH_MAGIC, FLASH_SLOTS, SaveMeta, encode_checked,
};
use engine::debug::{boot, save};
use engine::{Buttons, GameData, World};

use crate::save_logic::{BootChoice, SlotSeen, boot_choice};

pub type Slots = SaveSlotManager<SaveMeta>;

/// Declares 64 KiB flash and opens its slots. agb reformats blank or foreign flash here, which
/// then boots as an empty slot (`debug::boot::NEW_GAME`). `None` only when that fails (no
/// chip, or the format did not stick): the game runs without saves (`NO_MEDIA`). Timer 2
/// bounds every flash operation.
pub fn init(gba: &mut agb::Gba) -> Option<Slots> {
    let timer = gba.timers.timers().timer2;
    gba.save
        .init_flash_64k(FLASH_SLOTS, FLASH_MAGIC, Some(timer))
        .ok()
}

/// The World to boot and its `debug::boot` code: slot 0 restored, or a new game saying why
/// not. Without a valid slot this never reaches `World::restore`.
pub fn boot_world(
    game: &impl GameData,
    slots: Option<&mut Slots>,
    held_at_boot: Buttons,
) -> (World, u32) {
    let new_game = |code| (World::new(game), code);
    let Some(slots) = slots else {
        return new_game(boot::NO_MEDIA);
    };
    let seen = match slots.slot(0) {
        Slot::Empty => SlotSeen::Empty,
        Slot::Valid(meta) => SlotSeen::Valid(*meta),
        Slot::Corrupted => SlotSeen::Corrupted,
    };
    match boot_choice(seen, held_at_boot) {
        BootChoice::NewGame(code) => new_game(code),
        BootChoice::Restore => match slots.read::<Checkpoint>(0) {
            Err(_) => new_game(boot::CORRUPT),
            Ok(cp) => match World::restore(game, &cp) {
                Ok(world) => (world, boot::RESTORED),
                Err(_) => new_game(boot::REJECTED),
            },
        },
    }
}

/// Writes `cp` to slot 0 (crash-safe through agb's ghost slot). Returns a `debug::save` code.
pub fn write(slots: &mut Slots, cp: &Checkpoint) -> u32 {
    // agb postcard-encodes `cp` itself; this pass only enforces the flash budget.
    // TODO: use `engine::checkpoint::encoded_len` (Task C) instead of encoding twice.
    match encode_checked(cp) {
        Ok(_) => {}
        Err(DecodeError::TooLarge(_)) => return save::TOO_LARGE,
        Err(_) => return save::STORAGE,
    }
    let meta = SaveMeta {
        schema: cp.header.schema,
        build: cp.header.build,
        frame: cp.header.frame,
        map: cp.world.field.map,
    };
    match slots.write(0, cp, &meta) {
        Ok(()) => save::OK,
        Err(_) => save::STORAGE,
    }
}
