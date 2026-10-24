//! Checkpoints: versioned serde snapshots of a capturable [`crate::World`].
//!
//! The snapshot types (`WorldSnapshot` and the per-module `*Snap`) are explicit DTOs, separate
//! from the live structs, so the on-disk schema is visible, versioned (`SCHEMA`) and immune to
//! internal refactors. A snapshot holds only stable ids (Gen3 map, flag, var, graphics, species
//! and move numbers), never a content-assigned index, so it survives content rebuilds.
//!
//! Postcard is the binary form (GBA flash, [`encode`], [`state_digest`]); the native shell also
//! writes the same types as RON.

use crate::data::{
    Behavior, Dir, FlagId, GameData, GfxId, ItemId, MapId, MovementType, Pocket, VarId,
};
use crate::hash::Fnv64;
use crate::input::Buttons;
use crate::mon::{BoxedMon, Pokemon};
use crate::options::Options;
use crate::script::Speed;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Snapshot schema. Bump on any change to a DTO below (the golden-bytes test fails otherwise).
/// Schema 3 appends [`WorldSnapshot::party_held`]; schema 4 appends the Milestone 9 state
/// (options, Pokédex, PC, coins, registered item); schema 5 appends the berry trees (Wave E);
/// schema 6 appends the current map's layout override (Wave F, Gen3 `setmaplayoutindex`);
/// schema 7 appends the weather and the palette override (`crate::weather`, `crate::sky`);
/// schema 8 appends the sound state (Milestone 12, `crate::audio`).
pub const SCHEMA: u16 = 8;
/// The oldest schema [`decode`] still reads (see [`schema_readable`]).
pub const OLDEST_READABLE: u16 = 2;

/// Whether this build reads checkpoints (and trusts tree nodes) of schema `s`. Schema 2 is
/// schema 3 without held items, schema 3 is schema 4 without the Milestone 9 state, and schema
/// 4 is schema 5 without berry trees, schema 5 is schema 6 without a layout override, and
/// schema 6 is schema 7 without a weather or palette override, and schema 7 is schema 8 with
/// the sound a restore rebuilds:
/// [`decode`] reads the missing fields as empty (a new game's for the M9 state), and
/// [`state_digest`] of such a snapshot is its older digest, so recorded nodes stay valid.
pub const fn schema_readable(s: u16) -> bool {
    s >= OLDEST_READABLE && s <= SCHEMA
}
/// Largest postcard checkpoint the GBA flash slot accepts (route max is reported by the tests).
/// Milestone 9 Step 0: agb's one-slot layout on 64 KiB holds 24,527 bytes for every later save
/// (`game-native` `flash::tests::slot_capacity_holds_max_encoded`; 24,528 before schema 5 made
/// its probe checkpoints odd-sized: it grows them two bytes at a time); this keeps 512 bytes
/// under it. The worst-case save (`checkpoint/tests/worst_case.rs`) is 9,748 bytes plus 1,950 per
/// full PC box, so 7 boxes fit (decision 5: Emerald's `pc_boxes`).
pub const MAX_ENCODED: usize = 24_527 - 512;
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
    // Milestone 9: the main menu's CONTINUE window (Gen3 `main_menu.c`), with `frame` as the
    // play time. A slot written before Milestone 9 has none of these: agb reads its metadata
    // as corrupt, and the shells start a new game (`boot::CORRUPT`).
    pub name: PlayerName,
    /// Species caught (Gen3 `GetNationalPokedexCount(FLAG_GET_CAUGHT)`).
    pub dex_caught: u16,
    /// Badges owned (`SpecialIds::flag_badges` set); 0 from [`SaveMeta::of`].
    pub badges: u8,
}

impl SaveMeta {
    /// The metadata a flash slot holding `cp` carries (`game-gba` and `game-native::flash`),
    /// without the badges (they need the game's flags: [`Checkpoint::save_meta`]).
    pub fn of(cp: &Checkpoint) -> Self {
        Self {
            schema: cp.header.schema,
            build: cp.header.build,
            frame: cp.header.frame,
            map: cp.world.field.map,
            name: cp.world.player_name,
            dex_caught: (cp.world.pokedex.caught.iter())
                .map(|b| b.count_ones() as u16)
                .sum(),
            badges: 0,
        }
    }
}

