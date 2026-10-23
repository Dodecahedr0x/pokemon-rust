//! Static game data the engine consumes. Content builds these as `'static` tables (via build.rs);
//! the engine never owns or parses files.

use crate::battle::{self, BattleGfx};
use crate::mon::{self, BattleTables, LandEncounters, TrainerData};
use crate::script::{Ctx, Script};
use crate::video::Tile;

/// Facing / movement direction. Order matches Gen3 `DIR_SOUTH..DIR_EAST` minus one.
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize,
)]
pub enum Dir {
    Down,
    Up,
    Left,
    Right,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::Down, Dir::Up, Dir::Left, Dir::Right];

    /// One-step offset in map cells (y grows downwards).
    pub const fn delta(self) -> (i16, i16) {
        match self {
            Dir::Down => (0, 1),
            Dir::Up => (0, -1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

    pub const fn opposite(self) -> Dir {
        match self {
            Dir::Down => Dir::Up,
            Dir::Up => Dir::Down,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }
}

/// Up to four directions packed in a byte (2 bits each, the first in the low bits), the
/// payload of [`MovementType::FaceTwo`] and [`MovementType::WalkSequence`]: one byte keeps
/// `MovementType` two bytes, so the per-frame object and template hashes stay cheap.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Dirs(pub u8);

impl Dirs {
    pub const fn new(d: [Dir; 4]) -> Dirs {
        Dirs(d[0] as u8 | (d[1] as u8) << 2 | (d[2] as u8) << 4 | (d[3] as u8) << 6)
    }
    /// `[a, b, a, b]`: a two-direction set.
    pub const fn two(a: Dir, b: Dir) -> Dirs {
        Dirs::new([a, b, a, b])
    }
    /// Direction `i` (taken modulo 4).
    pub const fn get(self, i: usize) -> Dir {
        match (self.0 >> ((i & 3) * 2)) & 3 {
            0 => Dir::Down,
            1 => Dir::Up,
            2 => Dir::Left,
            _ => Dir::Right,
        }
    }
}

macro_rules! id {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {$(
        $(#[$m])*
        #[derive(
            Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug,
            serde::Serialize, serde::Deserialize,
        )]
        pub struct $name(pub u16);
    )*};
}

id! {
    /// Content-assigned map number.
    MapId,
    /// Gen3 flag number (`FLAG_*`): 0x1..0x20 are temporary flags (cleared on map change);
    /// 0x4000.. are special (system) flags.
    FlagId,
    /// Gen3 var number (`VAR_*`): 0x4000..=0x40FF saved vars, 0x8000..=0x80FF special vars.
    VarId,
    /// Content-assigned text number; `GameData::text` resolves it.
    TextId,
    /// Content-assigned script number; `GameData::script` instantiates it.
    ScriptId,
    /// Gen3 object graphics id (`OBJ_EVENT_GFX_*`).
    GfxId,
    /// Content-assigned = Gen3 `SPECIES_*` number; indexes `BattleTables::species`.
    SpeciesId,
    /// Content-assigned = Gen3 `MOVE_*` number; indexes `BattleTables::moves`.
    MoveId,
    /// Content-assigned = Gen3 `TRAINER_*` number; `GameData::trainer` resolves it.
    TrainerId,
    /// Gen3 `ITEM_*` number; `GameData::item` resolves it (Milestone 8).
    ItemId,
}

/// Gen3 bag pockets, in save-block order (`gSaveBlock1Ptr->bagPocket_*`).
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize,
)]
pub enum Pocket {
    Items,
    KeyItems,
    PokeBalls,
    TmHm,
    Berries,
}

impl Pocket {
    pub const ALL: [Pocket; 5] = [
        Pocket::Items,
        Pocket::KeyItems,
        Pocket::PokeBalls,
        Pocket::TmHm,
        Pocket::Berries,
    ];
}

/// One item (`gItems`): Gen3-encoded 0xFF-terminated name, pocket, price, and whether it is a
/// key item (`importance`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ItemInfo {
    pub name: &'static [u8],
    pub pocket: Pocket,
    pub price: u16,
    pub key: bool,
}

