//! The editing model: a map document ([`Doc`]), the edit operations ([`Op`], also the edit
//! script format) and an [`Editor`] with undo and redo. No Dioxus, no files: the UI, the CLI
//! and tests drive the same calls.

use super::schema::{
    GameConnection, GameHeader, GameMap, GameObject, GameWarp, MAP_DYNAMIC, WARP_ID_DYNAMIC,
};
use super::stamp::Stamp;
use anyhow::{Result, bail, ensure};
use engine::data::{
    BLOCK_COLLISION_SHIFT, BLOCK_ELEVATION_SHIFT, BLOCK_METATILE_MASK, block_collision,
    block_elevation,
};
use extract::schema::{
    Bg, BgKind, ConnectionDir, Coord, Layout, MapFile, MapScripts, VarScript, from_rows, to_rows,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Undo steps kept.
pub const UNDO_LIMIT: usize = 256;

/// Elevation of a new map's cells and of new objects and coord events (Gen3 ground level).
pub const GROUND: u8 = 3;

/// A raw block from its parts (`metatile | collision << 10 | elevation << 12`).
pub const fn block(metatile: u16, collision: u8, elevation: u8) -> u16 {
    (metatile & BLOCK_METATILE_MASK)
        | ((collision as u16 & 3) << BLOCK_COLLISION_SHIFT)
        | ((elevation as u16 & 15) << BLOCK_ELEVATION_SHIFT)
}

/// One map being edited: its [`GameMap`] and its layout, with the blocks flat.
#[derive(Clone, Debug, PartialEq)]
pub struct Doc {
    pub map: GameMap,
    pub width: u16,
    pub height: u16,
    /// Extracted tileset names.
    pub primary: String,
    pub secondary: String,
    /// 2x2 raw border blocks, row-major.
    pub border: [u16; 4],
    /// `width * height` raw blocks, row-major.
    pub blocks: Vec<u16>,
    /// Behaviour warnings accepted on purpose (`AllowCollision` ops). Not saved with the map:
    /// they live in its edit scripts, which `GamesDir::load` reads them from.
    pub allow: Vec<Allowance>,
}

/// Cells or metatiles whose behaviour warnings (`behavior::issues`) are accepted, and why.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Allowance {
    #[serde(default)]
    pub cells: Vec<(i32, i32)>,
    #[serde(default)]
    pub metatiles: Vec<u16>,
    pub reason: String,
}

/// What a new map starts with (`cargo xtask studio map create <spec.ron>`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NewMap {
    pub name: String,
    /// The layout's name; the map's name when absent.
    #[serde(default)]
    pub layout: Option<String>,
    /// Use the saved layout `layout` (its size, tilesets, border and blocks) instead of a new
    /// one; the fields below are then ignored. A new map never replaces a saved layout.
    #[serde(default)]
    pub shared_layout: bool,
    #[serde(default)]
    pub width: u16,
    #[serde(default)]
    pub height: u16,
    #[serde(default)]
    pub primary: String,
    #[serde(default)]
    pub secondary: String,
    /// Metatile index every cell starts with.
    #[serde(default)]
    pub fill: u16,
    #[serde(default)]
    pub fill_collision: u8,
    #[serde(default = "ground")]
    pub fill_elevation: u8,
    /// 2x2 raw border blocks (a list of 4, row-major); four times the fill metatile when absent.
    #[serde(default)]
    pub border: Option<Vec<u16>>,
}

impl NewMap {
    pub fn layout_name(&self) -> String {
        self.layout.clone().unwrap_or_else(|| self.name.clone())
    }

    /// The raw block every cell starts with (ranges checked).
    pub fn fill_block(&self) -> Result<u16> {
        fill_block(self.fill, self.fill_collision, self.fill_elevation)
    }

    /// The map file of a new map: no events, the new map header.
    pub fn blank_map(&self) -> GameMap {
        GameMap {
            name: self.name.clone(),
            layout: self.layout_name(),
            header: GameHeader::new_map(),
            objects: Vec::new(),
            warps: Vec::new(),
            coords: Vec::new(),
            bgs: Vec::new(),
            connections: Vec::new(),
            scripts: MapScripts::default(),
        }
    }
}

/// Gen3 `MAX_MAP_DATA_SIZE`: `(width + 15) * (height + 14)` must not exceed it.
pub const MAX_MAP_DATA_SIZE: usize = 10240;

/// A size the game can load (nonzero, within [`MAX_MAP_DATA_SIZE`]).
pub fn check_size(width: u16, height: u16) -> Result<()> {
    let data = (width as usize + 15) * (height as usize + 14);
    ensure!(
        width > 0 && height > 0 && data <= MAX_MAP_DATA_SIZE,
        "size {width}x{height}: a map is at least 1x1 and (w + 15) * (h + 14) = {data} must \
         not exceed {MAX_MAP_DATA_SIZE}"
    );
    Ok(())
}

/// `metatile | collision | elevation` after the [`Brush`] range checks.
pub fn fill_block(metatile: u16, collision: u8, elevation: u8) -> Result<u16> {
    let b = Brush {
        metatile: Some(metatile),
        collision: Some(collision),
        elevation: Some(elevation),
    };
    b.check()?;
    Ok(b.apply(0))
}

fn ground() -> u8 {
    GROUND
}

