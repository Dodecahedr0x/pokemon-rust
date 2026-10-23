//! Reads the games dir's maps and layouts (`maps/<Name>.ron`, `layouts/<Name>.ron`; the studio
//! map editor's format, `extract::games::maps`) and `check-exceptions.ron`.
//!
//! **Map limits** ([`check_limits`], build errors; the editor's `validate` has the same):
//! - size: at least 1x1, `(w + 15) * (h + 14)` at most Gen3's `MAX_MAP_DATA_SIZE`, one block
//!   per cell, a 2x2 border;
//! - objects: at most [`MAX_OBJECTS`] (`OBJECT_EVENT_TEMPLATES_COUNT`), local ids 1 to
//!   `MAX_OBJECTS - 1` and unique, `LOCALID_*` names unique in the map and `LOCALID_`-prefixed;
//! - warps, coord events and bg events: at most [`MAX_EVENTS`] each (Gen3 counts are `u8`);
//! - every event inside the map, every elevation at most 15.

use super::schema::{ExceptionsFile, GameMap, Layout};
use crate::codegen::ident_ok;
use anyhow::{Context, Result, bail, ensure};
use extract::export::read_ron;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Gen3 `OBJECT_EVENT_TEMPLATES_COUNT`: objects per map; local ids stay below it.
pub const MAX_OBJECTS: usize = 64;
/// Warps, coord events and bg events per map (Gen3 counts are `u8`).
pub const MAX_EVENTS: usize = 255;
/// Gen3 `MAX_MAP_DATA_SIZE`: `(width + 15) * (height + 14)` must not exceed it.
pub const MAX_MAP_DATA_SIZE: usize = 10240;

/// One map with its layout.
#[derive(Clone, Debug)]
pub struct MapIn {
    pub map: GameMap,
    pub layout: Layout,
}

/// Every `maps/*.ron`, in file name order, each with its layout. A map's `name` is its file
/// stem; its layout is `layouts/<layout>.ron`.
pub fn load_maps(games: &Path) -> Result<Vec<MapIn>> {
    let dir = games.join("maps");
    let mut paths: Vec<PathBuf> = match dir.is_dir() {
        true => std::fs::read_dir(&dir)
            .with_context(|| dir.display().to_string())?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<_>>()?,
        false => Vec::new(),
    };
    paths.retain(|p| p.extension().is_some_and(|x| x == "ron"));
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let map: GameMap = read_ron(&p)?;
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        ensure!(
            map.name == stem,
            "{}: the map is named {}",
            p.display(),
            map.name
        );
        let lp = games.join(format!("layouts/{}.ron", map.layout));
        let layout: Layout =
            read_ron(&lp).with_context(|| format!("map {}: its layout", map.name))?;
        out.push(MapIn { map, layout });
    }
    Ok(out)
}

