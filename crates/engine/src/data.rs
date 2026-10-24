//! Static game data the engine consumes. Content builds these as `'static` tables (via build.rs);
//! the engine never owns or parses files.

use crate::battle::{self, BattleGfx};
use crate::mon::{self, BattleTables, LandEncounters, TrainerData};
use crate::script::{Ctx, Script};
use crate::video::Tile;

mod puzzle;
pub use puzzle::{CrackedFloor, IcePuzzle, IceRows, StepCallback};

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

/// How a species evolves (Gen3 `EVO_*`). Until Milestone 9 Task E classifies them exactly
/// (`build/classify/evolution.rs`), content folds the exact kinds onto `Level`, `Friendship`,
/// `Item`, `Trade` and `Other`, and the engine evaluates `Level` only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvoMethod {
    /// `EVO_LEVEL` (and, while folded, its stat/personality variants): `param` is the level.
    Level,
    /// `EVO_FRIENDSHIP` (and, while folded, its day and night variants).
    Friendship,
    /// `EVO_ITEM`: `param` is the item.
    Item,
    /// `EVO_TRADE` (and, while folded, `EVO_TRADE_ITEM`).
    Trade,
    /// Anything else (`EVO_BEAUTY` while folded, …).
    Other,
    // ---- Milestone 9 (Wave 0 declares them; Task E classifies and evaluates them) ----
    /// `EVO_FRIENDSHIP_DAY`.
    FriendshipDay,
    /// `EVO_FRIENDSHIP_NIGHT`.
    FriendshipNight,
    /// `EVO_LEVEL_ATK_GT_DEF`: `param` is the level.
    LevelAtkGtDef,
    /// `EVO_LEVEL_ATK_EQ_DEF`.
    LevelAtkEqDef,
    /// `EVO_LEVEL_ATK_LT_DEF`.
    LevelAtkLtDef,
    /// `EVO_LEVEL_SILCOON` (by personality).
    LevelSilcoon,
    /// `EVO_LEVEL_CASCOON`.
    LevelCascoon,
    /// `EVO_LEVEL_NINJASK`.
    LevelNinjask,
    /// `EVO_LEVEL_SHEDINJA` (into an empty party slot, with a Poké Ball).
    LevelShedinja,
    /// `EVO_BEAUTY`: `param` is the beauty.
    Beauty,
    /// `EVO_TRADE_ITEM`: `param` is the item.
    TradeItem,
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
    /// Animated door: entering the cell, or pushing north into it, triggers its warp; arriving
    /// on one, the player is hidden during the fade-in, then walks one cell down (Gen3
    /// `Task_ExitDoor`).
    Door,
    /// Any other warp cell entered by stepping on it (gym and hideout warps, holes): entering
    /// triggers its warp; arriving on one is a plain arrival, facing down (Gen3
    /// `Task_ExitNonDoor`).
    Warp,
    /// Standing on it and pushing `Dir` triggers its warp (doormats, exits); arriving on one,
    /// the player faces away from `Dir`.
    ArrowWarp(Dir),
    /// Talking across it reaches the object one cell further (shop counters).
    Counter,
    TallGrass,
    /// Walkable, but its [`Sides`] cannot be crossed, leaving or entering (Gen3
    /// `MB_IMPASSABLE_*`, `MB_SECRET_BASE_BREAKABLE_DOOR`; see [`Behavior::walled`]).
    Impassable(Sides),
    /// Non-animated door (stairs, cave and forest exits): entering triggers its warp; arriving
    /// on one, the player faces `Dir`, is hidden during the fade-in, then walks one cell `Dir`
    /// (Gen3 `MetatileBehavior_IsNonAnimDoor`, `Task_ExitNonAnimDoor`).
    NonAnimDoor(Dir),
    /// Ladder: entering triggers its warp; arriving on one keeps the facing the player warped
    /// with, no exit walk (Gen3 `GetAdjustedInitialDirection`).
    Ladder,
    /// Escalator: entering triggers its warp; arriving on one, the player faces `Dir` and walks
    /// one cell `Dir`, visible (Gen3 `Task_EscalatorWarpIn`, without the ride).
    Escalator(Dir),
    // ---- forced movement (see `objects::forced`) ----
    /// Ice (Gen3 `MB_ICE`, `MB_TRICK_HOUSE_PUZZLE_8_FLOOR`): the player slips on in the
    /// direction they move, at `walk_fast`, until a cell that is not ice or an obstacle
    /// (`ForcedMovement_Slip`).
    Ice,
    /// Gen3 `MB_WALK_*`: a forced `walk_normal` step towards `Dir` (`ForcedMovement_Walk*`).
    ForcedWalk(Dir),
    /// Gen3 `MB_SLIDE_*`: a forced `walk_fast` step towards `Dir`, facing kept
    /// (`ForcedMovement_Slide*`).
    Slide(Dir),
    /// Gen3 `MB_MUDDY_SLOPE`: slides the player south, facing kept, unless they climb it north
    /// at the Mach Bike's top speed (`ForcedMovement_MuddySlope`; no bike yet: always slides).
    MuddySlope,
    /// Gen3 `MB_THIN_ICE`: plain floor that a map's [`StepCallback::Ice`] cracks.
    ThinIce,
    /// Gen3 `MB_CRACKED_ICE`: plain floor that a map's [`StepCallback::Ice`] breaks.
    CrackedIce,
    /// Gen3 `MB_CRACKED_FLOOR`: a forced-movement tile without a movement (no step triggers,
    /// no encounters on it); a map's [`StepCallback::CrackedFloor`] turns it into a hole.
    CrackedFloor,
    /// Gen3 `MB_CRACKED_FLOOR_HOLE`: finishing a step on it runs [`GameData::fall_script`]
    /// (Gen3 `TryStartMiscWalkingScripts`, `EventScript_FallDownHole`).
    CrackedFloorHole,
    /// Walkable ground whose raw Gen3 behaviour matters to wild encounters or the battle
    /// background: the land-encounter tiles besides tall grass, and sand and mountain tops
    /// (see [`Floor`]).
    ///
    /// Appended last, so no serialized (postcard) index shifts. **Merge order:** the
    /// forced-movement branch appends `Ice` … `CrackedFloorHole` (indices 12–19). This variant
    /// goes after them (index 20): its hash code is already 20 (`world::battle::behavior_code`),
    /// and `checkpoint::tests` pins it as the last variant. No committed checkpoint holds it yet.
    Floor(Floor),
    /// Surfable water (Gen3 `TILE_FLAG_SURFABLE`; see [`Water`]). Not a wall: as in Gen3, a
    /// player on foot cannot step onto it because water cells are elevation 1 and land cells
    /// elevation 3 (`IsElevationMismatchAt`); a surfing player moves on it (Milestone 10's Surf,
    /// pulled forward in Wave C).
    ///
    /// Appended after [`Behavior::Floor`] (postcard index 21, hash code 21).
    Water(Water),
    /// A [`Behavior::Warp`] that keeps the current map's object templates when it lands on the
    /// same map (Gen3 `MB_MOSSDEEP_GYM_WARP`, `DoMossdeepGymWarp`: `SKIP_OBJECT_EVENT_LOAD`,
    /// so the statues the gym's switches moved stay moved). Wave F, appended (postcard index
    /// 22, hash code 22).
    PadWarp,
    /// A warp cell the player pops out of when a warp from below lands on it (Gen3
    /// `MB_LAVARIDGE_GYM_1F_WARP`, the B1F geysers' destinations: `DoLavaridgeGymB1FWarp`,
    /// `FieldCB_LavaridgeGymB1FWarpExit`): entering it triggers its warp; arriving on one, the
    /// player is hidden during the fade-in, then jumps one cell `Dir`
    /// (`GetJumpMovementAction`), without the ash puff.
    ///
    /// Appended after [`Behavior::PadWarp`] (postcard index 23, hash code 23; Wave E2).
    PopOutWarp(Dir),
}