fn border4(b: &[u16]) -> Result<[u16; 4]> {
    <[u16; 4]>::try_from(b).map_err(|_| anyhow::anyhow!("a border is 4 blocks, not {}", b.len()))
}

impl Doc {
    pub fn new(spec: &NewMap) -> Result<Doc> {
        ensure!(!spec.name.is_empty(), "a map needs a name");
        ensure!(
            spec.width > 0 && spec.height > 0,
            "{}: size {}x{}",
            spec.name,
            spec.width,
            spec.height
        );
        check_size(spec.width, spec.height)?;
        let fill = spec.fill_block()?;
        let border = match &spec.border {
            None => [spec.fill; 4],
            Some(b) => border4(b)?,
        };
        Ok(Doc {
            map: spec.blank_map(),
            width: spec.width,
            height: spec.height,
            primary: spec.primary.clone(),
            secondary: spec.secondary.clone(),
            border,
            blocks: vec![fill; spec.width as usize * spec.height as usize],
            allow: Vec::new(),
        })
    }

    /// From a map file and its layout file.
    pub fn from_parts(map: GameMap, layout: Layout) -> Result<Doc> {
        let blocks = from_rows(&layout.blocks);
        let border = from_rows(&layout.border);
        ensure!(
            blocks.len() == layout.width as usize * layout.height as usize
                && layout
                    .blocks
                    .iter()
                    .all(|r| r.0.len() == layout.width as usize),
            "{}: the layout's blocks are not {}x{}",
            map.name,
            layout.width,
            layout.height
        );
        ensure!(
            layout.border.len() == 2 && layout.border.iter().all(|r| r.0.len() == 2),
            "{}: the border is not 2x2",
            map.name
        );
        let border = border4(&border)?;
        Ok(Doc {
            map,
            width: layout.width,
            height: layout.height,
            primary: layout.primary,
            secondary: layout.secondary,
            border,
            blocks,
            allow: Vec::new(),
        })
    }

    /// An extracted map (`content/extracted/maps/<Name>.ron`) as a document: the layout is
    /// named after the map; the header's numbers become names. Every warp is kept (indices
    /// matter): `MAP_DYNAMIC` stays, other unexported targets become `UNEXPORTED_<group>_<num>`
    /// (a validation error). Connections to unexported maps are dropped.
    pub fn from_map_file(f: &MapFile) -> Result<Doc> {
        let map = GameMap {
            name: f.name.clone(),
            layout: f.name.clone(),
            header: super::schema::header_from_extract(&f.header),
            objects: f.objects.iter().map(GameObject::from_extract).collect(),
            warps: (f.warps.iter())
                .map(|w| GameWarp {
                    x: w.x,
                    y: w.y,
                    elevation: w.elevation,
                    dest_map: match &w.dest_map {
                        Some(n) => n.clone(),
                        None if w.dest_map_id == "127.127" => MAP_DYNAMIC.into(),
                        None => format!("UNEXPORTED_{}", w.dest_map_id.replace('.', "_")),
                    },
                    dest_warp: w.dest_warp,
                })
                .collect(),
            coords: f.coords.clone(),
            bgs: f.bgs.clone(),
            connections: (f.connections.iter())
                .filter_map(|c| {
                    Some(GameConnection {
                        dir: c.dir,
                        offset: c.offset,
                        map: c.map.clone()?,
                    })
                })
                .collect(),
            scripts: f.scripts.clone(),
        };
        Doc::from_parts(map, f.layout.clone())
    }

