//! `studio-map lint-reach`: reachability findings over a games dir's maps, on the walk graph
//! of [`super::reach`]. A triage list for map and script owners, never a gate.
//!
//! **Objects.** The lint is optimistic about story state: an object with a hide flag may be
//! hidden at some point, so it is absent; objects without one stand in the way. Item balls and
//! field-move obstacles (breakable rocks, cut trees, Strength boulders) stand in the way until
//! the player can reach a cell to use them from (`reach::clear_reachable`), whatever their
//! flags.
//!
//! **Portals** are a map's ways in and out:
//! - each warp: an exit, and an entry when a warp of the games dir arrives on it, or when its
//!   destination has a dynamic (`MAP_DYNAMIC`) warp leading back. Arriving on an animated door,
//!   a non-animated door or an escalator, the player first walks one cell out (Gen3's exit
//!   walk), and the walk starts there;
//! - each run of edge cells, along a connection, whose neighbor cell is open (collision 0, not
//!   blocked but water) and whose own cell is too: an entry and an exit;
//! - the new game's start and the heal locations (`game.ron`) on the map: entries only.
//!
//! **Findings** (`<Map>: <severity>: <subject>: <message>`, in this order per map):
//! 1. portals: the main area is the largest set of portals connected both ways (leaving by one
//!    and coming back by it counts); a portal outside it that the main area cannot reach is a
//!    warning (info when Surf reaches it), one from which the main area cannot be reached is a
//!    warning for the start and heal locations and info for the others (one-way areas);
//! 2. looking trainers with sight 0 or an empty line of sight (warnings), or whose line of
//!    sight holds no reachable cell (info);
//! 3. item balls and field-move obstacles with no reachable cell to use them from (warnings;
//!    info when Surf reaches one);
//! 4. signs and hidden items with no reachable cell to read them from (same);
//! 5. coord-event groups (one var value) that can be walked around: from some entry that
//!    reaches them, every cell next to them is reachable without stepping on them (info: some
//!    triggers are optional, such as a cell to step on).
//!
//! **Allowances.** `AllowReach(subject: "warp 3", reason: "...")` in a map's edit scripts
//! accepts the finding with that exact subject; an allowance that silences nothing is a
//! warning.

use super::catalog::Catalog;
use super::games::GamesDir;
use super::model::{Doc, Op};
use super::reach::{
    Exit, Grid, Hidden, Node, Search, Walk, clear_reachable, clearable, dir_name, elevation_after,
    event_matches, facings,
};
use super::schema::{MAP_DYNAMIC, from_script_ron};
use anyhow::{Context, Result};
use engine::data::{Behavior, Dir};
use extract::games::GameFile;
use extract::schema::BgKind;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Info,
}

/// One finding of the lint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub map: String,
    pub severity: Severity,
    /// What it is about (`warp 3`, `edge north 4-6`, `object LOCALID_X`, `bg 2`,
    /// `coords VAR_X=1`…): what an `AllowReach` names.
    pub subject: String,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self.severity {
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        write!(f, "{}: {s}: {}: {}", self.map, self.subject, self.message)
    }
}

/// What leads into a map from elsewhere: the warps arrived on, and the named points (start,
/// heal locations) as (subject, x, y).
#[derive(Clone, Debug, Default)]
pub struct Entries {
    pub arrivals: BTreeSet<usize>,
    pub points: Vec<(String, i32, i32)>,
}

