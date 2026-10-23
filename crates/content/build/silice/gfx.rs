//! The games dir's original graphics (`tilesets/`, `sprites/`), in the extract's formats, laid
//! over the extract's: a tileset or sprite of the same name replaces the extract's, a new name
//! is added. They are made by `cargo xtask silice gfx` from generated art (`gfx/`), never from
//! the ROM.

use crate::input::{World, read_sprite_in, read_tileset_in};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;
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

/// Lays `games`' tilesets and sprites over `w`'s; returns how many of each.
pub fn overlay(games: &Path, w: &mut World) -> Result<(usize, usize)> {
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
    for n in &sprites {
        let s = read_sprite_in(games, n).with_context(|| format!("sprites/{n}"))?;
        ensure!(s.file.name == *n, "sprites/{n}.ron names {:?}", s.file.name);
        match w.sprites.iter_mut().find(|x| x.file.name == *n) {
            Some(x) => *x = s,
            None => w.sprites.push(s),
        }
    }
    Ok((tilesets.len(), sprites.len()))
}