/// The Gen3 limits of one map (see the module doc).
pub fn check_limits(m: &MapIn) -> Result<()> {
    let (g, l) = (&m.map, &m.layout);
    let (w, h) = (usize::from(l.width), usize::from(l.height));
    let data = (w + 15) * (h + 14);
    ensure!(
        w > 0 && h > 0 && data <= MAX_MAP_DATA_SIZE,
        "layout {}: {w}x{h}, (w + 15) * (h + 14) = {data} is over MAX_MAP_DATA_SIZE \
         {MAX_MAP_DATA_SIZE}",
        g.layout
    );
    let blocks = extract::schema::from_rows(&l.blocks).len();
    ensure!(
        blocks == w * h,
        "layout {}: {blocks} blocks for {w}x{h}",
        g.layout
    );
    ensure!(
        extract::schema::from_rows(&l.border).len() == 4,
        "layout {}: the border is not 2x2",
        g.layout
    );
    for (what, n, max) in [
        ("objects", g.objects.len(), MAX_OBJECTS),
        ("warps", g.warps.len(), MAX_EVENTS),
        ("coord events", g.coords.len(), MAX_EVENTS),
        ("bg events", g.bgs.len(), MAX_EVENTS),
    ] {
        ensure!(n <= max, "{n} {what}, at most {max}");
    }
    let inside = |at: String, x: i16, y: i16, e: u8| -> Result<()> {
        ensure!(
            x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h,
            "{at} at ({x}, {y}) is outside the {w}x{h} map"
        );
        ensure!(e <= 15, "{at}: elevation {e} is over 15");
        Ok(())
    };
    let mut local_ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for o in &g.objects {
        let at = format!("object {}", o.local_id);
        ensure!(
            (1..MAX_OBJECTS).contains(&usize::from(o.local_id)),
            "{at}: local ids run from 1 to {}",
            MAX_OBJECTS - 1
        );
        ensure!(local_ids.insert(o.local_id), "{at}: local id used twice");
        if let Some(n) = &o.id {
            ensure!(
                n.starts_with("LOCALID_") && ident_ok(n),
                "{at}: {n:?} is not a LOCALID_* name"
            );
            ensure!(names.insert(n.as_str()), "{at}: {n} names two objects");
        }
        inside(at, o.x, o.y, o.elevation)?;
    }
    for (i, wp) in g.warps.iter().enumerate() {
        inside(format!("warp {i}"), wp.x, wp.y, wp.elevation)?;
    }
    for (i, c) in g.coords.iter().enumerate() {
        inside(format!("coord event {i}"), c.x, c.y, c.elevation)?;
    }
    for (i, b) in g.bgs.iter().enumerate() {
        inside(format!("bg event {i}"), b.x, b.y, b.elevation)?;
    }
    Ok(())
}

/// The warps of `maps` that lead to a missing warp of a loaded map: an error naming each.
pub fn check_warps(maps: &[MapIn]) -> Result<()> {
    let mut bad = Vec::new();
    for m in maps {
        for (i, wp) in m.map.warps.iter().enumerate() {
            let Some(d) = maps.iter().find(|d| d.map.name == wp.dest_map) else {
                continue;
            };
            if usize::from(wp.dest_warp) >= d.map.warps.len() {
                bad.push(format!(
                    "{} warp {i} leads to warp {} of {}, which has {}",
                    m.map.name,
                    wp.dest_warp,
                    d.map.name,
                    d.map.warps.len()
                ));
            }
        }
    }
    if !bad.is_empty() {
        bail!("{}", bad.join("; "));
    }
    Ok(())
}

/// `check-exceptions.ron`: the known issues `cargo xtask silice check` lists instead of failing
/// on. The build follows the same file: a placeholder or a truncation is allowed only where an
/// exception names it (and is reported).
#[derive(Clone, Debug, Default)]
pub struct Exceptions(pub ExceptionsFile);

impl Exceptions {
    /// `<games>/check-exceptions.ron`; none when the file is missing.
    pub fn load(games: &Path) -> Result<Exceptions> {
        let p = games.join("check-exceptions.ron");
        Ok(Exceptions(match p.exists() {
            true => read_ron(&p)?,
            false => ExceptionsFile::default(),
        }))
    }

    /// The first exception of `rule` whose subject (a `*` glob) matches `subject`.
    pub fn find(&self, rule: &str, subject: &str) -> Option<&super::schema::Exception> {
        (self.0.exceptions.iter()).find(|e| e.rule == rule && glob(&e.subject, subject))
    }
}

/// Whether `s` matches `pat`, where `*` matches any run of characters (`silice check`'s rule).
pub fn glob(pat: &str, s: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == s;
    }
    let mut rest = s;
    for (i, p) in parts.iter().enumerate() {
        if i == 0 {
            let Some(r) = rest.strip_prefix(p) else {
                return false;
            };
            rest = r;
        } else if i == parts.len() - 1 {
            return rest.ends_with(p);
        } else {
            let Some(k) = rest.find(p) else { return false };
            rest = &rest[k + p.len()..];
        }
    }
    true
}