/// A way in or out of the map.
#[derive(Clone, Debug)]
struct Portal {
    kind: PortalKind,
    /// The nodes an arrival starts on (empty: exit only).
    starts: Vec<Node>,
    subject: String,
    label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PortalKind {
    Warp(usize),
    /// Connection side `side`, edge cells `from..=to` along it.
    Edge {
        side: usize,
        from: i32,
        to: i32,
    },
    Point,
}

/// The node a player arriving by warp `i` stands on once the exit walk is done.
pub fn arrival(walk: &Walk, i: usize) -> Option<Node> {
    let w = walk.doc.map.warps.get(i)?;
    let (x, y) = (w.x as i32, w.y as i32);
    let cell = walk.grid.cell(x, y)?;
    let n = walk.start(x, y);
    let out = match cell.behavior {
        Behavior::Door => Some(Dir::Down),
        Behavior::NonAnimDoor(d) | Behavior::Escalator(d) => Some(d),
        _ => None,
    };
    let Some(d) = out else {
        return Some(n);
    };
    let (dx, dy) = d.delta();
    let (tx, ty) = (x + dx as i32, y + dy as i32);
    match walk.grid.cell(tx, ty) {
        Some(t) => Some(Node {
            x: tx,
            y: ty,
            e: elevation_after(n.e, t.elevation()),
        }),
        None => Some(n),
    }
}

/// The edge cell, the outside cell and the coordinate along the edge, for each position of
/// side `d`.
/// An edge cell, the outside cell next to it and the coordinate along the edge.
type EdgeCell = ((i32, i32), (i32, i32), i32);

fn edge_cells(g: &Grid, d: Dir) -> Vec<EdgeCell> {
    let (w, h) = (g.width, g.height);
    match d {
        Dir::Down => (0..w).map(|x| ((x, h - 1), (x, h), x)).collect(),
        Dir::Up => (0..w).map(|x| ((x, 0), (x, -1), x)).collect(),
        Dir::Left => (0..h).map(|y| ((0, y), (-1, y), y)).collect(),
        Dir::Right => (0..h).map(|y| ((w - 1, y), (w, y), y)).collect(),
    }
}

/// The along coordinate of edge cell (x, y) on side `d`.
fn along(d: Dir, x: i32, y: i32) -> i32 {
    match d {
        Dir::Up | Dir::Down => x,
        Dir::Left | Dir::Right => y,
    }
}

fn portals(walk: &Walk, entries: &Entries) -> Vec<Portal> {
    let doc = walk.doc;
    let mut out = Vec::new();
    for (i, w) in doc.map.warps.iter().enumerate() {
        let starts = match entries.arrivals.contains(&i) {
            true => arrival(walk, i).into_iter().collect(),
            false => Vec::new(),
        };
        out.push(Portal {
            kind: PortalKind::Warp(i),
            starts,
            subject: format!("warp {i}"),
            label: format!(
                "warp {i} ({}, {}) to {} #{}",
                w.x, w.y, w.dest_map, w.dest_warp
            ),
        });
    }
    let g = walk.grid;
    for (si, s) in g.sides.iter().enumerate() {
        // Runs of open positions, with the arrival nodes of each.
        let mut runs: Vec<(i32, i32, Vec<Node>)> = Vec::new();
        for ((ex, ey), (ox, oy), a) in edge_cells(g, s.dir) {
            let Some((side, outer)) = g.outside(ox, oy) else {
                continue;
            };
            let Some(inner) = g.cell(ex, ey) else {
                continue;
            };
            let open = |c: &super::reach::Cell| {
                c.collision() == 0 && (c.behavior != Behavior::Blocked || c.water)
            };
            if side != si || !outer.as_ref().is_none_or(open) || !open(inner) {
                continue;
            }
            let pe = outer
                .map(|c| c.elevation())
                .filter(|&e| e != 15)
                .unwrap_or(inner.elevation());
            let n = Node {
                x: ex,
                y: ey,
                e: elevation_after(pe, inner.elevation()),
            };
            match runs.last_mut() {
                Some((_, to, nodes)) if *to == a - 1 => {
                    *to = a;
                    nodes.push(n);
                }
                _ => runs.push((a, a, vec![n])),
            }
        }
        for (from, to, starts) in runs {
            let name = dir_name(s.dir);
            let range = match from == to {
                true => from.to_string(),
                false => format!("{from}-{to}"),
            };
            let axis = match s.dir {
                Dir::Up | Dir::Down => "x",
                Dir::Left | Dir::Right => "y",
            };
            out.push(Portal {
                kind: PortalKind::Edge { side: si, from, to },
                starts,
                subject: format!("edge {name} {range}"),
                label: format!("{name} edge {axis} {range} to {}", s.map),
            });
        }
    }
    for (subject, x, y) in &entries.points {
        out.push(Portal {
            kind: PortalKind::Point,
            starts: vec![walk.start(*x, *y)],
            subject: subject.clone(),
            label: format!("{subject} ({x}, {y})"),
        });
    }
    out
}

/// The portal an exit belongs to.
fn exit_portal(ps: &[Portal], g: &Grid, x: Exit) -> Option<usize> {
    ps.iter().position(|p| match (&p.kind, x) {
        (PortalKind::Warp(i), Exit::Warp(j)) => *i == j,
        (PortalKind::Edge { side, from, to }, Exit::Edge { side: s, x, y }) => {
            *side == s && (*from..=*to).contains(&along(g.sides[s].dir, x, y))
        }
        _ => false,
    })
}

/// Per portal, the portals reached from its arrival (direct), then the transitive closure.
fn closure(direct: &[BTreeSet<usize>]) -> Vec<BTreeSet<usize>> {
    (0..direct.len())
        .map(|a| {
            let mut seen: BTreeSet<usize> = BTreeSet::new();
            let mut todo: Vec<usize> = direct[a].iter().copied().collect();
            while let Some(b) = todo.pop() {
                if seen.insert(b) {
                    todo.extend(direct[b].iter().copied());
                }
            }
            seen
        })
        .collect()
}

fn direct_reach(walk: &Walk, ps: &[Portal]) -> Vec<BTreeSet<usize>> {
    ps.iter()
        .map(|p| {
            if p.starts.is_empty() {
                return BTreeSet::new();
            }
            let s = walk.search(&p.starts, &|_| false);
            (s.exits.keys())
                .filter_map(|&x| exit_portal(ps, walk.grid, x))
                .collect()
        })
        .collect()
}

/// Where a portal leads, for the games dir's portal graph.
#[derive(Clone, Debug)]
enum Link {
    Warp {
        dest: String,
        dest_warp: usize,
    },
    /// Edge cells `from..=to` along the side `dir` connected to `map` at `offset`.
    Edge {
        dir: Dir,
        map: String,
        offset: i32,
        from: i32,
        to: i32,
    },
    Point,
}

/// One map's portals in the games dir's portal graph.
#[derive(Clone, Debug)]
struct MapGraph {
    links: Vec<Link>,
    /// The portal can be arrived on.
    entry: Vec<bool>,
    /// Per portal, the portals its arrival leaves by: on foot, then with Surf.
    direct: [Vec<BTreeSet<usize>>; 2],
}

/// The games dir's portal graph: every map's portals, what each arrival reaches in its map,
/// and where each exit arrives (a warp's destination warp; for a dynamic warp, every warp
/// leading to the map; the connected neighbor's facing edge runs that overlap). Arriving
/// somewhere, leaving by an exit and arriving through it is a step; a lint finding about a
/// portal is dropped when the walk through other maps reaches it.
pub struct Links {
    maps: BTreeMap<String, MapGraph>,
}

impl Links {
    /// The graph of every map of `games`.
    pub fn new(
        games: &GamesDir,
        catalog: &Catalog,
        entries: &BTreeMap<String, Entries>,
    ) -> Result<Links> {
        let none = Entries::default();
        let mut maps = BTreeMap::new();
        for name in games.map_names()? {
            let doc = games.load(&name)?;
            let grid = Grid::with_neighbors(&doc, catalog, games);
            let e = entries.get(&name).unwrap_or(&none);
            maps.insert(name, map_graph(&doc, &grid, e));
        }
        Ok(Links { maps })
    }

