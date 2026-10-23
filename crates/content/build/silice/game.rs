//! `games/silice/game.ron` (SIL1 Task E; the format is `extract::games::GameFile`): the new
//! game, the system ids and the heal locations, checked against the ids and the maps, and
//! emitted as `content::consts` that the Silice root (`src/silice/mod.rs`) and its new-game
//! script read.
//!
//! **Constants** (each references the generated constant it names, so a renamed id also fails
//! to compile):
//! - `HEAL_LOCATION_SIL_*: u8`, numbered from 1 in file order (append-only);
//! - `SIL_HEAL_LOCATIONS: &[(MapId, i16, i16)]`, by number minus one; `SIL_RESPAWN: u8`;
//! - `SIL_START_FACING: Dir`, `SIL_RUNNING_FLAG: FlagId`,
//!   `SIL_FLAGS_CLEARED_ON_MAP_CHANGE: &[FlagId]`;
//! - `SIL_STARTERS: [SpeciesId; 3]`, `SIL_STARTER_LEVEL: u8`;
//! - `SIL_NEW_GAME_FLAGS: &[FlagId]`, `SIL_NEW_GAME_VARS: &[(VarId, u16)]`.
//!
//! The start cell itself goes through `input::AuthoredIn::start` (`data::NEW_GAME`), as in Wave 0.
//!
//! **Checks** ([`check`]): every map is a loaded map and every cell a walkable cell inside it;
//! a flag or var is a live Silice id, or a Gen3 one in `extract::games::SHARED_GEN3` (the
//! numbers no Silice id may take); species are Gen3 names; heal location names are
//! `HEAL_LOCATION_SIL_*` and unique; the respawn names one of them; the facing is `Down`
//! (the shared `GameData::new_game`, `content::game_data_tables!`, always faces down; reading
//! another facing would change a file the Emerald build hashes).

use super::ids::Ids;
use super::load::MapIn;
use super::schema::{Facing, GameFile, IdKind, SHARED_GEN3};
use anyhow::{Context, Result, bail, ensure};
use extract::names;
use std::collections::BTreeSet;
use std::path::Path;

/// `<games>/game.ron`.
pub fn load(games: &Path) -> Result<GameFile> {
    extract::export::read_ron(&games.join("game.ron"))
}

/// A flag or var `game.ron` may name: a live Silice id, or a reserved Gen3 one.
fn flag_or_var(ids: &Ids, kind: IdKind, name: &str) -> Result<()> {
    if ids.live(kind, name)?.is_some() {
        return Ok(());
    }
    let table = match kind {
        IdKind::Flags => &names::FLAGS,
        _ => &names::VARS,
    };
    ensure!(
        table.value(name).is_some(),
        "{name} is neither a Silice id (ids/{}.ron) nor a Gen3 name",
        kind.file()
    );
    ensure!(
        SHARED_GEN3.contains(&name),
        "{name} is a Gen3 name the shared code does not reserve (extract::games::SHARED_GEN3), \
         so a Silice id may take its number"
    );
    Ok(())
}

/// A walkable cell of a loaded map.
fn cell(maps: &[MapIn], what: &str, map: &str, x: i16, y: i16) -> Result<()> {
    let m = (maps.iter().find(|m| m.map.name == map))
        .with_context(|| format!("{what}: map {map} is not in maps/"))?;
    let l = &m.layout;
    ensure!(
        x >= 0 && y >= 0 && (x as u16) < l.width && (y as u16) < l.height,
        "{what}: ({x}, {y}) is outside {map} ({} x {})",
        l.width,
        l.height
    );
    let block = l.blocks[y as usize].0[x as usize];
    ensure!(
        (block >> 10) & 3 == 0,
        "{what}: ({x}, {y}) of {map} is not walkable (collision {})",
        (block >> 10) & 3
    );
    Ok(())
}

