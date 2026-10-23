//! The reachability commands of `studio-map` (`reach`, `bypass`, `lint-reach`) and the flags
//! `render --reach` shares with them. Plain text, in a stable order.

use super::catalog::Catalog;
use super::games::GamesDir;
use super::model::Doc;
use super::reach::{
    Exit, Grid, Hidden, Node, Search, Walk, clear_reachable, dir_name, event_matches, path_text,
};
use super::reach_lint::{Severity, cells_text, lint};
use anyhow::{Context, Result, bail, ensure};
use engine::data::{BLOCK_COLLISION_SHIFT, Behavior};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The flags of `reach`, `bypass` and `render --reach`.
#[derive(Default)]
pub struct ReachFlags {
    pub from: Option<(i32, i32)>,
    pub to: Option<(i32, i32)>,
    pub surf: bool,
    pub hidden: Hidden,
    pub field_moves: bool,
    pub avoid: Vec<String>,
    pub sight: bool,
    /// Cells made walkable (collision 0, a normal behaviour), as a script's `setmetatile`
    /// would.
    pub open: Vec<(i32, i32)>,
}

/// `x,y;x,y…`.
pub fn cells(v: &str) -> Result<Vec<(i32, i32)>> {
    (v.split(';'))
        .filter(|c| !c.trim().is_empty())
        .map(point)
        .collect()
}

/// `x,y`.
pub fn point(v: &str) -> Result<(i32, i32)> {
    let bad = || format!("{v:?}: a cell is x,y");
    let (x, y) = v.split_once(',').with_context(bad)?;
    Ok((
        x.trim().parse().with_context(bad)?,
        y.trim().parse().with_context(bad)?,
    ))
}

fn flag_set(v: &str) -> BTreeSet<String> {
    (v.split(','))
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map(String::from)
        .collect()
}

impl ReachFlags {
    /// Reads one flag at the head of `f` (and its value); `None` when it is not one of these.
    pub fn take<'a>(&mut self, f: &'a [String]) -> Result<Option<&'a [String]>> {
        Ok(Some(match f {
            [k, v, t @ ..] if k == "--from" => {
                self.from = Some(point(v)?);
                t
            }
            [k, v, t @ ..] if k == "--to" => {
                self.to = Some(point(v)?);
                t
            }
            [k, t @ ..] if k == "--surf" => {
                self.surf = true;
                t
            }
            [k, t @ ..] if k == "--field-moves" => {
                self.field_moves = true;
                t
            }
            [k, t @ ..] if k == "--sight" => {
                self.sight = true;
                t
            }
            [k, v, t @ ..] if k == "--hidden" || k == "--shown" => {
                ensure!(
                    self.hidden == Hidden::None,
                    "--hidden and --shown: give one of them, once"
                );
                self.hidden = match (k.as_str(), v.as_str()) {
                    ("--hidden", "all") => Hidden::AllBut(BTreeSet::new()),
                    ("--hidden", _) => Hidden::Flags(flag_set(v)),
                    _ => Hidden::AllBut(flag_set(v)),
                };
                t
            }
            [k, v, t @ ..] if k == "--open" => {
                self.open.extend(cells(v)?);
                t
            }
            [k, v, t @ ..] if k == "--avoid" => {
                self.avoid.push(v.clone());
                t
            }
            _ => return Ok(None),
        }))
    }

    /// `doc`'s grid with its neighbors, the `--open` cells opened.
    fn grid(&self, doc: &Doc, catalog: &Catalog, games: Option<&GamesDir>) -> Result<Grid> {
        let mut g = match games {
            Some(games) => Grid::with_neighbors(doc, catalog, games),
            None => Grid::new(doc, catalog),
        };
        for &(x, y) in &self.open {
            let c = g
                .cell_mut(x, y)
                .with_context(|| format!("--open ({x}, {y}) is outside the map"))?;
            c.block &= !(3 << BLOCK_COLLISION_SHIFT);
            c.behavior = Behavior::Normal;
        }
        Ok(g)
    }

    fn walk<'a>(&self, doc: &'a Doc, grid: &'a Grid) -> Walk<'a> {
        Walk::new(doc, grid, &self.hidden, self.surf)
    }

    fn describe(&self, walk: &Walk) -> String {
        let all = walk.doc.map.objects.len();
        let absent = (walk.doc.map.objects.iter())
            .filter(|o| self.hidden.hides(o.flag.as_deref()))
            .count();
        format!(
            "Surf {}; {} objects in the way, {absent} hidden{}{}",
            if self.surf { "on" } else { "off" },
            walk.obstacles.len(),
            match self.field_moves {
                true => format!(", {} cleared", all - absent - walk.obstacles.len()),
                false => String::new(),
            },
            match self.open.len() {
                0 => String::new(),
                n => format!("; {n} cells opened"),
            }
        )
    }
}

