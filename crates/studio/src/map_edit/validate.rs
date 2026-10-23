//! Checks of an edited map against a [`Catalog`]: errors make the CLI's `validate` fail (and
//! are shown in the UI); warnings (unresolved script names, one-way links, duplicates the game
//! tolerates) do not.

use super::catalog::Catalog;
use super::model::Doc;
use super::schema::{MAP_DYNAMIC, WARP_ID_DYNAMIC, dynamic_warp_id};
use engine::data::block_metatile;
use extract::schema::{BgKind, ConnectionDir};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Gen3 `OBJECT_EVENT_TEMPLATES_COUNT`: objects per map, and local ids stay below it.
pub const MAX_OBJECTS: usize = 64;
pub use super::model::MAX_MAP_DATA_SIZE;
/// Warps, coord events and bg events per map (counts are `u8`).
pub const MAX_EVENTS: usize = 255;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    /// What it is about, e.g. `cell (3, 4)`, `object 2`, `warp 0`.
    pub at: String,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{s}: {}: {}", self.at, self.message)
    }
}

pub fn errors(issues: &[Issue]) -> usize {
    issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count()
}

pub(crate) struct Out(pub Vec<Issue>);

impl Out {
    pub(crate) fn error(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.0.push(Issue {
            severity: Severity::Error,
            at: at.into(),
            message: message.into(),
        });
    }

    pub(crate) fn warn(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.0.push(Issue {
            severity: Severity::Warning,
            at: at.into(),
            message: message.into(),
        });
    }

    fn prefixed(&mut self, at: &str, what: &str, value: &str, prefix: &str) {
        if !value.starts_with(prefix) || value.len() == prefix.len() {
            self.error(at, format!("{what} {value:?} is not a {prefix}* name"));
        }
    }

    fn script(&mut self, c: &Catalog, at: &str, script: Option<&String>) {
        if let Some(s) = script
            && !c.scripts.contains(s)
        {
            self.warn(at, format!("script {s:?} is not in the build (yet)"));
        }
    }

    fn pos(&mut self, d: &Doc, at: &str, x: i16, y: i16) {
        if !d.in_bounds(x as i32, y as i32) {
            self.error(
                at,
                format!("({x}, {y}) is outside the {}x{} map", d.width, d.height),
            );
        }
    }

    fn elevation(&mut self, at: &str, e: u8) {
        if e > 15 {
            self.error(at, format!("elevation {e} is over 15"));
        }
    }

    fn count(&mut self, what: &str, n: usize, max: usize) {
        if n > max {
            self.error("map", format!("{n} {what} (at most {max})"));
        }
    }
}

const TRAINER_TYPES: [&str; 4] = [
    "TRAINER_TYPE_NONE",
    "TRAINER_TYPE_NORMAL",
    "TRAINER_TYPE_SEE_ALL_DIRECTIONS",
    "TRAINER_TYPE_BURIED",
];

fn opposite(d: ConnectionDir) -> ConnectionDir {
    match d {
        ConnectionDir::North => ConnectionDir::South,
        ConnectionDir::South => ConnectionDir::North,
        ConnectionDir::East => ConnectionDir::West,
        ConnectionDir::West => ConnectionDir::East,
        ConnectionDir::Dive => ConnectionDir::Emerge,
        ConnectionDir::Emerge => ConnectionDir::Dive,
    }
}

