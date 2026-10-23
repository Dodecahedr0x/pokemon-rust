//! The headless editor for agents: `cargo xtask studio map …` (the `studio-map` bin).
//!
//! ```text
//! studio-map [--games DIR | --game NAME] <command>
//!   list                                   the games dir's maps
//!   create <spec.ron>                      a new map from a NewMap spec (never replaces one)
//!   apply <Map> <script.ron> [--dry-run]   applies an edit script (a RON list of Op) and saves
//!   validate [<Map>...]                    every map by default; fails on errors
//!   render <Map> [--out FILE] [--margin N] a PNG (default target/studio/maps/<Map>.png)
//!   palette <Primary> <Secondary> [--out FILE]
//!                                          the metatile palette (default
//!                                          target/studio/maps/palette-<P>-<S>.png), 8 per row
//! ```
//!
//! The games dir defaults to `games/silice` (`--game` picks `games/<NAME>`). Pictures go to the
//! git-ignored scratch dir unless `--out` says otherwise; they are never committed.

use super::catalog::Catalog;
use super::games::{DEFAULT_GAME, GamesDir};
use super::model::{Doc, Editor, NewMap, Op};
use super::render::{palette, render, write_png};
use super::schema::from_script_ron;
use super::validate::{Issue, errors, validate};
use crate::paths::StudioPaths;
use anyhow::{Context, Result, bail};
use std::fmt::Write as _;
use std::path::PathBuf;

pub const USAGE: &str = "usage: studio-map [--games DIR | --game NAME] \
     (list | create <spec.ron> | apply <Map> <script.ron> [--dry-run] | validate [<Map>...] | \
     render <Map> [--out FILE] [--margin N] | palette <Primary> <Secondary> [--out FILE])";

/// Margin cells of a CLI render.
pub const RENDER_MARGIN: u16 = 2;

/// Runs one command; returns what to print. `Err` for a failed command (including a
/// `validate` that found errors).
pub fn run(args: &[String]) -> Result<String> {
    let paths = StudioPaths::detect();
    let mut games = GamesDir::for_game(&paths.workspace, DEFAULT_GAME);
    let mut rest = args;
    loop {
        match rest {
            [flag, dir, tail @ ..] if flag == "--games" => {
                games = GamesDir::new(dir);
                rest = tail;
            }
            [flag, name, tail @ ..] if flag == "--game" => {
                games = GamesDir::for_game(&paths.workspace, name);
                rest = tail;
            }
            _ => break,
        }
    }
    let scratch = paths.scratch.join("maps");
    let mut out = String::new();
    match rest {
        [cmd] if cmd == "list" => {
            for name in games.map_names()? {
                writeln!(out, "{name}")?;
            }
        }
        [cmd, spec] if cmd == "create" => {
            let text = std::fs::read_to_string(spec).with_context(|| format!("reading {spec}"))?;
            let spec: NewMap = from_script_ron(&text).with_context(|| format!("parsing {spec}"))?;
            let doc = Doc::new(&spec)?;
            let catalog = Catalog::for_games(&games)?;
            let issues = validate(&doc, &catalog);
            if errors(&issues) > 0 {
                report(&mut out, &doc.map.name, &issues);
                bail!("{out}{}: not created", doc.map.name);
            }
            games.create(&doc)?;
            writeln!(out, "created {}", games.map_path(&doc.map.name).display())?;
            report(&mut out, &doc.map.name, &issues);
        }
        [cmd, map, script, flags @ ..] if cmd == "apply" => {
            let dry = match flags {
                [] => false,
                [f] if f == "--dry-run" => true,
                _ => bail!(USAGE),
            };
            let text =
                std::fs::read_to_string(script).with_context(|| format!("reading {script}"))?;
            let ops: Vec<Op> =
                from_script_ron(&text).with_context(|| format!("parsing {script}"))?;
            let mut ed = Editor::new(games.load(map)?);
            ed.apply_all(&ops)?;
            let doc = ed.doc();
            let issues = validate(doc, &Catalog::for_games(&games)?);
            if dry {
                writeln!(out, "{map}: {} ops applied (dry run, not saved)", ops.len())?;
            } else {
                games.save(doc)?;
                writeln!(out, "{map}: {} ops applied, saved", ops.len())?;
            }
            report(&mut out, map, &issues);
        }
        [cmd, maps @ ..] if cmd == "validate" => {
            let names = if maps.is_empty() {
                games.map_names()?
            } else {
                maps.to_vec()
            };
            let catalog = Catalog::for_games(&games)?;
            let mut total = 0;
            for name in &names {
                let issues = validate(&games.load(name)?, &catalog);
                total += errors(&issues);
                report(&mut out, name, &issues);
            }
            if total > 0 {
                bail!("{out}{total} errors in {} maps", names.len());
            }
            writeln!(out, "{} maps, no errors", names.len())?;
        }
        [cmd, map, flags @ ..] if cmd == "render" => {
            let (mut file, mut margin) = (scratch.join(format!("{map}.png")), RENDER_MARGIN);
            let mut f = flags;
            loop {
                match f {
                    [] => break,
                    [k, v, t @ ..] if k == "--out" => (file, f) = (PathBuf::from(v), t),
                    [k, v, t @ ..] if k == "--margin" => {
                        margin = v.parse().with_context(|| format!("--margin {v}"))?;
                        f = t;
                    }
                    _ => bail!(USAGE),
                }
            }
            let doc = games.load(map)?;
            let img = render(&doc, &Catalog::for_games(&games)?, margin)?;
            write_png(&img, &file)?;
            writeln!(
                out,
                "{map}: {}x{} cells, margin {margin}: {}",
                doc.width,
                doc.height,
                file.display()
            )?;
        }
        [cmd, primary, secondary, flags @ ..] if cmd == "palette" => {
            let file = match flags {
                [] => scratch.join(format!("palette-{primary}-{secondary}.png")),
                [k, v] if k == "--out" => PathBuf::from(v),
                _ => bail!(USAGE),
            };
            let p = palette(&Catalog::from_content()?, primary, secondary)?;
            write_png(&p.image, &file)?;
            let first_secondary = p.metatiles.iter().position(|m| m.is_some_and(|m| m >= 512));
            writeln!(
                out,
                "{primary} + {secondary}: {} cells, {} per row; cell (x, y) is metatile \
                 cols*y + x for the primary, the secondary from cell {}: {}",
                p.metatiles.len(),
                p.cols,
                first_secondary.unwrap_or(p.metatiles.len()),
                file.display()
            )?;
        }
        _ => bail!(USAGE),
    }
    Ok(out)
}

fn report(out: &mut String, map: &str, issues: &[Issue]) {
    for i in issues {
        let _ = writeln!(out, "{map}: {i}");
    }
}