    /// The layout file.
    pub fn layout(&self) -> Layout {
        Layout {
            width: self.width,
            height: self.height,
            primary: self.primary.clone(),
            secondary: self.secondary.clone(),
            border: to_rows(&self.border, 2),
            blocks: to_rows(&self.blocks, self.width as usize),
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32
    }

    fn in_bounds64(&self, x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && x < self.width as i64 && y < self.height as i64
    }

    /// The raw block at (x, y), `None` outside the map.
    pub fn block(&self, x: i32, y: i32) -> Option<u16> {
        self.in_bounds(x, y)
            .then(|| self.blocks[y as usize * self.width as usize + x as usize])
    }

    fn set(&mut self, x: i32, y: i32, b: u16) {
        let w = self.width as usize;
        self.blocks[y as usize * w + x as usize] = b;
    }

    fn need(&self, x: i64, y: i64) -> Result<()> {
        ensure!(
            self.in_bounds64(x, y),
            "({x}, {y}) is outside the {}x{} map",
            self.width,
            self.height
        );
        Ok(())
    }

    /// Every event's position, for hit-testing: `(kind, index, x, y)`.
    pub fn events(&self) -> Vec<(EventKind, usize, i16, i16)> {
        let m = &self.map;
        let o = m.objects.iter().map(|e| (EventKind::Object, e.x, e.y));
        let w = m.warps.iter().map(|e| (EventKind::Warp, e.x, e.y));
        let c = m.coords.iter().map(|e| (EventKind::Coord, e.x, e.y));
        let b = m.bgs.iter().map(|e| (EventKind::Bg, e.x, e.y));
        let mut out = Vec::new();
        let mut n = BTreeMap::<EventKind, usize>::new();
        for (k, x, y) in o.chain(w).chain(c).chain(b) {
            let i = n.entry(k).or_default();
            out.push((k, *i, x, y));
            *i += 1;
        }
        out
    }

    /// The topmost event on a cell (bg, then coord, warp and object; the last added first).
    pub fn event_at(&self, x: i16, y: i16) -> Option<(EventKind, usize)> {
        let ev = self.events();
        ev.iter()
            .rev()
            .find(|e| e.2 == x && e.3 == y)
            .map(|e| (e.0, e.1))
    }

    pub fn event(&self, kind: EventKind, index: usize) -> Option<Event> {
        let m = &self.map;
        match kind {
            EventKind::Object => m.objects.get(index).cloned().map(Event::Object),
            EventKind::Warp => m.warps.get(index).cloned().map(Event::Warp),
            EventKind::Coord => m.coords.get(index).cloned().map(Event::Coord),
            EventKind::Bg => m.bgs.get(index).cloned().map(Event::Bg),
        }
    }

    fn event_count(&self, kind: EventKind) -> usize {
        let m = &self.map;
        match kind {
            EventKind::Object => m.objects.len(),
            EventKind::Warp => m.warps.len(),
            EventKind::Coord => m.coords.len(),
            EventKind::Bg => m.bgs.len(),
        }
    }

    /// The local id a new object gets: the highest + 1 (ids stay stable; deletions leave gaps).
    pub fn next_local_id(&self) -> u8 {
        (self.map.objects.iter())
            .map(|o| o.local_id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    fn shift_events(&mut self, dx: i16, dy: i16) {
        let m = &mut self.map;
        let all = (m.objects.iter_mut().map(|e| (&mut e.x, &mut e.y)))
            .chain(m.warps.iter_mut().map(|e| (&mut e.x, &mut e.y)))
            .chain(m.coords.iter_mut().map(|e| (&mut e.x, &mut e.y)))
            .chain(m.bgs.iter_mut().map(|e| (&mut e.x, &mut e.y)));
        for (x, y) in all {
            *x = x.saturating_add(dx);
            *y = y.saturating_add(dy);
        }
        for c in &mut m.connections {
            match c.dir {
                ConnectionDir::North | ConnectionDir::South => c.offset += dx as i32,
                ConnectionDir::East | ConnectionDir::West => c.offset += dy as i32,
                ConnectionDir::Dive | ConnectionDir::Emerge => {}
            }
        }
    }
}

/// The four kinds of map events.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    Object,
    Warp,
    Coord,
    Bg,
}

/// One event, in the files' types.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Event {
    Object(GameObject),
    Warp(GameWarp),
    Coord(Coord),
    Bg(Bg),
}

impl Event {
    pub fn kind(&self) -> EventKind {
        match self {
            Event::Object(_) => EventKind::Object,
            Event::Warp(_) => EventKind::Warp,
            Event::Coord(_) => EventKind::Coord,
            Event::Bg(_) => EventKind::Bg,
        }
    }

    pub fn pos(&self) -> (i16, i16) {
        match self {
            Event::Object(e) => (e.x, e.y),
            Event::Warp(e) => (e.x, e.y),
            Event::Coord(e) => (e.x, e.y),
            Event::Bg(e) => (e.x, e.y),
        }
    }

    fn set_pos(&mut self, x: i16, y: i16) {
        let (ex, ey) = match self {
            Event::Object(e) => (&mut e.x, &mut e.y),
            Event::Warp(e) => (&mut e.x, &mut e.y),
            Event::Coord(e) => (&mut e.x, &mut e.y),
            Event::Bg(e) => (&mut e.x, &mut e.y),
        };
        (*ex, *ey) = (x, y);
    }
}

/// What a paint writes into a cell: each part that is set replaces that part of the block.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Brush {
    #[serde(default)]
    pub metatile: Option<u16>,
    #[serde(default)]
    pub collision: Option<u8>,
    #[serde(default)]
    pub elevation: Option<u8>,
}

impl Brush {
    pub fn metatile(m: u16) -> Brush {
        Brush {
            metatile: Some(m),
            ..Brush::default()
        }
    }

    pub fn apply(self, b: u16) -> u16 {
        let mt = self.metatile.unwrap_or(b & BLOCK_METATILE_MASK);
        let col = self.collision.unwrap_or(engine::data::block_collision(b));
        let el = self.elevation.unwrap_or(engine::data::block_elevation(b));
        block(mt, col, el)
    }

    /// Something is set and every part is in range (metatile ≤ 1023, collision ≤ 3,
    /// elevation ≤ 15).
    pub fn check(self) -> Result<()> {
        ensure!(
            self.metatile.is_some() || self.collision.is_some() || self.elevation.is_some(),
            "an empty brush (set metatile, collision or elevation)"
        );
        ensure!(
            self.metatile.is_none_or(|m| m <= BLOCK_METATILE_MASK),
            "metatile {} is over 1023",
            self.metatile.unwrap_or(0)
        );
        ensure!(
            self.collision.is_none_or(|c| c <= 3),
            "collision {} is over 3",
            self.collision.unwrap_or(0)
        );
        ensure!(
            self.elevation.is_none_or(|e| e <= 15),
            "elevation {} is over 15",
            self.elevation.unwrap_or(0)
        );
        Ok(())
    }
}

