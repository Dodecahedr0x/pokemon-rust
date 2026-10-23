//! Reachability over a map's cells, by the engine's movement rules, and trainer sight: the
//! walk graph behind `studio-map reach`, `bypass`, `lint-reach` and `render --reach`
//! (`reach_lint` holds the lint).
//!
//! **Nodes** are (x, y, player elevation): Gen3 keeps the player's elevation across cells of
//! elevation 15 (bridges), so the same cell can be reached at different elevations.
//!
//! **A step** from node (x, y, e) towards `d` (Gen3 `MovePlayerNotOnBike`, the engine's
//! `Objects::update_player` and `collision_at`), in this order:
//! 1. on an `ArrowWarp(d)` cell with a warp event matching `e`, pushing `d` takes the warp
//!    (`TryArrowWarp`), and the player does not move;
//! 2. pushing up into a `Door` cell with a warp event matching the facing elevation takes the
//!    warp, whatever the door's collision (`TryDoorWarp`);
//! 3. a `Ledge(d)` cell in front is jumped: the player lands two cells ahead, with no
//!    collision test on the landing cell, as the engine (Gen3 `COLLISION_LEDGE_JUMP` comes
//!    before the landing is looked at);
//! 4. otherwise the cell in front must have collision 0, a behaviour other than `Blocked`
//!    (water without Surf, currents…), no wall between the two cells (`Behavior::walled`,
//!    `MB_IMPASSABLE_*`), an elevation that does not mismatch (Gen3 `IsElevationMismatchAt`:
//!    the player or the cell at 0, or the cell at 15, never mismatch) and no object at a
//!    compatible elevation (`AreElevationsCompatible`: 0 on either side, or equal);
//! 5. after the step the player's elevation is the cell's, unless the cell's is 15
//!    (`ObjectEventUpdateElevation`); elevation 0 cells thus free the player to any elevation.
//!
//! Finishing a step on a cell whose behaviour enters a warp (doors, stairs, ladders…) with a
//! warp event matching the new elevation takes the warp: the search records the exit and goes
//! no further from that cell. A cell the search starts on never warps (an arrival).
//!
//! **Connections**: a step off the map's edge into a connected neighbor exits the map when the
//! neighbor's cell passes the same tests (its collision, behaviour with its own tilesets and
//! elevation; its objects are ignored). A neighbor that is not loaded is assumed open. The
//! border (no connection) is solid.
//!
//! **Surf** (`surf`): water cells (`Blocked` behaviours named `*WATER*`, not waterfalls) are
//! walkable, and a step onto or off water ignores the elevation mismatch (Gen3 starts and stops
//! surfing on exactly that mismatch); objects on the cell are then matched at the cell's
//! elevation, as Gen3's `CanStopSurfing`. While surfing the player is at the water's
//! elevation (1), so objects on land (3) cannot be talked to from the water, as in Gen3.
//! Gen3's further tests (elevation 3 to start, a surfable behaviour list) are not modelled.
//!
//! **Objects** are obstacles on their template cell (wanderers are not followed). Their
//! elevation is their cell's unless that is 15 (then the template's), as the engine's update
//! gives a still object. Objects with given hide flags can be treated as absent ([`Hidden`]).
//!
//! **Trainer sight** ([`Walk::sight`]): for each facing the trainer may have (its movement
//! type, [`facings`]; buried trainers look all four ways), the cells straight ahead up to its
//! sight range while each passes the engine's sight test (`Objects::sight_collision`):
//! collision 0, no wall against the trainer's own cell, no elevation mismatch with the trainer,
//! no other object at a compatible elevation. Water does not block sight. Standing on any of
//! them starts the battle (while the trainer is undefeated; flags are not modelled).

use super::behavior::behavior;
use super::catalog::Catalog;
use super::games::GamesDir;
use super::model::Doc;
use engine::data::{Behavior, Dir, block_collision, block_elevation, block_metatile};
use extract::schema::{BgKind, ConnectionDir};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One cell as the walk sees it: its raw block and its classified behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub block: u16,
    pub behavior: Behavior,
    /// A water behaviour (`*WATER*`, not a waterfall): walkable with Surf when `Blocked`.
    pub water: bool,
}

