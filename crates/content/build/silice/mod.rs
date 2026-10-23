//! The Silice flavour's build (SIL1; `PKR_GAME=silice`, `cfg(pkr_silice)`): the extract's base
//! assets plus the authored games dir (`games/silice`), compiled by the same generator as
//! Emerald (`codegen.rs`, `battle.rs`, `blob_data.rs`) without the Emerald game (its maps, script
//! IR and texts are never loaded).
//!
//! - [`schema`]: the games dir file types (shared through `extract::games`);
//! - [`ids`]: the id allocation contract (append-only, tombstones, constants);
//! - [`registry`]: the script registry (`src/silice/scripts/*.rs`, `syn`) and the unknown
//!   script rule;
//! - [`load`], [`merge`]: the maps into the generator's `World`;
//! - [`text`]: the `text/fr.ron` reader (Task A merges the texts).
//!
//! **Wave 0 world** ([`world`]): every map of `maps/` with its layout, numbered by
//! `ids/maps.ron`, on the extract's tilesets, sprites, font, window, battle data and tables; of
//! the extract's texts only Gen3's shared `gText_*` ([`STD_TEXT_PREFIX`]); the Emerald encounters and heal locations dropped (they name Emerald map numbers,
//! which Silice's alias); the new game on [`START`]. Task A adds texts, trainers, encounters,
//! items, header names and `LOCALID_*`; Task E moves the start to `game.ron`.

pub mod ids;
pub mod load;
pub mod merge;
pub mod registry;
pub mod schema;
#[allow(dead_code)]
pub mod text;

use crate::input::{SiliceIn, World};
use anyhow::{Context, Result};
use std::path::Path;

/// The new game: the player's bedroom, in front of the desk (Task E: `game.ron`).
pub const START: (&str, i16, i16) = ("BricoBourg_PlayerHouse_2F", 4, 3);

/// The extract's texts Silice keeps: Gen3's shared `gText_*` (`data/text/*.inc`), which the
/// prelude prints by label. Task A adds `text/fr.ron`'s.
pub const STD_TEXT_PREFIX: &str = "gText_";

/// The Silice world: the base assets of the extracted dir `extracted`, the games dir `games`
/// and the scripts of `scripts` (`src/silice/scripts`). `strict`: an unknown script fails
/// ([`registry::strict`]).
pub fn world(extracted: &Path, games: &Path, scripts: &Path, strict: bool) -> Result<World> {
    let ids = ids::load(games).context("ids")?;
    let reg = registry::scan(scripts).context("scripts")?;
    let mut resolver = registry::Resolver::new(&reg, strict);
    let maps = load::load_maps(games)?
        .iter()
        .map(|m| {
            merge::to_map_file(m, &ids, &mut resolver)
                .with_context(|| format!("map {}", m.map.name))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut w = crate::input::load_assets(extracted)?;
    w.maps = maps;
    w.ir = None;
    // The Gen3 shared texts the prelude prints by decomp label (`text_named`: obtain item,
    // bag full…); the Emerald map texts go.
    w.texts.retain(|k, _| k.starts_with(STD_TEXT_PREFIX));
    w.battle.encounters.clear();
    w.battle.heal = Default::default();
    w.silice = Some(SiliceIn {
        scripts: reg.scripts,
        modules: reg.modules,
        consts: ids.consts(),
        start: (START.0.to_string(), START.1, START.2),
        unwritten: resolver.unwritten,
    });
    Ok(w)
}