/// Checks `g` against the ids and the loaded maps (see the module doc).
pub fn check(g: &GameFile, ids: &Ids, maps: &[MapIn]) -> Result<()> {
    let s = &g.start;
    cell(maps, "start", &s.map, s.x, s.y)?;
    ensure!(
        s.facing == Facing::Down,
        "start: facing {:?}; the shared GameData::new_game faces down",
        s.facing
    );
    flag_or_var(ids, IdKind::Flags, &g.running_flag).context("running_flag")?;
    for f in &g.cleared_on_map_change {
        flag_or_var(ids, IdKind::Flags, f).context("cleared_on_map_change")?;
    }
    for f in &g.new_game_flags {
        flag_or_var(ids, IdKind::Flags, f).context("new_game_flags")?;
    }
    for (v, _) in &g.new_game_vars {
        flag_or_var(ids, IdKind::Vars, v).context("new_game_vars")?;
    }
    let mut seen = BTreeSet::new();
    ensure!(!g.heal_locations.is_empty(), "no heal location");
    ensure!(g.heal_locations.len() < 256, "more than 255 heal locations");
    for h in &g.heal_locations {
        ensure!(
            h.name.starts_with("HEAL_LOCATION_SIL_"),
            "heal location {}: a Silice name takes the HEAL_LOCATION_SIL_ prefix",
            h.name
        );
        ensure!(
            seen.insert(&h.name),
            "heal location {} is listed twice",
            h.name
        );
        cell(maps, &h.name, &h.map, h.x, h.y)?;
    }
    if g.heal_location(&g.respawn).is_none() {
        bail!("respawn {} is not a heal location", g.respawn);
    }
    ensure!(
        g.starters.len() == 3,
        "{} starters; choose_starter offers three",
        g.starters.len()
    );
    for sp in &g.starters {
        ensure!(
            names::SPECIES.value(sp).is_some(),
            "starter {sp} is not a Gen3 species"
        );
    }
    ensure!(
        (1..=100).contains(&g.starter_level),
        "starter_level {} is outside 1..=100",
        g.starter_level
    );
    Ok(())
}

/// The `content::consts` of `g` (see the module doc): (name, type, value).
pub fn consts(g: &GameFile, ids: &Ids) -> Result<Vec<(String, &'static str, String)>> {
    let list = |v: &[String]| format!("&[{}]", v.join(", "));
    let map = |file: &str| -> Result<String> {
        let e = (ids.map_file(file).filter(|e| !e.removed))
            .with_context(|| format!("map {file} is not live in ids/maps.ron"))?;
        Ok(e.name.clone())
    };
    let mut out = Vec::new();
    let mut heal = Vec::new();
    for (i, h) in g.heal_locations.iter().enumerate() {
        out.push((h.name.clone(), "u8", format!("{}", i + 1)));
        heal.push(format!("({}, {}, {})", map(&h.map)?, h.x, h.y));
    }
    let vars: Vec<String> = (g.new_game_vars.iter())
        .map(|(v, x)| format!("({v}, {x})"))
        .collect();
    let facing = match g.start.facing {
        Facing::Down => "Dir::Down",
        Facing::Up => "Dir::Up",
        Facing::Left => "Dir::Left",
        Facing::Right => "Dir::Right",
    };
    out.extend([
        (
            "SIL_HEAL_LOCATIONS".into(),
            "&[(MapId, i16, i16)]",
            list(&heal),
        ),
        ("SIL_RESPAWN".into(), "u8", g.respawn.clone()),
        ("SIL_START_FACING".into(), "Dir", facing.into()),
        ("SIL_RUNNING_FLAG".into(), "FlagId", g.running_flag.clone()),
        (
            "SIL_FLAGS_CLEARED_ON_MAP_CHANGE".into(),
            "&[FlagId]",
            list(&g.cleared_on_map_change),
        ),
        (
            "SIL_STARTERS".into(),
            "[SpeciesId; 3]",
            format!("[{}]", g.starters.join(", ")),
        ),
        (
            "SIL_STARTER_LEVEL".into(),
            "u8",
            g.starter_level.to_string(),
        ),
        (
            "SIL_NEW_GAME_FLAGS".into(),
            "&[FlagId]",
            list(&g.new_game_flags),
        ),
        ("SIL_NEW_GAME_VARS".into(), "&[(VarId, u16)]", list(&vars)),
    ]);
    Ok(out)
}