impl Cell {
    /// A normal cell holding `block`.
    pub fn new(block: u16) -> Cell {
        Cell {
            block,
            behavior: Behavior::Normal,
            water: false,
        }
    }

    /// `block` classified with these tilesets (`Normal` when unknown).
    pub fn classify(c: &Catalog, primary: &str, secondary: &str, block: u16) -> Cell {
        let mb = behavior(c, primary, secondary, block_metatile(block));
        Cell {
            block,
            behavior: mb.as_ref().map_or(Behavior::Normal, |m| m.class),
            water: mb.is_some_and(|m| m.name.contains("WATER") && !m.name.contains("WATERFALL")),
        }
    }

    pub fn collision(&self) -> u8 {
        block_collision(self.block)
    }

    pub fn elevation(&self) -> u8 {
        block_elevation(self.block)
    }

    fn surfable(&self) -> bool {
        self.water && self.behavior == Behavior::Blocked
    }
}

/// A connected neighbor's cells, for the steps that leave the map.
#[derive(Clone, Debug)]
pub struct Neighbor {
    pub width: i32,
    pub height: i32,
    pub cells: Vec<Cell>,
}

/// One north, south, east or west connection of the map.
#[derive(Clone, Debug)]
pub struct Side {
    /// Its index in the map's connections.
    pub connection: usize,
    pub dir: Dir,
    pub offset: i32,
    pub map: String,
    /// `None`: not loaded (assumed open).
    pub neighbor: Option<Neighbor>,
}

/// The engine direction of a connection (`None` for dive and emerge).
pub fn connection_dir(d: ConnectionDir) -> Option<Dir> {
    Some(match d {
        ConnectionDir::South => Dir::Down,
        ConnectionDir::North => Dir::Up,
        ConnectionDir::West => Dir::Left,
        ConnectionDir::East => Dir::Right,
        ConnectionDir::Dive | ConnectionDir::Emerge => return None,
    })
}

/// A compass word for `d`.
pub fn dir_name(d: Dir) -> &'static str {
    match d {
        Dir::Down => "south",
        Dir::Up => "north",
        Dir::Left => "west",
        Dir::Right => "east",
    }
}

/// A map's cells, classified, and its connections.
#[derive(Clone, Debug)]
pub struct Grid {
    pub width: i32,
    pub height: i32,
    /// Row-major.
    pub cells: Vec<Cell>,
    pub sides: Vec<Side>,
}

impl Grid {
    /// `doc`'s cells classified with `catalog`; connections without their neighbors.
    pub fn new(doc: &Doc, catalog: &Catalog) -> Grid {
        let cells = (doc.blocks.iter())
            .map(|&b| Cell::classify(catalog, &doc.primary, &doc.secondary, b))
            .collect();
        let sides = (doc.map.connections.iter().enumerate())
            .filter_map(|(i, c)| {
                Some(Side {
                    connection: i,
                    dir: connection_dir(c.dir)?,
                    offset: c.offset,
                    map: c.map.clone(),
                    neighbor: None,
                })
            })
            .collect();
        Grid {
            width: doc.width as i32,
            height: doc.height as i32,
            cells,
            sides,
        }
    }

    /// [`Grid::new`] with each connected neighbor loaded from `games` when it can be.
    pub fn with_neighbors(doc: &Doc, catalog: &Catalog, games: &GamesDir) -> Grid {
        let mut g = Grid::new(doc, catalog);
        for s in &mut g.sides {
            if let Ok(n) = games.load(&s.map) {
                s.neighbor = Some(Neighbor {
                    width: n.width as i32,
                    height: n.height as i32,
                    cells: (n.blocks.iter())
                        .map(|&b| Cell::classify(catalog, &n.primary, &n.secondary, b))
                        .collect(),
                });
            }
        }
        g
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn cell(&self, x: i32, y: i32) -> Option<&Cell> {
        self.in_bounds(x, y)
            .then(|| &self.cells[(y * self.width + x) as usize])
    }

    pub fn cell_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        let w = self.width;
        self.in_bounds(x, y)
            .then(move || &mut self.cells[(y * w + x) as usize])
    }