/// Every problem of `d`, errors first, each group in map order.
pub fn validate(d: &Doc, c: &Catalog) -> Vec<Issue> {
    let mut o = Out(Vec::new());
    let m = &d.map;
    if super::games::check_name(&m.name).is_err() {
        o.error("map", format!("{:?} is not a map name", m.name));
    }
    if super::games::check_name(&m.layout).is_err() {
        o.error("map", format!("layout {:?} is not a layout name", m.layout));
    }
    if d.width == 0 || d.height == 0 || d.blocks.len() != d.width as usize * d.height as usize {
        o.error(
            "layout",
            format!("{} blocks for {}x{}", d.blocks.len(), d.width, d.height),
        );
    }
    let data = (d.width as usize + 15) * (d.height as usize + 14);
    if data > MAX_MAP_DATA_SIZE {
        o.error(
            "layout",
            format!(
                "{}x{} is too big: (w + 15) * (h + 14) = {data} > {MAX_MAP_DATA_SIZE}",
                d.width, d.height
            ),
        );
    }
    tilesets(d, c, &mut o);
    // Header.
    let h = &m.header;
    for (what, v, prefix) in [
        ("music", &h.music, "MUS_"),
        ("map section", &h.region_map_section, "MAPSEC_"),
        ("weather", &h.weather, "WEATHER_"),
        ("map type", &h.map_type, "MAP_TYPE_"),
        ("battle scene", &h.battle_scene, "MAP_BATTLE_SCENE_"),
    ] {
        o.prefixed("header", what, v, prefix);
    }
    objects(d, c, &mut o);
    warps_and_coords(d, c, &mut o);
    o.count("bg events", m.bgs.len(), MAX_EVENTS);
    let mut hidden = BTreeMap::<&str, usize>::new();
    for (i, bg) in m.bgs.iter().enumerate() {
        let at = format!("bg {i}");
        o.pos(d, &at, bg.x, bg.y);
        o.elevation(&at, bg.elevation);
        match &bg.kind {
            BgKind::Script { facing, script } => {
                if *facing > 4 {
                    o.error(&at, format!("facing {facing} (0..=4)"));
                }
                o.script(c, &at, script.as_ref());
            }
            BgKind::HiddenItem { item, flag } => {
                o.prefixed(&at, "item", item, "ITEM_");
                o.prefixed(&at, "flag", flag, "FLAG_");
                if let Some(first) = hidden.insert(flag, i) {
                    o.warn(&at, format!("hidden item flag {flag} is also bg {first}'s"));
                }
            }
            BgKind::SecretBase { .. } => o.error(&at, "secret bases are not supported"),
        }
    }
    connections(d, c, &mut o);
    for (what, s) in [
        ("on_load", &m.scripts.on_load),
        ("on_transition", &m.scripts.on_transition),
        ("on_resume", &m.scripts.on_resume),
        ("on_return_to_field", &m.scripts.on_return_to_field),
        ("on_dive_warp", &m.scripts.on_dive_warp),
    ] {
        o.script(c, &format!("map script {what}"), s.as_ref());
    }
    for v in m.scripts.on_frame.iter().chain(&m.scripts.on_warp_into) {
        o.prefixed("map scripts", "var", &v.var, "VAR_");
        o.script(c, "map scripts", Some(&v.script));
    }
    let mut v = o.0;
    v.sort_by_key(|i| i.severity);
    v
}

fn tilesets(d: &Doc, c: &Catalog, o: &mut Out) {
    let mut ok = true;
    for (name, secondary) in [(&d.primary, false), (&d.secondary, true)] {
        let which = if secondary { "secondary" } else { "primary" };
        match c.tilesets.get(name) {
            None => {
                ok = false;
                o.error("layout", format!("unknown {which} tileset {name:?}"));
            }
            Some(t) if t.secondary != secondary => {
                ok = false;
                o.error("layout", format!("{name:?} is not a {which} tileset"));
            }
            Some(_) => {}
        }
    }
    let Some((p, s)) = c.metatile_ranges(&d.primary, &d.secondary).filter(|_| ok) else {
        return;
    };
    let valid = |b: u16| {
        let mt = block_metatile(b);
        p.contains(&mt) || s.contains(&mt)
    };
    let w = d.width.max(1) as usize;
    for (i, &b) in d.blocks.iter().enumerate() {
        if !valid(b) {
            o.error(
                format!("cell ({}, {})", i % w, i / w),
                format!(
                    "metatile {} is not in {} (0..{}) or {} ({}..{})",
                    block_metatile(b),
                    d.primary,
                    p.end,
                    d.secondary,
                    s.start,
                    s.end
                ),
            );
        }
    }
    for (i, &b) in d.border.iter().enumerate() {
        if !valid(b) {
            o.error(
                format!("border {i}"),
                format!("metatile {} is not in the tilesets", block_metatile(b)),
            );
        }
    }
}

fn objects(d: &Doc, c: &Catalog, o: &mut Out) {
    let m = &d.map;
    o.count("objects", m.objects.len(), MAX_OBJECTS);
    let mut ids = BTreeMap::<u8, usize>::new();
    let mut names = BTreeMap::<&str, usize>::new();
    let mut flags = BTreeMap::<&str, usize>::new();
    for (i, ob) in m.objects.iter().enumerate() {
        let at = format!("object {i}");
        o.pos(d, &at, ob.x, ob.y);
        o.elevation(&at, ob.elevation);
        if ob.local_id == 0 || ob.local_id as usize >= MAX_OBJECTS {
            o.error(&at, format!("local id {} (1..{MAX_OBJECTS})", ob.local_id));
        }
        if let Some(first) = ids.insert(ob.local_id, i) {
            o.error(
                &at,
                format!("local id {} is also object {first}'s", ob.local_id),
            );
        }
        if let Some(id) = &ob.id {
            o.prefixed(&at, "id", id, "LOCALID_");
            if let Some(first) = names.insert(id, i) {
                o.error(&at, format!("id {id} is also object {first}'s"));
            }
        }
        o.prefixed(&at, "graphics", &ob.gfx, "OBJ_EVENT_GFX_");
        if !c.gfx.is_empty() && ob.gfx.starts_with("OBJ_EVENT_GFX_") && !c.gfx.contains(&ob.gfx) {
            o.error(&at, format!("graphics {:?} are not extracted", ob.gfx));
        }
        o.prefixed(&at, "movement", &ob.movement, "MOVEMENT_TYPE_");
        if !TRAINER_TYPES.contains(&ob.trainer_type.as_str()) {
            o.error(&at, format!("trainer type {:?}", ob.trainer_type));
        }
        if let Some(f) = &ob.flag {
            o.prefixed(&at, "flag", f, "FLAG_");
            if let Some(first) = flags.insert(f, i) {
                o.warn(&at, format!("hide flag {f} is also object {first}'s"));
            }
        }
        o.script(c, &at, ob.script.as_ref());
    }
}