    /// The arrivals leaving `map` by its portal `exit` leads to.
    fn arrivals(&self, map: &str, exit: usize) -> Vec<(String, usize)> {
        let Some(g) = self.maps.get(map) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        match &g.links[exit] {
            Link::Warp { dest, .. } if dest == MAP_DYNAMIC => {
                for (n, ng) in &self.maps {
                    for (c, l) in ng.links.iter().enumerate() {
                        if matches!(l, Link::Warp { dest, .. } if dest == map) {
                            out.push((n.clone(), c));
                        }
                    }
                }
            }
            Link::Warp { dest, dest_warp } => {
                if let Some(ng) = self.maps.get(dest)
                    && matches!(ng.links.get(*dest_warp), Some(Link::Warp { .. }))
                {
                    out.push((dest.clone(), *dest_warp));
                }
            }
            Link::Edge {
                dir,
                map: n,
                offset,
                from,
                to,
            } => {
                if let Some(ng) = self.maps.get(n) {
                    for (c, l) in ng.links.iter().enumerate() {
                        if let Link::Edge {
                            dir: d2,
                            map: back,
                            from: f2,
                            to: t2,
                            ..
                        } = l
                            && *d2 == dir.opposite()
                            && back == map
                            && from - offset <= *t2
                            && *f2 <= to - offset
                        {
                            out.push((n.clone(), c));
                        }
                    }
                }
            }
            Link::Point => {}
        }
        out.retain(|(n, c)| self.maps[n].entry[*c]);
        out
    }

