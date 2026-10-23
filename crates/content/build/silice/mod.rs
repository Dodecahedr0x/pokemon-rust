//! The Silice flavour's build (SIL1; `PKR_GAME=silice`, `cfg(pkr_silice)`): the extract's base
//! assets plus the authored games dir (`games/silice`), compiled by the same generator as
//! Emerald (`codegen.rs`, `battle.rs`, `blob_data.rs`) without the Emerald game (its maps, script
//! IR and texts are never loaded).
//!
//! - [`schema`]: the games dir file types (shared through `extract::games`);
//! - [`ids`]: the id allocation contract (append-only, tombstones, constants);
//! - [`registry`]: the script registry (`src/silice/scripts/*.rs`, `syn`) and the unknown
//!   script rule;
//! - [`load`]: the maps and layouts, their Gen3 limits, and `check-exceptions.ron`;
//! - [`merge`]: the maps into the generator's `MapFile`s (names to numbers, headers,
//!   `LOCALID_*`);
//! - [`text`]: `text/fr.ron` and the Silice text encoder (with [`font`]'s glyphs);
//! - [`gfx`]: the original tilesets and sprites (`tilesets/`, `sprites/`) over the extract's;
//! - [`font`]: the authored glyphs (`font/*.ron`) merged into the extract's font (Task G);
//! - [`game`]: `game.ron`: the new game, system ids and heal locations (Task E);
//! - [`data`]: trainers, encounters, items and marts;
//! - [`dispatch`]: the generated script files and the report.
//!
//! **World** ([`world`], SIL1 Task A): every map of `maps/` with its layout, numbered by
//! `ids/maps.ron`, on the extract's tilesets, sprites, font, window, battle data and tables; the
//! texts of `text/fr.ron` with Gen3's shared `gText_*` ([`STD_TEXT_PREFIX`]); Silice's trainers,
//! encounters and items instead of Emerald's; the new game, system ids and heal locations from
//! `game.ron` ([`game`]). The generator sees Silice only through `input::AuthoredIn` (script names,
//! id constants, map numbers, item bases, battle rules, the marts, start); the script files are
//! [`dispatch`]'s. Every placeholder or cut is allowed by `check-exceptions.ron` and listed in
//! `content::REPORT` ([`Silice::report`]).

pub mod data;
pub mod dispatch;
pub mod font;
pub mod game;
pub mod gfx;
pub mod ids;
pub mod load;
pub mod merge;
pub mod registry;
pub mod schema;
pub mod text;

use crate::input::{AuthoredIn, World};
use anyhow::{Context, Result, bail, ensure};
use extract::schema::MapFile;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The extract's texts Silice keeps: Gen3's shared `gText_*` (`data/text/*.inc`), which the
/// prelude prints by label. `text/fr.ron`'s join them.
pub const STD_TEXT_PREFIX: &str = "gText_";