    /// The connection covering (x, y) outside the map (as the engine's `grid::neighbor`:
    /// corners are never connected) and the neighbor's cell there, when loaded.
    pub fn outside(&self, x: i32, y: i32) -> Option<(usize, Option<Cell>)> {
        let (w, h) = (self.width, self.height);
        let dir = match (x < 0, x >= w, y < 0, y >= h) {
            (false, false, false, true) => Dir::Down,
            (false, false, true, false) => Dir::Up,
            (true, false, false, false) => Dir::Left,
            (false, true, false, false) => Dir::Right,
            _ => return None,
        };
        self.sides
            .iter()
            .enumerate()
            .filter(|(_, s)| s.dir == dir)
            .find_map(|(i, s)| {
                let Some(n) = &s.neighbor else {
                    return Some((i, None));
                };
                let (nx, ny) = match dir {
                    Dir::Down => (x - s.offset, y - h),
                    Dir::Up => (x - s.offset, y + n.height),
                    Dir::Left => (x + n.width, y - s.offset),
                    Dir::Right => (x - w, y - s.offset),
                };
                (nx >= 0 && ny >= 0 && nx < n.width && ny < n.height)
                    .then(|| (i, Some(n.cells[(ny * n.width + nx) as usize])))
            })
    }
}

/// Gen3 `IsElevationMismatchAt`.
pub const fn elevation_mismatch(obj: u8, cell: u8) -> bool {
    obj != 0 && cell != 0 && cell != 15 && cell != obj
}

/// Gen3 `AreElevationsCompatible`.
pub const fn elevations_compatible(a: u8, b: u8) -> bool {
    a == 0 || b == 0 || a == b
}

/// Gen3 `Get{Warp,Coord,Background}EventAtPosition`: an event at elevation 0 matches any.
pub const fn event_matches(event: u8, position: u8) -> bool {
    event == 0 || event == position
}

/// The player's elevation after finishing a step on a cell of elevation `cell`.
pub const fn elevation_after(player: u8, cell: u8) -> u8 {
    if cell != 15 { cell } else { player }
}

/// The elevation events in front of the player are matched with (Gen3
/// `GetInFrontOfPlayerPosition`): 0 when the player stands on an elevation 0 cell.
pub const fn facing_elevation(cell: u8, player: u8) -> u8 {
    if cell != 0 { player } else { 0 }
}

/// A search node: a cell and the player's elevation there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Node {
    pub x: i32,
    pub y: i32,
    pub e: u8,
}

/// A way out of the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exit {
    /// The warp event at this index.
    Warp(usize),
    /// Off the edge into the connection `side` ([`Grid::sides`]), from map cell (x, y) (for a
    /// ledge jumped across the edge, the ledge's cell).
    Edge { side: usize, x: i32, y: i32 },
}

/// Which objects with a hide flag count as absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Hidden {
    /// Every object is there.
    #[default]
    None,
    /// The objects with these flags are absent.
    Flags(BTreeSet<String>),
    /// Every object with a hide flag is absent, but those with these flags.
    AllBut(BTreeSet<String>),
}

impl Hidden {
    pub fn hides(&self, flag: Option<&str>) -> bool {
        match (self, flag) {
            (_, None) | (Hidden::None, _) => false,
            (Hidden::Flags(f), Some(x)) => f.contains(x),
            (Hidden::AllBut(f), Some(x)) => !f.contains(x),
        }
    }
}

/// An object standing in the way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Obstacle {
    /// Its index in the map's objects.
    pub object: usize,
    pub x: i32,
    pub y: i32,
    pub elevation: u8,
}

/// Gen3 `TRAINER_TYPE_NORMAL` and `TRAINER_TYPE_BURIED` by name.
pub const TRAINER_NORMAL: &str = "TRAINER_TYPE_NORMAL";
pub const TRAINER_BURIED: &str = "TRAINER_TYPE_BURIED";