impl Checkpoint {
    /// [`SaveMeta::of`] with the badges of `ids` (Milestone 9 Task S writes slots with it).
    pub fn save_meta(&self, ids: &crate::data::SpecialIds) -> SaveMeta {
        let flags = &self.world.vars.flags;
        let badges = (ids.flag_badges.iter().flatten())
            .filter(|f| flags.binary_search(f).is_ok())
            .count() as u8;
        SaveMeta {
            badges,
            ..SaveMeta::of(self)
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
    // Schema 2 (Milestone 8).
    pub bag: BagSnap,
    pub money: u32,
    /// `gStringVar1..3`, each up to its 0xFF terminator.
    #[serde(deserialize_with = "str_vars")]
    pub str_vars: [Vec<u8>; 3],
    pub warp_slots: WarpSlotsSnap,
    // Schema 3 (Silice engine). Keep it last: an empty list is one trailing 0 byte, which
    // `decode` adds to a schema-2 checkpoint and `state_digest` leaves out.
    /// The party's held items (`Pokemon::held`, by party slot); empty when no mon holds one.
    #[serde(default, deserialize_with = "seq")]
    pub party_held: Vec<Option<ItemId>>,
    // Schema 4 (Milestone 9). At a new game's values they encode as [`M9_DEFAULT_TAIL`], which
    // `decode` adds to an older checkpoint and `state_digest` leaves out. `serde(default)` reads
    // an older RON snapshot (the stores' caches) the same way.
    /// `None` at Gen3's defaults.
    #[serde(default)]
    pub options: Option<Options>,
    #[serde(default)]
    pub pokedex: PokedexSnap,
    #[serde(default)]
    pub pc: PcSnap,
    #[serde(default)]
    pub coins: u16,
    #[serde(default)]
    pub registered_item: Option<ItemId>,
    // Schema 5 (Wave E). Empty, it encodes as one 0 byte, which `decode` adds to an older
    // checkpoint and `state_digest` leaves out.
    /// The non-blank berry trees, ascending by id.
    #[serde(default, deserialize_with = "seq")]
    pub berry_trees: Vec<BerryTreeSnap>,
    // Schema 6 (Wave F). `None` encodes as one 0 byte, which `decode` adds to an older
    // checkpoint and `state_digest` leaves out.
    /// The current map's layout override (Gen3 `gSaveBlock1Ptr->mapLayoutId` when a
    /// `setmaplayoutindex` changed it; `None`: the map's own).
    #[serde(default)]
    pub map_layout: Option<u16>,
    // Schema 7 (the Silice weather wave). `None` encodes as one 0 byte, which `decode` adds to
    // an older checkpoint and `state_digest` leaves out.
    /// The weather when it is not the map's settled one, and the palette override; `None`
    /// when neither.
    #[serde(default)]
    pub sky: Option<SkySnap>,
    // Schema 8 (Milestone 12). `None` encodes as one 0 byte, which `decode` adds to an older
    // checkpoint and `state_digest` leaves out.
    /// The music players and sound tasks, when they differ from what a restore rebuilds (the
    /// map's music playing, no `savedMusic`, nothing else running).
    #[serde(default)]
    pub audio: Option<crate::audio::Audio>,
}

/// The weather and the palette override of a snapshot (schema 7).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SkySnap {
    /// The weather (its map default is derived from the map's header on restore).
    pub weather: crate::weather::Weather,
    pub palette: Option<crate::sky::PaletteFx>,
}

/// One non-blank berry tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BerryTreeSnap {
    pub id: u8,
    pub tree: crate::berry::BerryTree,
}

/// The postcard bytes of the schema 4 fields at a new game's values: `options` `None`, two
/// empty Pokédex bitsets, an empty PC (no items, no mons, box 0), 0 coins, no registered item.
pub const M9_DEFAULT_TAIL: [u8; 8] = [0; 8];

impl WorldSnapshot {
    /// Whether the schema 4 fields are a new game's (they then encode as [`M9_DEFAULT_TAIL`]).
    pub fn m9_is_default(&self) -> bool {
        self.options.is_none()
            && self.pokedex == PokedexSnap::default()
            && self.pc == PcSnap::default()
            && self.coins == 0
            && self.registered_item.is_none()
    }
}

/// `Pokedex` as its two bitsets (bit `n` = species id `n`), without trailing zero bytes.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct PokedexSnap {
    #[serde(deserialize_with = "seq")]
    pub seen: Vec<u8>,
    #[serde(deserialize_with = "seq")]
    pub caught: Vec<u8>,
}

