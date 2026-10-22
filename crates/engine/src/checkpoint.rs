//! Checkpoints: versioned serde snapshots of a capturable [`crate::World`].
//!
//! The snapshot types (`WorldSnapshot` and the per-module `*Snap`) are explicit DTOs, separate
//! from the live structs, so the on-disk schema is visible, versioned (`SCHEMA`) and immune to
//! internal refactors. A snapshot holds only stable ids (Gen3 map, flag, var, graphics, species
//! and move numbers), never a content-assigned index, so it survives content rebuilds.
//!
//! Postcard is the binary form (GBA flash, [`encode`], [`state_digest`]); the native shell also
//! writes the same types as RON.

use crate::data::{Behavior, Dir, FlagId, GameData, GfxId, MapId, MovementType, VarId};
use crate::input::Buttons;
use crate::mon::Pokemon;
use crate::script::Speed;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Snapshot schema. Bump on any change to a DTO below (the golden-bytes test fails otherwise).
pub const SCHEMA: u16 = 1;
/// Largest postcard checkpoint the GBA flash slot accepts (route max is reported by the tests).
pub const MAX_ENCODED: usize = 8 * 1024;
/// agb_save layout, shared by game-gba and game-native::flash (byte-identical images).
pub const FLASH_MAGIC: [u8; 32] = *b"pokemon-rust checkpoint flash v1";
pub const FLASH_SLOTS: usize = 1;
/// 64 KiB flash (`init_flash_64k`): what mGBA presents to a homebrew ROM (FLASH512, Panasonic
/// id; it ignores agb's `FLASH1M_V` marker and the `.sav` size), so a host-built `.sav` boots
/// in a stock mGBA. agb lays the slot out by the chip it detects, so images are byte-identical
/// only on a chip of this size.
pub const FLASH_SIZE: usize = 64 * 1024;
pub const FLASH_SECTOR: usize = 4096;
/// Gen3 `PLAYER_NAME_LENGTH` (7) + EOS; 0xFF-terminated and 0xFF-padded.
pub const PLAYER_NAME_SIZE: usize = 8;
pub type PlayerName = [u8; PLAYER_NAME_SIZE];

// agb_save keeps a ghost copy of the slot, and erases whole sectors.
const _: () =
    assert!(FLASH_SIZE / FLASH_SECTOR * FLASH_SECTOR == FLASH_SIZE && MAX_ENCODED < FLASH_SIZE / 2);

/// Engine sources hash (build.rs, Task C; Wave 0: 0) and content build id (`GameData::build_id`).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct BuildHash {
    pub engine: u64,
    pub content: u64,
}

pub const ENGINE_SRC_HASH: u64 = include!(concat!(env!("OUT_DIR"), "/engine_src_hash.rs"));