/// The facings an object of this `MOVEMENT_TYPE_*` may have, in `Dir::ALL` order: the
/// directions its name gives for `FACE_*`, the in-place walks and the back-and-forth walks;
/// all four for looking around, rotating, wandering and walk sequences; down otherwise (the
/// content build's fallback).
pub fn facings(movement: &str) -> Vec<Dir> {
    let t = movement.strip_prefix("MOVEMENT_TYPE_").unwrap_or(movement);
    let named: Vec<Dir> = (t.split('_'))
        .filter_map(|w| match w {
            "DOWN" | "SOUTH" => Some(Dir::Down),
            "UP" | "NORTH" => Some(Dir::Up),
            "LEFT" | "WEST" => Some(Dir::Left),
            "RIGHT" | "EAST" => Some(Dir::Right),
            _ => None,
        })
        .collect();
    let all = ["LOOK_AROUND", "ROTATE", "WANDER", "WALK_SEQUENCE"];
    let ds: Vec<Dir> = if all.iter().any(|p| t.starts_with(p)) {
        Dir::ALL.to_vec()
    } else if (t.starts_with("FACE_") || t.starts_with("WALK_") || t.contains("IN_PLACE"))
        && !named.is_empty()
    {
        named
    } else {
        vec![Dir::Down]
    };
    Dir::ALL.into_iter().filter(|d| ds.contains(d)).collect()
}

/// A map's walk graph: its document, classified grid, the objects in the way, and Surf.
pub struct Walk<'a> {
    pub doc: &'a Doc,
    pub grid: &'a Grid,
    pub obstacles: Vec<Obstacle>,
    pub surf: bool,
}

/// One move out of a node.
enum Step {
    To(Node),
    Exit(Exit),
}

/// The steps' direction order (paths prefer earlier ones).
const ORDER: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

