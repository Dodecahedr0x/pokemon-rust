//! The editing model: a map document ([`Doc`]), the edit operations ([`Op`], also the edit
//! script format) and an [`Editor`] with undo and redo. No Dioxus, no files: the UI, the CLI
//! and tests drive the same calls.

use super::schema::{GameConnection, GameMap, GameWarp, default_header};
use anyhow::{Result, bail, ensure};
use engine::data::{BLOCK_COLLISION_SHIFT, BLOCK_ELEVATION_SHIFT, BLOCK_METATILE_MASK};
use extract::schema::{
    Bg, BgKind, Coord, Header, Layout, MapFile, MapScripts, Object, from_rows, to_rows,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
}

/// What a new map starts with (`cargo xtask studio map create <spec.ron>`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NewMap {
    pub name: String,
    /// The layout's name; the map's name when absent.
    #[serde(default)]
    pub layout: Option<String>,
    pub width: u16,
    pub height: u16,
    pub primary: String,
    pub secondary: String,
    /// Metatile index every cell starts with.
    pub fill: u16,
    #[serde(default)]
    pub fill_collision: u8,
    #[serde(default = "ground")]
    pub fill_elevation: u8,
    /// 2x2 raw border blocks (a list of 4, row-major); four times the fill metatile when absent.
    #[serde(default)]
    pub border: Option<Vec<u16>>,
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
        let fill = block(spec.fill, spec.fill_collision, spec.fill_elevation);
        let border = match &spec.border {
            None => [spec.fill & BLOCK_METATILE_MASK; 4],
            Some(b) => border4(b)?,
        };
        Ok(Doc {
            map: GameMap {
                name: spec.name.clone(),
                layout: spec.layout.clone().unwrap_or_else(|| spec.name.clone()),
                header: default_header(),
                objects: Vec::new(),
                warps: Vec::new(),
                coords: Vec::new(),
                bgs: Vec::new(),
                connections: Vec::new(),
                scripts: MapScripts::default(),
            },
            width: spec.width,
            height: spec.height,
            primary: spec.primary.clone(),
            secondary: spec.secondary.clone(),
            border,
            blocks: vec![fill; spec.width as usize * spec.height as usize],
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
        })
    }

    /// An extracted map (`content/extracted/maps/<Name>.ron`) as a document: the layout is
    /// named after the map, warps and connections to unexported maps are dropped.
    pub fn from_map_file(f: &MapFile) -> Result<Doc> {
        let map = GameMap {
            name: f.name.clone(),
            layout: f.name.clone(),
            header: f.header.clone(),
            objects: f.objects.clone(),
            warps: (f.warps.iter())
                .filter_map(|w| {
                    Some(GameWarp {
                        x: w.x,
                        y: w.y,
                        elevation: w.elevation,
                        dest_map: w.dest_map.clone()?,
                        dest_warp: w.dest_warp,
                    })
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

    /// The raw block at (x, y), `None` outside the map.
    pub fn block(&self, x: i32, y: i32) -> Option<u16> {
        self.in_bounds(x, y)
            .then(|| self.blocks[y as usize * self.width as usize + x as usize])
    }

    fn set(&mut self, x: i32, y: i32, b: u16) {
        let w = self.width as usize;
        self.blocks[y as usize * w + x as usize] = b;
    }

    fn need(&self, x: i32, y: i32) -> Result<()> {
        ensure!(
            self.in_bounds(x, y),
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

    /// Objects' `local_id`s are their index + 1 (Gen3), renumbered after every change.
    fn renumber(&mut self) {
        for (i, o) in self.map.objects.iter_mut().enumerate() {
            o.local_id = (i + 1) as u8;
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
    Object(Object),
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

    fn check(self) -> Result<()> {
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
/// map's (0, 0 at the top-left). Every op but `Undo`/`Redo` is one undo step.
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
    Ascii {
        x: i32,
        y: i32,
        rows: Vec<String>,
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
    /// The 2x2 border (a list of 4 raw blocks, row-major).
    Border {
        blocks: Vec<u16>,
    },
    /// New size; kept cells stay at the same (x, y), new ones get `fill` (a raw block).
    Resize {
        width: u16,
        height: u16,
        fill: u16,
    },
    Tilesets {
        primary: String,
        secondary: String,
    },
    Header(Header),
    Scripts(MapScripts),
    /// A new object (local id = its index + 1).
    Object {
        x: i16,
        y: i16,
        /// `OBJ_EVENT_GFX_*`.
        gfx: String,
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
    /// Replaces event `index` of the event's kind (an object keeps its local id).
    Replace {
        index: usize,
        event: Event,
    },
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
    /// Changes since the last [`Editor::mark_saved`].
    dirty: bool,
}

impl Editor {
    pub fn new(doc: Doc) -> Editor {
        Editor {
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            clip: None,
            stroke: None,
            dirty: false,
        }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
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

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
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
        apply_op(&mut next, op, self.clip.as_ref())?;
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
        self.dirty = true;
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
                self.dirty = true;
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
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn copy(&self, x: i32, y: i32, w: i32, h: i32) -> Result<Clip> {
        rect_fits(&self.doc, x, y, w, h)?;
        let mut blocks = Vec::with_capacity((w * h) as usize);
        for cy in y..y + h {
            for cx in x..x + w {
                blocks.push(self.doc.block(cx, cy).unwrap_or_default());
            }
        }
        Ok(Clip { w, h, blocks })
    }
}

fn rect_fits(d: &Doc, x: i32, y: i32, w: i32, h: i32) -> Result<()> {
    ensure!(w > 0 && h > 0, "an empty rectangle {w}x{h}");
    d.need(x, y)?;
    d.need(x + w - 1, y + h - 1)
}

fn check_event_pos(d: &Doc, x: i16, y: i16) -> Result<()> {
    d.need(x as i32, y as i32)
}

/// Applies `op` to `d` (not `Undo`, `Redo` or `Copy`, which are the editor's).
fn apply_op(d: &mut Doc, op: &Op, clip: Option<&Clip>) -> Result<()> {
    match op {
        Op::Paint {
            x,
            y,
            metatile,
            collision,
            elevation,
        } => {
            let b = brush(*metatile, *collision, *elevation)?;
            d.need(*x, *y)?;
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
            d.need(*x, *y)?;
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
                    let (cx, cy) = (x + dx as i32, y + dy as i32);
                    d.need(cx, cy)?;
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
                    let (cx, cy) = (x + dx, y + dy);
                    if d.in_bounds(cx, cy) {
                        d.set(cx, cy, c.blocks[(dy * c.w + dx) as usize]);
                    }
                }
            }
        }
        Op::Border { blocks } => d.border = border4(blocks)?,
        Op::Resize {
            width,
            height,
            fill,
        } => {
            ensure!(*width > 0 && *height > 0, "size {width}x{height}");
            let mut blocks = vec![*fill; *width as usize * *height as usize];
            for y in 0..(*height).min(d.height) as usize {
                for x in 0..(*width).min(d.width) as usize {
                    blocks[y * *width as usize + x] = d.blocks[y * d.width as usize + x];
                }
            }
            (d.width, d.height, d.blocks) = (*width, *height, blocks);
        }
        Op::Tilesets { primary, secondary } => {
            d.primary = primary.clone();
            d.secondary = secondary.clone();
        }
        Op::Header(h) => d.map.header = h.clone(),
        Op::Scripts(s) => d.map.scripts = s.clone(),
        Op::Object {
            x,
            y,
            gfx,
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
            d.map.objects.push(Object {
                local_id: 0,
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
            d.renumber();
        }
        Op::Warp {
            x,
            y,
            dest_map,
            dest_warp,
            elevation,
        } => {
            check_event_pos(d, *x, *y)?;
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
            d.renumber();
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
            d.renumber();
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