/// The search from `--from` (clearing reachable item balls and field-move obstacles with
/// `--field-moves`).
fn search_from(f: &ReachFlags, walk: &mut Walk, from: (i32, i32)) -> Result<Search> {
    ensure!(
        walk.grid.in_bounds(from.0, from.1),
        "--from ({}, {}) is outside the map",
        from.0,
        from.1
    );
    let start = [walk.start(from.0, from.1)];
    Ok(match f.field_moves {
        true => clear_reachable(walk, &start),
        false => walk.search(&start, &|_| false),
    })
}

/// The cells reached by `render --reach`.
pub fn reached_cells(
    games: &GamesDir,
    catalog: &Catalog,
    doc: &Doc,
    f: &ReachFlags,
) -> Result<BTreeSet<(i32, i32)>> {
    let from = f.from.context("--reach needs x,y")?;
    let grid = f.grid(doc, catalog, Some(games))?;
    let mut walk = f.walk(doc, &grid);
    Ok(search_from(f, &mut walk, from)?.cells())
}

/// Every trainer's line of sight, for `render --sight`.
pub fn sight_cells(catalog: &Catalog, doc: &Doc, f: &ReachFlags) -> Result<BTreeSet<(i32, i32)>> {
    let grid = f.grid(doc, catalog, None)?;
    let walk = f.walk(doc, &grid);
    Ok((0..doc.map.objects.len())
        .filter(|&i| walk.obstacle(i).is_some())
        .filter_map(|i| walk.sight(i))
        .flatten()
        .collect())
}

fn exit_text(walk: &Walk, x: Exit) -> String {
    match x {
        Exit::Warp(i) => {
            let w = &walk.doc.map.warps[i];
            format!(
                "warp {i} ({}, {}) to {} #{}",
                w.x, w.y, w.dest_map, w.dest_warp
            )
        }
        Exit::Edge { side, .. } => {
            let s = &walk.grid.sides[side];
            format!("{} edge to {}", dir_name(s.dir), s.map)
        }
    }
}

fn object_name(walk: &Walk, i: usize) -> String {
    let o = &walk.doc.map.objects[i];
    o.id.clone()
        .unwrap_or_else(|| format!("object {}", o.local_id))
}