/// The raw Gen3 surfable behaviours behind [`Behavior::Water`], one variant each (the
/// new-metatile check compares raw behaviours). Append-only (postcard).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Water {
    /// `MB_POND_WATER`: encounters; the pond background.
    Pond,
    /// `MB_INTERIOR_DEEP_WATER`: encounters, diveable.
    InteriorDeep,
    /// `MB_DEEP_WATER`: encounters, diveable.
    Deep,
    /// `MB_WATERFALL`: surfable, no encounters (climbing it is Waterfall's, not modelled).
    Waterfall,
    /// `MB_SOOTOPOLIS_DEEP_WATER`: no encounters, diveable.
    SootopolisDeep,
    /// `MB_OCEAN_WATER`: encounters.
    Ocean,
    /// `MB_NO_SURFACING` (underwater): no encounters.
    NoSurfacing,
    /// `MB_SEAWEED` (underwater): encounters.
    Seaweed,
    /// `MB_SEAWEED_NO_SURFACING` (underwater): encounters.
    SeaweedNoSurfacing,
    /// `MB_{EAST,WEST,NORTH,SOUTH}WARD_CURRENT`: pushes a surfing player towards `Dir`
    /// (`ForcedMovement_Pushed*ByCurrent`).
    Current(Dir),
    /// `MB_UNUSED_6F`: surfable, nothing else.
    Unused6F,
    /// `MB_WATER_DOOR`: surfable, and a non-animated door (Gen3 `MetatileBehavior_IsNonAnimDoor`):
    /// surfing onto it takes its warp; arriving on one faces south and steps out
    /// (`Task_ExitNonAnimDoor`). No encounters.
    Door,
    /// `MB_WATER_SOUTH_ARROW_WARP`: surfable, and a south arrow warp
    /// (`MetatileBehavior_IsSouthArrowWarp`): pushing south on it takes its warp. No encounters.
    SouthArrowWarp,
}

impl Water {
    /// Gen3 `TILE_FLAG_HAS_ENCOUNTERS` (`MetatileBehavior_IsWaterWildEncounter`).
    pub const fn has_encounters(self) -> bool {
        matches!(
            self,
            Water::Pond
                | Water::InteriorDeep
                | Water::Deep
                | Water::Ocean
                | Water::Seaweed
                | Water::SeaweedNoSurfacing
        )
    }

    /// Gen3 `MetatileBehavior_IsSurfableFishableWater`: what Surf is used on (the A press) and
    /// fishing rods cast into.
    pub const fn fishable(self) -> bool {
        matches!(
            self,
            Water::Pond
                | Water::Ocean
                | Water::InteriorDeep
                | Water::Deep
                | Water::SootopolisDeep
                | Water::Current(_)
        )
    }

    /// One byte per variant, for hashes.
    pub const fn code(self) -> u8 {
        match self {
            Water::Pond => 0,
            Water::InteriorDeep => 1,
            Water::Deep => 2,
            Water::Waterfall => 3,
            Water::SootopolisDeep => 4,
            Water::Ocean => 5,
            Water::NoSurfacing => 6,
            Water::Seaweed => 7,
            Water::SeaweedNoSurfacing => 8,
            Water::Unused6F => 9,
            Water::Door => 10,
            Water::SouthArrowWarp => 11,
            Water::Current(d) => 16 + d as u8,
        }
    }

    /// Gen3 `MetatileBehavior_IsDiveable`: Dive goes down from it.
    pub const fn diveable(self) -> bool {
        matches!(
            self,
            Water::InteriorDeep | Water::Deep | Water::SootopolisDeep
        )
    }

    /// Gen3 `MetatileBehavior_IsUnableToEmerge` (without the `BUGFIX` water door): Dive does
    /// not surface from it.
    pub const fn unable_to_emerge(self) -> bool {
        matches!(self, Water::NoSurfacing | Water::SeaweedNoSurfacing)
    }