impl<'a> Walk<'a> {
    /// The walk with every object of `doc` in bounds but those `hidden` hides.
    pub fn new(doc: &'a Doc, grid: &'a Grid, hidden: &Hidden, surf: bool) -> Walk<'a> {
        let mut w = Walk {
            doc,
            grid,
            obstacles: Vec::new(),
            surf,
        };
        let keep: Vec<usize> = (doc.map.objects.iter().enumerate())
            .filter(|(_, o)| !hidden.hides(o.flag.as_deref()))
            .map(|(i, _)| i)
            .collect();
        w.set_objects(&keep);
        w
    }

    /// Makes exactly these objects (indices into the map's objects) obstacles.
    pub fn set_objects(&mut self, objects: &[usize]) {
        self.obstacles = (objects.iter())
            .filter_map(|&i| {
                let o = self.doc.map.objects.get(i)?;
                let (x, y) = (o.x as i32, o.y as i32);
                let cell = self.grid.cell(x, y)?;
                let e = match cell.elevation() {
                    15 => o.elevation,
                    e => e,
                };
                Some(Obstacle {
                    object: i,
                    x,
                    y,
                    elevation: e,
                })
            })
            .collect();
    }

    /// The obstacle of object `i`, if it is one.
    pub fn obstacle(&self, i: usize) -> Option<&Obstacle> {
        self.obstacles.iter().find(|o| o.object == i)
    }

    /// An object at (x, y) at an elevation compatible with `e` (but object `except`).
    fn blocked_by_object(&self, x: i32, y: i32, e: u8, except: Option<usize>) -> bool {
        (self.obstacles.iter()).any(|o| {
            (o.x, o.y) == (x, y)
                && Some(o.object) != except
                && elevations_compatible(e, o.elevation)
        })
    }

    /// The node of a player placed on (x, y): the cell's elevation (3 on an elevation 15
    /// cell).
    pub fn start(&self, x: i32, y: i32) -> Node {
        let e = match self.grid.cell(x, y).map_or(3, |c| c.elevation()) {
            15 => 3,
            e => e,
        };
        Node { x, y, e }
    }

    /// The warp event on (x, y) matching elevation `e`.
    pub fn warp_at(&self, x: i32, y: i32, e: u8) -> Option<usize> {
        (self.doc.map.warps.iter())
            .position(|w| (w.x as i32, w.y as i32) == (x, y) && event_matches(w.elevation, e))
    }

    /// The warp a step finishing on `n` takes.
    pub fn entered_warp(&self, n: Node) -> Option<usize> {
        let c = self.grid.cell(n.x, n.y)?;
        if !c.behavior.enters_warp() {
            return None;
        }
        self.warp_at(n.x, n.y, n.e)
    }

    /// The step tests on the cell in front (rule 4 of the module docs, objects aside).
    fn passable(&self, from: &Cell, e: u8, to: &Cell, d: Dir) -> bool {
        if to.collision() != 0 {
            return false;
        }
        if to.behavior == Behavior::Blocked && !(self.surf && to.surfable()) {
            return false;
        }
        if Behavior::walled(from.behavior, to.behavior, d) {
            return false;
        }
        let surfing = self.surf && (from.surfable() || to.surfable());
        surfing || !elevation_mismatch(e, to.elevation())
    }

    /// The connection a step from `n` (on `from`) towards `d` into (tx, ty), outside the map,
    /// leaves by.
    fn edge_exit(&self, from: &Cell, n: Node, d: Dir, tx: i32, ty: i32) -> Option<usize> {
        let (side, cell) = self.grid.outside(tx, ty)?;
        match cell {
            None => Some(side),
            Some(c) if c.behavior == Behavior::Ledge(d) => Some(side),
            Some(c) => self.passable(from, n.e, &c, d).then_some(side),
        }
    }

    fn steps(&self, n: Node, out: &mut Vec<Step>) {
        let Some(cur) = self.grid.cell(n.x, n.y) else {
            return;
        };
        out.extend(ORDER.into_iter().filter_map(|d| self.step(n, cur, d)));
    }

    /// The one move pushing `d` from node `n` (on `cur`) makes, if any: every per-behaviour
    /// step rule of the module docs is here, in Gen3's order, so forced movement (ice, slides,
    /// walk arrows, cracked floors) belongs here too.
    fn step(&self, n: Node, cur: &Cell, d: Dir) -> Option<Step> {
        if cur.behavior == Behavior::ArrowWarp(d)
            && let Some(i) = self.warp_at(n.x, n.y, n.e)
        {
            return Some(Step::Exit(Exit::Warp(i)));
        }
        let (dx, dy) = d.delta();
        let (dx, dy) = (dx as i32, dy as i32);
        let (tx, ty) = (n.x + dx, n.y + dy);
        let Some(to) = self.grid.cell(tx, ty) else {
            let side = self.edge_exit(cur, n, d, tx, ty)?;
            return Some(Step::Exit(Exit::Edge {
                side,
                x: n.x,
                y: n.y,
            }));
        };
        if d == Dir::Up
            && to.behavior == Behavior::Door
            && let Some(i) = self.warp_at(tx, ty, facing_elevation(cur.elevation(), n.e))
        {
            return Some(Step::Exit(Exit::Warp(i)));
        }
        if to.behavior == Behavior::Ledge(d) {
            let (lx, ly) = (tx + dx, ty + dy);
            return match self.grid.cell(lx, ly) {
                Some(l) => Some(Step::To(Node {
                    x: lx,
                    y: ly,
                    e: elevation_after(n.e, l.elevation()),
                })),
                None => {
                    let side = self.edge_exit(to, n, d, lx, ly)?;
                    Some(Step::Exit(Exit::Edge { side, x: tx, y: ty }))
                }
            };
        }
        // Starting or stopping to surf, objects are matched at the cell's elevation (Gen3
        // `CanStopSurfing` looks for an object at elevation 3).
        let oe = match self.surf && cur.surfable() != to.surfable() {
            true => to.elevation(),
            false => n.e,
        };
        (self.passable(cur, n.e, to, d) && !self.blocked_by_object(tx, ty, oe, None)).then(|| {
            Step::To(Node {
                x: tx,
                y: ty,
                e: elevation_after(n.e, to.elevation()),
            })
        })
    }

    /// Breadth-first search from `starts`, never stepping on a node `avoid` accepts (the
    /// starts themselves excepted).
    pub fn search(&self, starts: &[Node], avoid: &dyn Fn(Node) -> bool) -> Search {
        let mut s = Search::new(self.grid.width, self.grid.height);
        let mut q = VecDeque::new();
        for &n in starts {
            if let Some(i) = s.index(n)
                && s.prev[i] == UNSEEN
            {
                s.prev[i] = i as u32;
                s.starts.push(n);
                q.push_back(n);
            }
        }
        let mut out = Vec::new();
        while let Some(n) = q.pop_front() {
            let i = s.index(n).unwrap_or_default();
            if s.prev[i] != i as u32
                && let Some(w) = self.entered_warp(n)
            {
                s.exits.entry(Exit::Warp(w)).or_insert(n);
                continue;
            }
            out.clear();
            self.steps(n, &mut out);
            for st in &out {
                match *st {
                    Step::Exit(x) => {
                        s.exits.entry(x).or_insert(n);
                    }
                    Step::To(m) => {
                        let Some(j) = s.index(m) else { continue };
                        if s.prev[j] == UNSEEN && !avoid(m) {
                            s.prev[j] = i as u32;
                            q.push_back(m);
                        }
                    }
                }
            }
        }
        s
    }

    /// The cells the player may talk to an object on (x, y) from: next to it, or across a
    /// counter (Gen3 `GetInteractionScript`).
    fn talk_cells(&self, x: i32, y: i32) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for d in ORDER {
            let (dx, dy) = d.delta();
            let (dx, dy) = (dx as i32, dy as i32);
            out.push((x - dx, y - dy));
            if (self.grid.cell(x - dx, y - dy)).is_some_and(|c| c.behavior == Behavior::Counter) {
                out.push((x - 2 * dx, y - 2 * dy));
            }
        }
        out
    }

    /// Whether the player on `n` talks to an object on (x, y) at elevation `oe`: facing it
    /// from next to it or across a counter, the facing elevation compatible with the
    /// object's.
    pub fn talks_from(&self, n: Node, x: i32, y: i32, oe: u8) -> bool {
        let Some(c) = self.grid.cell(n.x, n.y) else {
            return false;
        };
        self.talk_cells(x, y).contains(&(n.x, n.y))
            && elevations_compatible(facing_elevation(c.elevation(), n.e), oe)
    }

    /// A reached node from which the player talks to an object on (x, y) at elevation `oe`.
    pub fn talks_to(&self, s: &Search, x: i32, y: i32, oe: u8) -> Option<Node> {
        (self.talk_cells(x, y).into_iter())
            .find_map(|(sx, sy)| s.at(sx, sy).find(|&n| self.talks_from(n, x, y, oe)))
    }

    /// The directions bg event `i` may be read facing.
    fn read_dirs(&self, i: usize) -> &'static [Dir] {
        match self.doc.map.bgs.get(i).map(|b| &b.kind) {
            Some(BgKind::Script { facing: 1, .. }) => &[Dir::Up],
            Some(BgKind::Script { facing: 2, .. }) => &[Dir::Down],
            Some(BgKind::Script { facing: 3, .. }) => &[Dir::Right],
            Some(BgKind::Script { facing: 4, .. }) => &[Dir::Left],
            Some(BgKind::SecretBase { .. }) | None => &[],
            _ => &ORDER,
        }
    }

    /// Whether the player on `n` reads bg event `i` (facing it as its kind allows, at a
    /// matching elevation).
    pub fn reads_from(&self, n: Node, i: usize) -> bool {
        let (Some(bg), Some(c)) = (self.doc.map.bgs.get(i), self.grid.cell(n.x, n.y)) else {
            return false;
        };
        self.read_dirs(i).iter().any(|d| {
            let (dx, dy) = d.delta();
            (n.x + dx as i32, n.y + dy as i32) == (bg.x as i32, bg.y as i32)
        }) && event_matches(bg.elevation, facing_elevation(c.elevation(), n.e))
    }

    /// A reached node from which the player reads bg event `i`.
    pub fn reads(&self, s: &Search, i: usize) -> Option<Node> {
        let bg = self.doc.map.bgs.get(i)?;
        self.read_dirs(i).iter().find_map(|d| {
            let (dx, dy) = d.delta();
            let (sx, sy) = (bg.x as i32 - dx as i32, bg.y as i32 - dy as i32);
            s.at(sx, sy).find(|&n| self.reads_from(n, i))
        })
    }

    /// The elevation object `i` stands at (its cell's, or its template's on a 15 cell).
    pub fn object_elevation(&self, i: usize) -> Option<u8> {
        let o = self.doc.map.objects.get(i)?;
        let c = self.grid.cell(o.x as i32, o.y as i32)?;
        Some(match c.elevation() {
            15 => o.elevation,
            e => e,
        })
    }

    /// Whether object `i` is a looking trainer (normal or buried).
    pub fn is_trainer(&self, i: usize) -> bool {
        self.doc
            .map
            .objects
            .get(i)
            .is_some_and(|o| o.trainer_type == TRAINER_NORMAL || o.trainer_type == TRAINER_BURIED)
    }

    /// The cells trainer `i` sees (module docs), in facing order then distance; `None` when it
    /// is not a looking trainer.
    pub fn sight(&self, i: usize) -> Option<Vec<(i32, i32)>> {
        if !self.is_trainer(i) {
            return None;
        }
        let o = &self.doc.map.objects[i];
        let (ox, oy) = (o.x as i32, o.y as i32);
        let own = self.grid.cell(ox, oy)?;
        let te = self.object_elevation(i)?;
        let ways = match o.trainer_type.as_str() {
            TRAINER_BURIED => Dir::ALL.to_vec(),
            _ => facings(&o.movement),
        };
        let range = o.trainer_sight_or_berry_tree as i32;
        let mut cells = Vec::new();
        for d in ways {
            let (dx, dy) = d.delta();
            for k in 1..=range {
                let (x, y) = (ox + dx as i32 * k, oy + dy as i32 * k);
                let Some(c) = self.grid.cell(x, y) else {
                    break;
                };
                let clear = c.collision() == 0
                    && !Behavior::walled(own.behavior, c.behavior, d)
                    && !elevation_mismatch(te, c.elevation())
                    && !self.blocked_by_object(x, y, te, Some(i));
                if !clear {
                    break;
                }
                cells.push((x, y));
            }
        }
        Some(cells)
    }

    /// The fewest objects in the way of reaching a goal from `starts`: a node `node_goal`
    /// accepts, or an exit `exit_goal` accepts. Walking through an object costs one; the
    /// objects of a cheapest path, in path order (empty when none is in the way), or `None`
    /// when the goal cannot be reached even through objects.
    pub fn fewest_objects(
        &self,
        starts: &[Node],
        node_goal: &dyn Fn(Node) -> bool,
        exit_goal: &dyn Fn(Exit) -> bool,
    ) -> Option<Vec<usize>> {
        let free = Walk {
            doc: self.doc,
            grid: self.grid,
            obstacles: Vec::new(),
            surf: self.surf,
        };
        let mut s = Search::new(self.grid.width, self.grid.height);
        let mut cost = vec![u32::MAX; s.prev.len()];
        let mut q = VecDeque::new();
        for &n in starts {
            if let Some(i) = s.index(n)
                && cost[i] != 0
            {
                cost[i] = 0;
                s.prev[i] = i as u32;
                q.push_back(n);
            }
        }
        let mut out = Vec::new();
        let mut done = vec![false; s.prev.len()];
        while let Some(n) = q.pop_front() {
            let i = s.index(n).unwrap_or_default();
            if std::mem::replace(&mut done[i], true) {
                continue;
            }
            let terminal = s.prev[i] != i as u32 && free.entered_warp(n).is_some();
            let hit = node_goal(n)
                || (terminal
                    && free
                        .entered_warp(n)
                        .is_some_and(|w| exit_goal(Exit::Warp(w))));
            if !hit && terminal {
                continue;
            }
            out.clear();
            if !hit {
                free.steps(n, &mut out);
            }
            let exit_hit = (out.iter()).any(|st| matches!(*st, Step::Exit(x) if exit_goal(x)));
            if hit || exit_hit {
                let path = s.path(n);
                let mut objs: Vec<usize> = Vec::new();
                for m in path {
                    let ob = (self.obstacles.iter())
                        .find(|o| (o.x, o.y) == (m.x, m.y))
                        .map(|o| o.object);
                    if let Some(ob) = ob
                        && !objs.contains(&ob)
                    {
                        objs.push(ob);
                    }
                }
                return Some(objs);
            }
            for st in &out {
                let Step::To(m) = *st else { continue };
                let Some(j) = s.index(m) else { continue };
                let c = cost[i] + u32::from(self.blocked_by_object(m.x, m.y, n.e, None));
                if c < cost[j] {
                    cost[j] = c;
                    s.prev[j] = i as u32;
                    match c == cost[i] {
                        true => q.push_front(m),
                        false => q.push_back(m),
                    }
                }
            }
        }
        None
    }

    /// The object whose `id` (`LOCALID_*`) or local id is `key`.
    pub fn object_by_key(&self, key: &str) -> Option<usize> {
        (self.doc.map.objects.iter()).position(|o| {
            o.id.as_deref() == Some(key) || key.parse::<u8>().is_ok_and(|n| n == o.local_id)
        })
    }
}