/// Emote bubble shown above an object (Gen3 field effects `FLDEFF_EXCLAMATION_MARK_ICON`,
/// `FLDEFF_QUESTION_MARK_ICON`, `FLDEFF_HEART_ICON`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Emote {
    Exclamation,
    Question,
    Heart,
}

/// How a species evolves, Gen3 `EVO_*` folded (the exact kind is kept for Milestone 9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvoMethod {
    /// `EVO_LEVEL` and its stat/personality variants: `param` is the level.
    Level,
    /// `EVO_FRIENDSHIP*`.
    Friendship,
    /// `EVO_ITEM`: `param` is the item.
    Item,
    /// `EVO_TRADE*`.
    Trade,
    /// Anything else (`EVO_BEAUTY`, `EVO_LEVEL_NINJASK`, …).
    Other,
}

/// One `gEvolutionTable` entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Evolution {
    pub method: EvoMethod,
    pub param: u16,
    pub target: SpeciesId,
}

/// Gen3 local id addressing the player in object commands (`OBJ_EVENT_ID_PLAYER`).
pub const PLAYER: u8 = 0xFF;
/// Gen3 local id addressing the camera object (`OBJ_EVENT_ID_CAMERA`).
pub const CAMERA: u8 = 0x7F;

// ---- Blocks and metatiles (Gen3 `map.bin` / `metatile_attributes.bin` bit layout) ----

pub const BLOCK_METATILE_MASK: u16 = 0x03FF;
pub const BLOCK_COLLISION_SHIFT: u16 = 10;
pub const BLOCK_ELEVATION_SHIFT: u16 = 12;

pub const fn block_metatile(block: u16) -> u16 {
    block & BLOCK_METATILE_MASK
}

pub const fn block_collision(block: u16) -> u8 {
    ((block >> BLOCK_COLLISION_SHIFT) & 3) as u8
}

pub const fn block_elevation(block: u16) -> u8 {
    (block >> BLOCK_ELEVATION_SHIFT) as u8
}

/// Metatile ids `>= METATILES_IN_PRIMARY` index the secondary tileset.
pub const METATILES_IN_PRIMARY: u16 = 512;
/// Tile indices `>= TILES_IN_PRIMARY` index the secondary tileset.
pub const TILES_IN_PRIMARY: u16 = 512;
/// BG palette banks 0..6 come from the primary tileset, 6..13 from the secondary.
pub const PALS_IN_PRIMARY: usize = 6;
pub const PALS_TOTAL: usize = 13;

pub const fn attr_behavior(attr: u16) -> u8 {
    (attr & 0x00FF) as u8
}

/// How a metatile's 8 tile entries map onto BG layers (Gen3 `METATILE_LAYER_TYPE_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayerType {
    /// Bottom 4 -> middle BG, top 4 -> top BG, bottom BG gets the 0x3014 underlay.
    Normal,
    /// Bottom 4 -> bottom BG, top 4 -> middle BG.
    Covered,
    /// Bottom 4 -> bottom BG, top 4 -> top BG.
    Split,
}

pub const fn attr_layer(attr: u16) -> LayerType {
    match attr >> 12 {
        1 => LayerType::Covered,
        2 => LayerType::Split,
        _ => LayerType::Normal,
    }
}

/// A Gen3 tileset. `tiles[i]` is tile `i` (primary) or `512 + i` (secondary); metatiles and
/// attributes likewise start at 0 or 512.
pub struct Tileset {
    pub tiles: &'static [Tile],
    /// 16 palettes; only 0..6 (primary) or 6..13 (secondary) are used.
    pub palettes: &'static [[u16; 16]],
    /// 8 tile entries per metatile: bottom layer TL, TR, BL, BR, then top layer.
    pub metatiles: &'static [[u16; 8]],
    pub attributes: &'static [u16],
}

