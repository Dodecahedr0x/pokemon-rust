//! The games dir's original graphics (`tilesets/`, `sprites/`), in the extract's formats, laid
//! over the extract's: a tileset or sprite of the same name replaces the extract's, a new name
//! is added. They are made by `cargo xtask silice gfx` from generated art (`gfx/`), never from
//! the ROM.
//!
//! A sprite whose name is not an Emerald graphics id (`SIL_ADRIEN`) is a new one: maps name it
//! `OBJ_EVENT_GFX_SIL_ADRIEN`, and its `gfx_id` (0x100 and up, past Gen3's u8 ids) is the id the
//! build gives those objects ([`Overlay::new_gfx`], [`rename_gfx`]).

use crate::input::{World, read_sprite_in, read_tileset_in};
use anyhow::{Context, Result, ensure};
use extract::names;
use extract::schema::MapFile;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The `<name>.ron` stems of `dir/sub` (none when it does not exist).
fn names(dir: &Path, sub: &str) -> Result<BTreeSet<String>> {
    let d = dir.join(sub);
    let mut out = BTreeSet::new();
    if !d.is_dir() {
        return Ok(out);
    }
    for e in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "ron")
            && let Some(s) = p.file_stem().and_then(|s| s.to_str())
        {
            out.insert(s.to_string());
        }
    }
    Ok(out)
}

/// What [`overlay`] laid.
pub struct Overlay {
    /// The new graphics: `OBJ_EVENT_GFX_<sprite>` to its id.
    pub new_gfx: BTreeMap<String, u16>,
}

/// The first id of the new graphics (Gen3's are u8).
pub const FIRST_NEW_GFX: u16 = 0x100;

/// Lays `games`' tilesets and sprites over `w`'s.
pub fn overlay(games: &Path, w: &mut World) -> Result<Overlay> {
    let tilesets = names(games, "tilesets")?;
    for n in &tilesets {
        let t = read_tileset_in(games, n).with_context(|| format!("tilesets/{n}"))?;
        ensure!(
            t.file.name == *n,
            "tilesets/{n}.ron names {:?}",
            t.file.name
        );
        match w.tilesets.iter_mut().find(|x| x.file.name == *n) {
            Some(x) => *x = t,
            None => w.tilesets.push(t),
        }
    }
    let sprites = names(games, "sprites")?;
    let mut new_gfx = BTreeMap::new();
    for n in &sprites {
        let s = read_sprite_in(games, n).with_context(|| format!("sprites/{n}"))?;
        ensure!(s.file.name == *n, "sprites/{n}.ron names {:?}", s.file.name);
        let gfx = format!("OBJ_EVENT_GFX_{n}");
        match names::OBJ_EVENT_GFX.value(&gfx) {
            Some(id) => ensure!(
                id == u32::from(s.file.gfx_id),
                "sprites/{n}: gfx_id {:#x}, Emerald's {gfx} is {id:#x}",
                s.file.gfx_id
            ),
            None => {
                ensure!(
                    s.file.gfx_id >= FIRST_NEW_GFX,
                    "sprites/{n}: a new graphics id is {FIRST_NEW_GFX:#x} or more"
                );
                ensure!(
                    !new_gfx.values().any(|&v| v == s.file.gfx_id),
                    "sprites/{n}: gfx_id {:#x} is taken",
                    s.file.gfx_id
                );
                new_gfx.insert(gfx, s.file.gfx_id);
            }
        }
        match w.sprites.iter_mut().find(|x| x.file.name == *n) {
            Some(x) => *x = s,
            None => w.sprites.push(s),
        }
    }
    Ok(Overlay { new_gfx })
}

/// Gives the objects of `maps` that use a new graphics its numeric name
/// (`OBJ_EVENT_GFX_0x100`), which the generator resolves.
pub fn rename_gfx(maps: &mut [MapFile], new_gfx: &BTreeMap<String, u16>) {
    for o in maps.iter_mut().flat_map(|m| m.objects.iter_mut()) {
        if let Some(id) = new_gfx.get(&o.gfx) {
            o.gfx = format!("OBJ_EVENT_GFX_0x{id:X}");
        }
    }
}