/// The Silice world: the base assets of the extracted dir `extracted`, the games dir `games`
/// and the scripts of `scripts` (`src/silice/scripts`). An unknown script of a claimed map (or
/// of any map when `strict`, [`registry::strict`]) fails once, after every map, listing them
/// all; the other maps' are dropped and reported ([`registry`]).
pub fn world(extracted: &Path, games: &Path, scripts: &Path, strict: bool) -> Result<Silice> {
    let ids = ids::load(games).context("ids")?;
    let exc = load::Exceptions::load(games).context("check-exceptions.ron")?;
    let mut reg = registry::scan(scripts).context("scripts")?;
    let mut resolver = registry::Resolver::new(&reg, strict);
    let loaded = load::load_maps(games)?;
    for m in &loaded {
        load::check_limits(m).with_context(|| format!("map {}", m.map.name))?;
    }
    load::check_warps(&loaded).context("warps")?;
    let game = game::load(games).context("game.ron")?;
    game::check(&game, &ids, &loaded).context("game.ron")?;
    let maps = (loaded.iter())
        .map(|m| {
            merge::to_map_file(m, &ids, &mut resolver)
                .with_context(|| format!("map {}", m.map.name))
        })
        .collect::<Result<Vec<_>>>()?;
    resolver.finish()?;
    let mut w = crate::input::load_assets(extracted)?;
    gfx::overlay(games, &mut w).context("original graphics")?;
    font::merge(&mut w.glyphs, &mut w.widths, &font::load(games)?).context("font")?;
    check_assets(&loaded, &w)?;
    resolver.check_claims()?;
    check_trainer_objects(&maps, &reg, &resolver)?;
    reg.trainer_table = trainer_scripts(&reg, &ids)?;
    w.maps = maps;
    w.ir = None;
    let mut report = Vec::new();
    let mut consts = ids.consts();
    for m in &loaded {
        consts.extend(merge::localid_consts(m));
    }
    consts.extend(game::consts(&game, &ids)?);

    // Texts: the Gen3 shared texts the prelude prints by decomp label (`text_named`: obtain
    // item, bag full…), then Silice's; the Emerald map texts go.
    let enc = text::Encoder::new(w.lang, text::glyph_codes());
    w.texts.retain(|k, _| k.starts_with(STD_TEXT_PREFIX));
    let texts = text::load(games, &enc, &exc, &mut report).context("text/fr.ron")?;
    let text_count = texts.len();
    for (k, v) in texts {
        if w.texts.insert(k.clone(), v).is_some() {
            bail!("text id {k} is also a Gen3 text");
        }
    }

    // Battle and items: Silice's replace Emerald's. The extract's heal locations go: Silice's
    // come from `game.ron` (`heal_location()`, Task E).
    let items = data::items(games, &ids, &exc, &enc, &mut w, &mut report).context("items.ron")?;
    consts.extend(items.consts);
    consts.extend(
        data::trainers(games, &ids, &exc, &enc, &mut w, &mut report)
            .context("battle/trainers.ron")?,
    );
    let trainer_count = w
        .battle
        .trainers
        .iter()
        .filter(|t| !t.party.is_empty())
        .count();
    let encounter_maps = data::encounters(games, &ids, &mut w).context("battle/encounters.ron")?;
    w.battle.heal = Default::default();
    let forms = data::form_changes(&ids)?;

    let map_numbers: BTreeMap<String, u16> = (ids.files.get(&schema::IdKind::Maps).into_iter())
        .flat_map(|f| &f.ids)
        .filter(|e| !e.removed)
        .filter_map(|e| Some((e.map.clone()?, e.num as u16)))
        .collect();
    let mut summary = vec![format!(
        "silice: {} maps, {text_count} texts, {trainer_count} trainers, {encounter_maps} \
         encounter maps, {} items, {} marts",
        w.maps.len(),
        items.defined,
        items.marts
    )];
    summary.push(forms);
    summary.extend(report);
    w.authored = Some(AuthoredIn {
        script_names: reg.scripts.iter().map(|s| s.name.clone()).collect(),
        consts,
        start: (
            game.start.map.clone(),
            game.start.x,
            game.start.y,
            game::facing(game.start.facing),
        ),
        player_name: game.player_name.clone(),
        map_numbers,
        item_bases: items.bases,
        rules: crate::classify::Rules::SILICE,
        data_rs: items.data_rs,
    });
    Ok(Silice {
        world: w,
        registry: reg,
        unwritten: resolver.unwritten.into_keys().collect(),
        report: summary,
    })
}