/// One edit; a list of them is an edit script (`cargo xtask studio map apply`). Cells are the
/// map's (0, 0 at the top-left). Every op but `Undo`/`Redo`/`Copy` is one undo step.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Op {
    /// One cell.
    Paint {
        x: i32,
        y: i32,
        #[serde(default)]
        metatile: Option<u16>,
        #[serde(default)]
        collision: Option<u8>,
        #[serde(default)]
        elevation: Option<u8>,
    },
    /// A `w x h` rectangle (must fit in the map).
    Rect {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        #[serde(default)]
        metatile: Option<u16>,
        #[serde(default)]
        collision: Option<u8>,
        #[serde(default)]
        elevation: Option<u8>,
    },
    /// Flood fill: the 4-connected cells holding the same raw block as (x, y).
    Fill {
        x: i32,
        y: i32,
        #[serde(default)]
        metatile: Option<u16>,
        #[serde(default)]
        collision: Option<u8>,
        #[serde(default)]
        elevation: Option<u8>,
    },
    /// Rows of characters from (x, y), each character painted with its legend brush (every
    /// character must be in the legend). For drafting from an ASCII plan.
    ///
    /// Any character is a key, written as a RON char (`'#'`, `'"'`, `' '`, `'é'`; escapes
    /// `'\''` and `'\\'`) or a one-character string (`"\""`). In the rows, `"` and `\` are
    /// escaped (`"\"\\"`), or the rows are raw strings (`r#"say "hi""#`).
    Ascii {
        x: i32,
        y: i32,
        rows: Vec<String>,
        #[serde(deserialize_with = "legend")]
        legend: BTreeMap<char, Brush>,
    },
    /// Copies a `w x h` block of raw cells to the editor's clipboard (not an undo step).
    Copy {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    /// Pastes the clipboard with its top-left at (x, y); cells outside the map are dropped.
    Paste {
        x: i32,
        y: i32,
    },
    /// Paints stamp `name` (`stamps/<name>.ron` of the games dir, [`super::stamp`]) with its
    /// top-left at (x, y). It must fit in the map, and its tilesets must be the map's.
    Stamp {
        x: i32,
        y: i32,
        name: String,
    },
    /// Sets the collision of each cell (in `rect`, or the whole map) whose metatile has a usual
    /// collision ([`super::behavior::usual_collision`]: the extract's clear verdict, else its
    /// behaviour) and disagrees with it (walkable against solid); other cells keep theirs.
    AutoCollision {
        #[serde(default)]
        rect: Option<Area>,
    },
    /// Accepts the behaviour warnings of `cells` and of cells holding `metatiles`, for
    /// `reason` (required). Changes no cell; `validate` honours it, reading it from the map's
    /// edit scripts. An allowance that silences nothing is a warning.
    AllowCollision {
        #[serde(default)]
        cells: Vec<(i32, i32)>,
        #[serde(default)]
        metatiles: Vec<u16>,
        reason: String,
    },
    /// Accepts the `lint-reach` finding whose subject is `subject` (`warp 3`, `object
    /// LOCALID_X`, `coords VAR_X=1`…; `super::reach_lint`), for `reason` (required). Changes
    /// nothing in the map; `lint-reach` reads it from the map's edit scripts. An allowance that
    /// silences nothing is a finding.
    AllowReach {
        subject: String,
        reason: String,
    },
    /// The 2x2 border (a list of 4 raw blocks, row-major).
    Border {
        blocks: Vec<u16>,
    },
    /// New size. Old cell (x, y) moves to (x + dx, y + dy), and so do the events and the
    /// connection offsets along their edge; new cells get `fill` (a metatile, with
    /// `fill_collision` and `fill_elevation`, as a new map). Events left outside the map are
    /// validation errors.
    Resize {
        width: u16,
        height: u16,
        fill: u16,
        #[serde(default)]
        fill_collision: u8,
        #[serde(default = "ground")]
        fill_elevation: u8,
        #[serde(default)]
        dx: i16,
        #[serde(default)]
        dy: i16,
    },
    Tilesets {
        primary: String,
        secondary: String,
    },
    Header(GameHeader),
    /// Replaces every map script (fields left out of the RON are not defaulted: all seven are
    /// required). [`Op::ScriptsSet`] changes only some.
    Scripts(MapScripts),
    /// Sets only the map scripts given, keeping the others: `ScriptsSet(on_load:
    /// "Map_OnLoad")`. A string field set to `""` clears it; `on_frame` and `on_warp_into`
    /// replace their whole list. At least one field must be given.
    ScriptsSet {
        #[serde(default)]
        on_load: Option<String>,
        #[serde(default)]
        on_transition: Option<String>,
        #[serde(default)]
        on_resume: Option<String>,
        #[serde(default)]
        on_return_to_field: Option<String>,
        #[serde(default)]
        on_dive_warp: Option<String>,
        #[serde(default)]
        on_frame: Option<Vec<VarScript>>,
        #[serde(default)]
        on_warp_into: Option<Vec<VarScript>>,
    },
    /// A new object. Its local id is `local_id` when given (1..64, unused in the map), else
    /// the highest + 1.
    Object {
        x: i16,
        y: i16,
        /// `OBJ_EVENT_GFX_*`.
        gfx: String,
        /// `LOCALID_*` name scripts refer to it by.
        #[serde(default)]
        id: Option<String>,
        /// The local id (`None`: the highest + 1).
        #[serde(default)]
        local_id: Option<u8>,
        #[serde(default = "face_down")]
        movement: String,
        #[serde(default)]
        range_x: u8,
        #[serde(default)]
        range_y: u8,
        #[serde(default)]
        script: Option<String>,
        /// Hide flag (`FLAG_*`).
        #[serde(default)]
        flag: Option<String>,
        #[serde(default = "trainer_none")]
        trainer_type: String,
        /// Trainer sight (or berry tree id).
        #[serde(default)]
        sight: u16,
        #[serde(default = "ground")]
        elevation: u8,
    },
    /// A warp to `dest_map`'s warp `dest_warp` (`MAP_DYNAMIC` with 127: set at run time).
    Warp {
        x: i16,
        y: i16,
        dest_map: String,
        dest_warp: u8,
        #[serde(default)]
        elevation: u8,
    },
    Coord {
        x: i16,
        y: i16,
        /// `VAR_*`.
        var: String,
        value: u16,
        script: String,
        #[serde(default = "ground")]
        elevation: u8,
    },
    /// A sign (bg script event); `facing` is Gen3's 0..=4 (0: any direction).
    Sign {
        x: i16,
        y: i16,
        script: String,
        #[serde(default)]
        facing: u8,
        #[serde(default)]
        elevation: u8,
    },
    HiddenItem {
        x: i16,
        y: i16,
        /// `ITEM_*`.
        item: String,
        /// `FLAG_HIDDEN_ITEM_*`.
        flag: String,
        #[serde(default)]
        elevation: u8,
    },
    /// Moves event `index` of `kind` to (x, y).
    Move {
        kind: EventKind,
        index: usize,
        x: i16,
        y: i16,
    },
    /// Replaces event `index` of the event's kind with `event`, as given (an object's
    /// `local_id` and `id` included: duplicates are validation errors).
    Replace {
        index: usize,
        event: Event,
    },
    /// Deletes an event; objects keep their local ids (a gap is left).
    Delete {
        kind: EventKind,
        index: usize,
    },
    Connect(GameConnection),
    /// Replaces connection `index`.
    SetConnection {
        index: usize,
        connection: GameConnection,
    },
    Disconnect {
        index: usize,
    },
    Undo,
    Redo,
}