    /// From the arrivals `starts` of `map`, through every map: the exits left by and the
    /// arrivals reached, as (map, portal).
    #[allow(clippy::type_complexity)]
    fn walk(
        &self,
        map: &str,
        starts: &[usize],
        surf: bool,
    ) -> (BTreeSet<(String, usize)>, BTreeSet<(String, usize)>) {
        let mut exits = BTreeSet::new();
        let mut seen: BTreeSet<(String, usize)> = BTreeSet::new();
        let mut todo: Vec<(String, usize)> = starts.iter().map(|&a| (map.to_string(), a)).collect();
        while let Some((m, a)) = todo.pop() {
            if !seen.insert((m.clone(), a)) {
                continue;
            }
            let Some(g) = self.maps.get(&m) else {
                continue;
            };
            for &b in &g.direct[surf as usize][a] {
                if exits.insert((m.clone(), b)) {
                    todo.extend(self.arrivals(&m, b));
                }
            }
        }
        (exits, seen)
    }
}

/// The lint's walks of a map: on foot and with Surf, the objects of [`lint_objects`] cleared
/// as the player reaches them; its portals; and the searches from all its arrivals.
struct Setup<'a> {
    walk: Walk<'a>,
    surf: Walk<'a>,
    ps: Vec<Portal>,
    all_starts: Vec<Node>,
    land: Search,
    water: Search,
}

fn setup<'a>(doc: &'a Doc, grid: &'a Grid, entries: &Entries) -> Setup<'a> {
    let mut walk = Walk::new(doc, grid, &Hidden::None, false);
    walk.set_objects(&lint_objects(doc));
    let mut surf = Walk::new(doc, grid, &Hidden::None, true);
    surf.set_objects(&lint_objects(doc));
    let ps = portals(&walk, entries);
    let all_starts: Vec<Node> = ps.iter().flat_map(|p| p.starts.clone()).collect();
    let land = clear_reachable(&mut walk, &all_starts);
    let water = clear_reachable(&mut surf, &all_starts);
    Setup {
        walk,
        surf,
        ps,
        all_starts,
        land,
        water,
    }
}

fn map_graph(doc: &Doc, grid: &Grid, entries: &Entries) -> MapGraph {
    let s = setup(doc, grid, entries);
    let links = (s.ps.iter())
        .map(|p| match &p.kind {
            PortalKind::Warp(i) => {
                let w = &doc.map.warps[*i];
                Link::Warp {
                    dest: w.dest_map.clone(),
                    dest_warp: w.dest_warp as usize,
                }
            }
            PortalKind::Edge { side, from, to } => {
                let sd = &grid.sides[*side];
                Link::Edge {
                    dir: sd.dir,
                    map: sd.map.clone(),
                    offset: sd.offset,
                    from: *from,
                    to: *to,
                }
            }
            PortalKind::Point => Link::Point,
        })
        .collect();
    MapGraph {
        links,
        entry: s.ps.iter().map(|p| !p.starts.is_empty()).collect(),
        direct: [direct_reach(&s.walk, &s.ps), direct_reach(&s.surf, &s.ps)],
    }
}

/// The objects the lint starts with (module docs): no hide flag, or clearable.
fn lint_objects(doc: &Doc) -> Vec<usize> {
    (doc.map.objects.iter().enumerate())
        .filter(|(_, o)| o.flag.is_none() || clearable(&o.gfx))
        .map(|(i, _)| i)
        .collect()
}

fn object_subject(doc: &Doc, i: usize) -> String {
    let o = &doc.map.objects[i];
    match &o.id {
        Some(id) => format!("object {id}"),
        None => format!("object {}", o.local_id),
    }
}

fn gfx_what(gfx: &str) -> &'static str {
    match gfx {
        "OBJ_EVENT_GFX_ITEM_BALL" => "item ball",
        "OBJ_EVENT_GFX_BREAKABLE_ROCK" => "breakable rock",
        "OBJ_EVENT_GFX_CUTTABLE_TREE" => "cuttable tree",
        "OBJ_EVENT_GFX_PUSHABLE_BOULDER" => "boulder",
        _ => "object",
    }
}