/// What the engine needs to know about a metatile behavior. Content classifies Gen3's raw
/// `MB_*` values (`GameData::behavior`), so the engine never sees game constants.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Behavior {
    Normal,
    /// Never walkable, whatever the collision bits say (water without Surf, …).
    Blocked,
    /// Walking in `Dir` onto it jumps two cells.
    Ledge(Dir),
    /// Animated door: entering the cell triggers its warp.
    Door,
    /// Non-animated warp cell (stairs, ladders, holes): entering triggers its warp.
    Warp,
    /// Standing on it and pushing `Dir` triggers its warp (doormats, exits).
    ArrowWarp(Dir),
    /// Talking across it reaches the object one cell further (shop counters).
    Counter,
    TallGrass,
    /// Walkable, but its [`Sides`] cannot be crossed, leaving or entering (Gen3
    /// `MB_IMPASSABLE_*`, `MB_SECRET_BASE_BREAKABLE_DOOR`; see [`Behavior::walled`]).
    Impassable(Sides),
}

/// Sides of a cell, one bit per [`Dir`] (`1 << dir as u8`): the payload of
/// [`Behavior::Impassable`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Sides(pub u8);

impl Sides {
    pub const fn of(dirs: &[Dir]) -> Sides {
        let mut bits = 0;
        let mut i = 0;
        while i < dirs.len() {
            bits |= 1 << dirs[i] as u8;
            i += 1;
        }
        Sides(bits)
    }

    pub const fn has(self, d: Dir) -> bool {
        self.0 & (1 << d as u8) != 0
    }
}

impl Behavior {
    /// Gen3 `IsMetatileDirectionallyImpassable`: a step towards `d` from a cell with behavior
    /// `from` into one with behavior `to` crosses a wall (`from`'s side `d`, or `to`'s side
    /// opposite `d`).
    pub const fn walled(from: Behavior, to: Behavior, d: Dir) -> bool {
        matches!(from, Behavior::Impassable(s) if s.has(d))
            || matches!(to, Behavior::Impassable(s) if s.has(d.opposite()))
    }
}

/// Object graphics: fixed, or read from a var when the object spawns (`OBJ_EVENT_GFX_VAR_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Gfx {
    Fixed(GfxId),
    Var(VarId),
}

/// Idle behavior of an object (Gen3 `MOVEMENT_TYPE_*`; content maps the ones without a variant,
/// copy-player, disguises, buried and berry trees, to the closest one).
///
/// New variants are appended: checkpoints encode the variant index.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum MovementType {
    /// Stays put facing `Dir`.
    Face(Dir),
    /// Turns to random directions.
    LookAround,
    /// Walks randomly within its range.
    Wander,
    WanderVertical,
    WanderHorizontal,
    /// Walks on the spot facing `Dir`.
    WalkInPlace(Dir),
    Invisible,
    // ---- Milestone 8 ----
    /// Gen3 `FACE_DOWN_AND_UP`, `FACE_LEFT_AND_RIGHT`, `FACE_UP_AND_LEFT`, `FACE_UP_AND_RIGHT`,
    /// `FACE_DOWN_AND_LEFT`, `FACE_DOWN_AND_RIGHT`: turns to one of the two (Gen3's array order;
    /// the first is the initial facing) after medium delays. [`Dirs::two`].
    FaceTwo(Dirs),
    /// Gen3 `FACE_DOWN_UP_AND_LEFT` (`Right` missing), `FACE_DOWN_UP_AND_RIGHT` (`Left`),
    /// `FACE_UP_LEFT_AND_RIGHT` (`Down`), `FACE_DOWN_LEFT_AND_RIGHT` (`Up`): turns among the other
    /// three after short delays.
    FaceAllBut(Dir),
    /// Gen3 `ROTATE_CLOCKWISE` / `ROTATE_COUNTERCLOCKWISE`: a quarter turn every 48 frames.
    Rotate {
        clockwise: bool,
    },
    /// Gen3 `WALK_UP_AND_DOWN` (`Up`), `WALK_DOWN_AND_UP` (`Down`), `WALK_LEFT_AND_RIGHT` (`Left`),
    /// `WALK_RIGHT_AND_LEFT` (`Right`): walks to the end of its range and back, starting towards
    /// `Dir`.
    WalkBackAndForth(Dir),
    /// Gen3 `WALK_SEQUENCE_*`: walks the four directions in this order around its range.
    WalkSequence(Dirs),
    /// Gen3 `JOG_IN_PLACE_*` (`walk_in_place_fast`).
    JogInPlace(Dir),
    /// Gen3 `RUN_IN_PLACE_*` (`walk_in_place_faster`).
    RunInPlace(Dir),
    /// Gen3 `WALK_SLOWLY_IN_PLACE_*` (`walk_in_place_slow`).
    WalkSlowlyInPlace(Dir),
}