/// `reach <Map> --from x,y [--surf] [--hidden F,..|all | --shown F,..] [--field-moves]`.
pub fn reach(games: &GamesDir, map: &str, f: &ReachFlags) -> Result<String> {
    let from = f.from.context("reach needs --from x,y")?;
    let doc = games.load(map)?;
    let catalog = Catalog::for_games(games)?;
    let grid = f.grid(&doc, &catalog, Some(games))?;
    let mut walk = f.walk(&doc, &grid);
    let s = search_from(f, &mut walk, from)?;
    let cells = s.cells();
    let mut out = String::new();
    writeln!(
        out,
        "{map} from ({}, {}): {} cells reachable ({})",
        from.0,
        from.1,
        cells.len(),
        f.describe(&walk)
    )?;
    let mut exits: Vec<(String, Node)> = Vec::new();
    let mut edges = BTreeSet::new();
    for (&x, &n) in &s.exits {
        if let Exit::Edge { side, .. } = x
            && !edges.insert(side)
        {
            continue;
        }
        exits.push((exit_text(&walk, x), n));
    }
    writeln!(out, "exits: {}", exits.len())?;
    for (t, n) in &exits {
        writeln!(out, "  {t}, from ({}, {})", n.x, n.y)?;
    }
    let coords: Vec<String> = (walk.doc.map.coords.iter().enumerate())
        .filter(|(_, c)| {
            s.at(c.x as i32, c.y as i32)
                .any(|n| event_matches(c.elevation, n.e))
        })
        .map(|(i, c)| {
            format!(
                "C{i} ({}, {}) {}",
                c.x,
                c.y,
                c.script.as_deref().unwrap_or("-")
            )
        })
        .collect();
    writeln!(out, "coord events reached: {}", coords.len())?;
    for c in coords {
        writeln!(out, "  {c}")?;
    }
    let mut trainers = Vec::new();
    for i in 0..doc.map.objects.len() {
        if walk.obstacle(i).is_none() {
            continue;
        }
        let Some(sight) = walk.sight(i) else {
            continue;
        };
        let seen: Vec<String> = (sight.iter())
            .filter(|&&(x, y)| s.cell_seen(x, y))
            .map(|(x, y)| format!("({x}, {y})"))
            .collect();
        trainers.push(format!(
            "{}: sees {} cells, {} reachable{}{}",
            object_name(&walk, i),
            sight.len(),
            seen.len(),
            if seen.is_empty() { "" } else { ": " },
            seen.join(" ")
        ));
    }
    writeln!(out, "trainers: {}", trainers.len())?;
    for t in trainers {
        writeln!(out, "  {t}")?;
    }
    out.push_str(
        "S start, + reachable, o object in the way, # solid or blocked, ~ water, . walkable \
         but not reached\n",
    );
    out.push_str(&grid_text(&walk, &cells, from));
    Ok(out)
}

