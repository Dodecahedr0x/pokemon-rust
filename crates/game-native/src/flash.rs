//! GBA flash images on the host (Milestone 5 Task E): `agb_save` over a `FLASH_SIZE` buffer,
//! with the layout `game-gba` writes (`engine::checkpoint::{FLASH_MAGIC, FLASH_SLOTS,
//! FLASH_SIZE}`), so images are byte-identical to the ROM's: mGBA's `savedataClone` after the
//! ROM saved `cp` on erased flash equals `encode_sav(cp)`, and the ROM boots from it.

use agb_save::{SaveSlotManager, StorageInfo, StorageMedium};
use core::num::NonZeroUsize;
use engine::checkpoint::{
    Checkpoint, FLASH_MAGIC, FLASH_SECTOR, FLASH_SIZE, FLASH_SLOTS, SaveMeta,
};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

/// 64 KiB flash (Panasonic `0x1B32`, what mGBA answers) as agb's `SaveData` reports it: 4 KiB
/// erase sectors, byte writes. Erase fills with 0xFF; a write can only clear bits, as on the
/// chip (agb_save erases every sector before writing it, so this never changes the bytes).
///
/// Shared, because `SaveSlotManager` does not give its storage back.
#[derive(Clone)]
struct Flash(Rc<RefCell<Vec<u8>>>);

#[derive(Debug)]
struct OutOfBounds;

fn range(offset: usize, len: usize) -> Result<Range<usize>, OutOfBounds> {
    match offset.checked_add(len) {
        Some(end) if end <= FLASH_SIZE => Ok(offset..end),
        _ => Err(OutOfBounds),
    }
}

impl StorageMedium for Flash {
    type Error = OutOfBounds;

    fn info(&self) -> StorageInfo {
        StorageInfo {
            size: FLASH_SIZE,
            erase_size: NonZeroUsize::new(FLASH_SECTOR),
            write_size: NonZeroUsize::MIN,
        }
    }

    fn read(&mut self, offset: usize, buf: &mut [u8]) -> Result<(), OutOfBounds> {
        buf.copy_from_slice(&self.0.borrow()[range(offset, buf.len())?]);
        Ok(())
    }

    fn erase(&mut self, offset: usize, len: usize) -> Result<(), OutOfBounds> {
        assert!(offset.is_multiple_of(FLASH_SECTOR) && len.is_multiple_of(FLASH_SECTOR));
        self.0.borrow_mut()[range(offset, len)?].fill(0xFF);
        Ok(())
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), OutOfBounds> {
        let r = range(offset, data.len())?;
        for (dst, src) in self.0.borrow_mut()[r].iter_mut().zip(data) {
            *dst &= src;
        }
        Ok(())
    }
}

/// `SaveSlotManager::new` over a copy of `sav` (formatting it if the magic differs, as agb does
/// at boot), plus a handle on the bytes.
fn open(sav: &[u8]) -> Result<(SaveSlotManager<Flash, SaveMeta>, Flash), String> {
    if sav.len() != FLASH_SIZE {
        return Err(format!(
            "flash image is {} bytes, expected {FLASH_SIZE}",
            sav.len()
        ));
    }
    let flash = Flash(Rc::new(RefCell::new(sav.to_vec())));
    let slots = SaveSlotManager::new(flash.clone(), FLASH_SLOTS, FLASH_MAGIC)
        .map_err(|e| format!("flash: {e:?}"))?;
    Ok((slots, flash))
}

/// A `FLASH_SIZE`-byte `.sav` image holding `cp` in slot 0: what the ROM leaves on erased flash
/// after booting (agb formats it) and saving `cp` once.
pub fn encode_sav(cp: &Checkpoint) -> Result<Vec<u8>, String> {
    write_sav(&vec![0xFF; FLASH_SIZE], cp)
}

/// `sav` after the ROM booted on it and saved `cp` (`encode_sav` is this on erased flash).
///
/// Byte-identical to a ROM that booted on `sav` and saved once; several saves in one session
/// may place sectors differently, so compare `decode_sav` content, not bytes.
pub fn write_sav(sav: &[u8], cp: &Checkpoint) -> Result<Vec<u8>, String> {
    let (mut slots, flash) = open(sav)?;
    slots
        .write(0, cp, &SaveMeta::of(cp))
        .map_err(|e| format!("flash write: {e:?}"))?;
    drop(slots);
    Ok(flash.0.take())
}

/// Marks a slot written by the in-game save in `SaveMeta::badges`' high bit (Milestone 9 Task
/// S; the GBA shell's `save_logic::IN_GAME_SAVE`, equal by `emu/tests/save_logic.rs`): such a
/// slot boots into the main menu, any other is restored at once.
pub const IN_GAME_SAVE: u8 = 0x80;

/// The slot metadata of an in-game save of `cp` (the badges for the CONTINUE window, marked).
pub fn in_game_meta(cp: &Checkpoint, ids: &engine::data::SpecialIds) -> SaveMeta {
    let m = cp.save_meta(ids);
    SaveMeta {
        badges: m.badges | IN_GAME_SAVE,
        ..m
    }
}