/// An object template. `Copy` so the world can keep a per-visit copy that scripts edit
/// (`setobjectxyperm`, `setobjectmovementtype`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ObjectEvent {
    pub local_id: u8,
    pub gfx: Gfx,
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub movement: MovementType,
    /// Wander range in cells around the spawn point (x, y).
    pub range: (u8, u8),
    pub trainer_type: u8,
    pub trainer_sight: u8,
    pub script: Option<ScriptId>,
    /// While this flag is set the object does not exist.
    pub hide_flag: Option<FlagId>,
}

pub struct WarpEvent {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub dest_map: MapId,
    /// Index into the destination map's `warps`.
    pub dest_warp: u8,
}

/// Runs `script` when the player steps on (x, y) while `var == value`.
pub struct CoordEvent {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub var: VarId,
    pub value: u16,
    pub script: ScriptId,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BgKind {
    /// Interacting while facing one of the allowed directions (`None` = any) runs the script.
    Script {
        facing: Option<Dir>,
        script: ScriptId,
    },
    HiddenItem {
        item: u16,
        flag: FlagId,
    },
}

pub struct BgEvent {
    pub x: i16,
    pub y: i16,
    pub elevation: u8,
    pub kind: BgKind,
}

/// Neighbor map glued to one edge. `offset` shifts the neighbor along that edge (Gen3
/// `MapConnection.offset`: x offset for Up/Down, y offset for Left/Right).
pub struct Connection {
    pub dir: Dir,
    pub offset: i16,
    pub map: MapId,
}

/// Runs `script` when `var == value` (Gen3 `MAP_SCRIPT_ON_FRAME_TABLE` / `ON_WARP_INTO_MAP_TABLE`).
pub struct VarScript {
    pub var: VarId,
    pub value: u16,
    pub script: ScriptId,
}

#[derive(Default)]
pub struct MapScripts {
    pub on_load: Option<ScriptId>,
    pub on_transition: Option<ScriptId>,
    pub on_resume: Option<ScriptId>,
    pub on_return_to_field: Option<ScriptId>,
    pub on_frame: &'static [VarScript],
    pub on_warp_into: &'static [VarScript],
}

pub struct MapData {
    pub name: &'static str,
    pub width: u16,
    pub height: u16,
    /// `width * height` raw blocks, row-major.
    pub blocks: &'static [u16],
    /// 2x2 border blocks, row-major, tiled outside the map where no connection applies.
    pub border: [u16; 4],
    pub primary: &'static Tileset,
    pub secondary: &'static Tileset,
    pub objects: &'static [ObjectEvent],
    pub warps: &'static [WarpEvent],
    pub coords: &'static [CoordEvent],
    pub bgs: &'static [BgEvent],
    pub connections: &'static [Connection],
    pub scripts: MapScripts,
    pub allow_running: bool,
}

/// An object sprite sheet. Each frame is its tiles in GBA 1D OBJ order.
pub struct SpriteData {
    pub width: u8,
    pub height: u8,
    pub frames: &'static [&'static [Tile]],
    pub palette: &'static [u16; 16],
}

/// Gen3 Latin font: 16x16 glyph cells, 2bpp, in ROM layout (`crates/extract/src/font.rs`).
pub struct Font {
    pub glyphs: &'static [[u8; 64]],
    pub widths: &'static [u8],
}

/// Dialogue window graphics (Gen3 `gMessageBox_Gfx`) and the BG palette holding both the frame
/// colors and the text colors.
pub struct WindowGfx {
    pub tiles: &'static [Tile],
    pub palette: &'static [u16; 16],
    /// Palette indices the text printer uses (`WindowFile::text_fg` / `text_shadow` / `text_bg`).
    pub text_fg: u8,
    pub text_shadow: u8,
    pub text_bg: u8,
}

/// Fixed UI strings the engine draws itself (Gen3-encoded, 0xFF-terminated).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UiText {
    Yes,
    No,
    /// Expansion of the `{RIVAL}` placeholder.
    RivalName,
}