fn object_name(doc: &Doc, i: usize) -> String {
    let o = &doc.map.objects[i];
    o.id.clone()
        .unwrap_or_else(|| format!("local id {}", o.local_id))
}

/// Cells as text, runs along a row merged: `(2-5, 13) (7, 1)`.
pub fn cells_text(cells: &BTreeSet<(i32, i32)>) -> String {
    let mut by_row: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    for &(x, y) in cells {
        by_row.entry(y).or_default().push(x);
    }
    let mut runs: Vec<(i32, i32, i32)> = Vec::new();
    for (y, xs) in by_row {
        for x in xs {
            match runs.last_mut() {
                Some((_, b, ry)) if *ry == y && *b == x - 1 => *b = x,
                _ => runs.push((x, x, y)),
            }
        }
    }
    (runs.iter())
        .map(|&(a, b, y)| match a == b {
            true => format!("({a}, {y})"),
            false => format!("({a}-{b}, {y})"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The lint of one map (module docs), allowances not applied.
pub fn lint_map(doc: &Doc, grid: &Grid, entries: &Entries, links: Option<&Links>) -> Vec<Finding> {
    let name = doc.map.name.clone();
    let mut out = Vec::new();
    let mut push = |severity, subject: String, message: String| {
        out.push(Finding {
            map: name.clone(),
            severity,
            subject,
            message,
        })
    };
    let Setup {
        walk,
        surf,
        ps,
        all_starts,
        land,
        water,
    } = setup(doc, grid, entries);
    if all_starts.is_empty() {
        push(
            Severity::Info,
            "map".into(),
            "nothing leads here (no warp arrives, no connection, no start or heal location)".into(),
        );
        return out;
    }
    // The severity and the note of something unreachable on land: Surf reaches it, or it is
    // reached once the fewest objects in the way move (`objects`).
    let how = |surf_ok: bool, objects: Option<Vec<usize>>| match (surf_ok, objects) {
        (true, _) => (Severity::Info, "; Surf reaches it".to_string()),
        (false, Some(o)) if !o.is_empty() => {
            let names: Vec<String> = o.iter().map(|&i| object_name(doc, i)).collect();
            (
                Severity::Warning,
                format!("; only past {} (no hide flag)", names.join(", ")),
            )
        }
        (false, _) => (Severity::Warning, String::new()),
    };

    // 1. Portals: arrivals whose exit walk ends on a solid cell, then the main area.
    for &i in &entries.arrivals {
        let Some(w) = doc.map.warps.get(i) else {
            continue;
        };
        let (x, y) = (w.x as i32, w.y as i32);
        let Some(cell) = grid.cell(x, y) else {
            continue;
        };
        let (what, d) = match cell.behavior {
            Behavior::Door => ("door", Dir::Down),
            Behavior::NonAnimDoor(d) => ("non-animated door", d),
            Behavior::Escalator(d) => ("escalator", d),
            _ => continue,
        };
        let (dx, dy) = d.delta();
        let (tx, ty) = (x + dx as i32, y + dy as i32);
        let solid = grid
            .cell(tx, ty)
            .is_none_or(|c| c.collision() != 0 || c.behavior == Behavior::Blocked);
        if solid {
            push(
                Severity::Warning,
                format!("warp {i}"),
                format!(
                    "arriving on its {what} metatile at ({x}, {y}), the player walks {} onto \
                     ({tx}, {ty}), which is solid (Gen3's exit walk)",
                    dir_name(d)
                ),
            );
        }
    }
    if ps.len() >= 2 {
        let reach = closure(&direct_reach(&walk, &ps));
        let reach_surf = closure(&direct_reach(&surf, &ps));
        let group = |a: usize| -> BTreeSet<usize> {
            let mut g: BTreeSet<usize> = (reach[a].iter().copied())
                .filter(|&b| reach[b].contains(&a))
                .collect();
            g.insert(a);
            g
        };
        let main = (0..ps.len())
            .filter(|&a| !ps[a].starts.is_empty())
            .map(group)
            .fold(BTreeSet::new(), |best, g| match g.len() > best.len() {
                true => g,
                false => best,
            });
        let from_main = |r: &[BTreeSet<usize>], p: usize| main.iter().any(|&m| r[m].contains(&p));
        let to_main = |r: &[BTreeSet<usize>], p: usize| main.iter().any(|&m| r[p].contains(&m));
        let mut names: Vec<&str> = main.iter().map(|&m| ps[m].subject.as_str()).collect();
        if names.len() > 3 {
            names.truncate(3);
            names.push("…");
        }
        let area = format!("the main area ({})", names.join(", "));
        for (i, p) in ps.iter().enumerate() {
            if main.contains(&i) {
                continue;
            }
            let exit = p.kind != PortalKind::Point;
            let main_list: Vec<usize> = main.iter().copied().collect();
            let name = doc.map.name.as_str();
            // Through other maps (the games dir's portal graph).
            let via = |surf: bool| {
                links
                    .is_some_and(|l| (l.walk(name, &main_list, surf).0).contains(&(name.into(), i)))
            };
            let back_via = |surf: bool| {
                links.is_some_and(|l| {
                    let (exits, arrivals) = l.walk(name, &[i], surf);
                    (main.iter()).any(|&m| {
                        exits.contains(&(name.into(), m)) || arrivals.contains(&(name.into(), m))
                    })
                })
            };
            if exit && !from_main(&reach, i) {
                let starts: Vec<Node> = main.iter().flat_map(|&m| ps[m].starts.clone()).collect();
                let objs = walk.fewest_objects(&starts, &|_| false, &|x| {
                    exit_portal(&ps, grid, x) == Some(i)
                });
                let (mut sev, mut how) = how(from_main(&reach_surf, i), objs);
                if via(false) {
                    (sev, how) = (Severity::Info, "; other maps lead to it".into());
                }
                let back = match (p.starts.is_empty(), to_main(&reach, i)) {
                    (true, _) => "",
                    (false, true) => " (one-way: arriving by it reaches the main area)",
                    (false, false) => " (an isolated area)",
                };
                push(
                    sev,
                    p.subject.clone(),
                    format!(
                        "{}: not reachable from {area} within the map{back}{how}",
                        p.label
                    ),
                );
            } else if !p.starts.is_empty() && !to_main(&reach, i) {
                let mut sev = match p.kind {
                    PortalKind::Point => Severity::Warning,
                    _ => Severity::Info,
                };
                let objs = walk.fewest_objects(&p.starts, &|_| false, &|x| {
                    exit_portal(&ps, grid, x).is_some_and(|m| main.contains(&m))
                });
                let (_, mut how) = how(to_main(&reach_surf, i), objs);
                if back_via(false) {
                    (sev, how) = (Severity::Info, "; other maps lead back".into());
                }
                push(
                    sev,
                    p.subject.clone(),
                    format!(
                        "{}: arriving here, {area} cannot be reached within the map{how}",
                        p.label
                    ),
                );
            }
        }
    }

    // 2. Trainers.
    for (i, o) in doc.map.objects.iter().enumerate() {
        let Some(cells) = walk.sight(i) else {
            continue;
        };
        let face: Vec<&str> = match o.trainer_type.as_str() {
            super::reach::TRAINER_BURIED => vec!["all ways"],
            _ => facings(&o.movement).into_iter().map(dir_name).collect(),
        };
        let what = format!(
            "trainer at ({}, {}) facing {}, sight {}",
            o.x,
            o.y,
            face.join("/"),
            o.trainer_sight_or_berry_tree
        );
        if o.trainer_sight_or_berry_tree == 0 {
            push(
                Severity::Warning,
                object_subject(doc, i),
                format!("{what}: sees nothing (talk-only trainers have no trainer type)"),
            );
        } else if cells.is_empty() {
            let why = match facings(&o.movement)[..] {
                [d] if o.trainer_type != super::reach::TRAINER_BURIED => {
                    let (dx, dy) = d.delta();
                    format!(
                        " (the cell in front, ({}, {}), blocks it)",
                        o.x + dx,
                        o.y + dy
                    )
                }
                _ => String::new(),
            };
            push(
                Severity::Warning,
                object_subject(doc, i),
                format!("{what}: its line of sight is empty{why}"),
            );
        } else if !cells.iter().any(|&(x, y)| land.cell_seen(x, y)) {
            push(
                Severity::Info,
                object_subject(doc, i),
                format!("{what}: no cell of its line of sight is reachable"),
            );
        }
    }

    // 3. Item balls and field-move obstacles.
    for (i, o) in doc.map.objects.iter().enumerate() {
        if !clearable(&o.gfx) || walk.obstacle(i).is_none() {
            continue;
        }
        let oe = walk.object_elevation(i).unwrap_or(0);
        let (ox, oy) = (o.x as i32, o.y as i32);
        let mut others = walk.obstacles.clone();
        others.retain(|ob| ob.object != i);
        let near = Walk {
            obstacles: others,
            ..Walk::new(doc, grid, &Hidden::None, false)
        };
        let objs =
            near.fewest_objects(&all_starts, &|n| near.talks_from(n, ox, oy, oe), &|_| false);
        let (sev, how) = how(surf.obstacle(i).is_none(), objs);
        push(
            sev,
            object_subject(doc, i),
            format!(
                "{} at ({}, {}): no reachable cell to use it from{how}",
                gfx_what(&o.gfx),
                o.x,
                o.y
            ),
        );
    }

    // 4. Signs and hidden items.
    for (i, b) in doc.map.bgs.iter().enumerate() {
        let what = match &b.kind {
            BgKind::Script { script, .. } => {
                format!("sign {}", script.as_deref().unwrap_or("(no script)"))
            }
            BgKind::HiddenItem { item, .. } => format!("hidden {item}"),
            BgKind::SecretBase { .. } => continue,
        };
        if walk.reads(&land, i).is_some() {
            continue;
        }
        let objs = walk.fewest_objects(&all_starts, &|n| walk.reads_from(n, i), &|_| false);
        let (sev, how) = how(surf.reads(&water, i).is_some(), objs);
        push(
            sev,
            format!("bg {i}"),
            format!(
                "{what} at ({}, {}): no reachable cell to read it from{how}",
                b.x, b.y
            ),
        );
    }

    // 5. Coord-event groups.
    let mut groups: BTreeMap<(String, u16), Vec<usize>> = BTreeMap::new();
    for (i, c) in doc.map.coords.iter().enumerate() {
        if let Some(v) = &c.var {
            groups.entry((v.clone(), c.value)).or_default().push(i);
        }
    }
    let entry_searches: Vec<(usize, Search)> = (ps.iter().enumerate())
        .filter(|(_, p)| !p.starts.is_empty())
        .map(|(i, p)| (i, walk.search(&p.starts, &|_| false)))
        .collect();
    for ((var, value), idx) in groups {
        let coords: Vec<(i32, i32, u8)> = (idx.iter())
            .map(|&i| {
                let c = &doc.map.coords[i];
                (c.x as i32, c.y as i32, c.elevation)
            })
            .collect();
        let on = |n: Node| {
            (coords.iter()).any(|&(x, y, e)| (x, y) == (n.x, n.y) && event_matches(e, n.e))
        };
        let cells: BTreeSet<(i32, i32)> = coords.iter().map(|&(x, y, _)| (x, y)).collect();
        let around: BTreeSet<(i32, i32)> = (cells.iter())
            .flat_map(|&(x, y)| {
                Dir::ALL.map(|d| {
                    let (dx, dy) = d.delta();
                    (x + dx as i32, y + dy as i32)
                })
            })
            .filter(|c| !cells.contains(c))
            .collect();
        let mut from = Vec::new();
        let mut lands = false;
        for (p, s) in &entry_searches {
            if !(cells.iter()).any(|&(x, y)| s.at(x, y).any(on)) {
                continue;
            }
            let near: Vec<&(i32, i32)> = around.iter().filter(|c| s.cell_seen(c.0, c.1)).collect();
            if near.is_empty() {
                continue;
            }
            let avoid = walk.search(&ps[*p].starts, &on);
            if near.iter().all(|c| avoid.cell_seen(c.0, c.1)) {
                from.push(ps[*p].subject.clone());
                lands |= ps[*p].starts.iter().any(|&n| on(n));
            }
        }
        if from.is_empty() {
            continue;
        }
        let scripts: BTreeSet<&str> = (idx.iter())
            .filter_map(|&i| doc.map.coords[i].script.as_deref())
            .collect();
        push(
            Severity::Info,
            format!("coords {var}={value}"),
            format!(
                "{} on {}: can be walked around from {} (every cell next to them is reachable \
                 without stepping on them{})",
                scripts.into_iter().collect::<Vec<_>>().join(", "),
                cells_text(&cells),
                from.join(", "),
                match lands {
                    true => "; an arrival lands on them, which fires no coord event",
                    false => "",
                }
            ),
        );
    }
    out
}

/// The `AllowReach` subjects and reasons of `map`'s edit scripts, in build order.
pub fn reach_allowances(games: &GamesDir, map: &str) -> Vec<(String, String)> {
    let dir = games.edits_dir().join(map);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<(u32, PathBuf)> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.to_string();
            let (phase, what) = super::batch::split_name(&name)?;
            (what != super::batch::CREATE).then_some((phase, p))
        })
        .collect();
    files.sort();
    let mut out = Vec::new();
    for (_, p) in files {
        let Ok(ops) = std::fs::read_to_string(&p)
            .map_err(anyhow::Error::from)
            .and_then(|t| from_script_ron::<Vec<Op>>(&t))
        else {
            continue;
        };
        for op in ops {
            if let Op::AllowReach { subject, reason } = op
                && !out.iter().any(|(s, _)| *s == subject)
            {
                out.push((subject, reason));
            }
        }
    }
    out
}

/// What [`lint`] found.
pub struct Report {
    pub findings: Vec<Finding>,
    /// Findings an `AllowReach` accepted.
    pub allowed: usize,
    pub maps: usize,
}

/// The entries of every map of `games` (module docs): warps arrived on, and `game.ron`'s start
/// and heal locations.
pub fn entries(games: &GamesDir) -> Result<BTreeMap<String, Entries>> {
    let mut maps = BTreeMap::new();
    for name in games.map_names()? {
        let m = games.load_map(&name)?;
        maps.insert(name, m);
    }
    let mut out: BTreeMap<String, Entries> = BTreeMap::new();
    for (name, m) in &maps {
        for (i, w) in m.warps.iter().enumerate() {
            if w.dest_map == MAP_DYNAMIC {
                continue;
            }
            (out.entry(w.dest_map.clone()).or_default().arrivals).insert(w.dest_warp as usize);
            let back = (maps.get(&w.dest_map))
                .is_some_and(|d| d.warps.iter().any(|b| b.dest_map == MAP_DYNAMIC));
            if back {
                out.entry(name.clone()).or_default().arrivals.insert(i);
            }
        }
    }
    let game = games.root.join("game.ron");
    if game.is_file() {
        let g: GameFile = extract::export::read_ron(&game)
            .with_context(|| format!("reading {}", game.display()))?;
        out.entry(g.start.map.clone()).or_default().points.push((
            "start".into(),
            g.start.x as i32,
            g.start.y as i32,
        ));
        for h in &g.heal_locations {
            out.entry(h.map.clone()).or_default().points.push((
                format!("heal {}", h.name),
                h.x as i32,
                h.y as i32,
            ));
        }
    }
    Ok(out)
}

/// Lints `names` (module docs), with each map's `AllowReach` allowances applied.
pub fn lint(games: &GamesDir, catalog: &Catalog, names: &[String]) -> Result<Report> {
    let entries = entries(games)?;
    let mut r = Report {
        findings: Vec::new(),
        allowed: 0,
        maps: names.len(),
    };
    let links = Links::new(games, catalog, &entries)?;
    let none = Entries::default();
    for name in names {
        let doc = games.load(name)?;
        let grid = Grid::with_neighbors(&doc, catalog, games);
        let found = lint_map(
            &doc,
            &grid,
            entries.get(name).unwrap_or(&none),
            Some(&links),
        );
        let allow = reach_allowances(games, name);
        let mut used = vec![false; allow.len()];
        for f in found {
            match allow.iter().position(|(s, _)| *s == f.subject) {
                Some(a) => {
                    used[a] = true;
                    r.allowed += 1;
                }
                None => r.findings.push(f),
            }
        }
        for (i, (s, reason)) in allow.iter().enumerate() {
            if !used[i] {
                r.findings.push(Finding {
                    map: name.clone(),
                    severity: Severity::Warning,
                    subject: format!("allowance {s}"),
                    message: format!("{reason:?} silences no finding (remove it)"),
                });
            }
        }
    }
    Ok(r)
}