/// `TRAINER_SCRIPTS` (`Registry::trainer_table`): (`ScriptId`, trainer number,
/// `TRAINER_BATTLE_SINGLE`) of every trainer script of the registry, sorted by `ScriptId`. A
/// trainer constant missing from `ids/trainers.ron` fails.
fn trainer_scripts(reg: &registry::Registry, ids: &ids::Ids) -> Result<Vec<(u16, u16, u8)>> {
    let mut out = Vec::new();
    for (script, trainer) in &reg.trainers {
        let id = reg.id(script).context("a trainer script is a script")?;
        let num = (ids.live(schema::IdKind::Trainers, trainer)?)
            .with_context(|| format!("script {script}: {trainer} is not in ids/trainers.ron"))?;
        out.push((id, u16::try_from(num)?, 0));
    }
    out.sort();
    Ok(out)
}

/// Every trainer object (`trainer_type` other than `TRAINER_TYPE_NONE`) of a strict map runs a
/// trainer script, one whose first statement is the prelude's `trainer(..)`
/// ([`registry::Registry::trainers`]). Otherwise `TRAINER_SCRIPTS` would leave it out and the
/// engine's sight would silently ignore it. Fails once, listing every such object.
pub fn check_trainer_objects(
    maps: &[MapFile],
    reg: &registry::Registry,
    resolver: &registry::Resolver,
) -> Result<()> {
    let trainer_scripts: BTreeSet<&str> = reg.trainers.iter().map(|(s, _)| s.as_str()).collect();
    let mut bad = Vec::new();
    for m in maps.iter().filter(|m| resolver.is_strict(&m.name)) {
        for o in m
            .objects
            .iter()
            .filter(|o| o.trainer_type != "TRAINER_TYPE_NONE")
        {
            match o.script.as_deref() {
                Some(s) if trainer_scripts.contains(s) => {}
                s => bad.push(format!(
                    "{} object {} ({}, {}): {}",
                    m.name,
                    o.local_id,
                    o.x,
                    o.y,
                    s.unwrap_or("no script")
                )),
            }
        }
    }
    ensure!(
        bad.is_empty(),
        "{} trainer objects of strict maps do not run a trainer script (a `pub async fn` whose \
         first statement is the prelude's `trainer(&ctx, LOCALID_…, TRAINER_…, …)`): {}",
        bad.len(),
        bad.join("; ")
    );
    Ok(())
}

/// The extract assets the maps name: each layout's tilesets, each object's sprite.
fn check_assets(maps: &[load::MapIn], w: &World) -> Result<()> {
    let tilesets: BTreeSet<&str> = w.tilesets.iter().map(|t| t.file.name.as_str()).collect();
    let sprites: BTreeSet<&str> = w.sprites.iter().map(|s| s.file.name.as_str()).collect();
    let mut bad = Vec::new();
    for m in maps {
        let n = &m.map.name;
        for ts in [&m.layout.primary, &m.layout.secondary] {
            if !tilesets.contains(ts.as_str()) {
                bad.push(format!("map {n}: tileset {ts} is not in the extract nor the games dir"));
            }
        }
        for o in &m.map.objects {
            // A var graphics (`OBJ_EVENT_GFX_VAR_<n>`) is picked at run time.
            let s = o.gfx.trim_start_matches("OBJ_EVENT_GFX_");
            let fixed = !s.starts_with("VAR_");
            if crate::classify::gfx(&o.gfx).is_none() || fixed && !sprites.contains(s) {
                bad.push(format!(
                    "map {n}: object {}: graphics {} is not in the extract",
                    o.local_id, o.gfx
                ));
            }
        }
    }
    if !bad.is_empty() {
        bail!("{}", bad.join("; "));
    }
    Ok(())
}

/// The Silice build's inputs: the generator's world, plus what [`dispatch`] writes.
pub struct Silice {
    pub world: World,
    pub registry: registry::Registry,
    /// Script names the maps use that no script defines, dropped from the maps (the allowlist
    /// mode of the maps no script file claims, [`registry`]); empty when every map is strict.
    pub unwritten: BTreeSet<String>,
    /// The data part of `content::REPORT`: counts, then every placeholder and cut.
    pub report: Vec<String>,
}