fn warps_and_coords(d: &Doc, c: &Catalog, o: &mut Out) {
    let m = &d.map;
    o.count("warps", m.warps.len(), MAX_EVENTS);
    o.count("coord events", m.coords.len(), MAX_EVENTS);
    let mut cells = BTreeMap::<(i16, i16), String>::new();
    let mut on_cell = |o: &mut Out, at: &str, x: i16, y: i16| {
        if let Some(first) = cells.insert((x, y), at.to_string()) {
            o.warn(at, format!("on the same cell ({x}, {y}) as {first}"));
        }
    };
    for (i, w) in m.warps.iter().enumerate() {
        let at = format!("warp {i}");
        o.pos(d, &at, w.x, w.y);
        o.elevation(&at, w.elevation);
        on_cell(o, &at, w.x, w.y);
        if w.dest_map == MAP_DYNAMIC {
            if !dynamic_warp_id(w.dest_warp) {
                o.error(
                    &at,
                    format!("a {MAP_DYNAMIC} warp has dest_warp {WARP_ID_DYNAMIC}"),
                );
            }
            continue;
        }
        let (n, back) = if w.dest_map == m.name {
            (m.warps.len(), true)
        } else {
            match c.maps.get(&w.dest_map) {
                None => {
                    o.error(&at, format!("unknown destination map {:?}", w.dest_map));
                    continue;
                }
                Some(r) => (r.warps.len(), r.warps.contains(&m.name)),
            }
        };
        if w.dest_warp as usize >= n {
            o.error(
                &at,
                format!("{} has {n} warps, not warp {}", w.dest_map, w.dest_warp),
            );
        }
        if !back {
            o.warn(&at, format!("{} has no warp back here", w.dest_map));
        }
    }
    for (i, co) in m.coords.iter().enumerate() {
        let at = format!("coord {i}");
        o.pos(d, &at, co.x, co.y);
        o.elevation(&at, co.elevation);
        on_cell(o, &at, co.x, co.y);
        if let Some(v) = &co.var {
            o.prefixed(&at, "var", v, "VAR_");
        }
        o.script(c, &at, co.script.as_ref());
    }
}

fn connections(d: &Doc, c: &Catalog, o: &mut Out) {
    let m = &d.map;
    let mut dirs = BTreeSet::new();
    for (i, cn) in m.connections.iter().enumerate() {
        let at = format!("connection {i}");
        if !dirs.insert(format!("{:?}", cn.dir)) {
            o.warn(&at, format!("a second {:?} connection", cn.dir));
        }
        if cn.map == m.name {
            o.error(&at, "connects the map to itself");
            continue;
        }
        let Some(r) = c.maps.get(&cn.map) else {
            o.error(&at, format!("unknown map {:?}", cn.map));
            continue;
        };
        let back = r.connections.iter().any(|b| {
            b.map == m.name
                && b.dir == opposite(cn.dir)
                && (matches!(cn.dir, ConnectionDir::Dive | ConnectionDir::Emerge)
                    || b.offset == -cn.offset)
        });
        if !back {
            o.warn(
                &at,
                format!(
                    "{} has no {:?} connection back here at offset {}",
                    cn.map,
                    opposite(cn.dir),
                    -cn.offset
                ),
            );
        }
        // The neighbor must overlap the edge it is on.
        let (len, n_len) = match (cn.dir, r.size) {
            (ConnectionDir::North | ConnectionDir::South, Some((w, _))) => (d.width, w),
            (ConnectionDir::East | ConnectionDir::West, Some((_, h))) => (d.height, h),
            _ => continue,
        };
        if cn.offset >= len as i32 || cn.offset + n_len as i32 <= 0 {
            o.warn(
                &at,
                format!(
                    "{} at offset {} is outside the map's {len}-cell edge",
                    cn.map, cn.offset
                ),
            );
        }
    }
}

/// Warnings for named objects whose local id differs between `before` (the saved map) and
/// `after`: scripts and checkpoints use the number.
pub fn id_changes(before: &Doc, after: &Doc) -> Vec<Issue> {
    let mut o = Out(Vec::new());
    let old: BTreeMap<&str, u8> = (before.map.objects.iter())
        .filter_map(|ob| Some((ob.id.as_deref()?, ob.local_id)))
        .collect();
    for (i, ob) in after.map.objects.iter().enumerate() {
        if let Some(id) = &ob.id
            && let Some(&was) = old.get(id.as_str())
            && was != ob.local_id
        {
            o.warn(
                format!("object {i}"),
                format!("{id} was local id {was}, now {}", ob.local_id),
            );
        }
    }
    o.0
}
