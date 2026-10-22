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
use crate::hash::Fnv64;
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

/// Engine sources hash (`build.rs`) and content build id (`GameData::build_id`).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct BuildHash {
    pub engine: u64,
    pub content: u64,
}

/// FNV-1a-64 of the non-test engine sources (`build.rs`, `build_support.rs`); never 0.
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
    /// Permanently the first field of `Header`, and so of `Checkpoint`, encoded as a postcard
    /// varint `u16`: `decode` reads it alone before anything else, in every schema
    /// (`tests::encoding_starts_with_the_schema`). Never move or retype it.
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
    /// Permanently the first field, encoded as a postcard varint `u16`, so the metadata of any
    /// schema can be read (`tests::encoding_starts_with_the_schema`). Never move or retype it.
    pub schema: u16,
    pub build: BuildHash,
    pub frame: u32,
    pub map: MapId,
}

impl SaveMeta {
    /// The metadata a flash slot holding `cp` carries (`game-gba` and `game-native::flash`).
    pub fn of(cp: &Checkpoint) -> Self {
        Self {
            schema: cp.header.schema,
            build: cp.header.build,
            frame: cp.header.frame,
            map: cp.world.field.map,
        }
    }
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
    #[serde(deserialize_with = "seq")]
    pub templates: Vec<TemplateDelta>,
    pub vars: VarsSnap,
    pub fader: FaderSnap,
    /// Idle stage implied; the leftover fields are hashed.
    pub transition: TransitionSnap,
    pub selected: Option<u8>,
    pub last_mover: u8,
    pub delay: u16,
    pub door: u8,
    #[serde(deserialize_with = "seq")]
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
    #[serde(deserialize_with = "seq")]
    pub overrides: Vec<((i16, i16), u16)>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ObjectsSnap {
    pub map: MapId,
    pub run_state: RunStateSnap,
    /// `[0]` = player.
    #[serde(deserialize_with = "seq")]
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
    #[serde(deserialize_with = "seq")]
    pub flags: Vec<FlagId>,
    /// Non-zero vars, ascending.
    #[serde(deserialize_with = "seq")]
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
    /// The view still shows cells drawn from the map before a connection crossing, which a
    /// restore would redraw from the current map (`Field::view_stale`).
    StaleView,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestoreError {
    Schema(u16),
    UnknownMap(MapId),
    UnknownTemplate {
        map: MapId,
        local_id: u8,
    },
    NoPlayer,
    BadRng,
    PartyTooLarge,
    HashMismatch {
        want: u64,
        got: u64,
    },
    /// State the live world can never be in (forged data, or another engine build).
    Invalid(&'static str),
}

/// Why [`encode_checked`] or [`decode`] refused a checkpoint.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodecError {
    /// Not one well-formed checkpoint (decode only).
    Postcard,
    /// Another [`SCHEMA`] (decode only).
    Schema { found: u16 },
    /// More than [`MAX_ENCODED`] bytes.
    TooLarge(usize),
}

/// Postcard bytes of `cp`. The header comes first and the schema first of all, so any build can
/// read the schema of any checkpoint.
pub fn encode(cp: &Checkpoint) -> Vec<u8> {
    // Plain data into a growable buffer: postcard cannot fail here.
    postcard::to_allocvec(cp).expect("postcard encoding of a checkpoint")
}

/// [`encode`], `TooLarge` past [`MAX_ENCODED`] (the GBA flash slot).
/// Checks the size before encoding, so an oversized checkpoint is never materialised.
pub fn encode_checked(cp: &Checkpoint) -> Result<Vec<u8>, CodecError> {
    let len = encoded_len(cp);
    if len > MAX_ENCODED {
        return Err(CodecError::TooLarge(len));
    }
    Ok(encode(cp))
}

/// `encode(cp).len()`, without allocating (a counting postcard output).
pub fn encoded_len(cp: &Checkpoint) -> usize {
    postcard::serialize_with_flavor(cp, LenFlavor(0)).expect("postcard sizing of a checkpoint")
}

/// A postcard output that only counts the bytes.
struct LenFlavor(usize);

impl postcard::ser_flavors::Flavor for LenFlavor {
    type Output = usize;

    fn try_push(&mut self, _: u8) -> postcard::Result<()> {
        self.0 += 1;
        Ok(())
    }

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        self.0 += data.len();
        Ok(())
    }

    fn finalize(self) -> postcard::Result<usize> {
        Ok(self.0)
    }
}

/// Reads the leading u16 schema first, then the whole checkpoint.
///
/// **Schema policy: migrate or fail.** A checkpoint of another [`SCHEMA`] is refused with
/// `Schema { found }` before anything else is decoded. Schema 1 has no migrations. When a DTO
/// changes (the golden-bytes test fails), bump `SCHEMA`, then decide per bump: either keep the
/// old DTOs in `checkpoint/v<N>.rs` with a `migrate` to the current ones, called from here, or
/// accept `Schema` errors. Native snapshots are a cache that regenerates from their traces, and
/// the GBA starts a new game.
///
/// The input must be exactly one checkpoint: trailing bytes are a `Postcard` error, and more
/// than [`MAX_ENCODED`] bytes are `TooLarge` (nothing is decoded). With the snapshot `Vec`s
/// decoded by [`seq`], the memory a decode allocates is bounded by the input size.
pub fn decode(bytes: &[u8]) -> Result<Checkpoint, CodecError> {
    if bytes.len() > MAX_ENCODED {
        return Err(CodecError::TooLarge(bytes.len()));
    }
    let (schema, _) = postcard::take_from_bytes::<u16>(bytes).map_err(|_| CodecError::Postcard)?;
    if schema != SCHEMA {
        return Err(CodecError::Schema { found: schema });
    }
    match postcard::take_from_bytes::<Checkpoint>(bytes) {
        Ok((cp, [])) => Ok(cp),
        _ => Err(CodecError::Postcard),
    }
}

/// Semantic identity of a snapshot: Fnv64 over its postcard bytes (the tree's "content hash").
/// The header (build, frame, world hash) is not part of it.
pub fn state_digest(w: &WorldSnapshot) -> u64 {
    postcard::serialize_with_flavor(w, DigestFlavor(Fnv64::new()))
        .expect("postcard encoding of a snapshot")
}

/// A postcard output that hashes the bytes instead of storing them.
struct DigestFlavor(Fnv64);

impl postcard::ser_flavors::Flavor for DigestFlavor {
    type Output = u64;

    fn try_push(&mut self, data: u8) -> postcard::Result<()> {
        self.0.write(&[data]);
        Ok(())
    }

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        self.0.write(data);
        Ok(())
    }

    fn finalize(self) -> postcard::Result<u64> {
        Ok(self.0.finish())
    }
}

/// Deserializes a snapshot `Vec` without trusting the encoded length: elements are pushed one by
/// one, so a forged length (a party of 4000) cannot preallocate more than the input holds.
/// Serialization is the derived one, so the bytes are unchanged.
fn seq<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Seq<T>(core::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Seq<T> {
        type Value = Vec<T>;

        fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.write_str("a sequence")
        }

        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<Vec<T>, A::Error> {
            let mut v = Vec::new();
            while let Some(x) = a.next_element()? {
                v.push(x);
            }
            Ok(v)
        }
    }
    d.deserialize_seq(Seq(core::marker::PhantomData))
}

#[cfg(test)]
mod tests;

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