/// A rectangle of cells.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// What the ops that read outside the document need: stamps (from a games dir) and usual
/// collisions (from a catalog). [`super::resources::Resources`] is the games dir's.
pub trait Env: Send + Sync + std::fmt::Debug {
    fn stamp(&self, name: &str) -> Result<Stamp>;
    /// The usual collision of every cell of `d` (0 walkable, 1 solid), row-major.
    fn usual_collisions(&self, d: &Doc) -> Vec<Option<u8>>;
}

/// An `Ascii` legend key: a char or a one-character string.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct LegendKey(char);

impl<'de> Deserialize<'de> for LegendKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<LegendKey, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = LegendKey;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("one character ('x' or \"x\")")
            }
            fn visit_char<E: serde::de::Error>(self, c: char) -> Result<LegendKey, E> {
                Ok(LegendKey(c))
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<LegendKey, E> {
                let mut it = s.chars();
                match (it.next(), it.next()) {
                    (Some(c), None) => Ok(LegendKey(c)),
                    _ => Err(E::custom(format!("legend key {s:?} is not one character"))),
                }
            }
        }
        d.deserialize_any(V)
    }
}

fn legend<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<char, Brush>, D::Error> {
    let m = BTreeMap::<LegendKey, Brush>::deserialize(d)?;
    Ok(m.into_iter().map(|(k, v)| (k.0, v)).collect())
}

fn face_down() -> String {
    "MOVEMENT_TYPE_FACE_DOWN".into()
}

fn trainer_none() -> String {
    "TRAINER_TYPE_NONE".into()
}

fn brush(metatile: Option<u16>, collision: Option<u8>, elevation: Option<u8>) -> Result<Brush> {
    let b = Brush {
        metatile,
        collision,
        elevation,
    };
    b.check()?;
    Ok(b)
}

/// A copied block of raw cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub w: i32,
    pub h: i32,
    pub blocks: Vec<u16>,
}

/// A [`Doc`] with undo, redo and a clipboard.
#[derive(Clone, Debug)]
pub struct Editor {
    doc: Doc,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    clip: Option<Clip>,
    /// The state before an open stroke ([`Editor::begin_stroke`]).
    stroke: Option<Doc>,
    /// The document as last saved (`None`: never saved).
    saved: Option<Doc>,
    /// Stamps and usual collisions (`Stamp` and `AutoCollision` fail without).
    env: Option<Arc<dyn Env>>,
}

impl Editor {
    /// Editing a saved document (not dirty until it differs).
    pub fn new(doc: Doc) -> Editor {
        Editor {
            saved: Some(doc.clone()),
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            clip: None,
            stroke: None,
            env: None,
        }
    }

    /// With stamps and usual collisions (for `Stamp` and `AutoCollision`).
    pub fn with_env(mut self, env: Arc<dyn Env>) -> Editor {
        self.env = Some(env);
        self
    }

    pub fn set_env(&mut self, env: Option<Arc<dyn Env>>) {
        self.env = env;
    }

