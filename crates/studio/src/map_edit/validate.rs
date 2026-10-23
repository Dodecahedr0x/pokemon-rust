//! Checks of an edited map against a [`Catalog`]: errors block a save from the CLI's `validate`
//! (and are shown in the UI); warnings (unresolved script names, unknown graphics on a build
//! without a sprite list) do not.

use super::catalog::Catalog;
use super::model::Doc;
use engine::data::block_metatile;
use extract::schema::BgKind;
use std::fmt;

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

struct Out(Vec<Issue>);

impl Out {
    fn error(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.0.push(Issue {
            severity: Severity::Error,
            at: at.into(),
            message: message.into(),
        });
    }

    fn warn(&mut self, at: impl Into<String>, message: impl Into<String>) {
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
}

const TRAINER_TYPES: [&str; 4] = [
    "TRAINER_TYPE_NONE",
    "TRAINER_TYPE_NORMAL",
    "TRAINER_TYPE_SEE_ALL_DIRECTIONS",
    "TRAINER_TYPE_BURIED",
];

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
    // Tilesets and metatiles.
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
    if ok && let Some((p, s)) = c.metatile_ranges(&d.primary, &d.secondary) {
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
    // Events.
    for (i, ob) in m.objects.iter().enumerate() {
        let at = format!("object {i}");
        o.pos(d, &at, ob.x, ob.y);
        if ob.local_id as usize != i + 1 {
            o.error(&at, format!("local id {} (want {})", ob.local_id, i + 1));
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
        }
        o.script(c, &at, ob.script.as_ref());
    }
    for (i, w) in m.warps.iter().enumerate() {
        let at = format!("warp {i}");
        o.pos(d, &at, w.x, w.y);
        let dest = if w.dest_map == m.name {
            Some(m.warps.len())
        } else {
            c.maps.get(&w.dest_map).map(|r| r.warps)
        };
        match dest {
            None => o.error(&at, format!("unknown destination map {:?}", w.dest_map)),
            Some(n) if w.dest_warp as usize >= n => o.error(
                &at,
                format!("{} has {n} warps, not warp {}", w.dest_map, w.dest_warp),
            ),
            Some(_) => {}
        }
    }
    for (i, co) in m.coords.iter().enumerate() {
        let at = format!("coord {i}");
        o.pos(d, &at, co.x, co.y);
        if let Some(v) = &co.var {
            o.prefixed(&at, "var", v, "VAR_");
        }
        o.script(c, &at, co.script.as_ref());
    }
    for (i, bg) in m.bgs.iter().enumerate() {
        let at = format!("bg {i}");
        o.pos(d, &at, bg.x, bg.y);
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
            }
            BgKind::SecretBase { .. } => o.error(&at, "secret bases are not supported"),
        }
    }
    for (i, cn) in m.connections.iter().enumerate() {
        let at = format!("connection {i}");
        if cn.map == m.name {
            o.error(&at, "connects the map to itself");
        } else if !c.maps.contains_key(&cn.map) {
            o.error(&at, format!("unknown map {:?}", cn.map));
        }
    }
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