    /// Gen3 `MetatileBehavior_IsDeepOrOceanWater` (the water battle background).
    pub const fn deep_or_ocean(self) -> bool {
        matches!(self, Water::Ocean | Water::InteriorDeep | Water::Deep)
    }
}

/// The raw Gen3 behaviours behind [`Behavior::Floor`], one variant each: Gen3's new-metatile
/// check (`StandardWildEncounter`'s `prevMetatileBehavior != curMetatileBehavior`) compares raw
/// behaviours, so two encounter tiles must never share a variant. Append-only (postcard).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Floor {
    /// `MB_LONG_GRASS` (Routes 119, 120): encounters; the long-grass battle background.
    LongGrass,
    /// `MB_UNUSED_05`: encounters (Gen3 flags it, no map uses it).
    Unused05,
    /// `MB_DEEP_SAND` (Route 111's desert): encounters; the sand background.
    DeepSand,
    /// `MB_CAVE` (every cave floor): encounters.
    Cave,
    /// `MB_INDOOR_ENCOUNTER` (New Mauville, the Abandoned Ship's rooms): encounters; the building
    /// background underground.
    IndoorEncounter,
    /// `MB_ASHGRASS` (Route 113, Jagged Pass): encounters (the ash pickup, which turns the cell
    /// into plain grass, is not modelled).
    AshGrass,
    /// `MB_FOOTPRINTS` (unused in Emerald's maps): encounters.
    Footprints,
    /// `MB_SAND` (beaches, the desert's edges): no encounters; the sand background.
    Sand,
    /// `MB_MOUNTAIN_TOP` (Mt Chimney's summit): no encounters; the mountain background.
    MountainTop,
}

impl Floor {
    /// Gen3 `TILE_FLAG_HAS_ENCOUNTERS` (none of these is surfable).
    pub const fn has_encounters(self) -> bool {
        !matches!(self, Floor::Sand | Floor::MountainTop)
    }
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
    /// Whether finishing a step on a cell of this behavior takes the cell's warp (Gen3
    /// `IsWarpMetatileBehavior`).
    pub const fn enters_warp(self) -> bool {
        matches!(
            self,
            Behavior::Door
                | Behavior::Warp
                | Behavior::PadWarp
                | Behavior::NonAnimDoor(_)
                | Behavior::Ladder
                | Behavior::Escalator(_)
                | Behavior::Water(Water::Door)
                | Behavior::PopOutWarp(_)
        )
    }

    /// The direction an arrow warp is taken by pushing (Gen3 `IsArrowWarpMetatileBehavior`):
    /// [`Behavior::ArrowWarp`], and the surfable south arrow warp.
    pub const fn arrow_warp(self) -> Option<Dir> {
        match self {
            Behavior::ArrowWarp(d) => Some(d),
            Behavior::Water(Water::SouthArrowWarp) => Some(Dir::Down),
            _ => None,
        }
    }

    /// Gen3 `MetatileBehavior_IsForcedMovementTile` (the behaviors classified here): on such a
    /// cell a finished step runs no step triggers and no encounter check, and the field input
    /// of the frame after it is ignored (`FieldGetPlayerInput`).
    pub const fn is_forced_tile(self) -> bool {
        matches!(
            self,
            Behavior::Ice
                | Behavior::ForcedWalk(_)
                | Behavior::Slide(_)
                | Behavior::MuddySlope
                | Behavior::CrackedFloor
                | Behavior::Water(Water::Current(_))
        )
    }

    /// Gen3 `MetatileBehavior_IsSurfableWaterOrUnderwater`.
    pub const fn surfable(self) -> bool {
        matches!(self, Behavior::Water(_))
    }

    /// Gen3 `MetatileBehavior_IsWaterWildEncounter`: surfing onto one draws the water-encounter
    /// rolls (`wild::try_water_encounter`).
    pub const fn water_encounter(self) -> bool {
        matches!(self, Behavior::Water(w) if w.has_encounters())
    }