/// Where a new game starts and the script that sets up its flags and vars.
pub struct NewGame {
    pub map: MapId,
    pub x: i16,
    pub y: i16,
    pub facing: Dir,
    pub script: Option<ScriptId>,
    /// Gen3-encoded player name, 0xFF-terminated.
    pub player_name: &'static [u8],
}

/// Game constants the engine needs by role, never by name (content fills them in).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SpecialIds {
    /// Set to the player's facing (Gen3 `DIR_*` numbering: `Dir as u16 + 1`) when the player
    /// talks to an object (Gen3 `gSpecialVar_Facing`).
    pub var_facing: Option<VarId>,
    /// Set to the local id of the object talked to, 0 for other scripts (Gen3
    /// `gSpecialVar_LastTalked`).
    pub var_last_talked: Option<VarId>,
    /// Running is only allowed while this flag is set (Gen3 `FLAG_SYS_B_DASH`).
    pub flag_running: Option<FlagId>,
    /// Flags cleared with the temporary flags and vars on every map change (Gen3
    /// `ClearTempFieldEventData`).
    pub flags_cleared_on_map_change: &'static [FlagId],
    /// Heal location of a new game (`setrespawn` default; Gen3 `HEAL_LOCATION_*` number).
    pub respawn_default: u8,
    /// Selects the wild header of maps with several (Gen3 `VAR_ALTERING_CAVE_WILD_SET`).
    pub var_wild_set: Option<VarId>,
    /// Run immediately after a whiteout (Gen3 `EventScript_WhiteOut`, from `DoWhiteOut`).
    pub script_whiteout: Option<ScriptId>,
    /// The money cap (Gen3 `MAX_MONEY`): money added (scripts, prize money) stops there and a
    /// restore refuses more. 0 for a game without money.
    pub max_money: u32,
    /// A multichoice's answer when B cancels it (Gen3 `MULTI_B_PRESSED`), also the answer of a
    /// multichoice the engine cannot show.
    pub multichoice_cancel: u16,
    /// Slots per bag pocket, in [`Pocket::ALL`] order (Gen3 `BAG_ITEMS_COUNT`,
    /// `BAG_KEYITEMS_COUNT`, `BAG_POKEBALLS_COUNT`, `BAG_TMHM_COUNT`, `BAG_BERRIES_COUNT`). All 0
    /// for a game without a bag.
    pub pocket_slots: [u8; 5],
}

/// Everything game-specific the engine reads. Implemented by the `content` crate.
pub trait GameData {
    /// A map. Ids the game does not have may return any 0x0 map (see [`GameData::has_map`]).
    fn map(&self, id: MapId) -> &'static MapData;
    /// Whether `id` is a real map; warps to other maps are logged and ignored.
    fn has_map(&self, id: MapId) -> bool {
        self.map(id).width > 0
    }
    /// Special vars and flags the engine reads or writes.
    fn special_ids(&self) -> SpecialIds {
        SpecialIds::default()
    }
    /// Sprite for a Gen3 graphics id, `None` if the game has no such sprite.
    fn sprite(&self, gfx: GfxId) -> Option<&'static SpriteData>;
    fn player_sprite(&self) -> &'static SpriteData;
    fn font(&self) -> &'static Font;
    fn window(&self) -> &'static WindowGfx;
    /// Gen3-encoded text, 0xFF-terminated.
    fn text(&self, id: TextId) -> &'static [u8];
    fn ui_text(&self, t: UiText) -> &'static [u8];
    fn behavior(&self, raw: u8) -> Behavior;
    fn script(&self, id: ScriptId, ctx: Ctx) -> Script;
    fn new_game(&self) -> NewGame;
    /// Identity of the content build (inputs of the content build script); 0 for test games.
    fn build_id(&self) -> u64 {
        0
    }

    // ---- battle (Milestone 4); every method has a default so test games keep compiling ----