/// `sav` after the ROM booted on it and saved `cp` in game (`special SaveGame`, the start
/// menu's SAVE): [`write_sav`] with [`in_game_meta`]. The window build writes it to
/// `saves/<flavor>/game.sav`.
pub fn write_in_game_sav(
    sav: &[u8],
    cp: &Checkpoint,
    ids: &engine::data::SpecialIds,
) -> Result<Vec<u8>, String> {
    let (mut slots, flash) = open(sav)?;
    slots
        .write(0, cp, &in_game_meta(cp, ids))
        .map_err(|e| format!("flash write: {e:?}"))?;
    drop(slots);
    Ok(flash.0.take())
}

/// An erased (blank) flash image.
pub fn blank_sav() -> Vec<u8> {
    vec![0xFF; FLASH_SIZE]
}

/// Slot 0's metadata and checkpoint.
pub fn decode_sav(sav: &[u8]) -> Result<(SaveMeta, Checkpoint), String> {
    let (mut slots, _) = open(sav)?;
    let meta = *slots
        .metadata(0)
        .ok_or("flash: slot 0 is empty or corrupt")?;
    let cp = slots
        .read::<Checkpoint>(0)
        .map_err(|e| format!("flash read: {e:?}"))?;
    Ok((meta, cp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Buttons;
    use engine::checkpoint::*;
    use engine::data::{Behavior, Dir, FlagId, MapId, MovementType, VarId};

    fn sample(frame: u32) -> Checkpoint {
        let player = ObjectSnap {
            local_id: 0xFF,
            map: MapId(3),
            gfx: None,
            pos: (5, 7),
            prev: (5, 6),
            elevation: 3,
            facing: Dir::Down,
            movement: MovementType::Face(Dir::Down),
            home: (5, 7),
            range: (0, 0),
            visible: true,
            frozen: false,
            facing_locked: false,
            action: None,
            idle: IdleSnap::Done,
            anim: AnimSnap {
                table: AnimTableSnap::Face,
                cmd: 0,
                timer: 0,
            },
        };
        Checkpoint {
            header: Header {
                schema: SCHEMA,
                build: BuildHash {
                    engine: 0x1122_3344_5566_7788,
                    content: 0x99,
                },
                frame,
                world_hash: 0xDEAD_BEEF_0BAD_F00D,
            },
            world: WorldSnapshot {
                frame,
                rng: 0x1234_5678,
                input: InputSnap {
                    held: Buttons::NONE,
                    pressed: Buttons::NONE,
                },
                runner_issued: 4,
                player_name: [0xBB, 0xCC, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                player_id: 0xABCD,
                field: FieldSnap {
                    map: MapId(3),
                    camera: (16, 32),
                    drawn: Some([-1, -1, 16, 11]),
                    ring: (0, 0),
                    overrides: vec![((2, 3), 0x21)],
                    stale: None,
                },
                objects: ObjectsSnap {
                    map: MapId(3),
                    run_state: RunStateSnap::NotMoving,
                    objs: vec![player],
                },
                templates: vec![],
                vars: VarsSnap {
                    flags: vec![FlagId(1), FlagId(0x50)],
                    vars: vec![(VarId(0x4050), 2)],
                },
                fader: FaderSnap {
                    level: 0,
                    target: 0,
                },
                transition: TransitionSnap {
                    map: MapId(3),
                    x: 5,
                    y: 7,
                    facing: Dir::Down,
                    exit_walk: false,
                    player_hidden: false,
                },
                selected: None,
                last_mover: 0,
                delay: 0,
                door: 0,
                party: vec![],
                respawn: 0,
                prev_behavior: Behavior::Normal,
                encounter_immunity: 0,
                bag: Default::default(),
                money: 0,
                str_vars: Default::default(),
                warp_slots: Default::default(),
                party_held: Vec::new(),
                options: None,
                pokedex: Default::default(),
                pc: Default::default(),
                coins: 0,
                registered_item: None,
                berry_trees: Vec::new(),
                map_layout: None,
                sky: None,
                rival_name: None,
                box_names: Vec::new(),
                audio: None,
            },
        }
    }

    #[test]
    fn round_trip_is_one_flash_image_with_matching_meta() {
        let cp = sample(1234);
        let sav = encode_sav(&cp).unwrap();
        assert_eq!(sav.len(), FLASH_SIZE);
        let (meta, back) = decode_sav(&sav).unwrap();
        assert_eq!(back, cp);
        assert_eq!(
            meta,
            SaveMeta {
                schema: SCHEMA,
                build: cp.header.build,
                frame: 1234,
                map: MapId(3),
                name: cp.world.player_name,
                dex_caught: 0,
                badges: 0,
            }
        );
    }

    #[test]
    fn encoding_is_deterministic() {
        assert_eq!(
            encode_sav(&sample(9)).unwrap(),
            encode_sav(&sample(9)).unwrap()
        );
    }

    #[test]
    fn second_write_decodes_to_the_newest() {
        let sav = write_sav(&encode_sav(&sample(10)).unwrap(), &sample(20)).unwrap();
        assert_eq!(sav.len(), FLASH_SIZE);
        let (meta, cp) = decode_sav(&sav).unwrap();
        assert_eq!((meta.frame, cp), (20, sample(20)));
    }

    /// Milestone 9 Step 0: the largest checkpoint the 64 KiB one-slot layout takes, on erased
    /// flash and then for every later save (agb_save writes the new copy before it frees the
    /// old one). `MAX_ENCODED` keeps a 512-byte margin under it.
    #[test]
    fn slot_capacity_holds_max_encoded() {
        // `n` two-byte flags (ids 0x80..0x4000).
        let sized = |n: usize, frame: u32| {
            let mut cp = sample(frame);
            cp.world.vars.flags = (0..n).map(|i| FlagId(0x80 + (i % 0x3F00) as u16)).collect();
            cp
        };
        let fits = |n: usize| {
            let Ok(mut sav) = encode_sav(&sized(n, 1)) else {
                return false;
            };
            for frame in [2, 3] {
                match write_sav(&sav, &sized(n, frame)) {
                    Ok(s) => sav = s,
                    Err(_) => return false,
                }
            }
            decode_sav(&sav).is_ok_and(|(_, cp)| cp == sized(n, 3))
        };
        let (mut lo, mut hi) = (0, FLASH_SIZE / 2);
        assert!(fits(lo));
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let capacity = postcard::to_allocvec(&sized(lo, 3)).unwrap().len();
        std::println!("64 KiB one-slot capacity: {capacity} bytes (MAX_ENCODED {MAX_ENCODED})");
        assert!(MAX_ENCODED + 512 <= capacity);
    }

    #[test]
    fn three_max_size_writes_fit() {
        let big = |frame: u32| {
            let mut cp = sample(frame);
            // Two-byte flags (ids 0x80..0x4000; a two-byte length) up to MAX_ENCODED - 16 or a
            // byte more.
            cp.world.vars.flags.clear();
            let base = postcard::to_allocvec(&cp).unwrap().len() + 1;
            let n = (MAX_ENCODED - 16 - base).div_ceil(2);
            cp.world.vars.flags = (0..n).map(|i| FlagId(0x80 + (i % 0x3F00) as u16)).collect();
            cp
        };
        let size = postcard::to_allocvec(&big(1)).unwrap().len();
        assert!((MAX_ENCODED - 16..=MAX_ENCODED).contains(&size), "{size}");
        let mut sav = encode_sav(&big(1)).unwrap();
        for frame in [2, 3] {
            sav = write_sav(&sav, &big(frame)).expect("no OutOfSpace");
        }
        let (meta, cp) = decode_sav(&sav).unwrap();
        assert_eq!((meta.frame, cp), (3, big(3)));
    }

    #[test]
    fn erased_flash_is_an_error() {
        assert!(decode_sav(&vec![0xFF; FLASH_SIZE]).is_err());
    }

    #[test]
    fn wrong_size_is_an_error() {
        let sav = encode_sav(&sample(1)).unwrap();
        assert!(decode_sav(&sav[..FLASH_SIZE / 2]).is_err());
        assert!(decode_sav(&[sav.clone(), sav].concat()).is_err());
    }

    /// Flips one byte of the checkpoint's postcard bytes wherever agb_save stored them.
    fn corrupt_data(sav: &mut [u8], cp: &Checkpoint) {
        let bytes = postcard::to_allocvec(cp).unwrap();
        let at = sav
            .windows(bytes.len())
            .position(|w| w == bytes)
            .expect("data block not found in the image");
        sav[at + bytes.len() / 2] ^= 0x5A;
    }

    #[test]
    fn corrupt_single_write_is_an_error() {
        let cp = sample(10);
        let mut sav = encode_sav(&cp).unwrap();
        corrupt_data(&mut sav, &cp);
        assert!(decode_sav(&sav).is_err());
    }

    #[test]
    fn corrupt_newest_write_is_an_error() {
        // agb_save checks the data CRC at read time, after `new` picked the newest valid
        // header; it does not fall back to the ghost (previous) copy for a bad data block.
        let mut sav = write_sav(&encode_sav(&sample(10)).unwrap(), &sample(20)).unwrap();
        corrupt_data(&mut sav, &sample(20));
        assert!(decode_sav(&sav).is_err());
    }

    #[test]
    fn foreign_magic_is_an_error() {
        let cp = sample(5);
        let mut sav = encode_sav(&cp).unwrap();
        let at = sav
            .windows(FLASH_MAGIC.len())
            .position(|w| w == FLASH_MAGIC)
            .expect("magic not found");
        sav[at] ^= 1;
        assert!(decode_sav(&sav).is_err());
    }
}