const UNSEEN: u32 = u32::MAX;

/// What a [`Walk::search`] reached.
pub struct Search {
    width: i32,
    height: i32,
    /// Per node: the node it was reached from (itself for a start), or [`UNSEEN`].
    prev: Vec<u32>,
    pub starts: Vec<Node>,
    /// Each exit found, with the node the player takes it from (first found: a shortest
    /// path).
    pub exits: BTreeMap<Exit, Node>,
}

impl Search {
    fn new(width: i32, height: i32) -> Search {
        Search {
            width,
            height,
            prev: vec![UNSEEN; (width.max(0) * height.max(0)) as usize * 16],
            starts: Vec::new(),
            exits: BTreeMap::new(),
        }
    }

    fn index(&self, n: Node) -> Option<usize> {
        (n.x >= 0 && n.y >= 0 && n.x < self.width && n.y < self.height && n.e < 16)
            .then(|| ((n.y * self.width + n.x) as usize) * 16 + n.e as usize)
    }

    fn node(&self, i: usize) -> Node {
        let c = (i / 16) as i32;
        Node {
            x: c % self.width,
            y: c / self.width,
            e: (i % 16) as u8,
        }
    }

    pub fn seen(&self, n: Node) -> bool {
        self.index(n).is_some_and(|i| self.prev[i] != UNSEEN)
    }