/// `PcStorage`'s non-empty item slots and box slots, ascending, and the current box.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct PcSnap {
    #[serde(deserialize_with = "seq")]
    pub items: Vec<PcItemSnap>,
    #[serde(deserialize_with = "seq")]
    pub mons: Vec<PcMonSnap>,
    pub current_box: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PcItemSnap {
    pub index: u8,
    pub item: ItemId,
    pub qty: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PcMonSnap {
    pub pc_box: u8,
    pub slot: u8,
    pub mon: BoxedMon,
}

/// Why the in-game save ([`crate::World::save_snapshot`]) did not produce a checkpoint.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaveError {
    /// Not implemented yet (Milestone 9 Wave 0's stub).
    Unsupported,
    /// The normalised world still could not be captured.
    NotCapturable(NotCapturable),
    /// Larger than [`MAX_ENCODED`].
    TooLarge(usize),
}

/// The bag's non-empty slots, in pocket then slot order.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct BagSnap {
    #[serde(deserialize_with = "seq")]
    pub slots: Vec<BagSlotSnap>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BagSlotSnap {
    pub pocket: Pocket,
    /// Slot in the pocket.
    pub index: u8,
    pub item: ItemId,
    pub qty: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct WarpSlotsSnap {
    pub dynamic: Option<WarpDestSnap>,
    pub escape: Option<WarpDestSnap>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct WarpDestSnap {
    pub map: MapId,
    pub warp: Option<u8>,
    pub x: i16,
    pub y: i16,
}

/// `Field::stale` (inclusive cells).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct StaleSnap {
    pub x0: i16,
    pub y0: i16,
    pub x1: i16,
    pub y1: i16,
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
    /// Schema 2: always `None` (a restore rejects `Some`). Since Milestone 8 Task F a crossing
    /// redraws the cells that differ instead of keeping a stale rect; the field stays for the
    /// format.
    pub stale: Option<StaleSnap>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ObjectsSnap {
    pub map: MapId,
    pub run_state: RunStateSnap,
    /// `[0]` = player.
    #[serde(deserialize_with = "seq")]
    pub objs: Vec<ObjectSnap>,
}

impl ObjectsSnap {
    /// Whether the player surfs (Wave C: a sentinel in the player's `gfx`).
    pub fn player_surfing(&self) -> bool {
        (self.objs.first()).is_some_and(|p| p.gfx == Some(crate::objects::SURFING_SNAP))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RunStateSnap {
    NotMoving,
    TurnDirection,
    Moving,
    // Forced movement held off (Gen3 `PLAYER_AVATAR_FLAG_CONTROLLABLE`, `Objects::hold_forced`),
    // never while `Moving`. Appended: snapshots without them encode as before.
    NotMovingHeld,
    TurnDirectionHeld,
    // Gen3 `T_TILE_CENTER` on a forced-movement cell (`Objects::at_forced_tile_center`): the
    // frame after a step ended there. Appended as well.
    NotMovingCentered,
    TurnDirectionCentered,
    MovingCentered,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ObjectSnap {
    pub local_id: u8,
    pub map: MapId,
    pub gfx: Option<GfxId>,
    pub pos: (i16, i16),
    pub prev: (i16, i16),
    /// Gen3 `currentElevation` in the low nibble; when `previousElevation` differs, it is
    /// packed as `prev + 1` in the high nibble (read with [`ObjectSnap::current_elevation`]).
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

impl ObjectSnap {
    /// Gen3 `currentElevation` (the low nibble of [`ObjectSnap::elevation`]).
    pub const fn current_elevation(&self) -> u8 {
        self.elevation & 0x0F
    }
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
    /// A screen is open (Milestone 9: the in-game save captures a normalised copy instead).
    Screen,
    Transition,
    Script,
    TextBox,
    Fade,
    /// The local id of an object with a live movement script.
    Moving(u8),
    /// A field upload spread over frames is in progress (`Field::uploading`: a crossing into
    /// other tilesets, or a spread load): the view is not all drawn with the current map's
    /// tilesets yet, which a restore would upload whole. (The name predates Milestone 8 Task
    /// F, when a crossing left stale cells in view; it no longer does.)
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
    if !schema_readable(schema) {
        return Err(CodecError::Schema { found: schema });
    }
    let upgraded;
    let bytes = if schema < SCHEMA {
        // Schema 2 lacks the trailing `party_held` (an empty one is a 0 length byte), and
        // schemas 2 and 3 lack the Milestone 9 fields (a new game's are `M9_DEFAULT_TAIL`).
        // Schema 4 lacks the berry trees (none is a 0 length byte), schema 5 the layout
        // override (`None` is a 0 byte), schema 6 the sky (`None` is a 0 byte), schema 7 the
        // sound (`None` is a 0 byte).
        let mut v = Vec::with_capacity(bytes.len() + 4 + M9_DEFAULT_TAIL.len());
        v.extend_from_slice(bytes);
        if schema < 3 {
            v.push(0);
        }
        if schema < 4 {
            v.extend_from_slice(&M9_DEFAULT_TAIL);
        }
        if schema < 5 {
            v.push(0);
        }
        if schema < 6 {
            v.push(0);
        }
        if schema < 7 {
            v.push(0);
        }
        v.push(0);
        upgraded = v;
        &upgraded[..]
    } else {
        bytes
    };
    match postcard::take_from_bytes::<Checkpoint>(bytes) {
        Ok((mut cp, [])) => {
            cp.header.schema = SCHEMA;
            Ok(cp)
        }
        _ => Err(CodecError::Postcard),
    }
}

/// Semantic identity of a snapshot: Fnv64 over its postcard bytes (the tree's "content hash").
/// The header (build, frame, world hash) is not part of it. Trailing fields at their older
/// schema's defaults are left out: no sound (its single 0 byte), no sky (its single 0 byte), then no layout override (its
/// single 0 byte), then no berry trees
/// (their single 0 byte), then the Milestone 9 fields at a new game's values
/// ([`M9_DEFAULT_TAIL`]), then an empty `party_held` (its single 0 byte), so such a snapshot
/// keeps its schema 7, 6, 5, 4, 3 (or 2) digest.
pub fn state_digest(w: &WorldSnapshot) -> u64 {
    let mut drop = 0;
    if w.audio.is_none() {
        drop = 1;
        if w.sky.is_none() {
            drop += 1;
            if w.map_layout.is_none() {
                drop += 1;
                if w.berry_trees.is_empty() {
                    drop += 1;
                    if w.m9_is_default() {
                        drop += M9_DEFAULT_TAIL.len();
                        if w.party_held.is_empty() {
                            drop += 1;
                        }
                    }
                }
            }
        }
    }
    let flavor = DigestFlavor {
        h: Fnv64::new(),
        pending: [0; DIGEST_DROP_MAX],
        n: 0,
        drop,
    };
    postcard::serialize_with_flavor(w, flavor).expect("postcard encoding of a snapshot")
}

const DIGEST_DROP_MAX: usize = M9_DEFAULT_TAIL.len() + 5;

/// A postcard output that hashes the bytes instead of storing them, `drop` bytes behind so
/// that the last `drop` can be left out.
struct DigestFlavor {
    h: Fnv64,
    pending: [u8; DIGEST_DROP_MAX],
    n: usize,
    drop: usize,
}

impl postcard::ser_flavors::Flavor for DigestFlavor {
    type Output = u64;

    fn try_push(&mut self, data: u8) -> postcard::Result<()> {
        if self.drop == 0 {
            self.h.write(&[data]);
        } else if self.n < self.drop {
            self.pending[self.n] = data;
            self.n += 1;
        } else {
            self.h.write(&[self.pending[0]]);
            self.pending.copy_within(1..self.drop, 0);
            self.pending[self.drop - 1] = data;
        }
        Ok(())
    }

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        for &b in data {
            self.try_push(b)?;
        }
        Ok(())
    }

    fn finalize(self) -> postcard::Result<u64> {
        // The pending bytes are the dropped tail (a snapshot is always longer than it).
        Ok(self.h.finish())
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

/// [`seq`] for each of the three string vars (the derived array serialization is unchanged).
fn str_vars<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[Vec<u8>; 3], D::Error> {
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct Bytes(#[serde(deserialize_with = "seq")] Vec<u8>);
    let [a, b, c] = <[Bytes; 3]>::deserialize(d)?;
    Ok([a.0, b.0, c.0])
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod wave0_tests;