    /// Gen3 `MetatileBehavior_IsLandWildEncounter`: an encounter tile that is not water (tall
    /// grass, and the [`Floor`]s that [`Floor::has_encounters`]). Walking onto one draws the
    /// land-encounter rolls (`wild::try_land_encounter`).
    pub const fn land_encounter(self) -> bool {
        match self {
            Behavior::TallGrass => true,
            Behavior::Floor(g) => g.has_encounters(),
            _ => false,
        }
    }

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
    // ---- Silice wave ----
    /// [`MovementType::Wander`] walking backwards: each step faces away from where it goes
    /// (a game rule: Silice's Reverse-City residents).
    WanderBackwards,
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
    /// Gen3 `MAP_TYPE_*` ([`map_type`]; Milestone 9: the escape warp and Escape Rope read it).
    pub map_type: u8,
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

/// Fixed UI strings the engine draws itself (Gen3-encoded, 0xFF-terminated). Content answers
/// every variant through [`GameData::ui_text`]; a string the game does not have is `[0xFF]`
/// (empty), never a panic.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UiText {
    Yes,
    No,
    /// Expansion of the `{RIVAL}` placeholder.
    RivalName,
    // ---- Milestone 9 (Wave 0 declares them; Task X2 extracts them, Task C answers them).
    // Task X2 may append variants for strings the screens print that are missing here. ----
    // Party menu (`party_menu.c`).
    PartySummary,
    PartySwitch,
    PartyItem,
    PartyGive,
    PartyTake,
    PartyCancel,
    PartyShift,
    PartyChoosePokemon,
    PartyMoveToWhere,
    PartyAble,
    PartyNotAble,
    PartyCantSwitchFainted,
    PartyAlreadyInBattle,
    // Bag (`item_menu.c`).
    BagUse,
    BagGive,
    BagToss,
    BagRegister,
    BagCancel,
    BagSell,
    PocketItems,
    PocketKeyItems,
    PocketPokeBalls,
    PocketTmHm,
    PocketBerries,
    BagTossHowMany,
    BagThrewAway,
    BagCantUseHere,
    // Shop (`shop.c`).
    ShopBuy,
    ShopSell,
    ShopQuit,
    ShopHowMany,
    ShopIsThatOkay,
    ShopNotEnoughMoney,
    ShopNoRoom,
    ShopThankYou,
    ShopPremierBonus,
    ShopAnythingElse,
    // PC (`player_pc.c`, `pokemon_storage_system.c`).
    PcItemStorage,
    PcMailbox,
    PcDecoration,
    PcTurnOff,
    PcWithdrawItem,
    PcDepositItem,
    PcTossItem,
    PcWithdrawMon,
    PcDepositMon,
    PcMoveMon,
    PcReleaseMon,
    PcBox,
    PcSentToBox,
    PcBoxFull,
    // Save (`start_menu.c`).
    SaveAsk,
    SaveOverwrite,
    SaveSaving,
    SaveDone,
    SaveError,
    // Options (`option_menu.c`).
    OptionTextSpeed,
    OptionBattleScene,
    OptionBattleStyle,
    OptionSound,
    OptionButtonMode,
    OptionFrame,
    OptionCancel,
    OptionSlow,
    OptionMid,
    OptionFast,
    OptionOn,
    OptionOff,
    OptionShift,
    OptionSet,
    OptionMono,
    OptionStereo,
    OptionNormal,
    OptionLr,
    OptionLEqualsA,
    OptionType,
    // Main menu (`main_menu.c`).
    MainContinue,
    MainNewGame,
    MainOption,
    MainPlayer,
    MainTime,
    MainPokedex,
    MainBadges,
    // Summary (`pokemon_summary_screen.c`).
    SummaryInfo,
    SummarySkills,
    SummaryMoves,
    SummaryOt,
    SummaryIdNo,
    SummaryItem,
    SummaryNone,
    SummaryNature,
    SummaryAbility,
    SummaryExpPoints,
    SummaryNextLv,
    SummaryPower,
    SummaryAccuracy,
    SummaryHp,
    SummaryAttack,
    SummaryDefense,
    SummarySpAtk,
    SummarySpDef,
    SummarySpeed,
    // Evolution (`evolution_scene.c`).
    EvolutionWhat,
    EvolutionCongrats,
    EvolutionStopped,
    // ---- Added by Task K/PT (the party menu's teach flow and messages; `strings.c` names) ----
    /// `gText_DoWhatWithPokemon`.
    PartyDoWhatWithMon,
    /// `gText_TeachWhichPokemon`.
    PartyTeachWhichMon,
    /// `gText_Learned` (the TM descriptor of a mon that knows the move).
    PartyLearned,
    /// `gText_PkmnLearnedMove3`: `{STR_VAR_1}` the mon, `{STR_VAR_2}` the move.
    PartyLearnedMove,
    /// `gText_PkmnCantLearnMove`.
    PartyCantLearnMove,
    /// `gText_PkmnAlreadyKnows`.
    PartyAlreadyKnows,
    /// `gText_PkmnNeedsToReplaceMove`.
    PartyNeedsToReplaceMove,
    /// `gText_WhichMoveToForget`.
    PartyWhichMoveToForget,
    /// `gText_12PoofForgotMove`: `{STR_VAR_2}` the forgotten move.
    PartyForgotMove,
    /// `gText_StopLearningMove2`.
    PartyStopLearningMove,
    /// `gText_MoveNotLearned`.
    PartyMoveNotLearned,
    /// `gText_PkmnNotHolding`.
    PartyNotHolding,
    /// `gText_ReceivedItemFromPkmn`: `{STR_VAR_1}` the mon, `{STR_VAR_2}` the item.
    PartyReceivedItemFromMon,
    /// `gText_HMMovesCantBeForgotten2`.
    SummaryHmCantForget,
    // ---- Added by Task B (the bag and item use; `strings.c` names) ----
    /// `gText_CloseBag`: the list's last entry.
    BagCloseBag,
    /// `gText_Var1CantBeHeld`: `{STR_VAR_1}` the item.
    BagCantBeHeld,
    /// `gText_ConfirmTossItems`: `{STR_VAR_1}` the item, `{STR_VAR_2}` the quantity.
    BagConfirmToss,
    /// `gText_MenuDeselect` (REGISTER on the registered item).
    BagDeselect,
    /// `gText_BootedUpTM`.
    BagBootedUpTm,
    /// `gText_BootedUpHM`.
    BagBootedUpHm,
    /// `gText_TMHMContainedVar1`: `{STR_VAR_1}` the move.
    BagTmContains,
    /// `gText_HowManyToSell`: `{STR_VAR_2}` the item.
    BagSellHowMany,
    /// `gText_ICanPayVar1`: `{STR_VAR_1}` the price.
    BagSellOffer,
    /// `gText_TurnedOverVar1ForVar2`: `{STR_VAR_2}` the item, `{STR_VAR_1}` the price.
    BagSold,
    /// `gText_CantBuyKeyItem`: `{STR_VAR_2}` the item.
    BagCantBuy,
    /// `gText_PlayerUsedVar2`: `{STR_VAR_2}` the item.
    ItemUsed,
    /// `gText_RepelEffectsLingered`.
    ItemRepelLingers,
    /// `gText_WontHaveEffect`.
    ItemNoEffect,
    /// `gText_PkmnHPRestoredByVar2`: `{STR_VAR_1}` the mon, `{STR_VAR_2}` the HP.
    ItemHpRestored,
    /// `gText_PkmnCuredOfPoison`.
    ItemCuredPoison,
    /// `gText_PkmnWokeUp2`.
    ItemWokeUp,
    /// `gText_PkmnBurnHealed`.
    ItemBurnHealed,
    /// `gText_PkmnThawedOut`.
    ItemThawed,
    /// `gText_PkmnCuredOfParalysis`.
    ItemCuredParalysis,
    /// `gText_PkmnBecameHealthy`.
    ItemBecameHealthy,
    /// `gText_PPWasRestored`.
    ItemPpRestored,
    /// `gText_PkmnElevatedToLvVar2`: `{STR_VAR_1}` the mon, `{STR_VAR_2}` the level.
    ItemElevatedToLevel,
    /// `gText_PkmnBaseVar2StatIncreased`: `{STR_VAR_1}` the mon, `{STR_VAR_2}` the stat.
    ItemStatIncreased,
    /// `gText_MovesPPIncreased`.
    ItemPpIncreased,
    // ---- Added by Wave F (the party menu's choose-half mode; `strings.c` names) ----
    /// `gText_Enter` (`MENU_ENTER`).
    PartyEnter,
    /// `gText_NoEntry` (`MENU_NO_ENTRY`).
    PartyNoEntry,
    /// `gMenuText_Confirm` (the CONFIRM button).
    PartyConfirm,
    /// `gText_Able3` (a mon that may enter).
    PartyAble3,
    /// `gText_First_PM`, `gText_Second_PM`, `gText_Third_PM`, `gText_Fourth`: the entry order.
    PartyFirst,
    PartySecond,
    PartyThird,
    PartyFourth,
    /// `gText_NoMoreThanVar1Pkmn`: `{STR_VAR_1}` the count.
    PartyNoMoreThan,
    /// `gText_NoPokemonForBattle`.
    PartyNoMonForBattle,
    /// `gText_CancelBattle`.
    PartyCancelBattle,
}