/// The map as one character per cell (the legend of [`reach`]).
fn grid_text(walk: &Walk, cells: &BTreeSet<(i32, i32)>, from: (i32, i32)) -> String {
    let g = walk.grid;
    let mut out = String::from("y\\x ");
    for x in 0..g.width {
        out.push(char::from(b'0' + (x % 10) as u8));
    }
    out.push('\n');
    for y in 0..g.height {
        let _ = write!(out, "{y:>3} ");
        for x in 0..g.width {
            let c = g.cell(x, y).copied().unwrap_or(super::reach::Cell::new(0));
            let ch = if (x, y) == from {
                'S'
            } else if cells.contains(&(x, y)) {
                '+'
            } else if walk.obstacles.iter().any(|o| (o.x, o.y) == (x, y)) {
                'o'
            } else if c.water {
                '~'
            } else if c.collision() != 0 || c.behavior == Behavior::Blocked {
                '#'
            } else {
                '.'
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

/// What `--avoid` names: the nodes to avoid, and its description.
struct Avoid {
    /// Coord cells with their event elevation.
    coords: Vec<(i32, i32, u8)>,
    /// Cells avoided at any elevation.
    cells: BTreeSet<(i32, i32)>,
    what: Vec<String>,
}

fn avoid(walk: &Walk, specs: &[String]) -> Result<Avoid> {
    let mut a = Avoid {
        coords: Vec::new(),
        cells: BTreeSet::new(),
        what: Vec::new(),
    };
    let m = &walk.doc.map;
    for spec in specs {
        let (kind, v) = spec.split_once(':').with_context(|| {
            format!("--avoid {spec:?}: coord:<script|index>, trainer:<id>, cells:x,y;x,y")
        })?;
        match kind {
            "coord" => {
                let mut found: Vec<usize> = (m.coords.iter().enumerate())
                    .filter(|(_, c)| c.script.as_deref() == Some(v))
                    .map(|(i, _)| i)
                    .collect();
                if found.is_empty()
                    && let Ok(i) = v.parse::<usize>()
                    && i < m.coords.len()
                {
                    found.push(i);
                }
                ensure!(!found.is_empty(), "--avoid {spec}: no such coord event");
                let list: BTreeSet<(i32, i32)> = (found.iter())
                    .map(|&i| (m.coords[i].x as i32, m.coords[i].y as i32))
                    .collect();
                for i in found {
                    let c = &m.coords[i];
                    a.coords.push((c.x as i32, c.y as i32, c.elevation));
                }
                a.what.push(format!("coord {v} on {}", cells_text(&list)));
            }
            "trainer" => {
                let i = walk
                    .object_by_key(v)
                    .with_context(|| format!("--avoid {spec}: no such object"))?;
                let sight = walk
                    .sight(i)
                    .with_context(|| format!("--avoid {spec}: not a looking trainer"))?;
                let list: BTreeSet<(i32, i32)> = sight.iter().copied().collect();
                a.cells.extend(sight);
                a.what.push(format!(
                    "trainer {v}'s sight on {}",
                    match list.is_empty() {
                        true => "no cell".to_string(),
                        false => cells_text(&list),
                    }
                ));
            }
            "cells" => {
                let cells = cells(v)?;
                ensure!(!cells.is_empty(), "--avoid {spec}: no cell");
                let list: BTreeSet<(i32, i32)> = cells.iter().copied().collect();
                a.cells.extend(cells);
                a.what.push(format!("cells {}", cells_text(&list)));
            }
            _ => bail!("--avoid {spec}: coord:<script|index>, trainer:<id>, cells:x,y;x,y"),
        }
    }
    Ok(a)
}

/// The path to `to` in `s`: standing on it, or taking the warp on it.
fn path_to(walk: &Walk, s: &Search, to: (i32, i32)) -> Option<Vec<Node>> {
    if let Some(n) = s.at(to.0, to.1).next() {
        return Some(s.path(n));
    }
    let i = (walk.doc.map.warps.iter()).position(|w| (w.x as i32, w.y as i32) == to)?;
    let &n = s.exits.get(&Exit::Warp(i))?;
    let mut p = s.path(n);
    p.push(Node {
        x: to.0,
        y: to.1,
        e: n.e,
    });
    Some(p)
}

/// `bypass <Map> --from x,y --to x,y [--avoid …]... [--surf] [--hidden …]`.
pub fn bypass(games: &GamesDir, map: &str, f: &ReachFlags) -> Result<String> {
    let from = f.from.context("bypass needs --from x,y")?;
    let to = f.to.context("bypass needs --to x,y")?;
    let doc = games.load(map)?;
    let catalog = Catalog::for_games(games)?;
    let grid = f.grid(&doc, &catalog, Some(games))?;
    let mut walk = f.walk(&doc, &grid);
    ensure!(grid.in_bounds(to.0, to.1), "--to is outside the map");
    let plain = search_from(f, &mut walk, from)?;
    let a = avoid(&walk, &f.avoid)?;
    let start = [walk.start(from.0, from.1)];
    let avoided = |n: Node| {
        a.cells.contains(&(n.x, n.y))
            || (a.coords.iter()).any(|&(x, y, e)| (x, y) == (n.x, n.y) && event_matches(e, n.e))
    };
    let s = walk.search(&start, &avoided);
    let mut out = String::new();
    writeln!(
        out,
        "{map}: ({}, {}) to ({}, {}), avoiding {} ({})",
        from.0,
        from.1,
        to.0,
        to.1,
        match a.what.is_empty() {
            true => "nothing".to_string(),
            false => a.what.join("; "),
        },
        f.describe(&walk)
    )?;
    let Some(p) = path_to(&walk, &plain, to) else {
        writeln!(out, "bypass: no: the goal is not reachable at all")?;
        return Ok(out);
    };
    writeln!(out, "reachable: yes, {} steps", p.len().saturating_sub(1))?;
    match path_to(&walk, &s, to) {
        Some(p) => writeln!(
            out,
            "bypass: yes, {} steps: {}",
            p.len().saturating_sub(1),
            path_text(&p)
        )?,
        None => writeln!(out, "bypass: no: every path crosses the avoided cells")?,
    }
    Ok(out)
}

/// `lint-reach (<Map>... | --all)`.
pub fn lint_reach(games: &GamesDir, maps: &[String]) -> Result<String> {
    let names = match maps {
        [a] if a == "--all" => games.map_names()?,
        [] => bail!("lint-reach needs map names or --all"),
        _ => maps.to_vec(),
    };
    let catalog = Catalog::for_games(games)?;
    let r = lint(games, &catalog, &names)?;
    let mut out = String::new();
    for f in &r.findings {
        writeln!(out, "{f}")?;
    }
    let count = |s| r.findings.iter().filter(|f| f.severity == s).count();
    writeln!(
        out,
        "{} maps: {} warnings, {} infos, {} allowed",
        r.maps,
        count(Severity::Warning),
        count(Severity::Info),
        r.allowed
    )?;
    Ok(out)
}
