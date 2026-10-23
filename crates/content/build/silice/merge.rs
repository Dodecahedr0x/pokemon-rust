//! Turns the games dir's maps into the generator's `MapFile`s and merges them into the
//! extract's base assets (SIL1 Wave 0; Task A owns the rest of the merge).
//!
//! **Names.** A map number is its `ids/maps.ron` entry (by map file name), split into Gen3
//! group (high byte) and num (low byte). A Silice flag, var or item is written as its number
//! (`FLAG_0x113`, which `extract::names` and the generator read as a number); a Gen3 name the
//! decomp tables know stays as it is; anything else, or a tombstone, fails the build. Scripts go
//! through the registry's [`Resolver`].
//!
//! **Wave 0 limits** (Task A): the header's names are not resolved yet (its numbers are 0; the
//! engine reads `allow_running` only), `LOCALID_*` constants are not emitted, and the editor's
//! map limits are not checked again.

use super::ids::Ids;
use super::load::MapIn;
use super::registry::Resolver;
use super::schema::{IdKind, MAP_DYNAMIC};
use anyhow::{Context, Result, bail};
use extract::names::{self, Table};
use extract::schema::{
    Bg, BgKind, Connection, Coord, Header, MapFile, MapScripts, Object, VarScript, Warp,
};

/// A constant of `kind` from a data file: the Silice number as `<PREFIX>0x<HEX>`, or the Gen3
/// name.
fn constant(ids: &Ids, kind: IdKind, table: &Table, name: &str) -> Result<String> {
    if let Some(num) = ids.live(kind, name)? {
        return Ok(format!("{}0x{num:X}", table.prefix));
    }
    match table.value(name) {
        Some(_) => Ok(name.to_string()),
        None => bail!("unknown {}* name {name}", table.prefix),
    }
}

/// The Gen3 `"<group>.<num>"` of the games dir map named `name` (its file name).
fn map_id(ids: &Ids, name: &str) -> Result<String> {
    let e = ids
        .map_file(name)
        .with_context(|| format!("map {name} is not in ids/maps.ron"))?;
    if e.removed {
        bail!("map {name} is removed (a tombstone in ids/maps.ron)");
    }
    Ok(format!("{}.{}", e.num >> 8, e.num & 0xFF))
}

/// One games dir map as a `MapFile`.
pub fn to_map_file(m: &MapIn, ids: &Ids, scripts: &mut Resolver) -> Result<MapFile> {
    let g = &m.map;
    let n = g.name.as_str();
    let id = map_id(ids, n)?;
    let (group, num) = id.split_once('.').unwrap();
    let flag = |f: &str| constant(ids, IdKind::Flags, &names::FLAGS, f);
    let var = |v: &str| constant(ids, IdKind::Vars, &names::VARS, v);
    let mut objects = Vec::new();
    for o in &g.objects {
        objects.push(Object {
            local_id: o.local_id,
            gfx: o.gfx.clone(),
            x: o.x,
            y: o.y,
            elevation: o.elevation,
            movement: o.movement.clone(),
            range_x: o.range_x,
            range_y: o.range_y,
            trainer_type: o.trainer_type.clone(),
            trainer_sight_or_berry_tree: o.trainer_sight_or_berry_tree,
            script: scripts.opt(n, &o.script)?,
            flag: o.flag.as_deref().map(flag).transpose()?,
        });
    }
    let mut warps = Vec::new();
    for w in &g.warps {
        let (dest_map, dest_map_id) = match w.dest_map.as_str() {
            MAP_DYNAMIC => (None, "127.127".to_string()),
            d => (Some(d.to_string()), map_id(ids, d)?),
        };
        warps.push(Warp {
            x: w.x,
            y: w.y,
            elevation: w.elevation,
            dest_map,
            dest_map_id,
            dest_warp: w.dest_warp,
        });
    }
    let mut coords = Vec::new();
    for c in &g.coords {
        coords.push(Coord {
            var: c.var.as_deref().map(var).transpose()?,
            script: scripts.opt(n, &c.script)?,
            ..c.clone()
        });
    }
    let mut bgs = Vec::new();
    for b in &g.bgs {
        let kind = match &b.kind {
            BgKind::Script { facing, script } => BgKind::Script {
                facing: *facing,
                script: scripts.opt(n, script)?,
            },
            BgKind::HiddenItem { item, flag: f } => BgKind::HiddenItem {
                item: constant(ids, IdKind::Items, &names::ITEMS, item)?,
                flag: flag(f)?,
            },
            k => k.clone(),
        };
        bgs.push(Bg { kind, ..b.clone() });
    }
    let mut connections = Vec::new();
    for c in &g.connections {
        connections.push(Connection {
            dir: c.dir,
            offset: c.offset,
            map: Some(c.map.clone()),
            map_id: map_id(ids, &c.map)?,
        });
    }
    let s = &g.scripts;
    let mut var_scripts = |v: &[VarScript]| -> Result<Vec<VarScript>> {
        let mut out = Vec::new();
        for t in v {
            if let Some(script) = scripts.script(n, &t.script)? {
                out.push(VarScript {
                    var: var(&t.var)?,
                    value: t.value,
                    script,
                });
            }
        }
        Ok(out)
    };
    let on_frame = var_scripts(&s.on_frame)?;
    let on_warp_into = var_scripts(&s.on_warp_into)?;
    let map_scripts = MapScripts {
        on_load: scripts.opt(n, &s.on_load)?,
        on_transition: scripts.opt(n, &s.on_transition)?,
        on_resume: scripts.opt(n, &s.on_resume)?,
        on_return_to_field: scripts.opt(n, &s.on_return_to_field)?,
        on_dive_warp: scripts.opt(n, &s.on_dive_warp)?,
        on_frame,
        on_warp_into,
    };
    let h = &g.header;
    Ok(MapFile {
        name: g.name.clone(),
        group: group.parse()?,
        num: num.parse()?,
        layout: m.layout.clone(),
        header: Header {
            music: 0,
            region_map_section: 0,
            cave: h.cave,
            weather: 0,
            map_type: 0,
            allow_cycling: h.allow_cycling,
            allow_escaping: h.allow_escaping,
            allow_running: h.allow_running,
            show_map_name: h.show_map_name,
            battle_type: 0,
        },
        objects,
        warps,
        coords,
        bgs,
        connections,
        scripts: map_scripts,
    })
}