/// One start-menu entry (Gen3 `sStartMenuItems`), in display order (Milestone 9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StartItem {
    pub label: TextId,
    pub action: StartAction,
    /// Shown only while this flag is set (Gen3 `BuildNormalStartMenu`'s `FLAG_SYS_*_GET`).
    pub shown_if: Option<FlagId>,
    /// The help line printed for the entry, if the game has one.
    pub description: Option<TextId>,
}

/// What a [`StartItem`] does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartAction {
    Pokedex,
    Party,
    Bag,
    PlayerCard,
    Save,
    Options,
    Exit,
    /// Game-defined (Silice's Mallette, Emerald's PokéNav placeholder): runs the script with
    /// the field locked, after the menu closes (Gen3 `ScriptContext_SetupScript`).
    Script(ScriptId),
}

/// A field move (Gen3 `FIELD_MOVE_*`); Milestone 10 implements the uses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FieldMove {
    Cut,
    Flash,
    RockSmash,
    Strength,
    Surf,
    Fly,
    Dive,
    Waterfall,
    Teleport,
    Dig,
    SecretPower,
    SoftBoiled,
    MilkDrink,
    SweetScent,
}

impl FieldMove {
    /// Gen3 `FIELD_MOVE_*` order.
    pub const ALL: [FieldMove; 14] = [
        FieldMove::Cut,
        FieldMove::Flash,
        FieldMove::RockSmash,
        FieldMove::Strength,
        FieldMove::Surf,
        FieldMove::Fly,
        FieldMove::Dive,
        FieldMove::Waterfall,
        FieldMove::Teleport,
        FieldMove::Dig,
        FieldMove::SecretPower,
        FieldMove::SoftBoiled,
        FieldMove::MilkDrink,
        FieldMove::SweetScent,
    ];
}

/// How a field move becomes usable (M10 implements the uses).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldMoveGate {
    /// The game has no such field move.
    Never,
    /// A party mon knows `move_id`, and the badge flag is set when one is given (Gen3
    /// `sFieldMoves` and the HM badge checks).
    PartyMove {
        move_id: MoveId,
        badge: Option<FlagId>,
    },
    /// Usable while the flag is set, whatever the party knows (Silice's modules).
    Flag(FlagId),
}

/// A medicine's effect (Gen3 `gItemEffectTable` bytes, decoded by content).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MedicineEffect {
    /// Restores this much HP (`u16::MAX`: to full).
    Heal(u16),
    /// Cures these status bits (Gen3 `ITEM3_*` order: sleep, poison, burn, freeze, paralysis,
    /// confusion, infatuation; one bit each).
    Cure(u8),
    /// [`MedicineEffect::Heal`] and [`MedicineEffect::Cure`] together (Full Restore).
    HealCure(u16, u8),
    /// Revives with this fraction of the max HP, in 1/256ths (Revive 128, Max Revive 256).
    Revive(u16),
    /// Restores PP: `(amount, every move)` (`u8::MAX` amount: to full).
    Pp(u8, bool),
    /// Rare Candy: one level.
    LevelUp,
    /// A vitamin: `(stat index in Stat order, EVs)`.
    Ev(u8, u8),
    /// PP Up (`false`) or PP Max (`true`).
    PpUp(bool),
}

/// What a key item does when used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyUse {
    /// The bikes (M10).
    Bike,
    /// A rod (M10): `0` old, `1` good, `2` super.
    Rod(u8),
    /// Runs this script (content-defined uses).
    Script(ScriptId),
    /// No field use (Gen3 `ItemUseOutOfBattle_CannotUse`).
    None,
    /// Any other key use, by its Gen3 function name.
    Other(&'static str),
}

/// What using an item does (content classifies it from Gen3 `gItemEffectTable` and the item's
/// field and battle use functions; Milestone 9 Task C).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemUse {
    None,
    Medicine(MedicineEffect),
    /// A ball, with its catch multiplier ×10.
    Ball(u8),
    /// An X item or Guard Spec: `(stat index, stages)`.
    StatStage(u8, i8),
    /// An evolution stone (any item that evolves through `EVO_ITEM`).
    Evolution,
    /// An item that triggers trade evolutions, used as a stone (a game rule: Silice's PIERRE
    /// LIEN): `EVO_TRADE`, and `EVO_TRADE_ITEM` while the mon holds the item, which is used
    /// up with it.
    TradeEvolution,
    /// A repel lasting this many steps.
    Repel(u16),
    EscapeRope,
    /// A TM or HM teaching this move.
    Tm(MoveId),
    Berry,
    Key(KeyUse),
    /// Anything else, by its Gen3 name (the bag reports it).
    Other(&'static str),
}