    /// The reached nodes on (x, y), by elevation.
    pub fn at(&self, x: i32, y: i32) -> impl Iterator<Item = Node> + '_ {
        (0..16)
            .map(move |e| Node { x, y, e })
            .filter(|&n| self.seen(n))
    }

    pub fn cell_seen(&self, x: i32, y: i32) -> bool {
        self.at(x, y).next().is_some()
    }

    /// The reached cells.
    pub fn cells(&self) -> BTreeSet<(i32, i32)> {
        (0..self.prev.len())
            .filter(|&i| self.prev[i] != UNSEEN)
            .map(|i| {
                let n = self.node(i);
                (n.x, n.y)
            })
            .collect()
    }

    /// The path from a start to `n` (both included), empty when `n` was not reached.
    pub fn path(&self, n: Node) -> Vec<Node> {
        let Some(mut i) = self.index(n).filter(|&i| self.prev[i] != UNSEEN) else {
            return Vec::new();
        };
        let mut out = vec![self.node(i)];
        while self.prev[i] as usize != i {
            i = self.prev[i] as usize;
            out.push(self.node(i));
        }
        out.reverse();
        out
    }
}

/// The cells of `nodes`, in order, without repeats in a row, as `(x, y)` text.
pub fn path_text(nodes: &[Node]) -> String {
    let mut cells: Vec<(i32, i32)> = Vec::new();
    for n in nodes {
        if cells.last() != Some(&(n.x, n.y)) {
            cells.push((n.x, n.y));
        }
    }
    (cells.iter())
        .map(|(x, y)| format!("({x}, {y})"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Objects removed by interacting with them once reached: item balls (picked up), and the
/// field-move obstacles (Rock Smash, Cut; Strength boulders, approximated as removed).
pub fn clearable(gfx: &str) -> bool {
    matches!(
        gfx,
        "OBJ_EVENT_GFX_ITEM_BALL"
            | "OBJ_EVENT_GFX_BREAKABLE_ROCK"
            | "OBJ_EVENT_GFX_CUTTABLE_TREE"
            | "OBJ_EVENT_GFX_PUSHABLE_BOULDER"
    )
}

/// Searches from `starts`, then removes every clearable object ([`clearable`]) the player can
/// talk to, and again until nothing changes. Returns the last search; the walk keeps the
/// objects left.
pub fn clear_reachable(walk: &mut Walk, starts: &[Node]) -> Search {
    loop {
        let s = walk.search(starts, &|_| false);
        let gone: Vec<usize> = (walk.obstacles.iter())
            .filter(|o| clearable(&walk.doc.map.objects[o.object].gfx))
            .filter(|o| walk.talks_to(&s, o.x, o.y, o.elevation).is_some())
            .map(|o| o.object)
            .collect();
        if gone.is_empty() {
            return s;
        }
        walk.obstacles.retain(|o| !gone.contains(&o.object));
    }
}
