//! Static game data the engine consumes. Content builds these as `'static` tables (via build.rs);
//! the engine never owns or parses files.

use crate::Tile;
use crate::script::{Ctx, Script};

/// Facing / movement direction. Order matches Gen3 `DIR_SOUTH..DIR_EAST` minus one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
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

macro_rules! id {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {$(
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
}

/// Object graphics: fixed, or read from a var when the object spawns (`OBJ_EVENT_GFX_VAR_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gfx {
    Fixed(GfxId),
    Var(VarId),
}

/// Idle behavior of an object (subset of Gen3 `MOVEMENT_TYPE_*`; content maps the rest to
/// the closest variant).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
}

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

/// Everything game-specific the engine reads. Implemented by the `content` crate.
pub trait GameData {
    fn map(&self, id: MapId) -> &'static MapData;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_fields() {
        let b = 0x3000 | 0x0400 | 0x01D5;
        assert_eq!(block_metatile(b), 0x1D5);
        assert_eq!(block_collision(b), 1);
        assert_eq!(block_elevation(b), 3);
    }

    #[test]
    fn attr_fields() {
        assert_eq!(attr_behavior(0x1023), 0x23);
        assert_eq!(attr_layer(0x0000), LayerType::Normal);
        assert_eq!(attr_layer(0x1000), LayerType::Covered);
        assert_eq!(attr_layer(0x2000), LayerType::Split);
    }

    #[test]
    fn dir_delta_and_opposite() {
        for d in Dir::ALL {
            let (a, b) = (d.delta(), d.opposite().delta());
            assert_eq!((a.0 + b.0, a.1 + b.1), (0, 0));
        }
    }
}