/// One extracted UI picture (Milestone 9 Task X1 extracts them, Task C answers them); Task X1
/// may append variants.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UiGfx {
    PartyBg,
    PartySlots,
    PartyHpBar,
    BagBg,
    /// The bag sprite of pocket `n`.
    BagSprite(u8),
    SummaryPages,
    ShopBg,
    OptionsBg,
    EvolutionBg,
    MainMenuWindows,
    StatusIcons,
    TypeIcons,
    ScrollArrows,
    MenuCursor,
    MoneyBox,
}

/// A full-screen picture: 4bpp tiles (32 bytes each), a tilemap (u16 entries as little-endian
/// bytes, 32 entries a row) and its palettes (16 colours a bank).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UiPicture {
    pub tiles: &'static [u8],
    pub map: &'static [u8],
    pub palette: &'static [u16],
}

/// An item or mon icon: `frames` frames of 32x32 4bpp tiles in 1D OBJ order, and its palette.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Icon {
    pub tiles: &'static [u8],
    pub palette: &'static [u16],
    pub frames: u8,
}

/// A Gen3 `sIngameTrades` entry (Milestone 9: Task R makes the swap, Task J the specials).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InGameTrade {
    /// The species the NPC asks for.
    pub requested: SpeciesId,
    /// The species the NPC gives.
    pub species: SpeciesId,
    /// Gen3-encoded nickname, 0xFF-terminated.
    pub nickname: &'static [u8],
    pub ivs: [u8; 6],
    pub ability_slot: u8,
    pub personality: u32,
    /// Gen3-encoded OT name, 0xFF-terminated.
    pub ot_name: &'static [u8],
    pub ot_id: u32,
    pub held: Option<ItemId>,
    /// The mail item the traded mon holds (mail itself is not modelled).
    pub mail: Option<ItemId>,
}

/// Gen3 `MAP_TYPE_*` (`MapData::map_type`; Milestone 9 Task F reads it, content fills it).
pub mod map_type {
    pub const NONE: u8 = 0;
    pub const TOWN: u8 = 1;
    pub const CITY: u8 = 2;
    pub const ROUTE: u8 = 3;
    pub const UNDERGROUND: u8 = 4;
    pub const UNDERWATER: u8 = 5;
    pub const OCEAN_ROUTE: u8 = 6;
    pub const UNKNOWN: u8 = 7;
    pub const INDOOR: u8 = 8;
    pub const SECRET_BASE: u8 = 9;
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

/// Game constants the engine needs by role, never by name (content fills them in). The
/// default is a game without any: no vars or flags, no money, no bag, and a multichoice
/// cancel answer (`u16::MAX`) that no list index can take.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    // ---- Milestone 9 (a game without them gets 0 / `None` and the feature stays off) ----
    /// PC boxes (Gen3 `TOTAL_BOXES_COUNT`, 14), each of [`crate::pc::BOX_SLOTS`] mons; 0: no
    /// Pokémon storage.
    pub pc_boxes: u8,
    /// PC item slots (Gen3 `PC_ITEMS_COUNT`, 50); 0: no item storage.
    pub pc_item_slots: u8,
    /// The coin cap (Gen3 `MAX_COINS`, 9,999); 0: no coins.
    pub max_coins: u16,
    /// The badge flags, badge 1 first (Gen3 `FLAG_BADGE01_GET`..`FLAG_BADGE08_GET`).
    pub flag_badges: [Option<FlagId>; 8],
    /// Repel steps left (Gen3 `VAR_REPEL_STEP_COUNT`).
    pub var_repel_steps: Option<VarId>,
    /// The walking friendship counter (Gen3 `VAR_FRIENDSHIP_STEP_COUNTER`).
    pub var_friendship_steps: Option<VarId>,
    /// Run when a repel wears off (Gen3 `EventScript_RepelWoreOff`).
    pub script_repel_wore_off: Option<ScriptId>,
    /// The CONTINUE window shows the Pokédex line while this flag is set (Gen3
    /// `FLAG_SYS_POKEDEX_GET`).
    pub flag_pokedex: Option<FlagId>,
    /// The box the next mon sent to the PC tries first, set to the box it went to (Gen3
    /// `VAR_PC_BOX_TO_SEND_MON`, `CopyMonToPC`).
    pub var_pc_box_to_send_mon: Option<VarId>,
    /// Box and slot of the last mon sent to the PC (Gen3 `gSpecialVar_MonBoxId`,
    /// `gSpecialVar_MonBoxPos`: `VAR_MON_BOX_ID`, `VAR_MON_BOX_POS`).
    pub var_mon_box_id: Option<VarId>,
    pub var_mon_box_pos: Option<VarId>,
    /// Cleared when a mon goes to another box than `var_pc_box_to_send_mon` named (Gen3
    /// `FLAG_SHOWN_BOX_WAS_FULL_MESSAGE`).
    pub flag_shown_box_was_full: Option<FlagId>,
    // ---- Parity waves ----
    /// Counted up at every wild battle's start (Gen3 `IncrementDailyWildBattles`, `tv.c`, from
    /// every `BattleSetup_Start*WildBattle` and `CB2_StartFirstBattle`: `VAR_DAILY_WILDS`).
    pub var_daily_wilds: Option<VarId>,
    /// Berry trees kept (Gen3 `BERRY_TREES_COUNT`, 128; [`crate::berry`]); 0: none.
    pub berry_trees: u8,
    // ---- Silice wave (game rules Gen3 does not have; `None` keeps them off) ----
    /// The field's d-pad mapping ([`crate::InputMap::code`]; 0 is the normal walking), set by
    /// [`crate::script::Request::SetInputMap`] and cleared on every map change (the map's
    /// scripts set it again). Silice's inverted controls.
    pub var_input_map: Option<VarId>,
    /// Counts the field frames the player could act and held no button (saturating; any
    /// button resets it to 0), for scripts to read (Silice's idle timers).
    pub var_idle_frames: Option<VarId>,
}