/// The identity of this build: the engine sources and the content build.
pub fn build_hash(game: &impl GameData) -> BuildHash {
    BuildHash {
        engine: ENGINE_SRC_HASH,
        content: game.build_id(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Header {
    /// First field: `decode` reads it alone before anything else.
    pub schema: u16,
    /// The capturing build (informational across builds).
    pub build: BuildHash,
    /// `World::frame` at capture.
    pub frame: u32,
    /// `World::hash` at capture; restore checks it when builds are equal.
    pub world_hash: u64,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub header: Header,
    pub world: WorldSnapshot,
}

/// agb_save slot metadata (readable without loading the slot).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SaveMeta {
    pub schema: u16,
    pub build: BuildHash,
    pub frame: u32,
    pub map: MapId,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub frame: u32,
    /// Rng state (never 0).
    pub rng: u32,
    pub input: InputSnap,
    /// `Runner::issued` (hashed, survives script end).
    pub runner_issued: u32,
    pub player_name: PlayerName,
    pub player_id: u32,
    pub field: FieldSnap,
    pub objects: ObjectsSnap,
    /// Only templates differing from the current map's data.
    pub templates: Vec<TemplateDelta>,
    pub vars: VarsSnap,
    pub fader: FaderSnap,
    /// Idle stage implied; the leftover fields are hashed.
    pub transition: TransitionSnap,
    pub selected: Option<u8>,
    pub last_mover: u8,
    pub delay: u16,
    pub door: u8,
    pub party: Vec<Pokemon>,
    pub respawn: u8,
    pub prev_behavior: Behavior,
    pub encounter_immunity: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct InputSnap {
    pub held: Buttons,
    pub pressed: Buttons,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FieldSnap {
    pub map: MapId,
    pub camera: (i32, i32),
    /// `[x0, y0, x1, y1]`.
    pub drawn: Option<[i16; 4]>,
    pub ring: (i16, i16),
    /// Ascending.
    pub overrides: Vec<((i16, i16), u16)>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ObjectsSnap {
    pub map: MapId,
    pub run_state: RunStateSnap,
    /// `[0]` = player.
    pub objs: Vec<ObjectSnap>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RunStateSnap {
    NotMoving,
    TurnDirection,
    Moving,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ObjectSnap {
    pub local_id: u8,
    pub map: MapId,
    pub gfx: Option<GfxId>,
    pub pos: (i16, i16),
    pub prev: (i16, i16),
    pub elevation: u8,
    pub facing: Dir,
    pub movement: MovementType,
    pub home: (i16, i16),
    pub range: (u8, u8),
    pub visible: bool,
    pub frozen: bool,
    pub facing_locked: bool,
    /// A wander step in progress is fine; scripted `moves` are not.
    pub action: Option<ActionSnap>,
    pub idle: IdleSnap,
    pub anim: AnimSnap,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ActionSnap {
    pub kind: ActionKindSnap,
    pub dir: Dir,
    pub t: u8,
    pub off: (i16, i16),
    pub jump_y: i8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ActionKindSnap {
    Walk(Speed),
    Run,
    InPlace(u8),
    Jump(u8),
    Delay(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum IdleSnap {
    Start,
    Wait(u16),
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AnimSnap {
    pub table: AnimTableSnap,
    pub cmd: u8,
    pub timer: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AnimTableSnap {
    Face,
    Go(u8),
    Run,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TemplateDelta {
    pub local_id: u8,
    pub pos: Option<(i16, i16)>,
    pub movement: Option<MovementType>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct VarsSnap {
    /// Set flags, ascending.
    pub flags: Vec<FlagId>,
    /// Non-zero vars, ascending.
    pub vars: Vec<(VarId, u16)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FaderSnap {
    pub level: u8,
    pub target: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TransitionSnap {
    pub map: MapId,
    pub x: i16,
    pub y: i16,
    pub facing: Dir,
    pub exit_walk: bool,
    pub player_hidden: bool,
}

/// Why `World::capturable` refused (checked in this order).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotCapturable {
    Starter,
    Battle,
    Transition,
    Script,
    TextBox,
    Fade,
    /// The local id of an object with a live movement script.
    Moving(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestoreError {
    Schema(u16),
    UnknownMap(MapId),
    UnknownTemplate { map: MapId, local_id: u8 },
    NoPlayer,
    BadRng,
    PartyTooLarge,
    HashMismatch { want: u64, got: u64 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecodeError {
    Postcard,
    Schema { found: u16 },
    TooLarge(usize),
}

/// Postcard bytes of `cp` (Task C).
pub fn encode(cp: &Checkpoint) -> Vec<u8> {
    let _ = cp;
    todo!("Milestone 5 Task C")
}

/// [`encode`], `TooLarge` past [`MAX_ENCODED`].
pub fn encode_checked(cp: &Checkpoint) -> Result<Vec<u8>, DecodeError> {
    let _ = cp;
    todo!("Milestone 5 Task C")
}

/// Reads the leading u16 schema first, then the whole checkpoint.
pub fn decode(bytes: &[u8]) -> Result<Checkpoint, DecodeError> {
    let _ = bytes;
    todo!("Milestone 5 Task C")
}

/// Semantic identity of a snapshot: Fnv64 over its postcard bytes (the tree's "content hash").
pub fn state_digest(w: &WorldSnapshot) -> u64 {
    let _ = w;
    todo!("Milestone 5 Task C")
}

#[cfg(test)]
mod wave0_tests {
    use crate::script::Runner;
    use crate::text::TextBox;

    #[test]
    fn restored_runner_is_idle_and_keeps_issued() {
        let r = Runner::restore_idle(7);
        assert!(!r.is_running());
        assert!(r.active().is_none());
        assert_eq!(r.issued(), 7);
    }

    #[test]
    fn new_text_box_is_pristine() {
        assert!(TextBox::new().is_pristine());
    }
}