    /// Species, moves and the type chart.
    fn battle(&self) -> &'static BattleTables {
        &mon::EMPTY_TABLES
    }
    /// The land table of header `wild_set` of `map`, if it has one (Milestone 8). The engine
    /// passes the value of `SpecialIds::var_wild_set` (Gen3 `VAR_ALTERING_CAVE_WILD_SET`); maps
    /// with one header ignore it. Water, rock smash and fishing come later (M10).
    fn wild_encounters(&self, map: MapId, wild_set: u16) -> Option<&'static LandEncounters> {
        let _ = (map, wild_set);
        None
    }
    fn trainer(&self, id: TrainerId) -> Option<&'static TrainerData> {
        let _ = id;
        None
    }
    /// Gen3-encoded, 0xFF-terminated battle string (`gBattleStringsTable`, `gText_*`).
    fn battle_text(&self, s: battle::Str) -> &'static [u8] {
        let _ = s;
        &[0xFF]
    }
    /// Battle interface graphics; the default has empty slices and draws nothing.
    fn battle_gfx(&self) -> &'static BattleGfx {
        &battle::EMPTY_GFX
    }
    /// Gen3 heal location `id` (`setrespawn`, whiteout): map and player cell.
    fn heal_location(&self, id: u8) -> Option<(MapId, i16, i16)> {
        let _ = id;
        None
    }

    // ---- Milestone 8 ----

    fn item(&self, id: ItemId) -> Option<&'static ItemInfo> {
        let _ = id;
        None
    }
    /// The Gen3-encoded choices of multichoice list `list` (`MULTI_*`, `sMultichoiceLists`).
    fn multichoice(&self, list: u8) -> Option<&'static [&'static [u8]]> {
        let _ = list;
        None
    }
    /// Gen3-encoded `gStdStrings[id]` (`STDSTRING_*`).
    fn std_string(&self, id: u8) -> Option<&'static [u8]> {
        let _ = id;
        None
    }
    /// The trainer flag a trainer object's script checks (Gen3 `GetTrainerFlagFromScriptPointer`).
    fn trainer_flag_of(&self, script: ScriptId) -> Option<FlagId> {
        let _ = script;
        None
    }
    /// The `TRAINER_BATTLE_*` kind (`include/constants/battle_setup.h` numbering) of a trainer
    /// object's script, the `trainerbattle` it starts with (Milestone 8 Task D2).
    fn trainer_battle_kind(&self, script: ScriptId) -> Option<u8> {
        let _ = script;
        None
    }
    fn emote(&self, e: Emote) -> Option<&'static SpriteData> {
        let _ = e;
        None
    }
    /// Gen3 `gTrainerMoneyTable` value for the trainer's class (`GetTrainerMoneyToGive`). For a
    /// class the table does not list, content returns the table's terminator value (Gen3's
    /// `{0xFF, 5}` entry, `battle_main.c:531`, which the lookup loop stops on), so every known
    /// trainer pays; `None` only when the game has no money data.
    fn trainer_money(&self, id: TrainerId) -> Option<u8> {
        let _ = id;
        None
    }
    fn evolutions(&self, species: SpeciesId) -> &'static [Evolution] {
        let _ = species;
        &[]
    }

    // ---- Silice battle engine ----

    /// The battle effect of a held item (`gItems[].holdEffect` / `holdEffectParam`); `None`
    /// for an item with no effect the engine models.
    fn hold_effect(&self, id: ItemId) -> Option<mon::HoldEffect> {
        let _ = id;
        None
    }
    /// The in-battle form changes (Mega Evolution, Primal Reversion), in any order. A game
    /// without them (Emerald) returns the default, empty.
    fn form_changes(&self) -> &'static [FormChange] {
        &[]
    }
}

/// How a [`FormChange`] triggers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormKind {
    /// Mega Evolution: chosen by the trainer before the turn's moves, at most once per side
    /// per battle.
    Mega,
    /// Primal Reversion: automatic when the holder enters the field; free.
    Primal,
}

/// A battle-only form: `base` holding `item` becomes `form` (a species of `BattleTables`, its
/// stats, types and ability), until the battle ends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormChange {
    pub base: SpeciesId,
    pub item: ItemId,
    pub form: SpeciesId,
    pub kind: FormKind,
    /// The flag that must be set for the player's mons to change (the Mega Ring); `None`:
    /// always. Trainers ignore it.
    pub enable_flag: Option<FlagId>,
}

#[cfg(test)]
mod tests;