/// What [`GameData::step_count_script`] reads and may change, after a finished player step.
pub struct StepCount<'a> {
    pub map: MapId,
    /// The player's cell, where the step ended.
    pub cell: (i16, i16),
    pub vars: &'a mut crate::flow::Vars,
    pub party: &'a mut mon::Party,
}

impl Default for SpecialIds {
    fn default() -> Self {
        SpecialIds {
            var_facing: None,
            var_last_talked: None,
            flag_running: None,
            flags_cleared_on_map_change: &[],
            respawn_default: 0,
            var_wild_set: None,
            script_whiteout: None,
            max_money: 0,
            multichoice_cancel: u16::MAX,
            pocket_slots: [0; 5],
            pc_boxes: 0,
            pc_item_slots: 0,
            max_coins: 0,
            flag_badges: [None; 8],
            var_repel_steps: None,
            var_friendship_steps: None,
            script_repel_wore_off: None,
            flag_pokedex: None,
            var_pc_box_to_send_mon: None,
            var_mon_box_id: None,
            var_mon_box_pos: None,
            flag_shown_box_was_full: None,
            var_daily_wilds: None,
            berry_trees: 0,
            var_input_map: None,
            var_idle_frames: None,
        }
    }
}

/// Everything game-specific the engine reads. Implemented by the `content` crate.
pub trait GameData {
    /// A map. Ids the game does not have may return any 0x0 map (see [`GameData::has_map`]).
    fn map(&self, id: MapId) -> &'static MapData;
    /// Whether `id` is a real map; warps to other maps are logged and ignored.
    fn has_map(&self, id: MapId) -> bool {
        self.map(id).width > 0
    }
    /// Map `id` with layout `layout` instead of its own (Gen3 `setmaplayoutindex`, a
    /// `gMapLayouts` number): its header and events with that layout's size, blocks, border and
    /// tilesets. `None` when the game has no such variant: the map keeps its own layout.
    fn map_layout(&self, id: MapId, layout: u16) -> Option<&'static MapData> {
        let _ = (id, layout);
        None
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
    /// The in-game partner of the multi battle (Wave G: Steven, `FillPartnerParty`), if the
    /// game has one.
    fn partner(&self) -> Option<&'static crate::mon::PartnerData> {
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

    // ---- Milestone 9 (Wave 0 declares them; Task C answers them from Tasks X1 and X2's
    // extract). Every screen works with the defaults: frames and text only. ----

