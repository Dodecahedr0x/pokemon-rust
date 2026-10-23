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
//! - [`dispatch`]: the generated script files;
//! - [`text`]: the `text/fr.ron` reader (Task A merges the texts);
//! - [`font`]: the authored glyphs (`font/*.ron`) merged into the extract's font, and the
//!   encoder hook for them (Task G).
//!
//! **Wave 0 world** ([`world`]): every map of `maps/` with its layout, numbered by
//! `ids/maps.ron`, on the extract's tilesets, sprites, font, window, battle data and tables; of
//! the extract's texts only Gen3's shared `gText_*` ([`STD_TEXT_PREFIX`]); the Emerald
//! trainers (but `TRAINER_NONE`), encounters and heal locations dropped (they name Emerald
//! texts and map numbers, which Silice's alias); the new game on [`START`]. Task A adds texts,
//! trainers, encounters, items, header names and `LOCALID_*`; Task E moves the start to
//! `game.ron`. The generator sees Silice only through `input::AuthoredIn` (script names, id
//! constants, start); the script files are [`dispatch`]'s.

pub mod dispatch;
pub mod font;
pub mod ids;
pub mod load;
pub mod merge;
pub mod registry;
pub mod schema;
#[allow(dead_code)]
pub mod text;

use crate::input::{AuthoredIn, World};
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::Path;

/// The new game: the player's bedroom, in front of the desk (Task E: `game.ron`).
pub const START: (&str, i16, i16) = ("BricoBourg_PlayerHouse_2F", 4, 3);

/// The extract's texts Silice keeps: Gen3's shared `gText_*` (`data/text/*.inc`), which the
/// prelude prints by label. Task A adds `text/fr.ron`'s.
pub const STD_TEXT_PREFIX: &str = "gText_";

/// The Silice world: the base assets of the extracted dir `extracted`, the games dir `games`
/// and the scripts of `scripts` (`src/silice/scripts`). `strict`: an unknown script fails
/// once, after every map, listing them all ([`registry::strict`]).
pub fn world(extracted: &Path, games: &Path, scripts: &Path, strict: bool) -> Result<Silice> {
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
    resolver.finish()?;
    let mut w = crate::input::load_assets(extracted)?;
    font::merge(&mut w.glyphs, &mut w.widths, &font::load(games)?).context("font")?;
    w.maps = maps;
    w.ir = None;
    // The Gen3 shared texts the prelude prints by decomp label (`text_named`: obtain item,
    // bag full…); the Emerald map texts go.
    w.texts.retain(|k, _| k.starts_with(STD_TEXT_PREFIX));
    // Emerald's trainers, encounters and heal locations go (Task A merges Silice's). The
    // trainer list keeps `TRAINER_NONE`, number 0, so trainer numbers stay index-equal.
    w.battle.trainers.truncate(1);
    w.battle.encounters.clear();
    w.battle.heal = Default::default();
    w.authored = Some(AuthoredIn {
        script_names: reg.scripts.iter().map(|s| s.name.clone()).collect(),
        consts: ids.consts(),
        start: (START.0.to_string(), START.1, START.2),
    });
    Ok(Silice {
        world: w,
        registry: reg,
        unwritten: resolver.unwritten.into_keys().collect(),
    })
}

/// The Silice build's inputs: the generator's world, plus what [`dispatch`] writes.
pub struct Silice {
    pub world: World,
    pub registry: registry::Registry,
    /// Script names the maps use that no script defines, dropped from the maps (the Wave 0
    /// allowlist mode); empty when the build is strict.
    pub unwritten: BTreeSet<String>,
}