    /// Editing a document no file holds yet (dirty until saved).
    pub fn unsaved(doc: Doc) -> Editor {
        Editor {
            saved: None,
            ..Editor::new(doc)
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// The document as last saved (`None` for a new map).
    pub fn saved(&self) -> Option<&Doc> {
        self.saved.as_ref()
    }

    pub fn clipboard(&self) -> Option<&Clip> {
        self.clip.as_ref()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Whether the document differs from the saved one (undoing back to it clears this).
    pub fn is_dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.doc)
    }

    pub fn mark_saved(&mut self) {
        self.saved = Some(self.doc.clone());
    }

    /// Applies `op` as one undo step. On an error the document is unchanged.
    pub fn apply(&mut self, op: &Op) -> Result<()> {
        match op {
            Op::Undo => {
                self.undo();
                return Ok(());
            }
            Op::Redo => {
                self.redo();
                return Ok(());
            }
            Op::Copy { x, y, w, h } => {
                self.clip = Some(self.copy(*x, *y, *w, *h)?);
                return Ok(());
            }
            _ => {}
        }
        let mut next = self.doc.clone();
        apply_op(&mut next, op, self.clip.as_ref(), self.env.as_deref())?;
        self.commit(next);
        Ok(())
    }

    /// Applies every op in order; stops at the first error, naming the op.
    pub fn apply_all(&mut self, ops: &[Op]) -> Result<()> {
        for (i, op) in ops.iter().enumerate() {
            self.apply(op)
                .map_err(|e| e.context(format!("op {i} ({op:?})")))?;
        }
        Ok(())
    }

    /// Starts a stroke: the ops until [`Editor::end_stroke`] are one undo step (a pencil drag).
    pub fn begin_stroke(&mut self) {
        if self.stroke.is_none() {
            self.stroke = Some(self.doc.clone());
        }
    }

    pub fn end_stroke(&mut self) {
        if let Some(before) = self.stroke.take()
            && before != self.doc
        {
            self.push_undo(before);
            self.redo.clear();
        }
    }

    fn commit(&mut self, next: Doc) {
        if next == self.doc {
            return;
        }
        let before = std::mem::replace(&mut self.doc, next);
        if self.stroke.is_none() {
            self.push_undo(before);
            self.redo.clear();
        }
    }

    fn push_undo(&mut self, d: Doc) {
        self.undo.push(d);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self) -> bool {
        self.end_stroke();
        match self.undo.pop() {
            Some(d) => {
                self.redo.push(std::mem::replace(&mut self.doc, d));
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        self.end_stroke();
        match self.redo.pop() {
            Some(d) => {
                self.undo.push(std::mem::replace(&mut self.doc, d));
                true
            }
            None => false,
        }
    }

    pub fn copy(&self, x: i32, y: i32, w: i32, h: i32) -> Result<Clip> {
        rect_fits(&self.doc, x, y, w, h)?;
        let mut blocks = Vec::with_capacity(w as usize * h as usize);
        for cy in y..y + h {
            for cx in x..x + w {
                blocks.push(self.doc.block(cx, cy).unwrap_or_default());
            }
        }
        Ok(Clip { w, h, blocks })
    }
}

/// The rectangle is non-empty and inside the map (checked in `i64`, so no overflow).
fn rect_fits(d: &Doc, x: i32, y: i32, w: i32, h: i32) -> Result<()> {
    ensure!(w > 0 && h > 0, "an empty rectangle {w}x{h}");
    d.need(x as i64, y as i64)?;
    d.need(x as i64 + w as i64 - 1, y as i64 + h as i64 - 1)
}

fn check_event_pos(d: &Doc, x: i16, y: i16) -> Result<()> {
    d.need(x as i64, y as i64)
}

/// Applies `op` to `d` (not `Undo`, `Redo` or `Copy`, which are the editor's).
fn apply_op(d: &mut Doc, op: &Op, clip: Option<&Clip>, env: Option<&dyn Env>) -> Result<()> {
    let need_env = || env.ok_or_else(|| anyhow::anyhow!("{op:?} needs a games dir"));
    match op {
        Op::Paint {
            x,
            y,
            metatile,
            collision,
            elevation,
        } => {
            let b = brush(*metatile, *collision, *elevation)?;
            d.need(*x as i64, *y as i64)?;
            let old = d.block(*x, *y).unwrap_or_default();
            d.set(*x, *y, b.apply(old));
        }
        Op::Rect {
            x,
            y,
            w,
            h,
            metatile,
            collision,
            elevation,
        } => {
            let b = brush(*metatile, *collision, *elevation)?;
            rect_fits(d, *x, *y, *w, *h)?;
            for cy in *y..y + h {
                for cx in *x..x + w {
                    let old = d.block(cx, cy).unwrap_or_default();
                    d.set(cx, cy, b.apply(old));
                }
            }
        }
        Op::Fill {
            x,
            y,
            metatile,
            collision,
            elevation,
        } => {
            let b = brush(*metatile, *collision, *elevation)?;
            d.need(*x as i64, *y as i64)?;
            flood_fill(d, *x, *y, b);
        }
        Op::Ascii { x, y, rows, legend } => {
            for b in legend.values() {
                b.check()?;
            }
            for (dy, row) in rows.iter().enumerate() {
                for (dx, ch) in row.chars().enumerate() {
                    let Some(b) = legend.get(&ch) else {
                        bail!("row {dy} column {dx}: {ch:?} is not in the legend");
                    };
                    let (cx, cy) = (*x as i64 + dx as i64, *y as i64 + dy as i64);
                    d.need(cx, cy)?;
                    let (cx, cy) = (cx as i32, cy as i32);
                    let old = d.block(cx, cy).unwrap_or_default();
                    d.set(cx, cy, b.apply(old));
                }
            }
        }
        Op::Paste { x, y } => {
            let Some(c) = clip else {
                bail!("the clipboard is empty (Copy first)");
            };
            for dy in 0..c.h {
                for dx in 0..c.w {
                    let (cx, cy) = (*x as i64 + dx as i64, *y as i64 + dy as i64);
                    if d.in_bounds64(cx, cy) {
                        d.set(
                            cx as i32,
                            cy as i32,
                            c.blocks[dy as usize * c.w as usize + dx as usize],
                        );
                    }
                }
            }
        }
        Op::Stamp { x, y, name } => {
            let st = need_env()?.stamp(name)?;
            let cells = st.cells()?;
            st.check_tilesets(d)?;
            rect_fits(d, *x, *y, st.width as i32, st.height as i32)?;
            let w = st.width as i32;
            for (i, &b) in cells.iter().enumerate() {
                let (cx, cy) = (x + i as i32 % w, y + i as i32 / w);
                let old = d.block(cx, cy).unwrap_or_default();
                let el = match st.elevation {
                    true => block_elevation(b),
                    false => block_elevation(old),
                };
                d.set(
                    cx,
                    cy,
                    block(b & BLOCK_METATILE_MASK, block_collision(b), el),
                );
            }
        }
        Op::AutoCollision { rect } => {
            let env = need_env()?;
            let a = rect.unwrap_or(Area {
                x: 0,
                y: 0,
                w: d.width as i32,
                h: d.height as i32,
            });
            rect_fits(d, a.x, a.y, a.w, a.h)?;
            let usual = env.usual_collisions(d);
            for cy in a.y..a.y + a.h {
                for cx in a.x..a.x + a.w {
                    let b = d.block(cx, cy).unwrap_or_default();
                    let i = cy as usize * d.width as usize + cx as usize;
                    if let Some(c) = usual.get(i).copied().flatten()
                        && (block_collision(b) == 0) != (c == 0)
                    {
                        let (mt, el) = (b & BLOCK_METATILE_MASK, block_elevation(b));
                        d.set(cx, cy, block(mt, c, el));
                    }
                }
            }
        }
        Op::AllowCollision {
            cells,
            metatiles,
            reason,
        } => {
            let a = &Allowance {
                cells: cells.clone(),
                metatiles: metatiles.clone(),
                reason: reason.clone(),
            };
            ensure!(!a.reason.trim().is_empty(), "an allowance needs a reason");
            ensure!(
                !a.cells.is_empty() || !a.metatiles.is_empty(),
                "an allowance lists cells or metatiles"
            );
            for &(x, y) in &a.cells {
                d.need(x as i64, y as i64)?;
            }
            if !d.allow.contains(a) {
                d.allow.push(a.clone());
            }
        }
        Op::AllowReach { subject, reason } => {
            ensure!(!reason.trim().is_empty(), "an allowance needs a reason");
            ensure!(
                !subject.trim().is_empty(),
                "an allowance names a finding's subject"
            );
        }
        Op::Border { blocks } => d.border = border4(blocks)?,
        Op::Resize {
            width,
            height,
            fill,
            fill_collision,
            fill_elevation,
            dx,
            dy,
        } => {
            check_size(*width, *height)?;
            let fill = fill_block(*fill, *fill_collision, *fill_elevation)?;
            let (w, h) = (*width as i64, *height as i64);
            let mut blocks = vec![fill; (w * h) as usize];
            for y in 0..d.height as i64 {
                for x in 0..d.width as i64 {
                    let (nx, ny) = (x + *dx as i64, y + *dy as i64);
                    if (0..w).contains(&nx) && (0..h).contains(&ny) {
                        blocks[(ny * w + nx) as usize] =
                            d.blocks[(y * d.width as i64 + x) as usize];
                    }
                }
            }
            (d.width, d.height, d.blocks) = (*width, *height, blocks);
            d.shift_events(*dx, *dy);
        }
        Op::Tilesets { primary, secondary } => {
            d.primary = primary.clone();
            d.secondary = secondary.clone();
        }
        Op::Header(h) => d.map.header = h.clone(),
        Op::Scripts(s) => d.map.scripts = s.clone(),
        Op::ScriptsSet {
            on_load,
            on_transition,
            on_resume,
            on_return_to_field,
            on_dive_warp,
            on_frame,
            on_warp_into,
        } => {
            let strings = [
                on_load,
                on_transition,
                on_resume,
                on_return_to_field,
                on_dive_warp,
            ];
            ensure!(
                strings.iter().any(|f| f.is_some()) || on_frame.is_some() || on_warp_into.is_some(),
                "ScriptsSet sets no field (on_load, on_transition, on_resume, \
                 on_return_to_field, on_dive_warp, on_frame, on_warp_into)"
            );
            let s = &mut d.map.scripts;
            let slots = [
                &mut s.on_load,
                &mut s.on_transition,
                &mut s.on_resume,
                &mut s.on_return_to_field,
                &mut s.on_dive_warp,
            ];
            for (slot, v) in slots.into_iter().zip(strings) {
                if let Some(v) = v {
                    *slot = (!v.is_empty()).then(|| v.clone());
                }
            }
            if let Some(v) = on_frame {
                s.on_frame = v.clone();
            }
            if let Some(v) = on_warp_into {
                s.on_warp_into = v.clone();
            }
        }
        Op::Object {
            x,
            y,
            gfx,
            id,
            local_id,
            movement,
            range_x,
            range_y,
            script,
            flag,
            trainer_type,
            sight,
            elevation,
        } => {
            check_event_pos(d, *x, *y)?;
            let local_id = match *local_id {
                Some(l) => {
                    let max = super::validate::MAX_OBJECTS;
                    ensure!(
                        l != 0 && (l as usize) < max,
                        "local id {l} is outside 1..{max}"
                    );
                    if let Some(i) = d.map.objects.iter().position(|o| o.local_id == l) {
                        bail!("local id {l} is already object {i}'s");
                    }
                    l
                }
                None => d.next_local_id(),
            };
            d.map.objects.push(GameObject {
                local_id,
                id: id.clone(),
                gfx: gfx.clone(),
                x: *x,
                y: *y,
                elevation: *elevation,
                movement: movement.clone(),
                range_x: *range_x,
                range_y: *range_y,
                trainer_type: trainer_type.clone(),
                trainer_sight_or_berry_tree: *sight,
                script: script.clone(),
                flag: flag.clone(),
            });
        }
        Op::Warp {
            x,
            y,
            dest_map,
            dest_warp,
            elevation,
        } => {
            check_event_pos(d, *x, *y)?;
            if dest_map == MAP_DYNAMIC {
                ensure!(
                    super::schema::dynamic_warp_id(*dest_warp),
                    "a {MAP_DYNAMIC} warp has dest_warp {WARP_ID_DYNAMIC} or {} (got {dest_warp})",
                    super::schema::WARP_ID_SECRET_BASE
                );
            }
            d.map.warps.push(GameWarp {
                x: *x,
                y: *y,
                elevation: *elevation,
                dest_map: dest_map.clone(),
                dest_warp: *dest_warp,
            });
        }
        Op::Coord {
            x,
            y,
            var,
            value,
            script,
            elevation,
        } => {
            check_event_pos(d, *x, *y)?;
            d.map.coords.push(Coord {
                x: *x,
                y: *y,
                elevation: *elevation,
                var: Some(var.clone()),
                value: *value,
                script: Some(script.clone()),
            });
        }
        Op::Sign {
            x,
            y,
            script,
            facing,
            elevation,
        } => {
            check_event_pos(d, *x, *y)?;
            ensure!(*facing <= 4, "sign facing {facing} is over 4");
            d.map.bgs.push(Bg {
                x: *x,
                y: *y,
                elevation: *elevation,
                kind: BgKind::Script {
                    facing: *facing,
                    script: Some(script.clone()),
                },
            });
        }
        Op::HiddenItem {
            x,
            y,
            item,
            flag,
            elevation,
        } => {
            check_event_pos(d, *x, *y)?;
            d.map.bgs.push(Bg {
                x: *x,
                y: *y,
                elevation: *elevation,
                kind: BgKind::HiddenItem {
                    item: item.clone(),
                    flag: flag.clone(),
                },
            });
        }
        Op::Move { kind, index, x, y } => {
            let Some(mut e) = d.event(*kind, *index) else {
                bail!("no {kind:?} {index} (the map has {})", d.event_count(*kind));
            };
            check_event_pos(d, *x, *y)?;
            e.set_pos(*x, *y);
            put_event(d, *index, e);
        }
        Op::Replace { index, event } => {
            let kind = event.kind();
            ensure!(
                *index < d.event_count(kind),
                "no {kind:?} {index} (the map has {})",
                d.event_count(kind)
            );
            let (x, y) = event.pos();
            check_event_pos(d, x, y)?;
            put_event(d, *index, event.clone());
        }
        Op::Delete { kind, index } => {
            let n = d.event_count(*kind);
            ensure!(*index < n, "no {kind:?} {index} (the map has {n})");
            let m = &mut d.map;
            match kind {
                EventKind::Object => drop(m.objects.remove(*index)),
                EventKind::Warp => drop(m.warps.remove(*index)),
                EventKind::Coord => drop(m.coords.remove(*index)),
                EventKind::Bg => drop(m.bgs.remove(*index)),
            }
        }
        Op::Connect(c) => d.map.connections.push(c.clone()),
        Op::SetConnection { index, connection } => {
            let n = d.map.connections.len();
            ensure!(*index < n, "no connection {index} (the map has {n})");
            d.map.connections[*index] = connection.clone();
        }
        Op::Disconnect { index } => {
            let n = d.map.connections.len();
            ensure!(*index < n, "no connection {index} (the map has {n})");
            d.map.connections.remove(*index);
        }
        Op::Undo | Op::Redo | Op::Copy { .. } => unreachable!("handled by the editor"),
    }
    Ok(())
}

fn put_event(d: &mut Doc, index: usize, e: Event) {
    let m = &mut d.map;
    match e {
        Event::Object(o) => m.objects[index] = o,
        Event::Warp(w) => m.warps[index] = w,
        Event::Coord(c) => m.coords[index] = c,
        Event::Bg(b) => m.bgs[index] = b,
    }
}

/// Paints the 4-connected region of cells equal to (x, y)'s raw block.
fn flood_fill(d: &mut Doc, x: i32, y: i32, b: Brush) {
    let Some(target) = d.block(x, y) else {
        return;
    };
    if b.apply(target) == target {
        return;
    }
    let mut stack = vec![(x, y)];
    while let Some((cx, cy)) = stack.pop() {
        if d.block(cx, cy) != Some(target) {
            continue;
        }
        d.set(cx, cy, b.apply(target));
        stack.extend([(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)]);
    }
}