    /// The start menu's entries in display order (Gen3 `sStartMenuItems`); empty: START opens
    /// nothing.
    fn start_menu(&self) -> &'static [StartItem] {
        &[]
    }
    /// How field move `m` becomes usable.
    fn field_move_gate(&self, m: FieldMove) -> FieldMoveGate {
        let _ = m;
        FieldMoveGate::Never
    }
    fn item_use(&self, id: ItemId) -> ItemUse {
        let _ = id;
        ItemUse::None
    }
    /// The mart's purchase bonus `(item, at least, gift)`: buying at least that many of `item`
    /// at once adds one `gift` (Gen3 `Task_ReturnToItemListAfterItemPurchase`: 10 Poké Balls,
    /// a Premier Ball). `None`: no bonus.
    fn mart_bonus(&self) -> Option<(ItemId, u16, ItemId)> {
        None
    }
    /// Gen3-encoded, 0xFF-terminated item description (`gItems[].description`).
    fn item_description(&self, id: ItemId) -> Option<&'static [u8]> {
        let _ = id;
        None
    }
    /// Gen3-encoded, 0xFF-terminated move description (`gMoveDescriptionPointers`).
    fn move_description(&self, id: MoveId) -> Option<&'static [u8]> {
        let _ = id;
        None
    }
    /// Whether `m` is an HM move, which the learn-move pick refuses to forget (Gen3
    /// `IsHMMove2`, `sHMMoves`).
    fn is_hm_move(&self, m: MoveId) -> bool {
        let _ = m;
        false
    }
    /// The move a TM or HM item teaches.
    fn tm_move(&self, id: ItemId) -> Option<MoveId> {
        let _ = id;
        None
    }
    /// Gen3 `CanMonLearnTMHM` (`sTMHMLearnsets`).
    fn can_learn_tm(&self, species: SpeciesId, tm: ItemId) -> bool {
        let _ = (species, tm);
        false
    }
    fn ui_gfx(&self, g: UiGfx) -> Option<&'static UiPicture> {
        let _ = g;
        None
    }
    /// User window frame `n` (0..20, Gen3 `gUserFrames`); `None`: the dialogue frame.
    fn window_frame(&self, n: u8) -> Option<&'static WindowGfx> {
        let _ = n;
        None
    }
    fn item_icon(&self, id: ItemId) -> Option<&'static Icon> {
        let _ = id;
        None
    }
    fn mon_icon(&self, species: SpeciesId) -> Option<&'static Icon> {
        let _ = species;
        None
    }
    /// Gen3 `sIngameTrades[n]`.
    fn in_game_trade(&self, n: u8) -> Option<&'static InGameTrade> {
        let _ = n;
        None
    }
    /// Gen3-encoded, 0xFF-terminated name of nature `n` (`gNatureNamePointers`).
    fn nature_name(&self, n: u8) -> Option<&'static [u8]> {
        let _ = n;
        None
    }

    // ---- forced movement and field puzzles (every default: none) ----

    /// The per-step callback map `map` turns on (Gen3 `setstepcallback` in its `on_resume`
    /// script): the Sootopolis-style ice puzzle or cracked floors ([`StepCallback`]).
    fn step_callback(&self, map: MapId) -> Option<StepCallback> {
        let _ = map;
        None
    }
    /// The map a hole of `map` drops the player to, onto the same cell (Gen3 `setholewarp` in
    /// its `on_resume` script; read by `warphole` without a map, `Request::WarpHole(None)`).
    fn hole_warp(&self, map: MapId) -> Option<MapId> {
        let _ = map;
        None
    }
    /// Run when a player step finishes on a [`Behavior::CrackedFloorHole`] cell (Gen3
    /// `EventScript_FallDownHole`, from `TryStartMiscWalkingScripts`).
    fn fall_script(&self) -> Option<ScriptId> {
        None
    }
    /// Gen3 `TryStartStepCountScript`'s game checks (`field_control_avatar.c`), once per
    /// finished player step that started no coord, warp or walking script, after the friendship
    /// counter and before the repel counter: the game's step counters (poison, Match Call
    /// registrations, …), which may change the vars and the party. Returns the script to start
    /// (Gen3 `ScriptContext_SetupScript`), which then skips the repel counter and the wild
    /// encounter of this step.
    fn step_count_script(&self, step: StepCount<'_>) -> Option<ScriptId> {
        let _ = step;
        None
    }

    // ---- off-grass encounters ----

    /// Gen3 `gMapHeader.mapType` of `map`, as `BattleSetup_GetEnvironmentId` reads it (the
    /// battle background). The default, [`MapType::Outdoor`], is a route's.
    fn map_type(&self, map: MapId) -> MapType {
        let _ = map;
        MapType::Outdoor
    }

    // ---- Wave C: field moves (Milestone 10 pulled forward) ----

    /// The script the field starts for field move `m` (Gen3 `GetInteractedWaterScript`'s
    /// `EventScript_UseSurf` for [`FieldMove::Surf`], run when the player presses A facing
    /// surfable water with the move's [`GameData::field_move_gate`] open). `None`: the move is
    /// never offered from the field.
    fn field_move_script(&self, m: FieldMove) -> Option<ScriptId> {
        let _ = m;
        None
    }

    /// Gen3 `EventScript_CannotUseWaterfall`: run on an A press facing a waterfall when
    /// Waterfall is not usable there (no badge, or not surfing north).
    fn waterfall_refused_script(&self) -> Option<ScriptId> {
        None
    }

    /// Gen3 `EventScript_UseDiveUnderwater`: run on a B press underwater where Dive can surface
    /// (`TrySetupDiveEmergeScript`). [`GameData::field_move_script`]`(Dive)` is the way down
    /// (`EventScript_UseDive`, an A press on diveable water).
    fn dive_emerge_script(&self) -> Option<ScriptId> {
        None
    }

    /// Where Dive leads from `map` (Gen3 `SetDiveWarp`): `emerge` up from an underwater map,
    /// else down. The map's dive or emerge connection keeps the player's cell (`None`
    /// coordinates); else the fixed dive warp its `on_resume` script sets (`setdivewarp`, here
    /// answered per map as [`GameData::hole_warp`]). `None`: no dive warp.
    fn dive_warp(&self, map: MapId, emerge: bool) -> Option<(MapId, Option<(i16, i16)>)> {
        let _ = (map, emerge);
        None
    }

    /// Fly destination `i` of the fly map (Gen3 `CB2_OpenFlyMap`'s town and city sections,
    /// region-map order), `None` past the last. The picker lists those whose `visited` flag is
    /// set (Gen3 `GetMapsecType`'s `FLAG_VISITED_*`).
    fn fly_destination(&self, i: u8) -> Option<FlyDestination> {
        let _ = i;
        None
    }

    /// The water table of header `wild_set` of `map` (Gen3 `waterMonsInfo`, 5 slots), as
    /// [`GameData::wild_encounters`] for land.
    fn water_encounters(&self, map: MapId, wild_set: u16) -> Option<&'static mon::WaterEncounters> {
        let _ = (map, wild_set);
        None
    }

    /// The rock smash table of header `wild_set` of `map` (Gen3 `rockSmashMonsInfo`).
    fn rock_smash_encounters(
        &self,
        map: MapId,
        wild_set: u16,
    ) -> Option<&'static mon::WaterEncounters> {
        let _ = (map, wild_set);
        None
    }

    /// Strength's boulders: their object graphics and the flag that lets the player push them
    /// (Gen3 `OBJ_EVENT_GFX_PUSHABLE_BOULDER`, `FLAG_SYS_USE_STRENGTH`; `TryPushBoulder`).
    fn pushable_boulder(&self) -> Option<(GfxId, FlagId)> {
        None
    }

    /// The player's surfing sprite (Gen3 `PLAYER_AVATAR_STATE_SURFING`); `None` draws the
    /// walking one.
    fn player_surf_sprite(&self) -> Option<&'static SpriteData> {
        None
    }
}

/// One place Fly lands ([`GameData::fly_destination`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FlyDestination {
    /// Gen3-encoded, 0xFF-terminated (the region map section's name).
    pub name: &'static [u8],
    /// Listed once set (`FLAG_VISITED_*`).
    pub visited: FlagId,
    /// Where Fly lands: map and cell (Gen3 `SetWarpDestinationToHealLocation`).
    pub map: MapId,
    pub x: i16,
    pub y: i16,
}

/// Gen3 `MAP_TYPE_*`, as far as the battle background tells them apart
/// (`BattleSetup_GetEnvironmentId`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapType {
    /// `MAP_TYPE_TOWN`, `CITY`, `ROUTE` (and `NONE`, `UNKNOWN`, which Gen3's switch lets fall
    /// through the same way).
    Outdoor,
    /// An outdoor map whose plain ground fights on sand: Gen3's `MAP_ROUTE113` special case.
    /// (Its `WEATHER_SANDSTORM` case reads the current weather, which the engine does not
    /// model.)
    OutdoorSand,
    /// `MAP_TYPE_UNDERGROUND`: caves (the building background on `MB_INDOOR_ENCOUNTER`).
    Underground,
    /// `MAP_TYPE_INDOOR`, `MAP_TYPE_SECRET_BASE`.
    Indoor,
    /// `MAP_TYPE_UNDERWATER`.
    Underwater,
    /// `MAP_TYPE_OCEAN_ROUTE`: plain off the water.
    OceanRoute,
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
